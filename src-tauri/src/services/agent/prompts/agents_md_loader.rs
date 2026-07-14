//! AGENTS.md 自定义规则加载模块
//! 参照 OpenCode 的 AGENTS.md/CLAUDE.md 机制
//! 加载顺序:项目级(向上递归) > 全局级 > 配置指令

use std::path::{Path, PathBuf};

/// AGENTS.md 规则文件加载结果
#[derive(Debug, Default)]
pub struct AgentsMdContent {
    /// 项目级规则(从工作区目录及父目录加载)
    pub project_rules: Vec<(PathBuf, String)>,
    /// 全局级规则(从 ~/.agent/AGENTS.md 加载)
    pub global_rules: Option<String>,
}

impl AgentsMdContent {
    /// 合并所有规则为单一字符串,用于注入系统提示词
    /// 格式:每个文件的内容用分隔线分隔,并标注来源
    pub fn merge(&self) -> String {
        let mut parts = Vec::new();

        // 全局规则优先(优先级低,放在前面)
        if let Some(global) = &self.global_rules {
            if !global.trim().is_empty() {
                parts.push(format!(
                    "## 全局规则 (~/.agent/AGENTS.md)\n{}",
                    global.trim()
                ));
            }
        }

        // 项目级规则(从根到工作区,优先级递增)
        for (path, content) in &self.project_rules {
            if !content.trim().is_empty() {
                let path_str = path.to_string_lossy();
                parts.push(format!("## 项目规则 ({})\n{}", path_str, content.trim()));
            }
        }

        parts.join("\n\n---\n\n")
    }

    /// 是否有任何规则内容
    pub fn is_empty(&self) -> bool {
        self.global_rules
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
            && self.project_rules.iter().all(|(_, c)| c.trim().is_empty())
    }
}

/// 加载 AGENTS.md 规则文件
/// workspace_path: 当前工作区路径
/// global_config_dir: 全局配置目录(通常为 ~/.agent)
pub fn load_agents_md(workspace_path: &str, global_config_dir: Option<&Path>) -> AgentsMdContent {
    let project_rules = load_project_rules(workspace_path);
    let global_rules = load_global_rules(global_config_dir);

    AgentsMdContent {
        project_rules,
        global_rules,
    }
}

/// 加载项目级规则文件
/// 从工作区目录开始,向上递归查找 AGENTS.md / CLAUDE.md
/// 查找顺序:工作区目录 -> 父目录 -> ... -> 根目录
/// 返回的列表按"从根到工作区"的顺序排列(优先级递增)
fn load_project_rules(workspace_path: &str) -> Vec<(PathBuf, String)> {
    let workspace = Path::new(workspace_path);
    if !workspace.is_absolute() {
        log::warn!(
            "load_project_rules: 工作区路径不是绝对路径: {}",
            workspace_path
        );
        return Vec::new();
    }

    // 收集从工作区到根目录的所有候选目录(含工作区本身)
    let mut candidate_dirs = Vec::new();
    let mut current = Some(workspace);
    while let Some(dir) = current {
        candidate_dirs.push(dir.to_path_buf());
        current = dir.parent();
    }
    // 反转:从根到工作区(优先级递增)
    candidate_dirs.reverse();

    let mut rules = Vec::new();
    // 候选文件名:AGENTS.md 优先,其次 CLAUDE.md
    let candidates = ["AGENTS.md", "CLAUDE.md"];

    for dir in candidate_dirs {
        for filename in &candidates {
            let file_path = dir.join(filename);
            if file_path.is_file() {
                match std::fs::read_to_string(&file_path) {
                    Ok(content) => {
                        log::info!("加载规则文件: {}", file_path.display());
                        rules.push((file_path, content));
                        break; // 同一目录只加载第一个匹配的文件
                    }
                    Err(e) => {
                        log::warn!("读取规则文件失败: {}, 错误: {}", file_path.display(), e);
                    }
                }
            }
        }
    }

    rules
}

/// 加载全局级规则文件
/// 从 ~/.agent/AGENTS.md 加载(若存在)
fn load_global_rules(global_config_dir: Option<&Path>) -> Option<String> {
    let config_dir = global_config_dir.map(|p| p.to_path_buf()).or_else(|| {
        // 默认全局配置目录:~/.agent
        dirs_home_dir().map(|h| h.join(".agent"))
    })?;

    let global_file = config_dir.join("AGENTS.md");
    if global_file.is_file() {
        match std::fs::read_to_string(&global_file) {
            Ok(content) => {
                log::info!("加载全局规则文件: {}", global_file.display());
                Some(content)
            }
            Err(e) => {
                log::warn!(
                    "读取全局规则文件失败: {}, 错误: {}",
                    global_file.display(),
                    e
                );
                None
            }
        }
    } else {
        None
    }
}

/// 获取用户 home 目录(避免引入 dirs crate,简单实现)
fn dirs_home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_load_agents_md_empty_workspace() {
        let content = load_agents_md("", None);
        assert!(content.is_empty());
    }

    #[test]
    fn test_load_project_rules_from_temp() {
        // 创建临时目录结构测试递归加载
        let tmp = std::env::temp_dir().join("workmolde_test_agents_md");
        let subdir = tmp.join("subdir").join("deep");
        fs::create_dir_all(&subdir).unwrap();

        // 在父目录创建 AGENTS.md
        fs::write(tmp.join("AGENTS.md"), "# 根规则\n这是根目录规则").unwrap();
        // 在子目录创建 CLAUDE.md
        fs::write(subdir.join("CLAUDE.md"), "# 深层规则\n这是深层规则").unwrap();

        let rules = load_project_rules(subdir.to_str().unwrap());
        assert_eq!(rules.len(), 2);
        // 根规则在前,深层规则在后
        assert!(rules[0].1.contains("根规则"));
        assert!(rules[1].1.contains("深层规则"));

        // 清理
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn test_merge_content() {
        let content = AgentsMdContent {
            project_rules: vec![
                (PathBuf::from("/root/AGENTS.md"), "根规则内容".to_string()),
                (
                    PathBuf::from("/root/sub/AGENTS.md"),
                    "子规则内容".to_string(),
                ),
            ],
            global_rules: Some("全局规则内容".to_string()),
        };
        let merged = content.merge();
        assert!(merged.contains("全局规则"));
        assert!(merged.contains("根规则内容"));
        assert!(merged.contains("子规则内容"));
    }
}
