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
            let sidecar_script = app_data_dir.join("sidecar").join("main.py");
            let sidecar_script_str = if sidecar_script.exists() {
                sidecar_script.to_string_lossy().to_string()
            } else {
                let project_sidecar = std::path::Path::new("sidecar/main.py");
                if project_sidecar.exists() {
                    project_sidecar.to_string_lossy().to_string()
                } else {
                    log::error!("Sidecar 脚本未找到，请确保 sidecar/main.py 存在");
                    "sidecar/main.py".to_string()
                }
            };
            log::info!("Sidecar 脚本路径: {}", sidecar_script_str);

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
