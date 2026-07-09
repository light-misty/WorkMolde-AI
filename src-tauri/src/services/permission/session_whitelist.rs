use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::permission::PermissionRule;

use super::{PermissionAction, PermissionType, RuleScope};

/// 会话级临时白名单
/// 当用户选择 "always" 时,生成一条临时规则并缓存
/// 会话结束时通过 cleanup_session 清理
#[derive(Debug, Clone)]
pub struct SessionWhitelist {
    /// 按 session_id 隔离的临时规则列表
    sessions: Arc<RwLock<HashMap<String, Vec<PermissionRule>>>>,
}

impl SessionWhitelist {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 添加一条会话级临时规则
    /// 当用户选择 always 时调用
    pub async fn add_rule(&self, session_id: &str, rule: PermissionRule) {
        let mut sessions = self.sessions.write().await;
        sessions
            .entry(session_id.to_string())
            .or_insert_with(Vec::new)
            .push(rule);
    }

    /// 获取指定会话的所有临时规则
    pub async fn get_rules(&self, session_id: &str) -> Vec<PermissionRule> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned().unwrap_or_default()
    }

    /// 清理指定会话的所有临时规则
    pub async fn cleanup_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        if let Some(rules) = sessions.remove(session_id) {
            log::info!(
                "已清理会话 {} 的 {} 条临时权限规则",
                session_id,
                rules.len()
            );
        }
    }

    /// 根据 always 响应生成临时规则
    /// 自动推断通配符模式:如果具体路径,使用路径;如果命令,提取命令前缀 + 通配符
    pub fn generate_always_rule(
        session_id: &str,
        permission_type: PermissionType,
        target: &str,
    ) -> PermissionRule {
        let pattern = Self::infer_pattern(permission_type, target);
        PermissionRule::new(
            RuleScope::Session,
            permission_type,
            pattern,
            PermissionAction::Allow,
        )
        .with_session(session_id.to_string())
        .with_description("用户选择 always 自动生成")
    }

    /// 根据目标和权限类型推断通配符模式
    /// - 文件路径:使用具体路径(只放行该文件)
    /// - 命令:提取命令前缀 + *(如 "git status *" 放行所有 git status 子命令)
    /// - 其他:使用具体值
    fn infer_pattern(permission_type: PermissionType, target: &str) -> String {
        match permission_type {
            PermissionType::Bash | PermissionType::WriteScript => {
                // 命令:提取前两个 token + 通配符
                let tokens: Vec<&str> = target.split_whitespace().take(2).collect();
                if tokens.is_empty() {
                    "*".to_string()
                } else if tokens.len() == 1 {
                    format!("{} *", tokens[0])
                } else {
                    format!("{} {} *", tokens[0], tokens[1])
                }
            }
            _ => {
                // 文件路径或其他:使用具体值
                target.to_string()
            }
        }
    }

    /// 检查指定会话是否有匹配的临时规则
    pub async fn check(
        &self,
        session_id: &str,
        permission_type: PermissionType,
        target: &str,
    ) -> Option<PermissionAction> {
        let sessions = self.sessions.read().await;
        let rules = sessions.get(session_id)?;
        for rule in rules {
            if !rule.enabled {
                continue;
            }
            if rule.permission_type != permission_type
                && rule.permission_type != PermissionType::Wildcard
            {
                continue;
            }
            let matcher = super::WildcardMatcher::new(&rule.pattern);
            if matcher.matches(target) {
                return Some(rule.action);
            }
        }
        None
    }
}

impl Default for SessionWhitelist {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_pattern_command() {
        let p = SessionWhitelist::infer_pattern(PermissionType::Bash, "git push origin main");
        assert_eq!(p, "git push *");

        let p = SessionWhitelist::infer_pattern(PermissionType::Bash, "ls");
        assert_eq!(p, "ls *");
    }

    #[test]
    fn test_infer_pattern_file() {
        let p = SessionWhitelist::infer_pattern(PermissionType::Edit, "src/main.rs");
        assert_eq!(p, "src/main.rs");
    }

    #[tokio::test]
    async fn test_add_and_check_rule() {
        let whitelist = SessionWhitelist::new();
        let rule =
            SessionWhitelist::generate_always_rule("sess1", PermissionType::Bash, "git status");
        whitelist.add_rule("sess1", rule).await;

        // 相同前缀的命令应命中白名单
        let action = whitelist
            .check("sess1", PermissionType::Bash, "git status --short")
            .await;
        assert_eq!(action, Some(PermissionAction::Allow));

        // 不同会话不应命中
        let action = whitelist
            .check("sess2", PermissionType::Bash, "git status")
            .await;
        assert_eq!(action, None);
    }

    #[tokio::test]
    async fn test_cleanup_session() {
        let whitelist = SessionWhitelist::new();
        let rule =
            SessionWhitelist::generate_always_rule("sess1", PermissionType::Bash, "npm install");
        whitelist.add_rule("sess1", rule).await;

        whitelist.cleanup_session("sess1").await;
        let action = whitelist
            .check("sess1", PermissionType::Bash, "npm install")
            .await;
        assert_eq!(action, None);
    }
}
