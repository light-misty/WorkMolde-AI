#![recursion_limit = "256"]

use std::collections::HashMap;
use std::sync::Arc;

use tauri::Manager;

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
    pub skill_registry: Arc<tokio::sync::Mutex<crate::services::skill::registry::SkillRegistry>>,
    pub fs_watcher: Arc<crate::services::fs_watcher::FsWatcherService<tauri::Wry>>,
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

            // 初始化日志系统（必须在数据库和配置初始化之前，确保关键操作的错误能被记录）
            let log_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .map(|p| p.join("log"))
                .unwrap_or_else(|| std::path::PathBuf::from("log"));
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
            let llm_config = config_manager.load_llm_config().unwrap_or_else(|e| {
                log::error!("LLM 配置加载失败: {}, 使用默认配置", e);
                Default::default()
            });
            let llm_router = crate::services::llm::router::LlmRouter::from_config(&llm_config)
                .with_app_handle(Some(app.handle().clone()));

            let python_path = std::env::var("DOCAGENT_PYTHON")
                .unwrap_or_else(|_| "python".to_string());

            // 解析 Sidecar 脚本路径：按优先级尝试多个候选位置
            let sidecar_script_str = {
                // CARGO_MANIFEST_DIR 在编译期指向 src-tauri/ 目录，其上一级即为项目根目录
                let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .map(|p| p.join("sidecar").join("main.py"))
                    .unwrap_or_else(|| std::path::PathBuf::from("sidecar/main.py"));

                let candidates = [
                    // 1. 应用数据目录下的 sidecar（生产环境可能将脚本复制到此）
                    app_data_dir.join("sidecar").join("main.py"),
                    // 2. 可执行文件同级目录下的 sidecar（生产环境打包后）
                    {
                        let exe_path = std::env::current_exe().unwrap_or_default();
                        exe_path.parent()
                            .map(|p| p.join("sidecar").join("main.py"))
                            .unwrap_or_else(|| std::path::PathBuf::from("sidecar/main.py"))
                    },
                    // 3. 项目根目录下的 sidecar（开发模式，基于 CARGO_MANIFEST_DIR 推导）
                    project_root,
                ];

                let mut found = None;
                for candidate in &candidates {
                    if candidate.exists() {
                        // 转换为绝对路径，避免依赖工作目录
                        let abs_path = if candidate.is_absolute() {
                            candidate.clone()
                        } else {
                            std::fs::canonicalize(candidate)
                                .unwrap_or_else(|_| candidate.clone())
                        };
                        found = Some(abs_path.to_string_lossy().to_string());
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

            let sidecar_manager = crate::services::document::SidecarManager::new(
                python_path,
                sidecar_script_str,
            );
            let doc_service = crate::services::document::DocumentService::new(sidecar_manager);

            let mut skill_registry = crate::services::skill::registry::SkillRegistry::new();
            let doc_service_for_skills = Arc::new(doc_service);
            crate::services::skill::builtin::register_builtin_skills(
                &mut skill_registry,
                Arc::clone(&doc_service_for_skills),
            );

            // 初始化 Tool 注册表并注册内置工具
            let mut tool_registry = crate::services::tool::registry::ToolRegistry::new();
            crate::services::tool::builtin::register_builtin_tools(&mut tool_registry);

            // 从配置加载已禁用 Skill 列表
            // 过滤掉内置 Skill 的 ID，确保内置 Skill 不被意外禁用
            let app_settings = config_manager.load_app_settings().unwrap_or_default();
            let builtin_skill_ids: Vec<&str> = vec![
                "docx_skill", "xlsx_skill", "pptx_skill", "pdf_skill",
            ];
            let filtered_disabled: Vec<String> = app_settings.disabled_skills.iter()
                .filter(|id| !builtin_skill_ids.contains(&id.as_str()))
                .cloned()
                .collect();
            skill_registry = skill_registry.with_disabled_skills(filtered_disabled);

            log::info!("DocAgent 应用初始化完成");

            // 初始化文件监听服务
            let fs_watcher = crate::services::fs_watcher::FsWatcherService::new(app.handle().clone());

            let state = AppState {
                db: Arc::new(database),
                config: Arc::new(tokio::sync::Mutex::new(config_manager)),
                active_agents: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                confirm_channels: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                doc_service: doc_service_for_skills,
                llm_router: Arc::new(tokio::sync::RwLock::new(Arc::new(llm_router))),
                tool_registry: Arc::new(tool_registry),
                skill_registry: Arc::new(tokio::sync::Mutex::new(skill_registry)),
                fs_watcher: Arc::new(fs_watcher),
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
                    if router_snapshot.is_empty() {
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
            commands::llm::set_default_provider,
            commands::llm::health_check_providers,
            // 会话命令
            commands::session::create_session,
            commands::session::list_sessions,
            commands::session::get_session,
            commands::session::delete_session,
            commands::session::update_session_title,
            commands::session::clear_all_sessions,
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
            // Skill 命令
            commands::skill::list_tools,
            commands::skill::list_skills,
            // 设置命令
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::export_config,
            commands::settings::import_config,
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
            commands::log::get_error_log,
            // 更新命令
            #[cfg(desktop)]
            commands::update::check_update,
            #[cfg(desktop)]
            commands::update::download_and_install_update,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            log::error!("Tauri 应用运行失败: {}", e);
        });
}
