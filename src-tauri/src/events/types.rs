use serde::{Deserialize, Serialize};

// ================================================================
// Agent 事件名常量
// ================================================================

pub const AGENT_THINKING: &str = "agent:thinking";
pub const AGENT_DEEP_THINKING: &str = "agent:deep_thinking";
pub const AGENT_CONTENT: &str = "agent:content";
pub const AGENT_TOOL_CALL: &str = "agent:tool_call";
pub const AGENT_TOOL_RESULT: &str = "agent:tool_result";
pub const AGENT_CONFIRM: &str = "agent:confirm";
pub const AGENT_DONE: &str = "agent:done";
pub const AGENT_ERROR: &str = "agent:error";
pub const AGENT_STOPPED: &str = "agent:stopped";
pub const AGENT_CONTEXT_UPDATE: &str = "agent:context_update";
pub const AGENT_NETWORK_RETRY: &str = "agent:network_retry";

// ================================================================
// 系统事件名常量
// ================================================================

pub const SESSION_UPDATED: &str = "session:updated";
pub const WORKSPACE_DIRECTORY_DELETED: &str = "workspace:directory_deleted";
pub const FILE_CHANGE: &str = "file:change";
pub const LLM_PROVIDER_SWITCH: &str = "llm:provider_switch";
pub const SYSTEM_NETWORK_CHANGE: &str = "system:network_change";

// ================================================================
// Agent 事件 Payload 类型
// ================================================================

/// Agent 思考链增量
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingPayload {
    pub session_id: String,
    /// 当前思考步骤序号
    pub step: u32,
    pub thought: String,
}

/// Agent 深度思考链增量（Extended Thinking / reasoning_content）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeepThinkingPayload {
    pub session_id: String,
    pub step: u32,
    pub thought: String,
    pub is_streaming: bool,
    /// 当前迭代轮次序号（从 1 开始），用于前端按迭代分组展示
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration: Option<u32>,
}

/// Agent 回复内容增量
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentPayload {
    pub session_id: String,
    pub message_id: String,
    pub content: String,
    /// 是否为流式输出的中间片段
    pub is_streaming: bool,
    /// 当前迭代轮次序号（从 1 开始），用于前端按迭代分组展示
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration: Option<u32>,
}

/// Tool 调用开始
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallPayload {
    pub session_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    /// 当前迭代轮次序号（从 1 开始），用于前端按迭代分组展示
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration: Option<u32>,
}

/// Tool 执行结果
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultPayload {
    pub session_id: String,
    pub call_id: String,
    pub success: bool,
    pub result: serde_json::Value,
    pub error: Option<String>,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
}

/// 需要用户确认的操作
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmPayload {
    pub session_id: String,
    pub operation_id: String,
    pub operation_type: String,
    pub description: String,
    pub details: serde_json::Value,
    /// 风险等级: "low" | "medium" | "high" | "critical"
    pub risk_level: String,
}

/// Agent 执行完成
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DonePayload {
    pub session_id: String,
    pub summary: String,
    pub total_steps: u32,
    /// 总耗时（毫秒）
    pub duration_ms: u64,
}

/// Agent 执行错误
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ErrorPayload {
    pub session_id: String,
    pub code: u32,
    pub message: String,
    /// 是否可恢复
    pub recoverable: bool,
}

/// Agent 网络重试事件
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NetworkRetryPayload {
    pub session_id: String,
    pub attempt: u32,
    pub max_attempts: u32,
    pub reason: String,
}

/// Agent 执行中断
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StoppedPayload {
    pub session_id: String,
    pub completed_steps: u32,
    pub reason: String,
}

// ================================================================
// 系统事件 Payload 类型
// ================================================================

/// 会话更新事件
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdatePayload {
    pub session_id: String,
    /// 变更类型: "created" | "updated" | "deleted"
    pub change_type: String,
    pub data: Option<serde_json::Value>,
}

/// 工作区目录被外部删除事件
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDirectoryDeletedPayload {
    pub workspace_id: String,
    pub workspace_name: String,
    pub workspace_path: String,
}

/// 文件变更事件
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileChangePayload {
    pub workspace_id: String,
    /// 变更类型: "created" | "modified" | "deleted" | "renamed"
    pub change_type: String,
    pub path: String,
    /// 重命名时的旧路径
    pub old_path: Option<String>,
}

/// 网络状态变化事件
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NetworkChangePayload {
    /// 当前网络状态: "online" | "offline"
    pub status: String,
    /// 之前的网络状态
    pub previous_status: String,
}

// ================================================================
// LLM 事件 Payload 类型
// ================================================================

/// LLM Provider 切换通知
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSwitchPayload {
    /// 原始 Provider ID
    pub from_provider_id: String,
    /// 切换到的 Provider ID
    pub to_provider_id: String,
    /// 切换原因
    pub reason: String,
    /// 是否为自动切换
    pub is_automatic: bool,
}

// ================================================================
// 上下文窗口事件 Payload 类型
// ================================================================

/// 上下文窗口使用情况更新事件
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContextUsagePayload {
    pub session_id: String,
    /// 上下文使用详情
    pub context_usage: crate::models::llm::ContextUsageInfo,
}
