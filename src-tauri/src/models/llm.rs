use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// LLM Provider 配置
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub name: String,
    /// "openai" | "anthropic" | "ollama" | "custom" | "gemini"
    pub provider_type: String,
    pub api_base: String,
    /// API 密钥（加密存储）
    pub api_key: String,
    pub model: String,
    pub extra_params: Option<HashMap<String, serde_json::Value>>,
}

/// Provider 信息
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    /// "openai" | "anthropic" | "ollama" | "custom" | "gemini"
    pub provider_type: String,
    pub api_base: String,
    pub model: String,
    pub is_default: bool,
    pub is_available: bool,
    /// ISO 8601 格式
    pub created_at: String,
    /// 是否已连接（运行时填充）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_connected: Option<bool>,
}

/// 连接测试结果
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionResult {
    pub success: bool,
    /// Provider ID（运行时填充）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    pub latency_ms: u64,
    pub model_info: Option<ModelInfo>,
    /// 返回的模型名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub error_message: Option<String>,
    /// 简短错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 模型信息
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub model_name: String,
    pub max_tokens: u32,
    pub supports_streaming: bool,
    pub supports_tool_call: bool,
}

/// 聊天消息（用于 LLM 请求）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    /// "user" | "assistant" | "system" | "tool"
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Vec<LlmToolCall>>,
    /// tool 消息对应的调用 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// LLM 聊天请求
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub stream: Option<bool>,
    pub tools: Option<Vec<ToolDefinition>>,
}

/// LLM 聊天响应
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub id: String,
    pub choices: Vec<ChatChoice>,
    pub usage: Option<ChatUsage>,
}

/// 聊天选项
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatChoice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

/// Token 用量
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// 流式响应块
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StreamChunk {
    pub id: String,
    pub choices: Vec<StreamChoice>,
}

/// 流式选项
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StreamChoice {
    pub index: u32,
    pub delta: StreamDelta,
    pub finish_reason: Option<String>,
}

/// 流式增量内容
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StreamDelta {
    pub role: Option<String>,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<LlmToolCall>>,
}

/// 工具定义（用于 Function Calling）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// LLM 工具调用
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LlmToolCall {
    /// 流式响应中的索引，用于增量合并
    #[serde(default)]
    pub index: u32,
    pub id: String,
    pub name: String,
    pub arguments: String,
}
