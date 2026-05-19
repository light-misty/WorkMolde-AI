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
    pub skill_registry: Arc<tokio::sync::Mutex<crate::services::skill::registry::SkillRegistry>>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // 初始化应用数据目录
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("无法获取应用数据目录");
            std::fs::create_dir_all(&app_data_dir).expect("无法创建应用数据目录");

            // 初始化日志系统（必须在数据库和配置初始化之前，确保关键操作的错误能被记录）
            let log_dir = std::path::Path::new("log");
            crate::utils::logger::init(log_dir)
                .expect("日志系统初始化失败");

            // 初始化数据库
            let db_path = app_data_dir.join("docagent.db");
            let database = crate::db::Database::new(&db_path)
                .expect("数据库初始化失败");

            // 初始化配置管理器
            let config_manager = crate::config::ConfigManager::new(app_data_dir.clone());

            let llm_config = config_manager.load_llm_config().unwrap_or_default();
            let llm_router = crate::services::llm::router::LlmRouter::from_config(&llm_config);

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

            // 从配置加载已禁用 Skill 列表
            let app_settings = config_manager.load_app_settings().unwrap_or_default();
            skill_registry = skill_registry.with_disabled_skills(app_settings.disabled_skills.clone());

            log::info!("DocAgent 应用初始化完成");

            let state = AppState {
                db: Arc::new(database),
                config: Arc::new(tokio::sync::Mutex::new(config_manager)),
                active_agents: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                confirm_channels: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
                doc_service: doc_service_for_skills,
                llm_router: Arc::new(tokio::sync::RwLock::new(Arc::new(llm_router))),
                skill_registry: Arc::new(tokio::sync::Mutex::new(skill_registry)),
            };

            app.manage(state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // LLM 命令
            commands::llm::test_connection,
            commands::llm::list_providers,
            commands::llm::add_provider,
            commands::llm::update_provider,
            commands::llm::delete_provider,
            commands::llm::set_default_provider,
            // 会话命令
            commands::session::create_session,
            commands::session::list_sessions,
            commands::session::get_session,
            commands::session::delete_session,
            commands::session::update_session_title,
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
            // Skill 命令
            commands::skill::list_skills,
            commands::skill::toggle_skill,
            commands::skill::add_custom_skill,
            commands::skill::delete_custom_skill,
            // 设置命令
            commands::settings::get_settings,
            commands::settings::update_settings,
            // Agent 命令
            commands::agent::start_agent,
            commands::agent::stop_agent,
            commands::agent::confirm_operation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
