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

/// 从系统 PATH 中查找 Python 可执行文件（开发模式兜底）
///
/// 在开发模式下使用（嵌入式 Python 不存在时回退），生产环境优先使用嵌入式 Python。
/// Windows 上按 py（Python Launcher）、python、python3 顺序尝试。
fn find_system_python() -> String {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let candidates = ["py", "python", "python3"];
        for candidate in &candidates {
            // 使用 --version 检测可执行文件是否存在
            let check = std::process::Command::new(candidate)
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .status();
            if check.is_ok() {
                log::info!("检测到系统 Python: {}", candidate);
                return candidate.to_string();
            }
        }
        // 兜底：返回 "python"，后续 spawn 时若失败会输出明确错误
        "python".to_string()
    }
    #[cfg(not(target_os = "windows"))]
    {
        "python3".to_string()
    }
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
            #[cfg(target_os = "windows")]
            {
                apply_window_rounded_corners(app.handle());
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

            // 修复 Tauri drag resize 子窗口在 maximized 启动时未正确重置尺寸的问题
            // 必须在日志系统初始化之后（确保日志可记录）和窗口创建后（subclass_parent 已安装）调用
            #[cfg(target_os = "windows")]
            {
                fix_drag_resize_child_window_size(app.handle());
            }

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
            // 优先级：
            // 1. 环境变量 DOCAGENT_PYTHON（开发模式覆盖，最高优先级）
            // 2. 应用资源目录的嵌入式 Python（生产环境，sidecar_dist/python/python.exe）
            // 3. 系统 PATH 中的 py/python/python3（开发模式兜底）
            let python_path = if let Ok(p) = std::env::var("DOCAGENT_PYTHON") {
                log::info!("使用环境变量 DOCAGENT_PYTHON 指定的 Python: {}", p);
                p
            } else {
                // 优先尝试应用资源目录中的嵌入式 Python（生产环境）
                let embedded_python = app.path()
                    .resolve("sidecar_dist/python/python.exe", BaseDirectory::Resource)
                    .ok()
                    .map(|p| crate::utils::strip_unc_prefix(&p).to_string_lossy().to_string());

                if let Some(path) = embedded_python {
                    if std::path::Path::new(&path).exists() {
                        log::info!("使用嵌入式 Python: {}", path);
                        path
                    } else {
                        // 资源路径解析成功但文件不存在，回退到系统 PATH
                        log::warn!("嵌入式 Python 路径解析成功但文件不存在: {}，回退到系统 PATH", path);
                        find_system_python()
                    }
                } else {
                    // 资源路径解析失败（开发模式），使用系统 PATH
                    log::info!("未找到嵌入式 Python（开发模式），使用系统 PATH");
                    find_system_python()
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
                // 生产环境通过 sidecar_dist/ 打包，脚本路径为 sidecar_dist/sidecar/main.py
                let embedded_script = app.path()
                    .resolve("sidecar_dist/sidecar/main.py", BaseDirectory::Resource)
                    .ok();

                // 按优先级构建候选路径列表
                let mut candidates: Vec<std::path::PathBuf> = Vec::new();
                // 1. 嵌入式 Python 方案的脚本路径（生产环境，最高优先级）
                if let Some(path) = embedded_script {
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

/// 修复无边框最大化窗口的鼠标点击问题
///
/// 问题根因（多层叠加）：
/// 1. tao 在 decorations:false 时保留 WS_SIZEBOX(=WS_THICKFRAME) 样式以支持 resize
/// 2. 最大化时 DefWindowProcW 对 WS_THICKFRAME 窗口在边缘返回 resize hit test 值，
///    导致 Windows 拦截鼠标事件，WebView2 收不到 click
/// 3. Windows 11 Snap Layouts 基于 WS_MAXIMIZEBOX + WS_SYSMENU 拦截 mouseup
/// 4. Tauri 的 TAURI_DRAG_RESIZE_WINDOW 子窗口在 maximized 启动时未正确重置尺寸，
///    子窗口 region 覆盖按钮区域（WM_SIZE 在 subclass 安装前触发）
///
/// 修复方案：
/// 1. 最大化时移除 WS_SIZEBOX + WS_MAXIMIZEBOX + WS_SYSMENU 样式
/// 2. 还原时恢复这三个样式，确保 resize 功能正常
/// 3. 隐藏 TAURI_DRAG_RESIZE_WINDOW 子窗口并重置尺寸
/// 4. 监听 WM_SIZE 事件，在最大化/还原切换时自动调整样式和子窗口，并校正窗口尺寸
#[cfg(target_os = "windows")]
#[allow(non_camel_case_types)]
fn fix_drag_resize_child_window_size(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(hwnd) = window.hwnd() {
            // Windows API 类型定义
            type HWND_PTR = *mut std::ffi::c_void;
            type BOOL_T = i32;
            type LONG_PTR = isize;

            const GWL_STYLE: i32 = -16;
            const WS_SIZEBOX: u32 = 0x00040000;
            // WS_MAXIMIZEBOX: 系统菜单中的"最大化"按钮样式
            // WS_SYSMENU: 系统菜单样式（包含最小化/最大化/关闭系统按钮区域）
            // Windows 11 的 Snap Layouts 功能会基于这两个样式在鼠标悬停于窗口右上角时
            // 触发 snap layout 预览，拦截 mouseup 事件导致自定义按钮 click 失效
            const WS_MAXIMIZEBOX: u32 = 0x00010000;
            const WS_SYSMENU: u32 = 0x00080000;
            // SetWindowPos 标志位
            const SWP_NOMOVE: u32 = 0x0002;
            const SWP_NOACTIVATE: u32 = 0x0010;
            const SWP_NOOWNERZORDER: u32 = 0x0200;
            const SWP_ASYNCWINDOWPOS: u32 = 0x4000;
            const SWP_NOSIZE: u32 = 0x0001;
            const SWP_NOZORDER: u32 = 0x0004;
            const SWP_FRAMECHANGED: u32 = 0x0020;
            // ShowWindow 命令
            const SW_HIDE: i32 = 0;

            #[link(name = "user32")]
            extern "system" {
                fn IsZoomed(hwnd: HWND_PTR) -> BOOL_T;
                fn FindWindowExW(
                    parent: HWND_PTR,
                    child_after: HWND_PTR,
                    class: *const u16,
                    window: *const u16,
                ) -> HWND_PTR;
                fn SetWindowPos(
                    hwnd: HWND_PTR,
                    insert_after: HWND_PTR,
                    x: i32,
                    y: i32,
                    w: i32,
                    h: i32,
                    flags: u32,
                ) -> BOOL_T;
                fn ShowWindow(hwnd: HWND_PTR, cmd: i32) -> BOOL_T;
                fn GetWindowLongPtrW(hwnd: HWND_PTR, index: i32) -> LONG_PTR;
                fn SetWindowLongPtrW(hwnd: HWND_PTR, index: i32, new_long: LONG_PTR) -> LONG_PTR;
                fn SetWindowSubclass(
                    hwnd: HWND_PTR,
                    subclass_proc: unsafe extern "system" fn(*mut std::ffi::c_void, u32, usize, isize, usize, usize) -> isize,
                    uid: usize,
                    dw_ref_data: usize,
                ) -> BOOL_T;
            }

            // TAURI_DRAG_RESIZE_WINDOW 子窗口的类名和窗口名
            let class_name: Vec<u16> = "TAURI_DRAG_RESIZE_BORDERS\0".encode_utf16().collect();
            let window_name: Vec<u16> = "TAURI_DRAG_RESIZE_WINDOW\0".encode_utf16().collect();

            unsafe {
                let parent_hwnd = hwnd.0 as HWND_PTR;
                let is_maximized = IsZoomed(parent_hwnd) != 0;

                let style = GetWindowLongPtrW(parent_hwnd, GWL_STYLE) as u32;

                // 隐藏 TAURI_DRAG_RESIZE_WINDOW 子窗口并重置尺寸（仅最大化时）
                let child = FindWindowExW(
                    parent_hwnd,
                    std::ptr::null_mut(),
                    class_name.as_ptr(),
                    window_name.as_ptr(),
                );

                if !child.is_null() {
                    if is_maximized {
                        let _ = ShowWindow(child, SW_HIDE);
                        let _ = SetWindowPos(
                            child,
                            0 as HWND_PTR,
                            0, 0, 0, 0,
                            SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOMOVE,
                        );
                    }
                }

                // 最大化时移除 WS_SIZEBOX + WS_MAXIMIZEBOX + WS_SYSMENU 样式
                if is_maximized {
                    let mut remove_mask: u32 = 0;
                    if (style & WS_SIZEBOX) != 0 { remove_mask |= WS_SIZEBOX; }
                    if (style & WS_MAXIMIZEBOX) != 0 { remove_mask |= WS_MAXIMIZEBOX; }
                    if (style & WS_SYSMENU) != 0 { remove_mask |= WS_SYSMENU; }

                    if remove_mask != 0 {
                        let new_style = (style & !remove_mask) as LONG_PTR;
                        SetWindowLongPtrW(parent_hwnd, GWL_STYLE, new_style);
                        // SWP_FRAMECHANGED 通知 Windows 重新计算窗口框架
                        let _ = SetWindowPos(
                            parent_hwnd,
                            0 as HWND_PTR,
                            0, 0, 0, 0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_ASYNCWINDOWPOS,
                        );
                    }
                }

                // 安装 WM_SIZE subclass，在窗口状态切换时自动调整样式和子窗口
                let subclass_uid: usize = 0xD0CA6E77;
                let _ = SetWindowSubclass(
                    parent_hwnd,
                    fix_hit_test_subclass_proc,
                    subclass_uid,
                    0,
                );
            }
        }
    }
}

/// 自定义 subclass：监听 WM_SIZE 事件，在窗口状态切换时调整样式、子窗口可见性和窗口尺寸
#[cfg(target_os = "windows")]
#[allow(non_camel_case_types)]
unsafe extern "system" fn fix_hit_test_subclass_proc(
    hwnd: *mut std::ffi::c_void,
    msg: u32,
    wparam: usize,
    lparam: isize,
    _uid: usize,
    _dw_ref_data: usize,
) -> isize {
    type HWND_PTR = *mut std::ffi::c_void;
    type WPARAM_T = usize;
    type LPARAM_T = isize;
    type LONG_PTR = isize;

    const WM_SIZE: u32 = 0x0005;
    const GWL_STYLE: i32 = -16;
    const WS_SIZEBOX: u32 = 0x00040000;
    // WS_MAXIMIZEBOX + WS_SYSMENU: 最大化时移除以防止 Windows 11 Snap Layouts 干扰
    const WS_MAXIMIZEBOX: u32 = 0x00010000;
    const WS_SYSMENU: u32 = 0x00080000;
    const SIZE_RESTORED: usize = 0;
    const SIZE_MINIMIZED: usize = 1;
    // SetWindowPos 标志位
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_FRAMECHANGED: u32 = 0x0020;
    const SWP_NOOWNERZORDER: u32 = 0x0200;
    const SWP_ASYNCWINDOWPOS: u32 = 0x4000;
    const SW_HIDE: i32 = 0;

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct RECT {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    // 显示器信息结构体，用于 GetMonitorInfoW 获取多显示器工作区
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct MONITORINFO {
        cb_size: u32,
        rc_monitor: RECT,
        rc_work: RECT,
        dw_flags: u32,
    }

    // MonitorFromWindow 标志：返回最近显示器
    const MONITOR_DEFAULTTONEAREST: u32 = 0x00000002;

    #[link(name = "user32")]
    extern "system" {
        fn IsZoomed(hwnd: HWND_PTR) -> i32;
        fn GetWindowLongPtrW(hwnd: HWND_PTR, index: i32) -> LONG_PTR;
        fn SetWindowLongPtrW(hwnd: HWND_PTR, index: i32, new_long: LONG_PTR) -> LONG_PTR;
        fn SetWindowPos(
            hwnd: HWND_PTR,
            insert_after: HWND_PTR,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            flags: u32,
        ) -> i32;
        fn FindWindowExW(
            parent: HWND_PTR,
            child_after: HWND_PTR,
            class: *const u16,
            window: *const u16,
        ) -> HWND_PTR;
        fn ShowWindow(hwnd: HWND_PTR, cmd: i32) -> i32;
        fn DefSubclassProc(
            hwnd: HWND_PTR,
            msg: u32,
            wparam: WPARAM_T,
            lparam: LPARAM_T,
        ) -> LPARAM_T;
        // 获取窗口当前矩形
        fn GetWindowRect(hwnd: HWND_PTR, rect: *mut RECT) -> i32;
        // 获取窗口所在显示器句柄（多显示器支持）
        fn MonitorFromWindow(hwnd: HWND_PTR, flags: u32) -> HWND_PTR;
        // 获取显示器信息（含工作区 rc_work，已扣除任务栏）
        fn GetMonitorInfoW(monitor: HWND_PTR, info: *mut MONITORINFO) -> i32;
    }

    match msg {
        WM_SIZE => {
            let size_type = wparam;
            // 关键：从最小化恢复到最大化时，Windows 会发送 SIZE_RESTORED 而非 SIZE_MAXIMIZED
            // 此时 IsZoomed 返回 true，必须移除样式（而非恢复），否则最大化窗口带 WS_THICKFRAME
            // 会扩展到工作区外，覆盖任务栏
            if size_type == SIZE_MINIMIZED {
                // 最小化时不处理样式
                DefSubclassProc(hwnd, msg, wparam, lparam)
            } else if IsZoomed(hwnd) != 0 {
                // 窗口实际为最大化状态（含 SIZE_MAXIMIZED 和从最小化恢复的 SIZE_RESTORED）
                // 移除 WS_SIZEBOX + WS_MAXIMIZEBOX + WS_SYSMENU + 隐藏子窗口
                // - WS_SIZEBOX: 防止 DefWindowProcW 在窗口边缘返回 resize hit test 值
                // - WS_MAXIMIZEBOX + WS_SYSMENU: 防止 Windows 11 Snap Layouts 拦截 mouseup
                let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
                let mut remove_mask: u32 = 0;
                if (style & WS_SIZEBOX) != 0 { remove_mask |= WS_SIZEBOX; }
                if (style & WS_MAXIMIZEBOX) != 0 { remove_mask |= WS_MAXIMIZEBOX; }
                if (style & WS_SYSMENU) != 0 { remove_mask |= WS_SYSMENU; }

                if remove_mask != 0 {
                    let new_style = (style & !remove_mask) as LONG_PTR;
                    SetWindowLongPtrW(hwnd, GWL_STYLE, new_style);
                    let _ = SetWindowPos(
                        hwnd,
                        0 as HWND_PTR,
                        0, 0, 0, 0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_ASYNCWINDOWPOS,
                    );
                }
                // 关键修复：从最小化恢复到最大化时，Windows 在恢复样式后已设置了包含边框的
                // 窗口尺寸（扩展到屏幕外，覆盖任务栏）。移除样式后需要重新设置窗口尺寸为
                // 工作区尺寸（屏幕减去任务栏），否则任务栏会被覆盖且不可点击。
                // 使用 MonitorFromWindow + GetMonitorInfoW 获取窗口所在显示器的工作区，
                // 支持多显示器场景（SystemParametersInfoW 只返回主显示器，会导致副显示器窗口跳屏）
                let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                let mut monitor_info: MONITORINFO = MONITORINFO {
                    cb_size: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                let work_area = if GetMonitorInfoW(monitor, &mut monitor_info) != 0 {
                    monitor_info.rc_work
                } else {
                    // GetMonitorInfoW 失败时回退：使用窗口当前矩形作为工作区（不调整）
                    let mut win_rect_fallback: RECT = RECT::default();
                    let _ = GetWindowRect(hwnd, &mut win_rect_fallback);
                    win_rect_fallback
                };
                let mut win_rect: RECT = RECT::default();
                let _ = GetWindowRect(hwnd, &mut win_rect);
                let wa_w = work_area.right - work_area.left;
                let wa_h = work_area.bottom - work_area.top;
                let win_w = win_rect.right - win_rect.left;
                let win_h = win_rect.bottom - win_rect.top;
                if win_w != wa_w || win_h != wa_h || win_rect.left != work_area.left || win_rect.top != work_area.top {
                    let _ = SetWindowPos(
                        hwnd,
                        0 as HWND_PTR,
                        work_area.left,
                        work_area.top,
                        wa_w,
                        wa_h,
                        SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_ASYNCWINDOWPOS,
                    );
                }
                // 隐藏 TAURI_DRAG_RESIZE_WINDOW 子窗口
                let class_name: Vec<u16> = "TAURI_DRAG_RESIZE_BORDERS\0".encode_utf16().collect();
                let window_name: Vec<u16> = "TAURI_DRAG_RESIZE_WINDOW\0".encode_utf16().collect();
                let child = FindWindowExW(hwnd, std::ptr::null_mut(), class_name.as_ptr(), window_name.as_ptr());
                if !child.is_null() {
                    let _ = ShowWindow(child, SW_HIDE);
                    let _ = SetWindowPos(
                        child,
                        0 as HWND_PTR,
                        0, 0, 0, 0,
                        SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOMOVE,
                    );
                }
                DefSubclassProc(hwnd, msg, wparam, lparam)
            } else if size_type == SIZE_RESTORED {
                // 窗口非最大化（真正还原）：恢复 WS_SIZEBOX + WS_MAXIMIZEBOX + WS_SYSMENU
                let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
                let mut add_mask: u32 = 0;
                if (style & WS_SIZEBOX) == 0 { add_mask |= WS_SIZEBOX; }
                if (style & WS_MAXIMIZEBOX) == 0 { add_mask |= WS_MAXIMIZEBOX; }
                if (style & WS_SYSMENU) == 0 { add_mask |= WS_SYSMENU; }

                if add_mask != 0 {
                    let new_style = (style | add_mask) as LONG_PTR;
                    SetWindowLongPtrW(hwnd, GWL_STYLE, new_style);
                    let _ = SetWindowPos(
                        hwnd,
                        0 as HWND_PTR,
                        0, 0, 0, 0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_ASYNCWINDOWPOS,
                    );
                }
                DefSubclassProc(hwnd, msg, wparam, lparam)
            } else {
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

/// 为无边框窗口设置 DWM 圆角（Windows 11）
///
/// 仅通过 DWMWA_WINDOW_CORNER_PREFERENCE 设置圆角偏好，
/// 并通过 DWMNCRP_DISABLED 禁用 DWM 非客户区渲染。
///
/// 重要：不应向窗口添加 WS_THICKFRAME 风格。该风格会让 Windows 在窗口边缘
/// 创建不可见的非客户区用于 resize 命中测试（WM_NCHITTEST 返回 HTTOP/HTRIGHT
/// 等），导致贴合屏幕边缘的窗口控制按钮（关闭/还原/最小化）点击事件被系统
/// 拦截而无法传递到 webview 客户区，同时鼠标光标会变成双箭头 resize cursor。
/// tao 在 decorations:false 时本就会移除 WS_THICKFRAME，强行加回会与 tao
/// 的窗口管理逻辑冲突。DWMWA_WINDOW_CORNER_PREFERENCE 是独立的 DWM 属性，
/// 不依赖 WS_THICKFRAME 即可生效。
#[cfg(target_os = "windows")]
fn apply_window_rounded_corners(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(hwnd) = window.hwnd() {
            type DWORD = u32;

            const DWMWA_NCRENDERING_POLICY: DWORD = 2;
            const DWMNCRP_DISABLED: DWORD = 2;
            const DWMWA_WINDOW_CORNER_PREFERENCE: DWORD = 33;
            const DWMWCP_ROUND: DWORD = 2;

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
                // 禁用 DWM 非客户区渲染，避免系统绘制额外的窗口边框
                let render_policy = DWMNCRP_DISABLED;
                DwmSetWindowAttribute(
                    hwnd.0 as *const _,
                    DWMWA_NCRENDERING_POLICY,
                    &render_policy as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<DWORD>() as DWORD,
                );

                // 设置圆角偏好为标准圆角（Windows 11 Build 22000+）
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
