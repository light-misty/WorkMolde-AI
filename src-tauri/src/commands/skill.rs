use tauri::State;

use crate::errors::CommandError;
use crate::models::skill::{CustomSkillConfig, SkillInfo};
use crate::AppState;

/// 列出所有 Skill
#[tauri::command]
pub async fn list_skills(state: State<'_, AppState>) -> Result<Vec<SkillInfo>, CommandError> {
    log::info!("list_skills: 查询所有 Skill");
    let skills = {
        let reg = state.skill_registry.lock().await;
        reg.list_skills()
    };
    log::info!("list_skills: 查询完成, 共 {} 个 Skill", skills.len());
    Ok(skills)
}

/// 切换 Skill 启用/禁用状态，并持久化到配置
#[tauri::command]
pub async fn toggle_skill(
    skill_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("toggle_skill: skill_id={}, enabled={}", skill_id, enabled);

    // 更新注册表中的状态
    let disabled_list = {
        let mut registry = state.skill_registry.lock().await;
        registry.toggle_skill(&skill_id, enabled)
    };

    // 持久化到配置文件
    let cfg_manager = state.config.lock().await;
    let mut settings = cfg_manager.load_app_settings().map_err(|e| {
        log::error!("加载应用设置失败: {}", e);
        e
    })?;
    settings.disabled_skills = disabled_list;
    cfg_manager.save_app_settings(&settings).map_err(|e| {
        log::error!("保存应用设置失败: {}", e);
        e
    })?;

    log::info!("toggle_skill: 状态已持久化, skill_id={}, enabled={}", skill_id, enabled);
    Ok(())
}

/// 添加自定义 Skill
#[tauri::command]
pub async fn add_custom_skill(
    config: CustomSkillConfig,
    _state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("添加自定义 Skill: {}", config.name);
    Ok(())
}

/// 删除自定义 Skill
#[tauri::command]
pub async fn delete_custom_skill(
    skill_id: String,
    _state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("删除自定义 Skill: {}", skill_id);
    Ok(())
}
