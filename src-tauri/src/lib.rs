#![recursion_limit = "256"]

use std::collections::HashMap;
use std::sync::Arc;

use tauri::{Manager, Emitter};
use tauri::path::BaseDirectory;

pub mod commands;
pub mod config;
pub mod db;
pub mod errors;
pub mod events;
pub mod models;
pub mod services;
pub mod utils;

/// 用户确认决策
#[derive(Debug, Clone)]
pub struct ConfirmDecision {
    pub approved: bool,
    pub feedback: Option<String>,
}

/// 应用全局状态，通过 tauri::State 在命令中共享
pub struct AppState {
    pub db: Arc<crate::db::Database>,
    pub config: Arc<tokio::sync::Mutex<crate::config::ConfigManager>>,
    pub active_agents: Arc<tokio::sync::Mutex<HashMap<String, bool>>>,
    pub confirm_channels: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<ConfirmDecision>>>>,
    pub doc_service: Arc<crate::services::document::DocumentService>,
    pub llm_router: Arc<tokio::sync::RwLock<Arc<crate::services::llm::router::LlmRouter>>>,
    pub tool_registry: Arc<crate::services::tool::registry::ToolRegistry>,
    pub handler_registry: Arc<tokio::sync::Mutex<crate::services::handler::registry::HandlerRegistry>>,
    pub fs_watcher: Arc<crate::services::fs_watcher::FsWatcherService<tauri::Wry>>,
    pub network_monitor: Arc<crate::services::network_monitor::NetworkMonitor<tauri::Wry>>,
    /// Scratchpad 共享状态：智能体草稿本，按 session_id 隔离
    /// 由 ScratchpadTool 写入，由 AgentContext 在每轮迭代时读取摘要
    pub scratchpad_states: crate::services::tool::builtin::SharedScratchpadStates,
}

pub fn run() {
    // 安装增强版 panic hook：
    // 1. 将 panic 信息记录到日志
    // 2. 尝试向前端发送 runtime:error 事件（如果 app handle 可用）
    // 3. 保留默认行为（打印到 stderr + 终止进程）
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info.location().map(|l| l.to_string()).unwrap_or_default();
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "未知 panic".to_string()
        };

        log::error!("========== 应用发生 panic ==========");
        log::error!("panic 位置: {}", location);
        log::error!("panic 消息: {}", message);
        log::error!("====================================");
        // 调用默认 hook 完成标准 panic 流程（打印到 stderr + 终止）
        default_hook(info);
    }));

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init());

    // 桌面端插件：更新和进程管理（需在 Builder 级别注册，构建脚本才能发现权限定义）
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    builder
        .setup(|app| {
            // 初始化应用数据目录
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("无法获取应用数据目录: {}", e))?;
            std::fs::create_dir_all(&app_data_dir)
                .map_err(|e| format!("无法创建应用数据目录: {}", e))?;

            // 在 Windows 11 上为无边框窗口启用 DWM 圆角
            // decorations: false 默认使用 WS_POPUP 风格，DWM 对纯 POPUP 窗口不渲染圆角。
            // 需添加 WS_THICKFRAME 风格后 DWM 才会应用圆角，再通过 DWMNCRP_DISABLED 隐藏非客户区。
            #[cfg(target_os = "windows")]
            {
                apply_window_rounded_corners(app.handle());

                // setup 阶段窗口可能尚未完全初始化，延迟再应用一次确保生效
                let app_clone = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // 等待窗口首次渲染完成后刷新 DWM 样式
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    apply_window_rounded_corners(&app_clone);
                });
            }

            // 初始化日志系统（必须在数据库和配置初始化之前，确保关键操作的错误能被记录）
            // 开发模式：使用项目根目录的 log/ 子目录，与 Python Sidecar 保持一致
            // 生产模式：使用 Tauri 推荐的系统日志目录
            // Windows 生产模式: %LOCALAPPDATA%\<bundle_identifier>\logs
            // macOS 生产模式: ~/Library/Logs/<bundle_identifier>
            // Linux 生产模式: ~/.local/share/<bundle_identifier>/logs
            let log_dir = crate::utils::logger::resolve_log_dir(
                app.path().app_log_dir().ok(),
                app.path().app_data_dir().ok(),
            );
            crate::utils::logger::init(&log_dir)
                .map_err(|e| format!("日志系统初始化失败: {}", e))?;

            // 初始化数据库（含损坏检测和自动恢复）
            let db_path = app_data_dir.join("docagent.db");
            let database = match crate::db::Database::new(&db_path) {
                Ok(db) => db,
                Err(e) => {
                    log::error!("数据库初始化失败: {}, 尝试备份并重建...", e);
                    // 备份损坏的数据库文件
                    let backup_path = db_path.with_extension("db.corrupted");
                    let _ = std::fs::rename(&db_path, &backup_path);
                    log::info!("已将损坏的数据库备份到: {:?}", backup_path);
                    // 重新创建空数据库
                    crate::db::Database::new(&db_path)
                        .map_err(|e| format!("数据库重建失败: {}", e))?
                }
            };

            // 初始化配置管理器
            let config_manager = crate::config::ConfigManager::new(app_data_dir.clone());

            // 加载 LLM 配置（容错：损坏时使用默认配置）
            #[cfg_attr(not(builtin_provider), allow(unused_mut))]
            let mut llm_config = config_manager.load_llm_config().unwrap_or_else(|e| {
                log::error!("LLM 配置加载失败: {}, 使用默认配置", e);
                Default::default()
            });

            // 注入内置 Provider（仅在编译时检测到 builtin_provider.json 时启用）
            #[cfg(builtin_provider)]
            {
                let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."));
                crate::config::llm_config::inject_builtin_provider(&mut llm_config, project_root);

                // 保存更新后的配置（内置 Provider 注入后需持久化）
                if let Err(e) = config_manager.save_llm_config(&llm_config) {
                    log::error!("保存 LLM 配置失败: {}", e);
                }
            }

            let llm_router = crate::services::llm::router::LlmRouter::from_config(&llm_config)
                .with_app_handle(Some(app.handle().clone()));
            let llm_router_arc: Arc<tokio::sync::RwLock<Arc<crate::services::llm::router::LlmRouter>>> =
                Arc::new(tokio::sync::RwLock::new(Arc::new(llm_router)));

            // 解析 Python 可执行文件路径
            // 优先使用环境变量 DOCAGENT_PYTHON，否则按平台尝试常见命令名
            let python_path = if let Ok(p) = std::env::var("DOCAGENT_PYTHON") {
                p
            } else {
                // Windows 上优先尝试 py（Python Launcher），再尝试 python 和 python3
                // py 是 Windows 官方推荐的 Python 启动器，通常随 Python 一起安装
                #[cfg(target_os = "windows")]
                {
                    use std::os::windows::process::CommandExt;
                    const CREATE_NO_WINDOW: u32 = 0x08000000;
                    let candidates = ["py", "python", "python3"];
                    let mut found = "python".to_string();
                    for candidate in &candidates {
                        // 使用 --version 检测可执行文件是否存在
                        let check = std::process::Command::new(candidate)
                            .arg("--version")
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .creation_flags(CREATE_NO_WINDOW)
                            .status();
                        if check.is_ok() {
                            found = candidate.to_string();
                            log::info!("检测到 Python 可执行文件: {}", found);
                            break;
                        }
                    }
                    found
                }
                #[cfg(not(target_os = "windows"))]
                {
                    "python3".to_string()
                }
            };

            // 解析 Sidecar 脚本路径：按优先级尝试多个候选位置
            let sidecar_script_str = {
                // CARGO_MANIFEST_DIR 在编译期指向 src-tauri/ 目录，其上一级即为项目根目录
                let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .map(|p| p.join("sidecar").join("main.py"))
                    .unwrap_or_else(|| std::path::PathBuf::from("sidecar/main.py"));

                // Tauri 资源路径解析：生产环境中 bundle.resources 打包的文件通过此 API 定位
                // resolve() 会自动处理路径中的 .. -> _up_ 等转换，比手动拼接 resource_dir() 更可靠
                let resource_path = app.path().resolve("sidecar/main.py", BaseDirectory::Resource)
                    .ok();

                // 按优先级构建候选路径列表
                let mut candidates: Vec<std::path::PathBuf> = Vec::new();
                // 1. Tauri 资源路径解析（生产环境，通过 bundle.resources 打包）
                if let Some(path) = resource_path {
                    candidates.push(path);
                }
                // 2. 项目根目录下的 sidecar（开发模式，基于 CARGO_MANIFEST_DIR 推导）
                candidates.push(project_root);

                let mut found = None;
                for candidate in &candidates {
                    if candidate.exists() {
                        // 去除 Windows UNC 前缀（\\?\），Python 不支持 UNC 路径作为脚本参数
                        let clean_path = crate::utils::strip_unc_prefix(candidate);
                        found = Some(clean_path.to_string_lossy().to_string());
                        break;
                    }
                }

                match found {
                    Some(path) => {
                        log::info!("Sidecar 脚本已定位: {}", path);
                        path
                    }
                    None => {
                        log::error!(
                            "Sidecar 脚本未找到，已尝试以下路径: {:?}",
                            candidates.iter()
                                .map(|p| p.to_string_lossy().to_string())
                                .collect::<Vec<_>>()
                        );
                        // 兜底：使用绝对路径形式的最后候选，避免依赖 CWD
                        candidates.last()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| "sidecar/main.py".to_string())
                    }
                }
            };

            // 读取 Sidecar 超时配置（容错：加载失败时使用默认值 120 秒）
            let sidecar_timeout_secs = config_manager
                .load_app_settings()
                .map(|s| s.sidecar_timeout_secs)
                .unwrap_or(0);

            let sidecar_manager = crate::services::document::SidecarManager::new(
                python_path,
                sidecar_script_str,
                sidecar_timeout_secs,
            );
            let doc_service = crate::services::document::DocumentService::new(sidecar_manager);

            let mut handler_registry = crate::services::handler::registry::HandlerRegistry::new();
            let doc_service_for_handlers = Arc::new(doc_service);
            crate::services::handler::builtin::register_builtin_handlers(
                &mut handler_registry,
                Arc::clone(&doc_service_for_handlers),
            );

            // 初始化 Tool 注册表并注册内置工具
            let mut tool_registry = crate::services::tool::registry::ToolRegistry::new();
            let scratchpad_states = crate::services::tool::builtin::register_builtin_tools(&mut tool_registry);

            log::info!("DocAgent 应用初始化完成");

            // 初始化文件监听服务
            let fs_watcher = crate::services::fs_watcher::FsWatcherService::new(app.handle().clone());

            // 初始化网络监控服务
            let network_monitor = crate::services::network_monitor::NetworkMonitor::new(
                Arc::clone(&llm_router_arc),
                crate::events::emitter::AgentEmitter::new(app.handle().clone()),
            );

            let state = AppState {
                db: Arc::new(database),
                config: Arc::new(tokio::sync::Mutex::new(config_manager)),
                active_agents: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                confirm_channels: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                doc_service: doc_service_for_handlers,
                llm_router: llm_router_arc,
                tool_registry: Arc::new(tool_registry),
                handler_registry: Arc::new(tokio::sync::Mutex::new(handler_registry)),
                fs_watcher: Arc::new(fs_watcher),
                network_monitor: Arc::new(network_monitor),
                scratchpad_states,
            };

            app.manage(state);

            // 应用启动时，如果已有活动工作区，自动启动文件监听
            let fs_watcher = app.state::<AppState>().fs_watcher.clone();
            let config_manager = app.state::<AppState>().config.clone();
            tauri::async_runtime::spawn(async move {
                let cfg = config_manager.lock().await;
                if let Ok(ws_config) = cfg.load_workspaces() {
                    if let Ok(settings) = cfg.load_app_settings() {
                        let active_id = &settings.workspace.default_workspace_id;
                        if !active_id.is_empty() {
                            if let Some(ws) = ws_config.workspaces.iter().find(|w| w.id == *active_id) {
                                fs_watcher.watch(ws.id.clone(), ws.path.clone()).await;
                            }
                        }
                    }
                }
            });

            // 启动定期 Provider 健康检查（每 5 分钟执行一次）
            let llm_router_for_health = Arc::clone(&app.state::<AppState>().llm_router);
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
                // 跳过首次立即触发，等待第一个间隔
                interval.tick().await;
                loop {
                    interval.tick().await;
                    // 获取路由器快照后释放锁，避免长时间持锁
                    let router_snapshot = {
                        let guard = llm_router_for_health.read().await;
                        Arc::clone(&guard)
                    };
                    if router_snapshot.is_empty().await {
                        continue;
                    }
                    let results = router_snapshot.health_check_all().await;
                    let summary: Vec<(&String, bool)> = results.iter()
                        .map(|(k, v)| (k, v.success))
                        .collect();
                    log::info!("定期健康检查完成: {:?}", summary);
                }
            });

            // 启动定期 Sidecar 健康检查（每 3 分钟执行一次）
            let doc_service_for_health = Arc::clone(&app.state::<AppState>().doc_service);
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(180));
                interval.tick().await;
                loop {
                    interval.tick().await;
                    let healthy = doc_service_for_health.health_check().await;
                    if !healthy {
                        log::warn!("Sidecar 定期健康检查: 不健康");
                    }
                }
            });

            // 启动定期工作区目录存在性检查（每 10 秒执行一次）
            // 作为父目录监听器的兜底机制，当父目录监听器失效时仍能检测到目录删除
            let fs_watcher_for_check = app.state::<AppState>().fs_watcher.clone();
            let app_handle_for_check = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
                interval.tick().await;
                loop {
                    interval.tick().await;
                    // 如果已经通过 FsWatcher 检测到目录删除并发射过事件，跳过
                    if fs_watcher_for_check.is_deletion_emitted() {
                        // 停止监听已删除的工作区（如果尚未停止）
                        fs_watcher_for_check.stop().await;
                        continue;
                    }
                    if let Some((wid, wpath, wname)) = fs_watcher_for_check.get_active_watch_info().await {
                        if !wpath.exists() || !wpath.is_dir() {
                            log::warn!(
                                "定期检查: 工作区目录已不存在, workspace_id={}, path={}, name={}",
                                wid,
                                wpath.display(),
                                wname
                            );
                            // 发射工作区目录删除事件
                            let deleted_payload = crate::events::types::WorkspaceDirectoryDeletedPayload {
                                workspace_id: wid.clone(),
                                workspace_name: wname.clone(),
                                workspace_path: wpath.to_string_lossy().to_string(),
                            };
                            let _ = app_handle_for_check.emit(
                                crate::events::types::WORKSPACE_DIRECTORY_DELETED,
                                deleted_payload,
                            );
                            // 停止监听已删除的工作区
                            fs_watcher_for_check.stop().await;
                        }
                    }
                }
            });

            // 启动网络监控服务
            let network_monitor = app.state::<AppState>().network_monitor.clone();
            tauri::async_runtime::spawn(async move {
                network_monitor.start();
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // LLM 命令
            commands::llm::test_connection,
            commands::llm::test_connection_with_config,
            commands::llm::list_providers,
            commands::llm::add_provider,
            commands::llm::update_provider,
            commands::llm::delete_provider,
            commands::llm::health_check_providers,
            commands::llm::force_recover_providers,
            commands::llm::get_network_status,
            // 会话命令
            commands::session::create_session,
            commands::session::list_sessions,
            commands::session::get_session,
            commands::session::delete_session,
            commands::session::update_session_title,
            commands::session::clear_all_sessions,
            commands::session::update_session_workspace,
            // 工作区命令
            commands::workspace::list_workspaces,
            commands::workspace::add_workspace,
            commands::workspace::remove_workspace,
            commands::workspace::set_active_workspace,
            commands::workspace::get_file_tree,
            commands::workspace::search_files,
            // 文档命令
            commands::document::preview_document,
            commands::document::get_document_versions,
            commands::document::rollback_version,
            commands::document::get_version_content,
            commands::document::create_file,
            commands::document::create_directory,
            commands::document::rename_file,
            commands::document::delete_file,
            commands::document::show_in_file_manager,
            commands::document::get_pdf_data,
            // Handler 命令
            commands::handler::list_tools,
            commands::handler::list_handlers,
            // 设置命令
            commands::settings::get_settings,
            commands::settings::update_settings,
            // Agent 命令
            commands::agent::start_agent,
            commands::agent::stop_agent,
            commands::agent::confirm_operation,
            commands::agent::get_context_usage,
            commands::agent::is_agent_running,
            // 模板命令
            commands::template::list_templates,
            commands::template::get_template,
            commands::template::create_template,
            commands::template::update_template,
            commands::template::delete_template,
            // 日志命令
            commands::log::get_log_path,
            commands::log::open_directory,
            // 更新命令
            #[cfg(desktop)]
            commands::update::check_update,
            #[cfg(desktop)]
            commands::update::download_and_install_update,
            #[cfg(desktop)]
            commands::update::download_update,
            #[cfg(desktop)]
            commands::update::install_downloaded_update,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            log::error!("Tauri 应用运行失败: {}", e);
        });
}

/// 为无边框窗口设置 DWM 圆角（Windows 11）
///
/// 添加 WS_THICKFRAME 风格使 DWM 启用圆角渲染，
/// 再通过 DWMNCRP_DISABLED 隐藏非客户区边框，
/// 最后设置 DWMWCP_ROUND 应用标准圆角。
#[cfg(target_os = "windows")]
fn apply_window_rounded_corners(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(hwnd) = window.hwnd() {
            type DWORD = u32;
            type LongPtr = isize;

            const GWL_STYLE: i32 = -16;
            const WS_THICKFRAME: LongPtr = 0x00040000;
            const WS_POPUP: LongPtr = 0x80000000;

            const SWP_FRAMECHANGED: DWORD = 0x0020;
            const SWP_NOMOVE: DWORD = 0x0002;
            const SWP_NOSIZE: DWORD = 0x0001;
            const SWP_NOZORDER: DWORD = 0x0004;

            const DWMWA_NCRENDERING_POLICY: DWORD = 2;
            const DWMNCRP_DISABLED: DWORD = 2;
            const DWMWA_WINDOW_CORNER_PREFERENCE: DWORD = 33;
            const DWMWCP_ROUND: DWORD = 2;

            #[link(name = "user32")]
            extern "system" {
                fn GetWindowLongPtrW(
                    hwnd: *const std::ffi::c_void,
                    n_index: i32,
                ) -> LongPtr;
                fn SetWindowLongPtrW(
                    hwnd: *const std::ffi::c_void,
                    n_index: i32,
                    dw_new_long: LongPtr,
                ) -> LongPtr;
                fn SetWindowPos(
                    hwnd: *const std::ffi::c_void,
                    hwnd_insert_after: *const std::ffi::c_void,
                    x: i32,
                    y: i32,
                    cx: i32,
                    cy: i32,
                    u_flags: DWORD,
                ) -> i32;
            }

            #[link(name = "dwmapi")]
            extern "system" {
                fn DwmSetWindowAttribute(
                    hwnd: *const std::ffi::c_void,
                    dw_attribute: DWORD,
                    pv_attribute: *const std::ffi::c_void,
                    cb_attribute: DWORD,
                ) -> i32;
            }

            unsafe {
                let style = GetWindowLongPtrW(hwnd.0 as *const _, GWL_STYLE);
                SetWindowLongPtrW(
                    hwnd.0 as *mut _,
                    GWL_STYLE,
                    style | WS_THICKFRAME | WS_POPUP,
                );
                SetWindowPos(
                    hwnd.0 as *mut _,
                    std::ptr::null(),
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
                );

                let render_policy = DWMNCRP_DISABLED;
                DwmSetWindowAttribute(
                    hwnd.0 as *const _,
                    DWMWA_NCRENDERING_POLICY,
                    &render_policy as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<DWORD>() as DWORD,
                );

                let preference = DWMWCP_ROUND;
                DwmSetWindowAttribute(
                    hwnd.0 as *const _,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    &preference as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<DWORD>() as DWORD,
                );
            }
        }
    }
}
