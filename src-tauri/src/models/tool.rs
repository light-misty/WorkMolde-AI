use serde::{Deserialize, Serialize};

/// Tool 执行结果（与 HandlerResult 格式一致，便于 AgentExecutor 统一处理）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub success: bool,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
    /// 统一错误码（参见 errors.rs），success=true 时为 None
    /// 用于前端精确处理和日志统计，向后兼容（旧反序列化无此字段时为 None）
    #[serde(default)]
    pub error_code: Option<u32>,
}

/// Tool 信息（用于前端展示）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    /// 工具始终为内置
    pub is_builtin: bool,
    /// 工具始终启用
    pub enabled: bool,
    pub version: String,
    pub params_schema: Option<serde_json::Value>,
}
