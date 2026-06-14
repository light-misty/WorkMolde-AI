use tauri::State;

use crate::errors::CommandError;
use crate::models::handler::HandlerInfo;
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

/// 列出所有 Handler（内置，始终启用）
#[tauri::command]
pub async fn list_handlers(state: State<'_, AppState>) -> Result<Vec<HandlerInfo>, CommandError> {
    log::info!("list_handlers: 查询所有 Handler");
    let handlers = {
        let reg = state.handler_registry.lock().await;
        reg.list_handlers()
    };
    log::info!("list_handlers: 查询完成, 共 {} 个 Handler", handlers.len());
    Ok(handlers)
}
