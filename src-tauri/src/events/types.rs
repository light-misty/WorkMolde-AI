use serde::{Deserialize, Serialize};

// ================================================================
// Agent 事件名常量
// ================================================================

pub const AGENT_THINKING: &str = "agent:thinking";
pub const AGENT_CONTENT: &str = "agent:content";
pub const AGENT_TOOL_CALL: &str = "agent:tool_call";
pub const AGENT_TOOL_RESULT: &str = "agent:tool_result";
pub const AGENT_CONFIRM: &str = "agent:confirm";
pub const AGENT_TODO_UPDATE: &str = "agent:todo_update";
pub const AGENT_DONE: &str = "agent:done";
pub const AGENT_ERROR: &str = "agent:error";
pub const AGENT_STOPPED: &str = "agent:stopped";

// ================================================================
// 系统事件名常量
// ================================================================

pub const SESSION_UPDATED: &str = "session:updated";
pub const WORKSPACE_CHANGE: &str = "workspace:change";
pub const FILE_CHANGE: &str = "file:change";
pub const TOKEN_UPDATE: &str = "token:update";
pub const LLM_PROVIDER_SWITCH: &str = "llm:provider_switch";

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

/// Agent 回复内容增量
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentPayload {
    pub session_id: String,
    pub message_id: String,
    pub content: String,
    /// 是否为流式输出的中间片段
    pub is_streaming: bool,
}

/// Tool 调用开始
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallPayload {
    pub session_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
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

/// Todo 列表中的条目
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    /// 任务状态: "pending" | "in_progress" | "completed"
    pub status: String,
}

/// Todo 列表更新
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TodoUpdatePayload {
    pub session_id: String,
    pub todos: Vec<TodoItem>,
}

/// Agent 执行完成
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DonePayload {
    pub session_id: String,
    pub summary: String,
    pub total_steps: u32,
    pub total_tokens: u64,
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

/// 工作区变更事件
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceChangePayload {
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

/// Token 用量更新事件
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TokenUpdatePayload {
    pub session_id: String,
    pub provider_id: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_cost: f64,
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
