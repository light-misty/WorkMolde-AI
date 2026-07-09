use rusqlite::{params, Connection};

use crate::errors::{CommandError, DB_QUERY_FAILED, DB_RECORD_NOT_FOUND};
use crate::models::permission::{PermissionRule, PermissionRuleFilter};
use crate::services::permission::{PermissionAction, PermissionType, RuleScope};

/// 插入一条权限规则
pub fn insert_rule(conn: &Connection, rule: &PermissionRule) -> Result<(), CommandError> {
    let scope_str = match rule.scope {
        RuleScope::Global => "global",
        RuleScope::Project => "project",
        RuleScope::Session => "session",
    };
    let type_str = rule.permission_type.to_string();
    let action_str = rule.action.to_string();

    conn.execute(
        "INSERT INTO permission_rules
         (id, scope, workspace_id, session_id, permission_type, pattern, action, description, enabled, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            rule.id,
            scope_str,
            rule.workspace_id,
            rule.session_id,
            type_str,
            rule.pattern,
            action_str,
            rule.description,
            rule.enabled as i32,
            rule.created_at,
            rule.updated_at,
        ],
    ).map_err(|e| CommandError::db(DB_QUERY_FAILED, format!("插入权限规则失败: {}", e)))?;

    log::info!(
        "已插入权限规则: id={}, type={}, pattern={}, action={}",
        rule.id,
        type_str,
        rule.pattern,
        action_str
    );
    Ok(())
}

/// 根据过滤器查询权限规则列表
pub fn list_rules(
    conn: &Connection,
    filter: &PermissionRuleFilter,
) -> Result<Vec<PermissionRule>, CommandError> {
    let mut sql = String::from("SELECT id, scope, workspace_id, session_id, permission_type, pattern, action, description, enabled, created_at, updated_at FROM permission_rules WHERE 1=1");
    let mut param_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut param_idx = 1;

    if let Some(scope) = filter.scope {
        sql.push_str(&format!(" AND scope = ?{}", param_idx));
        let scope_str = match scope {
            RuleScope::Global => "global",
            RuleScope::Project => "project",
            RuleScope::Session => "session",
        };
        param_values.push(Box::new(scope_str.to_string()));
        param_idx += 1;
    }

    if let Some(ref workspace_id) = filter.workspace_id {
        sql.push_str(&format!(
            " AND (workspace_id = ?{} OR workspace_id IS NULL)",
            param_idx
        ));
        param_values.push(Box::new(workspace_id.clone()));
        param_idx += 1;
    }

    if let Some(ref session_id) = filter.session_id {
        sql.push_str(&format!(
            " AND (session_id = ?{} OR session_id IS NULL)",
            param_idx
        ));
        param_values.push(Box::new(session_id.clone()));
        param_idx += 1;
    }

    if let Some(ptype) = filter.permission_type {
        sql.push_str(&format!(" AND permission_type = ?{}", param_idx));
        param_values.push(Box::new(ptype.to_string()));
        param_idx += 1;
    }

    if filter.enabled_only {
        sql.push_str(" AND enabled = 1");
    }

    // 消费 param_idx 避免最后一个递增产生未读取赋值警告
    let _ = param_idx;

    sql.push_str(" ORDER BY created_at ASC");

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| CommandError::db(DB_QUERY_FAILED, format!("准备查询权限规则失败: {}", e)))?;

    let params_refs: Vec<&dyn rusqlite::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
    let rows = stmt
        .query_map(params_refs.as_slice(), |row| {
            let scope_str: String = row.get(1)?;
            let workspace_id: Option<String> = row.get(2)?;
            let session_id: Option<String> = row.get(3)?;
            let type_str: String = row.get(4)?;
            let pattern: String = row.get(5)?;
            let action_str: String = row.get(6)?;
            let description: String = row.get(7)?;
            let enabled: i32 = row.get(8)?;
            let created_at: String = row.get(9)?;
            let updated_at: String = row.get(10)?;

            let scope = match scope_str.as_str() {
                "global" => RuleScope::Global,
                "project" => RuleScope::Project,
                "session" => RuleScope::Session,
                _ => RuleScope::Global,
            };
            let permission_type =
                PermissionType::from_str(&type_str).unwrap_or(PermissionType::Wildcard);
            let action = PermissionAction::from_str(&action_str).unwrap_or(PermissionAction::Ask);

            Ok(PermissionRule {
                id: row.get(0)?,
                scope,
                workspace_id,
                session_id,
                permission_type,
                pattern,
                action,
                description,
                enabled: enabled != 0,
                created_at,
                updated_at,
            })
        })
        .map_err(|e| CommandError::db(DB_QUERY_FAILED, format!("查询权限规则失败: {}", e)))?;

    let mut rules = Vec::new();
    for row in rows {
        rules.push(row.map_err(|e| {
            CommandError::db(DB_QUERY_FAILED, format!("读取权限规则行失败: {}", e))
        })?);
    }
    Ok(rules)
}

/// 根据 ID 获取单条权限规则
pub fn get_rule(conn: &Connection, rule_id: &str) -> Result<PermissionRule, CommandError> {
    let mut stmt = conn.prepare(
        "SELECT id, scope, workspace_id, session_id, permission_type, pattern, action, description, enabled, created_at, updated_at FROM permission_rules WHERE id = ?1"
    ).map_err(|e| CommandError::db(DB_QUERY_FAILED, format!("准备查询权限规则失败: {}", e)))?;

    let rule = stmt
        .query_row(params![rule_id], |row| {
            let scope_str: String = row.get(1)?;
            let workspace_id: Option<String> = row.get(2)?;
            let session_id: Option<String> = row.get(3)?;
            let type_str: String = row.get(4)?;
            let pattern: String = row.get(5)?;
            let action_str: String = row.get(6)?;
            let description: String = row.get(7)?;
            let enabled: i32 = row.get(8)?;
            let created_at: String = row.get(9)?;
            let updated_at: String = row.get(10)?;

            let scope = match scope_str.as_str() {
                "global" => RuleScope::Global,
                "project" => RuleScope::Project,
                "session" => RuleScope::Session,
                _ => RuleScope::Global,
            };
            let permission_type =
                PermissionType::from_str(&type_str).unwrap_or(PermissionType::Wildcard);
            let action = PermissionAction::from_str(&action_str).unwrap_or(PermissionAction::Ask);

            Ok(PermissionRule {
                id: row.get(0)?,
                scope,
                workspace_id,
                session_id,
                permission_type,
                pattern,
                action,
                description,
                enabled: enabled != 0,
                created_at,
                updated_at,
            })
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                CommandError::db(DB_RECORD_NOT_FOUND, format!("权限规则不存在: {}", rule_id))
            }
            e => CommandError::db(DB_QUERY_FAILED, format!("查询权限规则失败: {}", e)),
        })?;

    Ok(rule)
}

/// 更新权限规则
pub fn update_rule(conn: &Connection, rule: &PermissionRule) -> Result<(), CommandError> {
    let scope_str = match rule.scope {
        RuleScope::Global => "global",
        RuleScope::Project => "project",
        RuleScope::Session => "session",
    };
    let type_str = rule.permission_type.to_string();
    let action_str = rule.action.to_string();
    let now = current_iso8601();

    conn.execute(
        "UPDATE permission_rules SET scope=?2, workspace_id=?3, session_id=?4, permission_type=?5, pattern=?6, action=?7, description=?8, enabled=?9, updated_at=?10 WHERE id=?1",
        params![
            rule.id,
            scope_str,
            rule.workspace_id,
            rule.session_id,
            type_str,
            rule.pattern,
            action_str,
            rule.description,
            rule.enabled as i32,
            now,
        ],
    ).map_err(|e| CommandError::db(DB_QUERY_FAILED, format!("更新权限规则失败: {}", e)))?;

    log::info!("已更新权限规则: id={}", rule.id);
    Ok(())
}

/// 删除权限规则
pub fn delete_rule(conn: &Connection, rule_id: &str) -> Result<(), CommandError> {
    conn.execute("DELETE FROM permission_rules WHERE id=?1", params![rule_id])
        .map_err(|e| CommandError::db(DB_QUERY_FAILED, format!("删除权限规则失败: {}", e)))?;
    log::info!("已删除权限规则: id={}", rule_id);
    Ok(())
}

/// 删除指定会话的所有临时规则(会话结束时清理)
pub fn delete_session_rules(conn: &Connection, session_id: &str) -> Result<u64, CommandError> {
    let affected = conn
        .execute(
            "DELETE FROM permission_rules WHERE session_id=?1 AND scope='session'",
            params![session_id],
        )
        .map_err(|e| CommandError::db(DB_QUERY_FAILED, format!("删除会话权限规则失败: {}", e)))?;
    log::info!("已删除会话 {} 的 {} 条临时权限规则", session_id, affected);
    Ok(affected as u64)
}

/// 生成 ISO 8601 格式的当前时间字符串
fn current_iso8601() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use std::path::Path;

    fn setup_test_db() -> Database {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        Database::new(Path::new(tmp.path())).unwrap()
    }

    #[test]
    fn test_insert_and_get_rule() {
        let db = setup_test_db();
        let conn = db.conn().unwrap();
        let rule = PermissionRule::new(
            RuleScope::Global,
            PermissionType::Bash,
            "rm *".to_string(),
            PermissionAction::Deny,
        )
        .with_description("禁止删除命令");

        insert_rule(&conn, &rule).unwrap();
        let fetched = get_rule(&conn, &rule.id).unwrap();
        assert_eq!(fetched.pattern, "rm *");
        assert_eq!(fetched.action, PermissionAction::Deny);
        assert_eq!(fetched.description, "禁止删除命令");
    }

    #[test]
    fn test_list_rules_by_scope() {
        let db = setup_test_db();
        let conn = db.conn().unwrap();

        let r1 = PermissionRule::new(
            RuleScope::Global,
            PermissionType::Edit,
            "*".into(),
            PermissionAction::Allow,
        );
        let r2 = PermissionRule::new(
            RuleScope::Project,
            PermissionType::Bash,
            "git *".into(),
            PermissionAction::Allow,
        )
        .with_workspace("ws1".into());
        insert_rule(&conn, &r1).unwrap();
        insert_rule(&conn, &r2).unwrap();

        let filter = PermissionRuleFilter {
            scope: Some(RuleScope::Global),
            ..Default::default()
        };
        let rules = list_rules(&conn, &filter).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, r1.id);
    }

    #[test]
    fn test_delete_session_rules() {
        let db = setup_test_db();
        let conn = db.conn().unwrap();
        let r1 = PermissionRule::new(
            RuleScope::Session,
            PermissionType::Bash,
            "npm *".into(),
            PermissionAction::Allow,
        )
        .with_session("sess1".into());
        let r2 = PermissionRule::new(
            RuleScope::Global,
            PermissionType::Edit,
            "*".into(),
            PermissionAction::Allow,
        );
        insert_rule(&conn, &r1).unwrap();
        insert_rule(&conn, &r2).unwrap();

        let deleted = delete_session_rules(&conn, "sess1").unwrap();
        assert_eq!(deleted, 1);

        let all = list_rules(&conn, &PermissionRuleFilter::default()).unwrap();
        assert_eq!(all.len(), 1);
    }
}
