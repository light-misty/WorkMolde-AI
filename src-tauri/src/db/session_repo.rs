use rusqlite::Connection;
use chrono::Utc;
use crate::errors::CommandError;
use crate::models::{Session, SessionSummary};

/// 创建新会话
pub fn create_session(
    conn: &Connection,
    id: &str,
    workspace_id: &str,
    title: &str,
    provider: &str,
    model: &str,
) -> Result<(), CommandError> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO sessions (id, workspace_id, title, created_at, updated_at, llm_provider, llm_model)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, workspace_id, title, now, now, provider, model],
    )?;
    Ok(())
}

/// 根据 ID 获取会话
pub fn get_session(conn: &Connection, id: &str) -> Result<Session, CommandError> {
    conn.query_row(
        "SELECT id, workspace_id, title, created_at, updated_at, llm_provider, llm_model
         FROM sessions WHERE id = ?1",
        rusqlite::params![id],
        |row| {
            let workspace_id: String = row.get(1)?;
            let provider_id: String = row.get(5)?;
            Ok(Session {
                id: row.get(0)?,
                workspace_id: if workspace_id.is_empty() {
                    None
                } else {
                    Some(workspace_id)
                },
                title: row.get(2)?,
                provider_id,
                template_id: None,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                status: String::from("active"),
            })
        },
    )
    .map_err(Into::into)
}

/// 查询会话列表，支持按工作区、状态、关键词筛选
pub fn list_sessions(
    conn: &Connection,
    workspace_id: Option<&str>,
    _status: Option<&str>,
    search: Option<&str>,
    limit: u32,
    offset: u32,
) -> Vec<SessionSummary> {
    let mut sql = String::from(
        "SELECT s.id, s.title, s.updated_at, s.created_at, s.workspace_id,
                (SELECT COUNT(*) FROM session_messages WHERE session_id = s.id) AS message_count,
                (SELECT content FROM session_messages WHERE session_id = s.id ORDER BY created_at DESC LIMIT 1) AS last_message_preview
         FROM sessions s WHERE 1=1"
    );
    let mut param_idx = 1u32;
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(wid) = workspace_id {
        sql.push_str(&format!(" AND s.workspace_id = ?{}", param_idx));
        param_values.push(Box::new(wid.to_string()));
        param_idx += 1;
    }

    if let Some(keyword) = search {
        sql.push_str(&format!(" AND s.title LIKE ?{}", param_idx));
        param_values.push(Box::new(format!("%{}%", keyword)));
        param_idx += 1;
    }

    sql.push_str(&format!(
        " ORDER BY s.updated_at DESC LIMIT ?{} OFFSET ?{}",
        param_idx,
        param_idx + 1
    ));
    param_values.push(Box::new(limit));
    param_values.push(Box::new(offset));

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
        let workspace_id: String = row.get(4).unwrap_or_default();
        let summary = SessionSummary {
            id: match row.get(0) {
                Ok(v) => v,
                Err(_) => continue,
            },
            title: match row.get(1) {
                Ok(v) => v,
                Err(_) => continue,
            },
            workspace_id: if workspace_id.is_empty() {
                None
            } else {
                Some(workspace_id)
            },
            status: String::from("active"),
            message_count: row.get(5).unwrap_or_default(),
            last_message_preview: row.get(6).ok(),
            created_at: match row.get(3) {
                Ok(v) => v,
                Err(_) => continue,
            },
            updated_at: match row.get(2) {
                Ok(v) => v,
                Err(_) => continue,
            },
        };
        result.push(summary);
    }
    result
}

/// 更新会话的工作区 ID（用于修复旧数据中 workspace_id 为空的会话）
pub fn update_session_workspace(conn: &Connection, id: &str, workspace_id: &str) -> Result<(), CommandError> {
    let affected = conn.execute(
        "UPDATE sessions SET workspace_id = ?1 WHERE id = ?2",
        rusqlite::params![workspace_id, id],
    )?;
    if affected == 0 {
        return Err(CommandError::db(
            crate::errors::DB_RECORD_NOT_FOUND,
            format!("会话不存在: {}", id),
        ));
    }
    Ok(())
}

/// 更新会话标题
pub fn update_session_title(conn: &Connection, id: &str, title: &str) -> Result<(), CommandError> {
    let now = Utc::now().to_rfc3339();
    let affected = conn.execute(
        "UPDATE sessions SET title = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![title, now, id],
    )?;
    if affected == 0 {
        return Err(CommandError::db(
            crate::errors::DB_RECORD_NOT_FOUND,
            format!("会话不存在: {}", id),
        ));
    }
    Ok(())
}

/// 更新会话的 updated_at 时间戳
pub fn update_session_timestamp(conn: &Connection, id: &str) -> Result<(), CommandError> {
    let now = Utc::now().to_rfc3339();
    let affected = conn.execute(
        "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    )?;
    if affected == 0 {
        return Err(CommandError::db(
            crate::errors::DB_RECORD_NOT_FOUND,
            format!("会话不存在: {}", id),
        ));
    }
    Ok(())
}

/// 删除会话（同时删除关联的消息记录）
pub fn delete_session(conn: &Connection, id: &str) -> Result<(), CommandError> {
    conn.execute(
        "DELETE FROM session_messages WHERE session_id = ?1",
        rusqlite::params![id],
    )?;

    let affected = conn.execute(
        "DELETE FROM sessions WHERE id = ?1",
        rusqlite::params![id],
    )?;
    if affected == 0 {
        return Err(CommandError::db(
            crate::errors::DB_RECORD_NOT_FOUND,
            format!("会话不存在: {}", id),
        ));
    }
    Ok(())
}

/// 删除指定工作区下的所有会话（同时删除关联的消息记录）
/// 用于删除工作区时清理关联会话，避免出现孤儿会话导致前端分组错乱
/// 返回被删除的会话 ID 列表，供前端清理本地状态使用
pub fn delete_sessions_by_workspace(conn: &Connection, workspace_id: &str) -> Result<Vec<String>, CommandError> {
    // 先查询将被删除的会话 ID 列表，用于事件通知
    let session_ids: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id FROM sessions WHERE workspace_id = ?1")?;
        let rows = stmt.query_map(rusqlite::params![workspace_id], |row| row.get::<_, String>(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };

    if session_ids.is_empty() {
        return Ok(Vec::new());
    }

    // 删除关联消息记录（使用子查询匹配 workspace_id 对应的会话）
    conn.execute(
        "DELETE FROM session_messages WHERE session_id IN (SELECT id FROM sessions WHERE workspace_id = ?1)",
        rusqlite::params![workspace_id],
    )?;

    // 删除会话记录
    let deleted = conn.execute(
        "DELETE FROM sessions WHERE workspace_id = ?1",
        rusqlite::params![workspace_id],
    )?;

    log::info!(
        "delete_sessions_by_workspace: workspace_id={}, 删除了 {} 条会话",
        workspace_id,
        deleted
    );

    Ok(session_ids)
}

/// 清除所有会话（同时删除所有关联的消息记录）
pub fn clear_all_sessions(conn: &Connection) -> Result<u64, CommandError> {
    // 先统计要删除的会话数量
    let count: u64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
        .unwrap_or(0);

    // 删除所有消息记录
    conn.execute("DELETE FROM session_messages", [])?;
    // 删除所有会话
    conn.execute("DELETE FROM sessions", [])?;

    Ok(count)
}

/// 保存上下文窗口使用信息到会话（持久化 JSON）
pub fn save_context_usage(conn: &Connection, session_id: &str, json: &str) -> Result<(), CommandError> {
    conn.execute(
        "UPDATE sessions SET context_usage_json = ?1 WHERE id = ?2",
        rusqlite::params![json, session_id],
    )?;
    Ok(())
}

/// 读取会话的上下文窗口使用信息（从持久化的 JSON 反序列化）
pub fn load_context_usage(conn: &Connection, session_id: &str) -> Option<crate::models::llm::ContextUsageInfo> {
    let json_str: String = conn
        .query_row(
            "SELECT context_usage_json FROM sessions WHERE id = ?1",
            rusqlite::params![session_id],
            |row| row.get(0),
        )
        .ok()?;

    if json_str.is_empty() {
        return None;
    }

    serde_json::from_str(&json_str).ok()
}
