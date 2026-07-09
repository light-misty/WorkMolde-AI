use serde::{Deserialize, Serialize};

use crate::services::permission::{PermissionAction, PermissionType, RuleScope};

/// 权限规则数据模型
/// 对应 permission_rules 表的一条记录
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRule {
    /// 规则 ID(格式:rule_<uuid>)
    pub id: String,
    /// 作用域:global / project / session
    pub scope: RuleScope,
    /// 工作区 ID(仅 scope=project 时有效)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// 会话 ID(仅 scope=session 时有效)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// 权限类型(对应工具类别)
    pub permission_type: PermissionType,
    /// 匹配模式(通配符,如 "src/**/*.ts"、"git *"、"*.env")
    pub pattern: String,
    /// 权限动作:allow / deny / ask
    pub action: PermissionAction,
    /// 规则描述(用户可读)
    #[serde(default)]
    pub description: String,
    /// 是否启用
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 创建时间(ISO 8601)
    pub created_at: String,
    /// 更新时间(ISO 8601)
    pub updated_at: String,
}

fn default_enabled() -> bool {
    true
}

impl PermissionRule {
    /// 创建新规则(自动生成 ID 和时间戳)
    pub fn new(
        scope: RuleScope,
        permission_type: PermissionType,
        pattern: String,
        action: PermissionAction,
    ) -> Self {
        let now = current_iso8601();
        Self {
            id: format!("rule_{}", uuid::Uuid::new_v4()),
            scope,
            workspace_id: None,
            session_id: None,
            permission_type,
            pattern,
            action,
            description: String::new(),
            enabled: true,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// 关联到工作区
    pub fn with_workspace(mut self, workspace_id: String) -> Self {
        self.workspace_id = Some(workspace_id);
        self
    }

    /// 关联到会话
    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// 设置描述
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }
}

/// 生成 ISO 8601 格式的当前时间字符串
fn current_iso8601() -> String {
    // 使用 Unix 时间戳作为简化处理,实际由数据库 DEFAULT 生成
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", now)
}

/// 权限规则过滤器(查询时使用)
#[derive(Debug, Clone, Default)]
pub struct PermissionRuleFilter {
    pub scope: Option<RuleScope>,
    pub workspace_id: Option<String>,
    pub session_id: Option<String>,
    pub permission_type: Option<PermissionType>,
    pub enabled_only: bool,
}
