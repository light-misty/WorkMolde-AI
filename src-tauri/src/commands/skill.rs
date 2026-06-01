use tauri::State;

use crate::errors::CommandError;
use crate::models::skill::SkillInfo;
use crate::models::tool::ToolInfo;
use crate::AppState;

/// 列出所有 Tool（内置工具）
#[tauri::command]
pub async fn list_tools(state: State<'_, AppState>) -> Result<Vec<ToolInfo>, CommandError> {
    log::info!("list_tools: 查询所有 Tool");
    let tools = state.tool_registry.list_tools();
    log::info!("list_tools: 查询完成, 共 {} 个 Tool", tools.len());
    Ok(tools)
}

/// 列出所有 Skill（内置）
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
