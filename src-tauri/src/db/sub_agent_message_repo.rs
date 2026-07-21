use crate::models::llm::ChatMessage;
use crate::models::{AttachmentMeta, Message, MessageRole, ToolCall};
use chrono::Utc;
use rusqlite::Connection;

/// 创建子Agent消息记录
/// 将 ChatMessage 的字段写入 sub_agent_messages 表
pub fn create_sub_agent_message(
    conn: &Connection,
    parent_session_id: &str,
    agent_id: &str,
    seq: u32,
    msg: &ChatMessage,
) -> Result<(), rusqlite::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();

    // 处理 tool_calls 字段，参考 agent.rs 中的写入模式
    let (tool_name, tool_args, tool_result, tool_call_id) =
        if let Some(tool_calls) = &msg.tool_calls {
            if tool_calls.is_empty() {
                (None, None, None, None)
            } else if tool_calls.len() == 1 {
                // 单个 tool_call：保持原有格式，同时保存 tool_call id
                let tc = &tool_calls[0];
                (
                    Some(tc.name.clone()),
                    Some(tc.arguments.clone()),
                    None,
                    Some(tc.id.clone()),
                )
            } else {
                // 多个 tool_calls：将所有调用信息序列化为 JSON 数组
                let ids: Vec<&str> = tool_calls.iter().map(|tc| tc.id.as_str()).collect();
                let names: Vec<&str> = tool_calls.iter().map(|tc| tc.name.as_str()).collect();
                let args: Vec<&str> = tool_calls.iter().map(|tc| tc.arguments.as_str()).collect();
                (
                    Some(serde_json::to_string(&names).unwrap_or_default()),
                    Some(serde_json::to_string(&args).unwrap_or_default()),
                    None,
                    Some(serde_json::to_string(&ids).unwrap_or_default()),
                )
            }
        } else if msg.role == "tool" {
            // tool 消息：content 作为 tool_result，tool_call_id 从 msg.tool_call_id 提取
            (
                None,
                None,
                Some(msg.content.clone()),
                msg.tool_call_id.clone(),
            )
        } else {
            (None, None, None, None)
        };

    // 序列化 attachments 为 JSON 字符串
    let attachments_json = msg
        .attachments
        .as_ref()
        .map(|atts| serde_json::to_string(atts).unwrap_or_else(|_| "[]".to_string()));

    // 序列化 metadata 为 JSON 字符串
    let metadata_json = msg
        .metadata
        .as_ref()
        .map(|m| serde_json::to_string(m).unwrap_or_default());

    conn.execute(
        "INSERT INTO sub_agent_messages
            (id, parent_session_id, agent_id, role, content, tool_name, tool_args,
             tool_result, tool_call_id, reasoning_content,
             attachments, metadata, created_at, seq)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        rusqlite::params![
            id,
            parent_session_id,
            agent_id,
            msg.role.as_str(),
            msg.content.as_str(),
            tool_name,
            tool_args,
            tool_result,
            tool_call_id,
            msg.reasoning_content.as_deref(),
            attachments_json,
            metadata_json,
            now,
            seq,
        ],
    )?;
    Ok(())
}

/// 查询指定子Agent的所有消息
/// 按 seq 升序返回，反序列化为 Message 模型
pub fn list_sub_agent_messages(
    conn: &Connection,
    agent_id: &str,
) -> Result<Vec<Message>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, role, content, tool_name, tool_args, tool_result,
                tool_call_id, reasoning_content, attachments, metadata, created_at
         FROM sub_agent_messages
         WHERE agent_id = ?1
         ORDER BY seq ASC",
    )?;

    let mut rows = stmt.query(rusqlite::params![agent_id])?;

    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        let msg_id: String = row.get(0)?;
        let role_str: String = row.get(1)?;
        // content 字段允许 NULL，使用 Option 处理
        let content: String = row.get::<_, Option<String>>(2)?.unwrap_or_default();
        let tool_name: Option<String> = row.get(3)?;
        let tool_args: Option<String> = row.get(4)?;
        let tool_result: Option<String> = row.get(5)?;
        let tool_call_id: Option<String> = row.get(6)?;
        let reasoning_content: Option<String> = row.get(7)?;
        let attachments_json: Option<String> = row.get(8)?;
        let metadata_json: Option<String> = row.get(9)?;
        let created_at_int: i64 = row.get(10)?;

        // 将时间戳转换为 RFC3339 字符串
        let created_at = chrono::DateTime::from_timestamp(created_at_int, 0)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_default();

        // 反序列化附件
        let attachments: Option<Vec<AttachmentMeta>> = attachments_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .filter(|atts: &Vec<AttachmentMeta>| !atts.is_empty());

        let message_role = match role_str.as_str() {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "system" => MessageRole::System,
            "tool" => MessageRole::Tool,
            _ => MessageRole::User,
        };

        // 重建 tool_calls，参考 message_repo::list_messages 的反序列化逻辑
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
            branch_id: None,
            branch_group_id: None,
            created_at,
        });
    }
    Ok(result)
}
