use rusqlite::Connection;
use chrono::Utc;
use crate::errors::CommandError;
use crate::models::{Message, MessageRole, ToolCall};

/// 创建新消息
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
    thinking_content: Option<&str>,
    input_tokens: i64,
    output_tokens: i64,
) -> Result<(), CommandError> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO session_messages
            (id, session_id, role, content, tool_name, tool_args, tool_result,
             thinking_content, input_tokens, output_tokens, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            id,
            session_id,
            role,
            content,
            tool_name,
            tool_args,
            tool_result,
            thinking_content,
            input_tokens,
            output_tokens,
            now,
        ],
    )?;
    Ok(())
}

/// 查询指定会话的所有消息，按创建时间升序排列
pub fn list_messages(conn: &Connection, session_id: &str) -> Vec<Message> {
    let mut stmt = match conn.prepare(
        "SELECT id, session_id, role, content, tool_name, tool_args, tool_result,
                thinking_content, input_tokens, output_tokens, created_at
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
        let msg_id: String = match row.get(0) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let created_at: String = match row.get(10) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // 将数据库 role 映射到 MessageRole 枚举
        let message_role = match role_str.as_str() {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "tool" => MessageRole::Tool,
            _ => MessageRole::User,
        };

        // 当 role 为 tool 时，构造 ToolCall 对象
        let tool_calls = if role_str == "tool" {
            let name = tool_name.unwrap_or_default();
            let arguments = tool_args
                .and_then(|args| serde_json::from_str(&args).ok())
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            let result_val = tool_result
                .and_then(|res| serde_json::from_str(&res).ok());
            Some(vec![ToolCall {
                id: msg_id.clone(),
                name,
                arguments,
                result: result_val,
            }])
        } else if role_str == "assistant" {
            // 助手消息可能包含 tool_calls
            // 存储格式：
            //   单个 tool_call: tool_name = "名称", tool_args = "参数JSON"
            //   多个 tool_calls: tool_name = "[\"名称1\",\"名称2\"]", tool_args = "[\"参数1\",\"参数2\"]"
            match (tool_name, tool_args) {
                (Some(ref name_str), Some(ref args_str)) => {
                    // 尝试解析为多个 tool_calls（名称是 JSON 数组）
                    if let Ok(names) = serde_json::from_str::<Vec<String>>(name_str) {
                        if let Ok(args_list) = serde_json::from_str::<Vec<String>>(args_str) {
                            let calls: Vec<ToolCall> = names.iter().zip(args_list.iter())
                                .enumerate()
                                .map(|(i, (name, args))| {
                                    let arguments = serde_json::from_str(args)
                                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                                    ToolCall {
                                        id: format!("{}_{}", msg_id, i),
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
                        // 单个 tool_call：tool_name 是名称字符串，tool_args 是参数 JSON
                        let arguments = serde_json::from_str(args_str)
                            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                        Some(vec![ToolCall {
                            id: msg_id.clone(),
                            name: name_str.clone(),
                            arguments,
                            result: None,
                        }])
                    }
                }
                _ => None,
            }
        } else {
            None }
;

        result.push(Message {
            id: msg_id,
            role: message_role,
            content,
            tool_calls,
            created_at,
        });
    }
    result
}

/// 删除指定会话的所有消息
pub fn delete_messages_by_session(
    conn: &Connection,
    session_id: &str,
) -> Result<(), CommandError> {
    conn.execute(
        "DELETE FROM session_messages WHERE session_id = ?1",
        rusqlite::params![session_id],
    )?;
    Ok(())
}
