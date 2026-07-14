pub mod git_utils;
pub mod logger;

use std::path::PathBuf;

/// 跨平台路径规范化函数
/// 在 Windows 上，std::fs::canonicalize() 会返回 UNC 路径（\\?\C:\...），
/// 这会导致路径比较失败和 Python Sidecar 无法正确处理路径。
/// 使用 dunce::canonicalize() 在 Windows 上去除 UNC 前缀，其他平台保持原行为。
pub fn canonicalize(path: impl AsRef<std::path::Path>) -> std::io::Result<PathBuf> {
    dunce::canonicalize(path.as_ref())
}

/// 去除 Windows UNC 路径前缀（\\?\）
/// Tauri 的 resource_dir() 和 resolve() 在 Windows 上可能返回 UNC 路径，
/// 而 Python 不支持 UNC 路径作为脚本参数，必须去除前缀才能正确传递。
pub fn strip_unc_prefix(path: &std::path::Path) -> PathBuf {
    let path_str = path.to_string_lossy();
    // Windows UNC 前缀：\\?\ 或 \\?\UNC\
    if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
        // 去除 \\?\ 前缀，保留剩余部分（如 C:\...）
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}
