use crate::models::permission::PermissionRule;

use super::{normalize_path_for_match, PermissionAction, PermissionType, WildcardMatcher};

/// 权限评估请求
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    /// 权限类型(根据工具名推断)
    pub permission_type: PermissionType,
    /// 匹配目标
    /// - 对于文件类工具:文件路径
    /// - 对于 bash 工具:命令字符串
    /// - 对于 task 工具:子 Agent 名称
    /// - 对于 webfetch 工具:URL
    pub target: String,
}

impl PermissionRequest {
    /// 从工具调用构建权限请求
    pub fn from_tool_call(tool_name: &str, params: &serde_json::Value) -> Self {
        let permission_type = PermissionType::from_tool_name(tool_name);
        let target = extract_target(tool_name, params);
        Self {
            permission_type,
            target,
        }
    }
}

/// 从工具参数中提取匹配目标
fn extract_target(tool_name: &str, params: &serde_json::Value) -> String {
    match tool_name {
        // 文件路径类工具:提取 path 或 file_path 参数
        "read" | "edit" | "write" | "apply_patch" | "remove" | "rename" | "copy" | "file_info"
        | "exists" | "hash" => params
            .get("path")
            .or_else(|| params.get("file_path"))
            .and_then(|v| v.as_str())
            .map(normalize_path_for_match)
            .unwrap_or_else(|| "*".to_string()),
        // 命令执行:提取 command 参数
        "bash" | "write_script" => params
            .get("command")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "*".to_string()),
        // 目录操作:提取 path 参数
        "list" | "mkdir" | "remove_dir" => params
            .get("path")
            .and_then(|v| v.as_str())
            .map(normalize_path_for_match)
            .unwrap_or_else(|| "*".to_string()),
        // 搜索类:提取 pattern 或 query 参数
        "glob" => params
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "*".to_string()),
        "grep" | "search" => params
            .get("pattern")
            .or_else(|| params.get("query"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "*".to_string()),
        // 网页类:提取 url 参数
        "webfetch" => params
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "*".to_string()),
        // 子 Agent:提取 agent 参数
        "task" => params
            .get("agent")
            .or_else(|| params.get("agent_type"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "*".to_string()),
        // v1.1: 文档 Handler:提取 input_path 或 path 参数
        "docx" | "xlsx" | "pptx" | "pdf" => params
            .get("input_path")
            .or_else(|| params.get("path"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "*".to_string()),
        // 默认:通配
        _ => "*".to_string(),
    }
}

/// 权限评估结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDecision {
    /// 最终动作:allow / deny / ask
    pub action: PermissionAction,
    /// 命中的规则 ID(用于日志和调试)
    pub matched_rule_id: Option<String>,
    /// 命中的规则描述
    pub matched_description: String,
    /// 评估的请求
    pub request: PermissionRequest,
}

/// 权限评估器
/// 根据生效规则评估单次工具调用的权限
pub struct PermissionEvaluator;

impl PermissionEvaluator {
    /// 评估权限请求
    /// 规则优先级:最后匹配优先(与 OpenCode 一致)
    /// 按规则定义顺序遍历,最后命中的规则生效;无匹配时默认 allow
    pub fn evaluate(request: &PermissionRequest, rules: &[PermissionRule]) -> PermissionDecision {
        // 按定义顺序遍历,收集所有匹配的规则(保持定义顺序)
        let matching_rules: Vec<&PermissionRule> = rules
            .iter()
            .filter(|r| r.enabled)
            .filter(|r| Self::rule_matches(r, request))
            .collect();

        if matching_rules.is_empty() {
            // 无匹配规则,默认允许
            return PermissionDecision {
                action: PermissionAction::Allow,
                matched_rule_id: None,
                matched_description: "无匹配规则,默认允许".to_string(),
                request: request.clone(),
            };
        }

        // 最后匹配优先(与 OpenCode 一致):取匹配规则中的最后一条
        let matched = matching_rules.last().unwrap();
        PermissionDecision {
            action: matched.action,
            matched_rule_id: Some(matched.id.clone()),
            matched_description: matched.description.clone(),
            request: request.clone(),
        }
    }

    /// 检查规则是否匹配请求
    fn rule_matches(rule: &PermissionRule, request: &PermissionRequest) -> bool {
        // 1. 权限类型匹配:Wildcard 匹配所有,否则需要精确匹配
        let type_matches = rule.permission_type == PermissionType::Wildcard
            || rule.permission_type == request.permission_type;
        if !type_matches {
            return false;
        }

        // 2. 模式匹配
        let matcher = WildcardMatcher::new(&rule.pattern);
        matcher.matches(&request.target)
    }

    /// 检查是否为外部目录访问
    /// 路径不在工作区内时触发 ExternalDirectory 权限
    pub fn is_external_directory(path: &str, workspace_root: &str) -> bool {
        let normalized_path = normalize_path_for_match(path);
        let normalized_workspace = normalize_path_for_match(workspace_root);
        // 相对路径解析:将相对路径 join 到工作区根目录后再比较
        let final_path = if is_relative_path(&normalized_path) {
            let joined = std::path::Path::new(&normalized_workspace).join(&normalized_path);
            // join 后再次规范化(Windows 的 join 会使用反斜杠)
            normalize_path_for_match(&joined.to_string_lossy())
        } else {
            normalized_path
        };
        !final_path.starts_with(&normalized_workspace)
    }
}

/// 判断路径是否为相对路径
/// 相对路径:不以 / 开头、不以 ~ 开头、不以盘符(如 c:/d:)开头
fn is_relative_path(path: &str) -> bool {
    if path.is_empty() {
        return true;
    }
    // 以 / 开头:Unix 绝对路径
    if path.starts_with('/') {
        return false;
    }
    // 以 ~ 开头:主目录路径
    if path.starts_with('~') {
        return false;
    }
    // 以盘符开头:Windows 绝对路径(不区分大小写)
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' {
        let drive = bytes[0].to_ascii_lowercase();
        if drive.is_ascii_lowercase() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::permission::RuleScope;

    fn make_rule(
        scope: RuleScope,
        ptype: PermissionType,
        pattern: &str,
        action: PermissionAction,
    ) -> PermissionRule {
        PermissionRule::new(scope, ptype, pattern.to_string(), action)
    }

    #[test]
    fn test_evaluate_allow_default() {
        let rules = vec![make_rule(
            RuleScope::Global,
            PermissionType::Wildcard,
            "*",
            PermissionAction::Allow,
        )];
        let req = PermissionRequest {
            permission_type: PermissionType::Read,
            target: "src/main.rs".to_string(),
        };
        let decision = PermissionEvaluator::evaluate(&req, &rules);
        assert_eq!(decision.action, PermissionAction::Allow);
    }

    #[test]
    fn test_evaluate_deny_env_file() {
        let rules = vec![
            make_rule(
                RuleScope::Global,
                PermissionType::Read,
                "*",
                PermissionAction::Allow,
            ),
            make_rule(
                RuleScope::Global,
                PermissionType::Read,
                "*.env",
                PermissionAction::Deny,
            ),
        ];
        let req = PermissionRequest {
            permission_type: PermissionType::Read,
            target: ".env".to_string(),
        };
        let decision = PermissionEvaluator::evaluate(&req, &rules);
        assert_eq!(decision.action, PermissionAction::Deny);
    }

    #[test]
    fn test_evaluate_specific_overrides_wildcard() {
        // 最后匹配优先:后定义的规则覆盖先定义的规则
        let rules = vec![
            make_rule(
                RuleScope::Global,
                PermissionType::Edit,
                "*",
                PermissionAction::Allow,
            ),
            make_rule(
                RuleScope::Global,
                PermissionType::Edit,
                "src/secret/*",
                PermissionAction::Ask,
            ),
        ];
        // 仅匹配通配规则 -> allow
        let req1 = PermissionRequest {
            permission_type: PermissionType::Edit,
            target: "docs/readme.md".to_string(),
        };
        assert_eq!(
            PermissionEvaluator::evaluate(&req1, &rules).action,
            PermissionAction::Allow
        );

        // 匹配两条规则,最后一条为 ask -> ask
        let req2 = PermissionRequest {
            permission_type: PermissionType::Edit,
            target: "src/secret/key.pem".to_string(),
        };
        assert_eq!(
            PermissionEvaluator::evaluate(&req2, &rules).action,
            PermissionAction::Ask
        );

        // 验证:当具体规则定义在通配规则之前时,通配规则(后定义)会覆盖具体规则
        let rules_reversed = vec![
            make_rule(
                RuleScope::Global,
                PermissionType::Edit,
                "src/secret/*",
                PermissionAction::Ask,
            ),
            make_rule(
                RuleScope::Global,
                PermissionType::Edit,
                "*",
                PermissionAction::Allow,
            ),
        ];
        // 匹配两条规则,最后一条为 allow -> allow
        assert_eq!(
            PermissionEvaluator::evaluate(&req2, &rules_reversed).action,
            PermissionAction::Allow
        );
    }

    #[test]
    fn test_evaluate_bash_command_pattern() {
        let rules = vec![
            make_rule(
                RuleScope::Global,
                PermissionType::Bash,
                "*",
                PermissionAction::Allow,
            ),
            make_rule(
                RuleScope::Global,
                PermissionType::Bash,
                "rm *",
                PermissionAction::Deny,
            ),
            make_rule(
                RuleScope::Global,
                PermissionType::Bash,
                "git push *",
                PermissionAction::Ask,
            ),
        ];
        // 普通命令 -> allow
        let req1 = PermissionRequest {
            permission_type: PermissionType::Bash,
            target: "ls -la".to_string(),
        };
        assert_eq!(
            PermissionEvaluator::evaluate(&req1, &rules).action,
            PermissionAction::Allow
        );

        // 删除命令 -> deny
        let req2 = PermissionRequest {
            permission_type: PermissionType::Bash,
            target: "rm -rf /tmp/test".to_string(),
        };
        assert_eq!(
            PermissionEvaluator::evaluate(&req2, &rules).action,
            PermissionAction::Deny
        );

        // 推送命令 -> ask
        let req3 = PermissionRequest {
            permission_type: PermissionType::Bash,
            target: "git push origin main".to_string(),
        };
        assert_eq!(
            PermissionEvaluator::evaluate(&req3, &rules).action,
            PermissionAction::Ask
        );
    }

    #[test]
    fn test_is_external_directory() {
        assert!(PermissionEvaluator::is_external_directory(
            "/tmp/other",
            "/home/user/project"
        ));
        assert!(!PermissionEvaluator::is_external_directory(
            "/home/user/project/src/main.rs",
            "/home/user/project"
        ));
    }

    #[test]
    fn test_is_external_directory_relative_path() {
        // 相对路径不应误判为外部目录
        assert!(!PermissionEvaluator::is_external_directory(
            "output.md",
            "d:/DeskTop/WorkMolde-AI"
        ));
    }

    #[test]
    fn test_is_external_directory_absolute_external() {
        // 绝对路径的外部目录仍被识别
        assert!(PermissionEvaluator::is_external_directory(
            "c:/Windows/system32/test.txt",
            "d:/DeskTop/WorkMolde-AI"
        ));
    }

    #[test]
    fn test_is_external_directory_subdir_relative() {
        // 工作区内子目录相对路径不误判
        assert!(!PermissionEvaluator::is_external_directory(
            "subdir/report.md",
            "d:/DeskTop/WorkMolde-AI"
        ));
    }

    #[test]
    fn test_is_external_directory_absolute_inside_workspace() {
        // 工作区内绝对路径不误判
        assert!(!PermissionEvaluator::is_external_directory(
            "d:/DeskTop/WorkMolde-AI/src/main.rs",
            "d:/DeskTop/WorkMolde-AI"
        ));
    }
}
