use tauri::State;

use crate::db::token_repo;
use crate::errors::CommandError;
use crate::AppState;

/// 获取最近 N 天的 Token 用量趋势
#[tauri::command]
pub async fn get_token_usage_trend(
    workspace_id: Option<String>,
    days: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<token_repo::DailyUsageItem>, CommandError> {
    let days = days.unwrap_or(30).min(90);
    let conn = state.db.conn()?;
    let wid = workspace_id.as_deref();
    let trend = token_repo::get_usage_trend(&conn, wid, days);
    Ok(trend)
}

/// 按 Provider/Model 分组获取 Token 用量
#[tauri::command]
pub async fn get_token_provider_usage(
    start_date: Option<String>,
    end_date: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<token_repo::ProviderUsageItem>, CommandError> {
    let conn = state.db.conn()?;
    let usage = token_repo::get_provider_usage(
        &conn,
        start_date.as_deref(),
        end_date.as_deref(),
    );
    Ok(usage)
}

/// 获取 Token 用量概览
#[tauri::command]
pub async fn get_token_usage_overview(
    workspace_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<token_repo::TokenUsageOverview, CommandError> {
    let conn = state.db.conn()?;
    let overview = token_repo::get_usage_overview(&conn, workspace_id.as_deref());
    Ok(overview)
}
