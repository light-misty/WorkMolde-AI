use tauri::State;

use crate::errors::CommandError;
use crate::models::skill::{CustomSkillConfig, SkillInfo};
use crate::AppState;

/// 列出所有 Skill
#[tauri::command]
pub async fn list_skills(state: State<'_, AppState>) -> Result<Vec<SkillInfo>, CommandError> {
    log::info!("list_skills: 查询所有 Skill");
    let skills = state.skill_registry.list_skills();
    log::info!("list_skills: 查询完成, 共 {} 个 Skill", skills.len());
    Ok(skills)
}

/// 切换 Skill 启用/禁用状态
#[tauri::command]
pub async fn toggle_skill(
    skill_id: String,
    enabled: bool,
    _state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "Skill '{}' 已{}",
        skill_id,
        if enabled { "启用" } else { "禁用" }
    );
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
