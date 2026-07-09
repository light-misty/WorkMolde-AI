use std::sync::Arc;

use crate::db::permission_repo;
use crate::db::Database;
use crate::models::permission::{PermissionRule, PermissionRuleFilter};

use super::{PermissionAction, PermissionType, RuleScope};

/// 权限注册表
/// 负责加载、合并、缓存权限规则
/// 规则优先级:会话级 > 项目级 > 全局级 > 默认级
pub struct PermissionRegistry {
    db: Arc<Database>,
    /// 默认规则(内置,不可修改)
    defaults: Vec<PermissionRule>,
}

impl PermissionRegistry {
    /// 创建权限注册表并加载默认规则
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            defaults: Self::builtin_defaults(),
        }
    }

    /// 内置默认权限规则(参照 OpenCode 默认配置)
    /// 用户未配置任何规则时使用
    fn builtin_defaults() -> Vec<PermissionRule> {
        vec![
            // 通配:默认允许
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::Wildcard,
                "*".into(),
                PermissionAction::Allow,
            ),
            // 读取:默认允许,但保护 .env 文件
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::Read,
                "*".into(),
                PermissionAction::Allow,
            ),
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::Read,
                "*.env".into(),
                PermissionAction::Deny,
            )
            .with_description("保护 .env 隐私配置文件"),
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::Read,
                "*.env.*".into(),
                PermissionAction::Deny,
            ),
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::Read,
                "*.env.example".into(),
                PermissionAction::Allow,
            ),
            // 编辑:默认允许
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::Edit,
                "*".into(),
                PermissionAction::Allow,
            ),
            // 搜索类:默认允许
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::Glob,
                "*".into(),
                PermissionAction::Allow,
            ),
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::Grep,
                "*".into(),
                PermissionAction::Allow,
            ),
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::List,
                "*".into(),
                PermissionAction::Allow,
            ),
            // 命令执行:默认允许
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::Bash,
                "*".into(),
                PermissionAction::Allow,
            ),
            // 脚本执行:默认允许
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::WriteScript,
                "*".into(),
                PermissionAction::Allow,
            ),
            // 安全防护类:默认询问
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::ExternalDirectory,
                "*".into(),
                PermissionAction::Ask,
            )
            .with_description("访问工作区外部目录需确认"),
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::DoomLoop,
                "*".into(),
                PermissionAction::Ask,
            )
            .with_description("连续 3 次相同调用触发死循环检测"),
            // v1.1: 文档处理 Handler:默认允许(Document 模式下可见,非 Document 模式被工具列表过滤)
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::Document,
                "*".into(),
                PermissionAction::Allow,
            )
            .with_description("文档 Handler(docx/xlsx/pptx/pdf)默认允许,仅 Document 模式下可见"),
            // 网络类:默认允许(阶段4实现)
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::WebFetch,
                "*".into(),
                PermissionAction::Allow,
            ),
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::WebSearch,
                "*".into(),
                PermissionAction::Allow,
            ),
            // 子 Agent:默认允许(阶段4实现)
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::Task,
                "*".into(),
                PermissionAction::Allow,
            ),
            // Skill:默认允许(阶段3实现)
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::Skill,
                "*".into(),
                PermissionAction::Allow,
            ),
            // LSP:默认允许(阶段5实现)
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::Lsp,
                "*".into(),
                PermissionAction::Allow,
            ),
            // 询问用户:默认允许(低风险,仅向用户提问以获取澄清信息)
            PermissionRule::new(
                RuleScope::Global,
                PermissionType::Question,
                "*".into(),
                PermissionAction::Allow,
            ),
        ]
    }

    /// 获取所有生效规则(默认 + 数据库)
    /// 按 workspace_id 和 session_id 过滤
    pub fn load_effective_rules(
        &self,
        workspace_id: Option<&str>,
        session_id: Option<&str>,
    ) -> Vec<PermissionRule> {
        let mut rules = self.defaults.clone();

        // 加载数据库中的用户规则
        if let Ok(conn) = self.db.conn() {
            let filter = PermissionRuleFilter {
                workspace_id: workspace_id.map(String::from),
                session_id: session_id.map(String::from),
                enabled_only: true,
                ..Default::default()
            };
            if let Ok(db_rules) = permission_repo::list_rules(&conn, &filter) {
                rules.extend(db_rules);
            }
        }

        rules
    }

    /// 添加用户规则到数据库
    pub fn add_rule(&self, rule: PermissionRule) -> Result<(), crate::errors::CommandError> {
        let conn = self.db.conn()?;
        permission_repo::insert_rule(&conn, &rule)
    }

    /// 更新用户规则
    pub fn update_rule(&self, rule: PermissionRule) -> Result<(), crate::errors::CommandError> {
        let conn = self.db.conn()?;
        permission_repo::update_rule(&conn, &rule)
    }

    /// 删除用户规则
    pub fn delete_rule(&self, rule_id: &str) -> Result<(), crate::errors::CommandError> {
        let conn = self.db.conn()?;
        permission_repo::delete_rule(&conn, rule_id)
    }

    /// 列出用户规则(不含默认规则)
    pub fn list_user_rules(
        &self,
        filter: &PermissionRuleFilter,
    ) -> Result<Vec<PermissionRule>, crate::errors::CommandError> {
        let conn = self.db.conn()?;
        permission_repo::list_rules(&conn, filter)
    }

    /// 清理会话临时规则(会话结束时调用)
    pub fn cleanup_session_rules(
        &self,
        session_id: &str,
    ) -> Result<u64, crate::errors::CommandError> {
        let conn = self.db.conn()?;
        permission_repo::delete_session_rules(&conn, session_id)
    }

    /// 获取默认规则(只读,用于前端展示)
    pub fn default_rules(&self) -> &[PermissionRule] {
        &self.defaults
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_registry() -> PermissionRegistry {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let db = Arc::new(Database::new(std::path::Path::new(tmp.path())).unwrap());
        PermissionRegistry::new(db)
    }

    #[test]
    fn test_builtin_defaults_loaded() {
        let registry = setup_registry();
        let defaults = registry.default_rules();
        // 验证默认规则包含 .env 保护
        assert!(defaults
            .iter()
            .any(|r| r.pattern == "*.env" && r.action == PermissionAction::Deny));
    }

    #[test]
    fn test_add_and_load_user_rule() {
        let registry = setup_registry();
        let rule = PermissionRule::new(
            RuleScope::Global,
            PermissionType::Bash,
            "rm *".into(),
            PermissionAction::Deny,
        );
        registry.add_rule(rule.clone()).unwrap();

        let effective = registry.load_effective_rules(None, None);
        // 默认规则 + 1 条用户规则
        assert!(effective.len() > registry.default_rules().len());
        assert!(effective
            .iter()
            .any(|r| r.pattern == "rm *" && r.action == PermissionAction::Deny));
    }
}
