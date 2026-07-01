use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 多模态内容部分 (ChatMessage 的 content 扩展)
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// 文本部分
    Text {
        text: String,
    },
    /// 图片部分 (base64)
    Image {
        mime_type: String,
        data: String,
    },
}

/// supports_vision 默认值：true（默认支持视觉）
fn default_supports_vision() -> bool {
    true
}

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
    /// 上下文窗口大小 (tokens)，None 表示自动推断
    #[serde(default)]
    pub context_window: Option<usize>,
    /// 是否支持视觉/图片多模态
    #[serde(default = "default_supports_vision")]
    pub supports_vision: bool,
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
    pub is_available: bool,
    /// ISO 8601 格式
    pub created_at: String,
    /// 是否已连接（运行时填充）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_connected: Option<bool>,
    /// 上下文窗口大小 (tokens)，运行时计算后的最终值
    pub context_window: usize,
    /// 是否支持视觉/图片多模态
    pub supports_vision: bool,
}

/// 上下文窗口使用信息
/// 用于前端实时展示上下文窗口的使用情况
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContextUsageInfo {
    /// 上下文窗口总大小 (tokens)
    pub context_window: usize,
    /// 系统提示词估算 Token 数
    pub system_prompt_tokens: usize,
    /// 工具定义估算 Token 数（包含 Tool + Handler 两部分）
    pub function_definitions_tokens: usize,
    /// 对话历史估算 Token 数
    pub conversation_tokens: usize,
    /// LLM 响应估算 Token 数（当前轮，迭代完成后估算）
    pub response_tokens: usize,
    /// 已使用 Token 总数
    pub total_used_tokens: usize,
    /// 压缩状态: "normal" | "compressed" | "critical"
    pub compression_status: String,
    /// 当前活跃 Provider 的模型名称
    pub model_name: String,
    /// 对话历史消息总数（压缩前）
    pub total_message_count: usize,
    /// 压缩后保留的消息数
    pub retained_message_count: usize,

    // --- 新增缓存统计字段 ---

    /// 本轮请求的缓存命中 tokens（来自 API 响应）
    #[serde(default)]
    pub cache_hit_tokens: u64,
    /// 本轮请求的缓存未命中 tokens（来自 API 响应）
    #[serde(default)]
    pub cache_miss_tokens: u64,
    /// 本轮请求的缓存创建 tokens（Anthropic）
    #[serde(default)]
    pub cache_creation_tokens: u64,
    /// 生命周期累计缓存命中 tokens
    #[serde(default)]
    pub lifetime_cache_hit_tokens: u64,
    /// 生命周期累计缓存未命中 tokens
    #[serde(default)]
    pub lifetime_cache_miss_tokens: u64,
    /// 缓存命中率（0.0 - 1.0），实时计算
    #[serde(default)]
    pub cache_hit_rate: f64,
    /// Provider 缓存类型标识: "deepseek" | "anthropic" | "gemini" | "none"
    #[serde(default)]
    pub provider_cache_type: String,
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
    /// 纯文本内容 (向后兼容，纯文本消息时使用)
    pub content: String,
    /// 多模态内容部分 (有附件时使用，content 为空或纯文本摘要)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_parts: Option<Vec<ContentPart>>,
    pub tool_calls: Option<Vec<LlmToolCall>>,
    /// tool 消息对应的调用 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// 深度思考链内容（Claude extended thinking / DeepSeek reasoning_content）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// 附件元信息 (用于持久化，不发送给 LLM)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<crate::models::message::AttachmentMeta>>,
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

/// Token 用量（含缓存信息）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,

    // --- 新增缓存字段 ---

    /// DeepSeek: 命中缓存的输入 tokens
    #[serde(default)]
    pub prompt_cache_hit_tokens: u64,
    /// DeepSeek: 未命中缓存的输入 tokens
    #[serde(default)]
    pub prompt_cache_miss_tokens: u64,
    /// Anthropic: 缓存创建消耗的 tokens
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    /// Anthropic: 缓存读取消耗的 tokens
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    /// Gemini: 缓存命中 tokens
    #[serde(default)]
    pub cached_content_token_count: u64,
}

/// 流式响应块（携带可选 usage，仅在最后一个 chunk 中存在）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StreamChunk {
    pub id: String,
    pub choices: Vec<StreamChoice>,
    #[serde(default)]
    pub usage: Option<ChatUsage>,
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
    /// 深度思考链增量（Extended Thinking / reasoning_content）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
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
