use std::path::PathBuf;
use wildmatch::WildMatch;

/// 获取用户 home 目录(避免引入 dirs crate,与 agents_md_loader 保持一致)
fn home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

/// 通配符匹配器
/// 支持 * (匹配任意数量字符) 和 ? (匹配单个字符)
#[derive(Debug, Clone)]
pub struct WildcardMatcher {
    pattern: String,
    matcher: WildMatch,
}

impl WildcardMatcher {
    /// 创建通配符匹配器
    pub fn new(pattern: &str) -> Self {
        Self {
            pattern: pattern.to_string(),
            matcher: WildMatch::new(pattern),
        }
    }

    /// 检查目标字符串是否匹配模式
    pub fn matches(&self, target: &str) -> bool {
        self.matcher.matches(target)
    }

    /// 获取原始模式字符串
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// 计算模式的具体性(specificity)
    /// 通配符越少,具体性越高,用于规则优先级排序
    /// 返回值越大表示越具体
    pub fn specificity(&self) -> usize {
        // 通配符 * 和 ? 的数量越少,具体性越高
        // 用 (长度 - 通配符数量) 作为具体性得分
        let wildcard_count = self
            .pattern
            .chars()
            .filter(|c| *c == '*' || *c == '?')
            .count();
        self.pattern.len().saturating_sub(wildcard_count)
    }
}

/// 展开路径中的 ~ 和 $HOME 为用户主目录
/// 仅用于文件路径匹配,不用于命令字符串
pub fn expand_home_path(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(stripped).to_string_lossy().to_string();
        }
    }
    if path == "~" {
        if let Some(home) = home_dir() {
            return home.to_string_lossy().to_string();
        }
    }
    if let Some(stripped) = path.strip_prefix("$HOME/") {
        if let Some(home) = home_dir() {
            return home.join(stripped).to_string_lossy().to_string();
        }
    }
    path.to_string()
}

/// 规范化文件路径用于匹配
/// 1. 将反斜杠转换为正斜杠(Windows 兼容)
/// 2. 展开主目录
/// 3. 移除尾部的路径分隔符
pub fn normalize_path_for_match(path: &str) -> String {
    let expanded = expand_home_path(path);
    let normalized = expanded.replace('\\', "/");
    // 移除尾部斜杠(但保留根路径 "/")
    if normalized.len() > 1 && normalized.ends_with('/') {
        normalized.trim_end_matches('/').to_string()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wildcard_basic_match() {
        let m = WildcardMatcher::new("*.ts");
        assert!(m.matches("file.ts"));
        assert!(m.matches("src/component.ts"));
        assert!(!m.matches("file.rs"));
    }

    #[test]
    fn test_wildcard_question_mark() {
        let m = WildcardMatcher::new("file?.txt");
        assert!(m.matches("file1.txt"));
        assert!(m.matches("file2.txt"));
        assert!(!m.matches("file.txt"));
    }

    #[test]
    fn test_wildcard_command_match() {
        let m = WildcardMatcher::new("git *");
        assert!(m.matches("git status"));
        assert!(m.matches("git push origin main"));
        assert!(!m.matches("npm install"));
    }

    #[test]
    fn test_specificity() {
        let s1 = WildcardMatcher::new("*").specificity();
        let s2 = WildcardMatcher::new("src/**/*.ts").specificity();
        let s3 = WildcardMatcher::new("src/component.ts").specificity();
        // 越具体的模式 specificity 越大
        assert!(s1 < s2);
        assert!(s2 < s3);
    }

    #[test]
    fn test_normalize_path_windows() {
        let n = normalize_path_for_match("src\\components\\Button.tsx");
        assert_eq!(n, "src/components/Button.tsx");
    }
}
