use serde::{Deserialize, Serialize};

/// 分支元数据
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub id: String,
    pub session_id: String,
    pub parent_branch_id: Option<String>,
    pub fork_message_id: Option<String>,
    pub branch_group_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
}

/// 分支组信息（用于前端渲染切换器）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BranchGroupInfo {
    pub branch_group_id: String,
    pub fork_message_id: Option<String>,
    pub branches: Vec<BranchInfo>,
}

/// 分支组内的单条分支信息
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub branch_id: String,
    pub name: String,
    pub sort_order: i64,
}

/// 创建分支命令的返回结果
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchResult {
    pub branch_id: String,
    pub branch_group_id: String,
}

/// 分支内的用户消息简要信息（用于跨分支搜索）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BranchUserMessage {
    /// 消息 ID
    pub message_id: String,
    /// 会话 ID
    pub session_id: String,
    /// 所属分支 ID
    pub branch_id: String,
    /// 消息内容
    pub content: String,
    /// 创建时间（ISO 字符串）
    pub created_at: String,
}
