use serde::{Deserialize, Serialize};

/// 工作区信息
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
    /// 工作区根路径
    pub path: String,
    /// 是否为当前活动工作区
    pub is_active: bool,
    /// 工作区目录是否存在于文件系统中
    pub path_exists: bool,
    /// 文件数量
    pub file_count: u32,
    pub created_at: String,
    pub last_accessed: String,
}

/// 工作区配置
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfig {
    pub name: String,
    pub path: String,
}

/// 文件树节点
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileNode {
    pub name: String,
    /// 相对路径
    pub path: String,
    pub is_dir: bool,
    /// 文件大小（字节）
    pub size: Option<u64>,
    /// 最后修改时间
    pub modified: Option<String>,
    /// 文件扩展名
    pub extension: Option<String>,
    /// 子节点（仅目录有）
    pub children: Option<Vec<FileNode>>,
}

/// 搜索选项
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SearchOptions {
    /// 限定文件扩展名，如 ["docx", "pdf"]
    pub extensions: Option<Vec<String>>,
    /// 最大结果数，默认50
    pub max_results: Option<u32>,
    /// 是否搜索文件内容，默认false
    pub include_content: Option<bool>,
}

/// Git 仓库状态
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGitStatus {
    /// 是否为 git 仓库
    pub is_git_repo: bool,
    /// 当前分支名（非 git 仓库时为 null）
    pub branch_name: Option<String>,
    /// 变更文件数量
    pub changed_file_count: u32,
}

/// 搜索结果
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    /// 文件相对路径
    pub path: String,
    pub name: String,
    pub extension: String,
    pub size: u64,
    pub modified: String,
    /// "name" | "content"
    pub match_type: String,
    /// 匹配内容预览
    pub match_preview: Option<String>,
    /// 内容匹配时的行号
    pub line_number: Option<u32>,
}
