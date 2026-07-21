use serde::{Deserialize, Serialize};

/// 会话信息
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub title: String,
    pub workspace_id: Option<String>,
    pub provider_id: String,
    pub template_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// 会话状态: "active" | "archived"
    pub status: String,
    /// 当前活跃分支 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_branch_id: Option<String>,
}

/// 创建会话的参数
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionParams {
    pub title: Option<String>,
    pub workspace_id: Option<String>,
    pub provider_id: Option<String>,
    pub template_id: Option<String>,
}

/// 会话筛选条件
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionFilter {
    pub workspace_id: Option<String>,
    /// "active" | "archived"
    pub status: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// 会话摘要信息
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub workspace_id: Option<String>,
    pub status: String,
    pub message_count: u32,
    pub last_message_preview: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 会话详情，包含消息历史
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub session: Session,
    pub messages: Vec<super::Message>,
    /// 会话的所有分支列表
    pub branches: Vec<crate::models::Branch>,
    /// 当前活跃分支 ID
    pub active_branch_id: String,
}
