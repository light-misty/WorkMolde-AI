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
/// 上下文压缩开始事件
pub const AGENT_COMPACTION_START: &str = "agent:compaction_start";
/// 上下文压缩完成事件
pub const AGENT_COMPACTION_DONE: &str = "agent:compaction_done";
/// 子 Agent 状态变更事件
pub const AGENT_SUB_AGENT_STATUS: &str = "agent:sub_agent_status";
/// 子 Agent 工具调用事件
pub const AGENT_SUB_AGENT_TOOL_CALL: &str = "agent:sub_agent_tool_call";
/// 子 Agent 思考链增量事件（流式）
pub const AGENT_SUB_AGENT_THINKING: &str = "agent:sub_agent_thinking";
/// 子 Agent 内容增量事件（流式）
pub const AGENT_SUB_AGENT_CONTENT: &str = "agent:sub_agent_content";
/// 子 Agent 工具执行结果事件
pub const AGENT_SUB_AGENT_TOOL_RESULT: &str = "agent:sub_agent_tool_result";
/// 向用户提问事件
pub const AGENT_QUESTION: &str = "agent:question";

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

/// 上下文压缩开始事件 Payload
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompactionStartPayload {
    pub session_id: String,
    /// 压缩前 token 数
    pub tokens_before: u64,
}

/// 上下文压缩完成事件 Payload
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompactionDonePayload {
    pub session_id: String,
    /// 压缩前 token 数
    pub tokens_before: u64,
    /// 压缩后 token 数
    pub tokens_after: u64,
    /// 是否实际执行了压缩
    pub compacted: bool,
    /// 压缩失败时的错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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
// 子 Agent 事件 Payload 类型
// ================================================================

/// 子 Agent 状态变更事件 Payload
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentStatusPayload {
    /// 父 Agent 会话 ID
    pub parent_session_id: String,
    /// 子 Agent ID
    pub agent_id: String,
    /// 状态: "running" | "completed" | "failed" | "cancelled"
    pub status: String,
    /// 附加消息（如错误信息或结果摘要）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// 任务描述（父 Agent 给子 Agent 的任务指令，来自 SubAgentConfig.task_description）
    pub task_description: String,
    /// 当前迭代次数
    pub iteration: u32,
}

/// 子 Agent 工具调用事件 Payload
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentToolCallPayload {
    /// 父 Agent 会话 ID
    pub parent_session_id: String,
    /// 子 Agent ID
    pub agent_id: String,
    /// 工具调用 ID（用于关联 tool_result）
    pub tool_call_id: String,
    /// 工具名称
    pub tool_name: String,
    /// 工具参数
    pub arguments: serde_json::Value,
    /// 当前迭代次数
    pub iteration: u32,
}

/// 子 Agent 思考链增量事件 Payload（流式）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentThinkingPayload {
    /// 父 Agent 会话 ID
    pub parent_session_id: String,
    /// 子 Agent ID
    pub agent_id: String,
    /// 思考内容增量
    pub content: String,
    /// 是否为流式输出的中间片段
    pub is_streaming: bool,
    /// 当前迭代次数
    pub iteration: u32,
}

/// 子 Agent 内容增量事件 Payload（流式）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentContentPayload {
    /// 父 Agent 会话 ID
    pub parent_session_id: String,
    /// 子 Agent ID
    pub agent_id: String,
    /// 内容增量
    pub content: String,
    /// 是否为流式输出的中间片段
    pub is_streaming: bool,
    /// 当前迭代次数
    pub iteration: u32,
}

/// 子 Agent 工具执行结果事件 Payload
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentToolResultPayload {
    /// 父 Agent 会话 ID
    pub parent_session_id: String,
    /// 子 Agent ID
    pub agent_id: String,
    /// 工具调用 ID（关联 tool_call 事件）
    pub tool_call_id: String,
    /// 工具名称
    pub tool_name: String,
    /// 成功时的结果
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// 失败时的错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// 是否成功
    pub success: bool,
    /// 当前迭代次数
    pub iteration: u32,
}

/// 向用户提问事件 Payload
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QuestionPayload {
    /// 会话 ID
    pub session_id: String,
    /// 问题 ID（用于关联答案）
    pub question_id: String,
    /// 问题列表
    pub questions: Vec<QuestionItem>,
}

/// 单个问题项
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QuestionItem {
    /// 短标签（最多 12 字符）
    pub header: String,
    /// 完整问题文本
    pub question: String,
    /// 选项列表（2-4 个）
    pub options: Vec<QuestionOption>,
    /// 是否允许多选
    pub multi_select: bool,
}

/// 问题选项
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QuestionOption {
    /// 选项标签
    pub label: String,
    /// 选项描述
    pub description: String,
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
