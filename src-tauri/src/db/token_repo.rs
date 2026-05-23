use rusqlite::Connection;
use chrono::{Utc, Datelike, Duration};
use crate::errors::CommandError;

/// 每日用量统计项
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsageItem {
    pub date: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
}

/// 按 Provider/Model 分组的用量统计项
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsageItem {
    pub provider: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
}

/// Token 用量概览
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsageOverview {
    pub total_input: i64,
    pub total_output: i64,
    pub today_input: i64,
    pub today_output: i64,
    pub month_input: i64,
    pub month_output: i64,
}

/// 记录一次 Token 使用的参数
pub struct RecordUsageParams<'a> {
    pub id: &'a str,
    pub session_id: &'a str,
    pub workspace_id: &'a str,
    pub provider: &'a str,
    pub model: &'a str,
    pub input_tokens: i64,
    pub output_tokens: i64,
}

/// 记录一次 Token 使用明细
pub fn record_usage(
    conn: &Connection,
    params: &RecordUsageParams,
) -> Result<(), CommandError> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO token_usage
            (id, session_id, workspace_id, llm_provider, llm_model, input_tokens, output_tokens, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            params.id, params.session_id, params.workspace_id,
            params.provider, params.model, params.input_tokens,
            params.output_tokens, now
        ],
    )?;
    Ok(())
}

/// 获取指定会话的累计 Token 用量，返回 (input_tokens, output_tokens)
pub fn get_session_usage(conn: &Connection, session_id: &str) -> (i64, i64) {
    conn.query_row(
        "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0)
         FROM token_usage WHERE session_id = ?1",
        rusqlite::params![session_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .unwrap_or((0, 0))
}

/// 获取指定日期的 Token 用量，返回 (input_tokens, output_tokens)
/// date 参数格式为 "YYYY-MM-DD"
pub fn get_daily_usage(conn: &Connection, workspace_id: Option<&str>, date: &str) -> (i64, i64) {
    let start = format!("{}T00:00:00.000Z", date);
    let end = format!("{}T23:59:59.999Z", date);

    if let Some(wid) = workspace_id {
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0)
             FROM token_usage
             WHERE workspace_id = ?1 AND created_at >= ?2 AND created_at <= ?3",
            rusqlite::params![wid, start, end],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((0, 0))
    } else {
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0)
             FROM token_usage
             WHERE created_at >= ?1 AND created_at <= ?2",
            rusqlite::params![start, end],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((0, 0))
    }
}

/// 按 Provider + Model 分组统计 Token 用量
/// 返回 Vec<(provider, model, total_input, total_output)>
pub fn get_usage_by_provider(
    conn: &Connection,
    start: Option<&str>,
    end: Option<&str>,
) -> Vec<(String, String, i64, i64)> {
    let mut sql = String::from(
        "SELECT llm_provider, llm_model,
                COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0)
         FROM token_usage WHERE 1=1"
    );
    let mut param_idx = 1u32;
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(s) = start {
        sql.push_str(&format!(" AND created_at >= ?{}", param_idx));
        param_values.push(Box::new(format!("{}T00:00:00.000Z", s)));
        param_idx += 1;
    }

    if let Some(e) = end {
        sql.push_str(&format!(" AND created_at <= ?{}", param_idx));
        param_values.push(Box::new(format!("{}T23:59:59.999Z", e)));
    }

    sql.push_str(" GROUP BY llm_provider, llm_model ORDER BY llm_provider, llm_model");

    let params: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut rows = match stmt.query(params.as_slice()) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut result = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        let provider: String = match row.get(0) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let model: String = match row.get(1) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let total_input: i64 = row.get(2).unwrap_or_default();
        let total_output: i64 = row.get(3).unwrap_or_default();
        result.push((provider, model, total_input, total_output));
    }
    result
}

/// 获取最近 N 天的每日用量趋势
/// 返回按日期升序排列的每日统计列表，无数据的日期补零
pub fn get_usage_trend(
    conn: &Connection,
    workspace_id: Option<&str>,
    days: u32,
) -> Vec<DailyUsageItem> {
    let mut result = Vec::new();
    let today = Utc::now().date_naive();

    for i in (0..days).rev() {
        let date = today - Duration::days(i as i64);
        let date_str = date.format("%Y-%m-%d").to_string();
        let (input, output) = get_daily_usage(conn, workspace_id, &date_str);
        result.push(DailyUsageItem {
            date: date_str,
            input_tokens: input,
            output_tokens: output,
        });
    }

    result
}

/// 按 Provider/Model 分组统计用量，返回结构化结果
pub fn get_provider_usage(
    conn: &Connection,
    start: Option<&str>,
    end: Option<&str>,
) -> Vec<ProviderUsageItem> {
    let raw = get_usage_by_provider(conn, start, end);
    raw.into_iter()
        .map(|(provider, model, input_tokens, output_tokens)| ProviderUsageItem {
            provider,
            model,
            input_tokens,
            output_tokens,
        })
        .collect()
}

/// 获取 Token 用量概览（总量、今日、本月）
pub fn get_usage_overview(conn: &Connection, workspace_id: Option<&str>) -> TokenUsageOverview {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let now = Utc::now();
    let month_start = format!("{:04}-{:02}-01", now.year(), now.month());

    let (total_input, total_output) = if let Some(wid) = workspace_id {
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0)
             FROM token_usage WHERE workspace_id = ?1",
            rusqlite::params![wid],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((0, 0))
    } else {
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0)
             FROM token_usage",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((0, 0))
    };

    let (today_input, today_output) = get_daily_usage(conn, workspace_id, &today);

    let month_start_ts = format!("{}T00:00:00.000Z", month_start);
    let (month_input, month_output) = if let Some(wid) = workspace_id {
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0)
             FROM token_usage WHERE workspace_id = ?1 AND created_at >= ?2",
            rusqlite::params![wid, month_start_ts],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((0, 0))
    } else {
        conn.query_row(
            "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0)
             FROM token_usage WHERE created_at >= ?1",
            rusqlite::params![month_start_ts],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((0, 0))
    };

    TokenUsageOverview {
        total_input,
        total_output,
        today_input,
        today_output,
        month_input,
        month_output,
    }
}
