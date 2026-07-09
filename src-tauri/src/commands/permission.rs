use tauri::State;

use crate::errors::{CommandError, CONFIG_INVALID_VALUE};
use crate::models::permission::{PermissionRule, PermissionRuleFilter};
use crate::services::permission::{PermissionAction, PermissionType, RuleScope};
use crate::AppState;

/// 解析作用域字符串为 RuleScope 枚举
fn parse_scope(s: &str) -> Result<RuleScope, CommandError> {
    match s.to_lowercase().as_str() {
        "global" => Ok(RuleScope::Global),
        "project" => Ok(RuleScope::Project),
        "session" => Ok(RuleScope::Session),
        _ => Err(CommandError::config(
            CONFIG_INVALID_VALUE,
            format!("无效的权限作用域: {}", s),
        )),
    }
}

/// 列出权限规则（默认规则 + 用户规则，可选过滤 scope/workspace_id/session_id）
#[tauri::command]
pub async fn list_permission_rules(
    scope: Option<String>,
    workspace_id: Option<String>,
    session_id: Option<String>,
    permission_type: Option<String>,
    enabled_only: Option<bool>,
    include_defaults: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<PermissionRule>, CommandError> {
    log::info!(
        "list_permission_rules: scope={:?}, include_defaults={:?}",
        scope,
        include_defaults
    );

    // 解析 scope 字符串为 RuleScope 枚举
    let scope_enum = match scope.as_deref() {
        Some(s) => Some(parse_scope(s)?),
        None => None,
    };

    // 解析 permission_type 字符串为 PermissionType 枚举
    let ptype_enum = match permission_type.as_deref() {
        Some(s) => Some(PermissionType::from_str(s).ok_or_else(|| {
            CommandError::config(CONFIG_INVALID_VALUE, format!("无效的权限类型: {}", s))
        })?),
        None => None,
    };

    // 构造 PermissionRuleFilter
    let filter = PermissionRuleFilter {
        scope: scope_enum,
        workspace_id,
        session_id,
        permission_type: ptype_enum,
        enabled_only: enabled_only.unwrap_or(false),
    };

    // 如果 include_defaults=true，先取默认规则再追加用户规则
    let include_def = include_defaults.unwrap_or(false);
    let mut rules: Vec<PermissionRule> = Vec::new();
    if include_def {
        rules.extend(state.permission_registry.default_rules().iter().cloned());
    }
    let user_rules = state.permission_registry.list_user_rules(&filter)?;
    rules.extend(user_rules);

    log::info!("list_permission_rules: 共 {} 条规则", rules.len());
    Ok(rules)
}

/// 添加权限规则
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn add_permission_rule(
    scope: String,
    permission_type: String,
    pattern: String,
    action: String,
    description: Option<String>,
    workspace_id: Option<String>,
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<PermissionRule, CommandError> {
    log::info!(
        "add_permission_rule: scope={}, type={}, pattern={}, action={}",
        scope,
        permission_type,
        pattern,
        action
    );

    // 解析参数
    let scope_enum = parse_scope(&scope)?;
    let ptype_enum = PermissionType::from_str(&permission_type).ok_or_else(|| {
        CommandError::config(
            CONFIG_INVALID_VALUE,
            format!("无效的权限类型: {}", permission_type),
        )
    })?;
    let action_enum = PermissionAction::from_str(&action).ok_or_else(|| {
        CommandError::config(CONFIG_INVALID_VALUE, format!("无效的权限动作: {}", action))
    })?;

    // 构造 PermissionRule
    let mut rule = PermissionRule::new(scope_enum, ptype_enum, pattern, action_enum);
    if let Some(desc) = description {
        rule = rule.with_description(&desc);
    }
    if let Some(ws) = workspace_id {
        rule = rule.with_workspace(ws);
    }
    if let Some(sid) = session_id {
        rule = rule.with_session(sid);
    }

    state.permission_registry.add_rule(rule.clone())?;
    log::info!("add_permission_rule: 已添加规则 id={}", rule.id);
    Ok(rule)
}

/// 更新权限规则
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn update_permission_rule(
    rule_id: String,
    scope: Option<String>,
    permission_type: Option<String>,
    pattern: Option<String>,
    action: Option<String>,
    description: Option<String>,
    enabled: Option<bool>,
    workspace_id: Option<String>,
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<PermissionRule, CommandError> {
    log::info!("update_permission_rule: rule_id={}", rule_id);

    // 先取出原规则（作用域内获取连接，离开作用域后自动释放锁）
    let mut rule = {
        let conn = state.db.conn()?;
        crate::db::permission_repo::get_rule(&conn, &rule_id)?
    };

    // 按 Option 字段覆盖
    if let Some(s) = scope {
        rule.scope = parse_scope(&s)?;
    }
    if let Some(t) = permission_type {
        rule.permission_type = PermissionType::from_str(&t).ok_or_else(|| {
            CommandError::config(CONFIG_INVALID_VALUE, format!("无效的权限类型: {}", t))
        })?;
    }
    if let Some(p) = pattern {
        rule.pattern = p;
    }
    if let Some(a) = action {
        rule.action = PermissionAction::from_str(&a).ok_or_else(|| {
            CommandError::config(CONFIG_INVALID_VALUE, format!("无效的权限动作: {}", a))
        })?;
    }
    if let Some(d) = description {
        rule.description = d;
    }
    if let Some(e) = enabled {
        rule.enabled = e;
    }
    if let Some(ws) = workspace_id {
        rule.workspace_id = Some(ws);
    }
    if let Some(sid) = session_id {
        rule.session_id = Some(sid);
    }

    // 更新（registry 内部加锁）
    state.permission_registry.update_rule(rule)?;

    // 重新获取并返回（DB 会更新 updated_at）
    let updated = {
        let conn = state.db.conn()?;
        crate::db::permission_repo::get_rule(&conn, &rule_id)?
    };
    log::info!("update_permission_rule: 已更新规则 id={}", rule_id);
    Ok(updated)
}

/// 删除权限规则
#[tauri::command]
pub async fn delete_permission_rule(
    rule_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("delete_permission_rule: rule_id={}", rule_id);
    state.permission_registry.delete_rule(&rule_id)?;
    Ok(())
}
