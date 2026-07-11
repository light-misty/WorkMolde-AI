use serde::{Deserialize, Serialize};

/// 附件类型枚举
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentType {
    Image,
    Document,
    Text,
}

/// 附件元信息 (从前端接收)
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentMeta {
    /// 文件在工作区中的相对路径 (工作区内文件)
    pub path: Option<String>,
    /// 文件绝对路径 (工作区外文件)
    pub absolute_path: Option<String>,
    /// 文件名
    pub name: String,
    /// MIME 类型
    pub mime_type: String,
    /// 文件大小 (字节)
    pub size: u64,
    /// 附件类型
    #[serde(rename = "type")]
    pub attachment_type: AttachmentType,
    /// 文件内容 base64 编码 (浏览器端读取后传入)
    #[serde(default)]
    pub data: Option<String>,
}

/// 消息角色枚举
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum MessageRole {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "system")]
    System,
    #[serde(rename = "tool")]
    Tool,
}

/// 对话消息
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// 附件元信息列表 (JSON 序列化存储)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<AttachmentMeta>>,
    /// 工作流节点扩展信息 (JSON 格式，用于持久化 question/confirm/error 节点详情)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
}

/// 工具调用记录
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    pub result: Option<serde_json::Value>,
}

impl Message {
    /// 将数据库消息模型转换为 LLM ChatMessage
    /// 用于从历史消息恢复 Agent 上下文
    pub fn to_chat_message(&self) -> Option<crate::models::llm::ChatMessage> {
        // 跳过错误节点消息（不发送给 LLM）
        if let Some(ref meta) = self.metadata {
            if meta.get("nodeType").and_then(|v| v.as_str()) == Some("error") {
                return None;
            }
        }
        match self.role {
            MessageRole::User => Some(crate::models::llm::ChatMessage {
                role: "user".to_string(),
                content: self.content.clone(),
                content_parts: None,
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
                attachments: None,
                metadata: None,
            }),
            MessageRole::Assistant => {
                // 将数据库 ToolCall 转换为 LlmToolCall
                let llm_tool_calls = self.tool_calls.as_ref().map(|calls| {
                    calls
                        .iter()
                        .map(|tc| {
                            crate::models::llm::LlmToolCall {
                                index: 0,
                                id: tc.id.clone(),
                                name: tc.name.clone(),
                                // 数据库中 arguments 是 serde_json::Value，需要转为 JSON 字符串
                                arguments: serde_json::to_string(&tc.arguments).unwrap_or_default(),
                            }
                        })
                        .collect::<Vec<_>>()
                });

                // 如果有 tool_calls 但全部转换失败，则跳过此消息
                if let Some(ref calls) = llm_tool_calls {
                    if calls.is_empty() {
                        log::warn!(
                            "助手消息的 tool_calls 转换结果为空，跳过: msg_id={}",
                            self.id
                        );
                        return None;
                    }
                }

                Some(crate::models::llm::ChatMessage {
                    role: "assistant".to_string(),
                    content: self.content.clone(),
                    content_parts: None,
                    tool_calls: llm_tool_calls,
                    tool_call_id: None,
                    reasoning_content: self.reasoning_content.clone(),
                    attachments: None,
                    metadata: None,
                })
            }
            MessageRole::Tool => {
                // tool 消息需要从 ToolCall 中提取 call_id
                // 数据库中 tool 消息的 tool_calls 字段存储了对应的 ToolCall 信息
                let call_id = self
                    .tool_calls
                    .as_ref()
                    .and_then(|calls| calls.first())
                    .map(|tc| tc.id.clone())
                    .unwrap_or_default();

                if call_id.is_empty() {
                    log::warn!("tool 消息缺少 call_id，跳过: msg_id={}", self.id);
                    return None;
                }

                Some(crate::models::llm::ChatMessage {
                    role: "tool".to_string(),
                    content: self.content.clone(),
                    content_parts: None,
                    tool_calls: None,
                    tool_call_id: Some(call_id),
                    reasoning_content: None,
                    attachments: None,
                    metadata: None,
                })
            }
            MessageRole::System => {
                // 系统消息通常不注入历史，跳过
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用户消息转换为 ChatMessage
    #[test]
    fn test_to_chat_message_user() {
        let msg = Message {
            id: "msg_1".to_string(),
            role: MessageRole::User,
            content: "帮我生成一份周报".to_string(),
            tool_calls: None,
            reasoning_content: None,
            attachments: None,
            metadata: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let chat_msg = msg.to_chat_message().unwrap();
        assert_eq!(chat_msg.role, "user");
        assert_eq!(chat_msg.content, "帮我生成一份周报");
        assert!(chat_msg.tool_calls.is_none());
        assert!(chat_msg.tool_call_id.is_none());
    }

    /// 测试助手消息（无工具调用）转换为 ChatMessage
    #[test]
    fn test_to_chat_message_assistant_no_tools() {
        let msg = Message {
            id: "msg_2".to_string(),
            role: MessageRole::Assistant,
            content: "好的，我来帮你生成周报".to_string(),
            tool_calls: None,
            reasoning_content: Some("思考中...".to_string()),
            attachments: None,
            metadata: None,
            created_at: "2026-01-01T00:00:01Z".to_string(),
        };

        let chat_msg = msg.to_chat_message().unwrap();
        assert_eq!(chat_msg.role, "assistant");
        assert_eq!(chat_msg.content, "好的，我来帮你生成周报");
        assert!(chat_msg.tool_calls.is_none());
        assert_eq!(chat_msg.reasoning_content, Some("思考中...".to_string()));
    }

    /// 测试助手消息（含工具调用）转换为 ChatMessage
    #[test]
    fn test_to_chat_message_assistant_with_tools() {
        let msg = Message {
            id: "msg_3".to_string(),
            role: MessageRole::Assistant,
            content: "".to_string(),
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                name: "write_text_file".to_string(),
                arguments: serde_json::json!({"path": "周报.md", "content": "# 项目周报"}),
                result: None,
            }]),
            reasoning_content: None,
            attachments: None,
            metadata: None,
            created_at: "2026-01-01T00:00:02Z".to_string(),
        };

        let chat_msg = msg.to_chat_message().unwrap();
        assert_eq!(chat_msg.role, "assistant");
        let tool_calls = chat_msg.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "write_text_file");
        assert_eq!(tool_calls[0].id, "call_1");
        // arguments 应该是 JSON 字符串
        let args: serde_json::Value = serde_json::from_str(&tool_calls[0].arguments).unwrap();
        assert_eq!(args["path"], "周报.md");
    }

    /// 测试 tool 消息转换为 ChatMessage
    #[test]
    fn test_to_chat_message_tool() {
        let msg = Message {
            id: "msg_4".to_string(),
            role: MessageRole::Tool,
            content: "文档已生成".to_string(),
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                name: "docx_handler".to_string(),
                arguments: serde_json::json!({}),
                result: Some(serde_json::json!({"success": true})),
            }]),
            reasoning_content: None,
            attachments: None,
            metadata: None,
            created_at: "2026-01-01T00:00:03Z".to_string(),
        };

        let chat_msg = msg.to_chat_message().unwrap();
        assert_eq!(chat_msg.role, "tool");
        assert_eq!(chat_msg.content, "文档已生成");
        assert_eq!(chat_msg.tool_call_id, Some("call_1".to_string()));
        assert!(chat_msg.tool_calls.is_none());
    }

    /// 测试系统消息转换为 ChatMessage 返回 None
    #[test]
    fn test_to_chat_message_system_returns_none() {
        let msg = Message {
            id: "msg_0".to_string(),
            role: MessageRole::System,
            content: "你是助手".to_string(),
            tool_calls: None,
            reasoning_content: None,
            attachments: None,
            metadata: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        assert!(msg.to_chat_message().is_none());
    }

    /// 测试 tool 消息缺少 call_id 时返回 None
    #[test]
    fn test_to_chat_message_tool_missing_call_id() {
        let msg = Message {
            id: "msg_5".to_string(),
            role: MessageRole::Tool,
            content: "结果".to_string(),
            tool_calls: None,
            reasoning_content: None,
            attachments: None,
            metadata: None,
            created_at: "2026-01-01T00:00:04Z".to_string(),
        };

        assert!(msg.to_chat_message().is_none());
    }

    /// 测试助手消息含多个工具调用
    #[test]
    fn test_to_chat_message_assistant_multiple_tools() {
        let msg = Message {
            id: "msg_6".to_string(),
            role: MessageRole::Assistant,
            content: "".to_string(),
            tool_calls: Some(vec![
                ToolCall {
                    id: "call_a".to_string(),
                    name: "docx_handler".to_string(),
                    arguments: serde_json::json!({"action": "read", "path": "a.docx"}),
                    result: None,
                },
                ToolCall {
                    id: "call_b".to_string(),
                    name: "xlsx_handler".to_string(),
                    arguments: serde_json::json!({"action": "read", "path": "b.xlsx"}),
                    result: None,
                },
            ]),
            reasoning_content: None,
            attachments: None,
            metadata: None,
            created_at: "2026-01-01T00:00:05Z".to_string(),
        };

        let chat_msg = msg.to_chat_message().unwrap();
        let tool_calls = chat_msg.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 2);
        assert_eq!(tool_calls[0].name, "docx_handler");
        assert_eq!(tool_calls[1].name, "xlsx_handler");
    }
}
