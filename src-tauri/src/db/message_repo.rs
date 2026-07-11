use crate::errors::CommandError;
use crate::models::{AttachmentMeta, Message, MessageRole, ToolCall};
use chrono::Utc;
use rusqlite::Connection;

#[allow(clippy::too_many_arguments)]
pub fn create_message(
    conn: &Connection,
    id: &str,
    session_id: &str,
    role: &str,
    content: &str,
    tool_name: Option<&str>,
    tool_args: Option<&str>,
    tool_result: Option<&str>,
    tool_call_id: Option<&str>,
    thinking_content: Option<&str>,
    reasoning_content: Option<&str>,
    attachments: Option<&[AttachmentMeta]>,
    metadata: Option<&str>,
) -> Result<(), CommandError> {
    let now = Utc::now().to_rfc3339();
    // 附件序列化为 JSON 字符串存储
    let attachments_json =
        attachments.map(|atts| serde_json::to_string(atts).unwrap_or_else(|_| "[]".to_string()));
    conn.execute(
        "INSERT INTO session_messages
            (id, session_id, role, content, tool_name, tool_args, tool_result,
             tool_call_id, thinking_content, reasoning_content, attachments, metadata, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        rusqlite::params![
            id,
            session_id,
            role,
            content,
            tool_name,
            tool_args,
            tool_result,
            tool_call_id,
            thinking_content,
            reasoning_content,
            attachments_json,
            metadata,
            now,
        ],
    )?;
    Ok(())
}

pub fn list_messages(conn: &Connection, session_id: &str) -> Vec<Message> {
    let mut stmt = match conn.prepare(
        "SELECT id, session_id, role, content, tool_name, tool_args, tool_result,
                tool_call_id, thinking_content, reasoning_content, attachments, metadata, created_at
         FROM session_messages
         WHERE session_id = ?1
         ORDER BY created_at ASC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut rows = match stmt.query(rusqlite::params![session_id]) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut result = Vec::new();
    while let Ok(Some(row)) = rows.next() {
        let role_str: String = match row.get(2) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let content: String = match row.get(3) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let tool_name: Option<String> = row.get(4).ok().flatten();
        let tool_args: Option<String> = row.get(5).ok().flatten();
        let tool_result: Option<String> = row.get(6).ok().flatten();
        let tool_call_id: Option<String> = row.get(7).ok().flatten();
        let reasoning_content: Option<String> = row.get(9).ok().flatten();
        let attachments_json: Option<String> = row.get(10).ok().flatten();
        let metadata_json: Option<String> = row.get(11).ok().flatten();
        let msg_id: String = match row.get(0) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let created_at: String = match row.get(12) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // 反序列化附件
        let attachments: Option<Vec<AttachmentMeta>> = attachments_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .filter(|atts: &Vec<AttachmentMeta>| !atts.is_empty());

        let message_role = match role_str.as_str() {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "tool" => MessageRole::Tool,
            _ => MessageRole::User,
        };

        let tool_calls = if role_str == "tool" {
            // tool 消息：优先使用数据库中存储的 tool_call_id
            // 如果 tool_call_id 不存在（旧数据），回退到使用 msg_id
            let call_id = tool_call_id.unwrap_or_else(|| msg_id.clone());
            let name = tool_name.unwrap_or_default();
            let arguments = tool_args
                .and_then(|args| serde_json::from_str(&args).ok())
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            let result_val = tool_result.and_then(|res| serde_json::from_str(&res).ok());
            Some(vec![ToolCall {
                id: call_id,
                name,
                arguments,
                result: result_val,
            }])
        } else if role_str == "assistant" {
            match (tool_name, tool_args) {
                (Some(ref name_str), Some(ref args_str)) => {
                    // 尝试解析为多个 tool_calls（JSON 数组格式）
                    if let Ok(names) = serde_json::from_str::<Vec<String>>(name_str) {
                        if let Ok(args_list) = serde_json::from_str::<Vec<String>>(args_str) {
                            // 尝试从 tool_call_id 字段恢复原始 id 列表
                            let ids: Vec<String> = tool_call_id
                                .as_ref()
                                .and_then(|id_str| serde_json::from_str::<Vec<String>>(id_str).ok())
                                .unwrap_or_else(|| {
                                    // 旧数据回退：使用 msg_id_index 格式
                                    names
                                        .iter()
                                        .enumerate()
                                        .map(|(i, _)| format!("{}_{}", msg_id, i))
                                        .collect()
                                });

                            let calls: Vec<ToolCall> = names
                                .iter()
                                .zip(args_list.iter())
                                .zip(ids.iter())
                                .map(|((name, args), id)| {
                                    let arguments = serde_json::from_str(args).unwrap_or(
                                        serde_json::Value::Object(serde_json::Map::new()),
                                    );
                                    ToolCall {
                                        id: id.clone(),
                                        name: name.clone(),
                                        arguments,
                                        result: None,
                                    }
                                })
                                .collect();
                            if !calls.is_empty() {
                                Some(calls)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        // 单个 tool_call
                        let arguments = serde_json::from_str(args_str)
                            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                        // 优先使用 tool_call_id 字段中存储的原始 id
                        let call_id = tool_call_id.unwrap_or_else(|| msg_id.clone());
                        Some(vec![ToolCall {
                            id: call_id,
                            name: name_str.clone(),
                            arguments,
                            result: None,
                        }])
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        result.push(Message {
            id: msg_id,
            role: message_role,
            content,
            tool_calls,
            reasoning_content,
            attachments,
            metadata: metadata_json.and_then(|json| serde_json::from_str(&json).ok()),
            created_at,
        });
    }
    result
}

pub fn delete_messages_by_session(conn: &Connection, session_id: &str) -> Result<(), CommandError> {
    conn.execute(
        "DELETE FROM session_messages WHERE session_id = ?1",
        rusqlite::params![session_id],
    )?;
    Ok(())
}
