// 允许在测试模块之后定义工具：项目原有结构将测试模块置于文件中部，
// WriteTextFileTool 及阶段三 3.5 新增的 5 个工具均位于测试模块之后。
// 完整重构文件结构（移动测试模块到末尾）超出当前任务范围，这里以 allow 抑制 lint。
#![allow(clippy::items_after_test_module)]

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::registry::ToolRegistry;
use super::trait_def::Tool;
use crate::db::Database;
use crate::models::tool::{ScratchpadEntry, ScratchpadState, ToolResult};

// 子模块声明
pub mod lsp_tools;
pub mod question;
mod sourcecode;
pub mod task;
mod todowrite;
pub mod webfetch;
pub mod websearch;

/// Scratchpad 共享状态类型
/// 全局唯一实例，按 session_id 隔离不同会话的笔记
/// 由 ScratchpadTool 持有写权限，AgentContext 持有读权限（用于每轮刷新摘要）
pub type SharedScratchpadStates = Arc<RwLock<HashMap<String, ScratchpadState>>>;

/// 将相对路径解析为绝对路径
fn resolve_path(path: &str, workspace_root: &str) -> String {
    if path.is_empty() {
        return path.to_string();
    }
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        return path.to_string();
    }
    let root = std::path::Path::new(workspace_root);
    root.join(path).to_string_lossy().to_string()
}

/// 内置工具注册结果
/// 包含 Scratchpad 共享状态和 TaskTool 引用（用于延迟注入 SubAgentExecutor）
pub struct BuiltinToolsRegistration {
    /// Scratchpad 共享状态
    pub scratchpad_states: SharedScratchpadStates,
    /// TaskTool 引用（用于延迟注入 SubAgentExecutor）
    pub task_tool: task::TaskTool,
}

/// 注册所有内置工具
/// 返回 BuiltinToolsRegistration，包含 Scratchpad 共享状态和 TaskTool（用于延迟注入 SubAgentExecutor）
/// git_bash_path: Git Bash 可执行文件路径（空字符串表示从 PATH 自动检测）
/// db: 数据库连接
/// web_search_config: WebSearch 配置（从 AppSettings 读取）
/// question_channels: Question 工具答案通道（与 submit_question_answer 命令共享）
/// app_handle: Tauri AppHandle（用于 QuestionTool 发射事件）
/// lsp_manager: LSP 服务器管理器（阶段 5）
/// lsp_router: LSP 语言路由器（阶段 5）
/// lsp_cache: LSP 结果缓存（阶段 5）
/// skill_registry: Skill 注册表（阶段 3，用于 SkillTool 注册）
/// lsp_experimental_enabled: 是否启用 LSP 实验性工具（阶段 5）
#[allow(clippy::too_many_arguments)]
pub fn register_builtin_tools(
    registry: &mut ToolRegistry,
    git_bash_path: String,
    db: Arc<Database>,
    web_search_config: crate::config::app_settings::WebSearchConfig,
    question_channels: question::QuestionChannels,
    app_handle: Option<tauri::AppHandle<tauri::Wry>>,
    lsp_manager: Arc<crate::services::lsp::manager::LspServerManager>,
    lsp_router: Arc<crate::services::lsp::router::LanguageRouter>,
    lsp_cache: Arc<crate::services::lsp::cache::LspResultCache>,
    skill_registry: Arc<crate::services::skill::registry::SkillRegistry>,
    lsp_experimental_enabled: bool,
) -> BuiltinToolsRegistration {
    log::info!("开始注册内置工具");
    registry.register(Box::new(ListDirectoryTool));
    registry.register(Box::new(SearchFilesTool));
    registry.register(Box::new(ReadFileTool));
    registry.register(Box::new(FileInfoTool));
    registry.register(Box::new(FileExistsTool));
    registry.register(Box::new(DeleteFileTool));
    registry.register(Box::new(CreateDirectoryTool));
    registry.register(Box::new(WriteTextFileTool));
    // 阶段三 3.5 新增的 5 个基础文件系统工具
    registry.register(Box::new(RenameFileTool));
    registry.register(Box::new(CopyFileTool));
    registry.register(Box::new(DeleteDirectoryTool));
    registry.register(Box::new(GetFileHashTool));
    // 阶段 1 编程 Agent 改造: 精确字符串替换工具
    registry.register(Box::new(EditTool));
    // 阶段 1 编程 Agent 改造: glob 模式查找工具
    registry.register(Box::new(GlobTool));
    // 阶段 1 编程 Agent 改造: 正则表达式搜索工具
    registry.register(Box::new(GrepTool));

    // Scratchpad 工具：智能体草稿本，由 agent 自主调用 update_notes 写入
    // 设计参考 Anthropic《Effective Context Engineering for AI Agents》的
    // "Structured Note-taking" 模式，替代外部硬编码迭代元数据注入
    let scratchpad_states: SharedScratchpadStates = Arc::new(RwLock::new(HashMap::new()));
    registry.register(Box::new(ScratchpadTool {
        states: scratchpad_states.clone(),
    }));

    // 代码执行工具：write_script + bash
    // 让智能体通过编写脚本文件并执行命令解决用户问题
    // 命令超时由 LLM 通过 timeout 参数自主决定，最大 300 秒
    registry.register(Box::new(WriteScriptTool));
    registry.register(Box::new(RunCommandTool { git_bash_path }));

    // TodoWrite 工具：结构化任务管理，按 session_id 隔离并持久化到数据库
    registry.register(Box::new(todowrite::TodoWriteTool::new(db)));

    // SourceCode 工具：基于 tree-sitter 的代码语义搜索
    // 支持按符号类型(function/class/struct 等)和名称通配符查询代码符号
    registry.register(Box::new(
        sourcecode::SourceCodeTool::new().expect("创建 SourceCodeTool 失败"),
    ));

    // 阶段 3: Skill 工具（按需加载领域能力）
    registry.register(Box::new(crate::services::skill::tool::SkillTool::new(
        skill_registry,
    )));

    // 阶段 4 新增工具：Task（子 Agent 委托）、WebFetch（URL 获取）、WebSearch（网络搜索）、Question（向用户提问）
    // TaskTool 采用延迟注入模式：先创建不含 sub_executor 的实例并注册，
    // 后续在 lib.rs 中通过 set_sub_executor 注入 SubAgentExecutor
    let task_tool = task::TaskTool::new();
    registry.register(Box::new(task_tool.clone()));
    registry.register(Box::new(webfetch::WebFetchTool::new()));
    registry.register(Box::new(websearch::WebSearchTool::new(web_search_config)));
    registry.register(Box::new(question::QuestionTool::new(
        question_channels,
        app_handle,
    )));

    log::info!("内置工具注册完成, 共注册 25 个工具");

    // 阶段 5: 注册 LSP 工具(实验性,仅在 lsp_experimental_enabled = true 时注册)
    // LSP 工具为单一工具,通过 operation 参数路由 8 种操作
    if lsp_experimental_enabled {
        registry.register(Box::new(
            crate::services::tool::builtin::lsp_tools::LspTool::new(
                lsp_manager,
                lsp_router,
                lsp_cache,
            ),
        ));
        log::info!("已注册 LSP 工具(实验性)");
    }

    BuiltinToolsRegistration {
        scratchpad_states,
        task_tool,
    }
}

// ============================================================
// list_directory - 列出目录内容
// ============================================================

struct ListDirectoryTool;

#[async_trait]
impl Tool for ListDirectoryTool {
    fn tool_name(&self) -> &str {
        "list"
    }
    fn description(&self) -> &str {
        "列出指定目录中的文件和子目录结构。使用场景：浏览工作区内容、查找文件位置、了解目录层级。支持深度控制和扩展名过滤。"
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "目录路径，默认为当前工作目录"
                },
                "depth": {
                    "type": "integer",
                    "description": "遍历深度，默认1",
                    "default": 1
                },
                "extensions": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "筛选文件扩展名，如 [\"docx\", \"pdf\"]"
                }
            }
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let dir_path = params["path"].as_str().unwrap_or(".");
        let max_depth = params["depth"].as_u64().unwrap_or(1) as u32;
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        // 入参校验：depth 必须 >= 1，否则会导致递归条件 u32 下溢（0-1=4294967295）无限递归
        if max_depth == 0 {
            return ToolResult {
                success: false,
                output: None,
                error: Some("depth 参数必须大于等于 1".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        let extensions: Vec<String> = params["extensions"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let resolved_dir = resolve_path(dir_path, workspace_root);
        let dir = std::path::Path::new(&resolved_dir);
        if !dir.exists() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("目录不存在: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        if !dir.is_dir() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是目录: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        // 路径安全校验
        if !workspace_root.is_empty() {
            let canonical_dir = match crate::utils::canonicalize(dir) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("目录路径无效: {}", dir_path)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
            };
            let canonical_root =
                match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                    Ok(p) => p,
                    Err(_) => {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some("工作区根目录路径无效".to_string()),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                };
            if !canonical_dir.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("目录不在工作区内，拒绝访问".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        let resolved_dir_owned = resolved_dir.clone();
        let extensions_clone = extensions.clone();

        let results = match tokio::task::spawn_blocking(move || {
            let dir = std::path::Path::new(&resolved_dir_owned);
            tool_list_dir(dir, dir, max_depth, 0, &extensions_clone)
        })
        .await
        {
            Ok(results) => results,
            Err(join_err) => {
                // spawn_blocking 任务可能因 panic 失败，不应静默吞掉
                log::error!("list_directory spawn_blocking 失败: {}", join_err);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("目录列出任务执行失败: {}", join_err)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                };
            }
        };

        log::info!("列出目录完成: {}, 结果数: {}", dir_path, results.len());
        ToolResult {
            success: true,
            output: Some(json!({
                "path": dir_path,
                "items": results,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            error_code: None,
        }
    }
}

/// 递归列出目录内容
fn tool_list_dir(
    dir: &std::path::Path,
    root: &std::path::Path,
    max_depth: u32,
    current_depth: u32,
    extensions: &[String],
) -> Vec<Value> {
    let mut nodes = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return nodes,
    };

    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by(|a, b| {
        let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_is_dir.cmp(&a_is_dir).then(
            a.file_name()
                .to_string_lossy()
                .to_lowercase()
                .cmp(&b.file_name().to_string_lossy().to_lowercase()),
        )
    });

    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }

        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let is_dir = metadata.is_dir();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if !is_dir && !extensions.is_empty() && !extensions.iter().any(|e| e.to_lowercase() == ext)
        {
            continue;
        }

        let mut node = json!({
            "name": name,
            "path": relative,
            "is_dir": is_dir,
        });

        if !is_dir {
            node["size"] = json!(metadata.len());
            if !ext.is_empty() {
                node["extension"] = json!(ext);
            }
        }

        // 递归条件使用加法避免 u32 下溢：max_depth=0 时 current_depth+1 < max_depth 为 false
        // 与原条件 current_depth < max_depth - 1 在 max_depth >= 1 时等价
        if is_dir && current_depth + 1 < max_depth {
            let children = tool_list_dir(&path, root, max_depth, current_depth + 1, extensions);
            node["children"] = json!(children);
        }

        nodes.push(node);
    }

    nodes
}

// ============================================================
// search_files - 搜索文件
// ============================================================

struct SearchFilesTool;

#[async_trait]
impl Tool for SearchFilesTool {
    fn tool_name(&self) -> &str {
        "search"
    }
    fn description(&self) -> &str {
        "在指定目录中搜索文件，支持按文件名或内容搜索。使用场景：按名称查找文件、按内容关键词搜索、按扩展名筛选。设置include_content=true可搜索文件内容。"
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "搜索关键词（可选，仅按扩展名过滤时可省略）"
                },
                "directory": {
                    "type": "string",
                    "description": "搜索的目录路径，默认为工作区根目录"
                },
                "extensions": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "限定文件扩展名，如 [\"docx\", \"pdf\"]"
                },
                "include_content": {
                    "type": "boolean",
                    "description": "是否搜索文件内容（仅对文本文件有效）",
                    "default": false
                },
                "max_results": {
                    "type": "integer",
                    "description": "最大结果数",
                    "default": 50
                }
            },
            "required": []
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let query = params["query"].as_str().unwrap_or("");
        let directory = params["directory"].as_str().unwrap_or(".");
        let max_results = params["max_results"].as_u64().unwrap_or(50) as usize;
        let include_content = params["include_content"].as_bool().unwrap_or(false);
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        let extensions: Vec<String> = params["extensions"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        if query.is_empty() && extensions.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("搜索关键词和文件扩展名不能同时为空，请至少提供一项".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        let resolved_directory = resolve_path(directory, workspace_root);
        let dir_path = std::path::Path::new(&resolved_directory);
        if !dir_path.exists() || !dir_path.is_dir() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("目录不存在或不是目录: {}", directory)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        if !workspace_root.is_empty() {
            let canonical_dir = match crate::utils::canonicalize(dir_path) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("目录路径无效: {}", directory)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
            };
            let canonical_root =
                match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                    Ok(p) => p,
                    Err(_) => {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some("工作区根目录路径无效".to_string()),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                };
            if !canonical_dir.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("搜索目录不在工作区内，拒绝访问".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        let query_lower = query.to_lowercase();
        let resolved_directory_owned = resolved_directory.clone();
        let extensions_clone = extensions.clone();

        let results = match tokio::task::spawn_blocking(move || {
            let dir_path = std::path::Path::new(&resolved_directory_owned);
            let mut results = Vec::new();
            tool_search_files(
                dir_path,
                dir_path,
                &query_lower,
                &extensions_clone,
                include_content,
                max_results,
                &mut results,
            );
            results
        })
        .await
        {
            Ok(results) => results,
            Err(join_err) => {
                // spawn_blocking 任务可能因 panic 失败，不应静默吞掉
                log::error!("search_files spawn_blocking 失败: {}", join_err);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("文件搜索任务执行失败: {}", join_err)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                };
            }
        };

        log::info!(
            "文件搜索完成: query={}, directory={}, 结果数: {}",
            query,
            directory,
            results.len()
        );
        ToolResult {
            success: true,
            output: Some(json!({
                "query": query,
                "directory": directory,
                "total": results.len(),
                "results": results,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            error_code: None,
        }
    }
}

/// 递归搜索文件
fn tool_search_files(
    dir: &std::path::Path,
    root: &std::path::Path,
    query: &str,
    extensions: &[String],
    include_content: bool,
    max_results: usize,
    results: &mut Vec<Value>,
) {
    if results.len() >= max_results {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        if results.len() >= max_results {
            return;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }

        let path = entry.path();

        if path.is_dir() {
            tool_search_files(
                &path,
                root,
                query,
                extensions,
                include_content,
                max_results,
                results,
            );
            continue;
        }

        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if !extensions.is_empty() && !extensions.iter().any(|e| e.to_lowercase() == ext) {
            continue;
        }

        let name_lower = name.to_lowercase();
        let mut name_matched = query.is_empty() || name_lower.contains(query);
        let mut content_preview = None;

        if include_content && !name_matched && !query.is_empty() {
            let text_extensions = [
                "txt", "md", "markdown", "csv", "json", "xml", "html", "css", "js", "ts", "py",
                "rs", "toml", "yaml", "yml",
            ];
            if text_extensions.contains(&ext.as_str()) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if content.to_lowercase().contains(query) {
                        name_matched = true;
                        if let Some(pos) = content.to_lowercase().find(query) {
                            // 修复：直接按字节切片可能切到非 UTF-8 字符边界导致 panic
                            // 使用 is_char_boundary 调整 start/end 到字符边界
                            let raw_start = pos.saturating_sub(30);
                            let raw_end = (pos + query.len() + 30).min(content.len());

                            // 调整 start 到字符边界（向后移动直到遇到边界）
                            let mut start = raw_start;
                            while start < raw_end && !content.is_char_boundary(start) {
                                start += 1;
                            }

                            // 调整 end 到字符边界（向前移动直到遇到边界）
                            let mut end = raw_end;
                            while end > start && !content.is_char_boundary(end) {
                                end -= 1;
                            }

                            // 仅在有效区间内生成预览，避免空切片
                            if start < end {
                                content_preview = Some(format!("...{}...", &content[start..end]));
                            }
                        }
                    }
                }
            }
        }

        if !name_matched {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let match_type = if content_preview.is_some() {
            "content"
        } else if !query.is_empty() {
            "name"
        } else {
            "extension"
        };

        let mut result = json!({
            "path": relative,
            "name": name,
            "extension": ext,
            "size": metadata.len(),
            "match_type": match_type,
        });

        if let Some(preview) = content_preview {
            result["match_preview"] = json!(preview);
        }

        results.push(result);
    }
}

// ============================================================
// read - 读取纯文本文件（带行号、二进制保护）
// ============================================================

/// 检测文件是否为二进制文件
/// 通过检查前 8KB 字节是否含 NUL 字节（0x00）判定
/// 含 NUL 字节通常表示为二进制文件（如图片、可执行文件、压缩包等）
fn is_binary_file(bytes: &[u8]) -> bool {
    let check_len = bytes.len().min(8192);
    bytes[..check_len].contains(&0x00)
}

/// 为文本内容添加行号
/// 格式：`   123→内容`（行号右对齐，宽度至少 5，后跟 `→` 和内容）
/// start_line: 起始行号（1-based）
/// end_line: 结束行号（1-based，包含在内），None 表示到文件末尾
fn add_line_numbers(content: &str, start_line: usize, end_line: Option<usize>) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    // start_line 是 1-based，转为 0-based 索引
    let start_idx = start_line.saturating_sub(1).min(total);
    // end_line 是 1-based 包含，end_idx 是切片末尾（不包含）
    let end_idx = match end_line {
        Some(end) => end.min(total),
        None => total,
    };

    if start_idx >= end_idx {
        return String::new();
    }

    // 计算行号显示宽度（至少 5）
    let max_line_num = start_line.saturating_add(end_idx - start_idx - 1);
    let width = max_line_num.to_string().len().max(5);

    let mut result = String::new();
    for (i, line) in lines[start_idx..end_idx].iter().enumerate() {
        let line_num = start_line + i;
        result.push_str(&format!("{:>width$}→{}\n", line_num, line, width = width));
    }
    result
}

struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn tool_name(&self) -> &str {
        "read"
    }
    fn description(&self) -> &str {
        "读取纯文本文件内容（.txt/.md/.csv/.json/.xml等），自动添加行号（格式 `   123→内容`），含二进制检测保护。不依赖Sidecar，速度更快。支持按行号范围读取（start_line/end_line参数）。文件大小限制2MB。注意：仅适用于纯文本文件，读取Word/Excel/PPT/PDF等结构化文档请使用docx_handler/xlsx_handler/pptx_handler/pdf_handler的read操作。"
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "start_line": {
                    "type": "integer",
                    "description": "起始行号（1-based），默认 1",
                    "default": 1
                },
                "end_line": {
                    "type": "integer",
                    "description": "结束行号（1-based，包含在内），不填则到文件末尾"
                },
                "encoding": {
                    "type": "string",
                    "description": "文件编码，默认utf-8",
                    "default": "utf-8"
                },
                "max_size": {
                    "type": "integer",
                    "description": "最大读取字节数，默认2MB",
                    "default": 2097152
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let max_size = params["max_size"].as_u64().unwrap_or(2097152) as usize; // 默认 2MB
        let start_line = params["start_line"].as_u64().unwrap_or(1) as usize; // 默认第 1 行
        let end_line = params["end_line"].as_u64().map(|v| v as usize); // 可选
                                                                        // 读取 encoding 参数（默认 utf-8），支持 GBK/GB2312/Big5/Shift_JIS/Latin1 等
        let encoding_label = params["encoding"].as_str().unwrap_or("utf-8");

        if file_path.is_empty() {
            log::warn!("read 失败: 缺少文件路径");
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);

        // 路径安全校验（使用统一的校验函数，包含词法归一化防线）
        if !workspace_root.is_empty() {
            let (canonical_file, _) =
                match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                    Ok(result) => result,
                    Err(e) => {
                        // 根据错误消息区分错误码：路径越界 vs 路径不存在
                        let is_out_of_bounds = e.contains("路径不在工作区内");
                        let error_code = if is_out_of_bounds {
                            Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                        } else {
                            None
                        };
                        log::warn!(
                            "read 失败: {}, path={}, workspace={}",
                            e,
                            file_path,
                            workspace_root
                        );
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some(e),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code,
                        };
                    }
                };
            // 校验通过后，使用 canonical 路径继续读取
            let _ = canonical_file; // 已通过校验，path 变量继续使用（下方会重新 canonicalize 或直接读取）
        }

        if !path.exists() {
            log::warn!("read 失败: 文件不存在, path={}", file_path);
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("文件不存在: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        if !path.is_file() {
            log::warn!("read 失败: 路径不是文件, path={}", file_path);
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是文件: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        // 检查文件大小
        let metadata = match tokio::fs::metadata(&resolved_path).await {
            Ok(m) => m,
            Err(e) => {
                log::warn!(
                    "read 失败: 获取文件信息失败, path={}, 错误: {}",
                    file_path,
                    e
                );
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("获取文件信息失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                };
            }
        };

        if metadata.len() as usize > max_size {
            log::warn!(
                "read 失败: 文件过大, path={}, size={}字节, max={}字节",
                file_path,
                metadata.len(),
                max_size
            );
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "文件过大 ({}字节)，超过最大读取限制 ({}字节)",
                    metadata.len(),
                    max_size
                )),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        // 读取文件字节，根据 encoding 参数解码
        // 支持 UTF-8/GBK/GB2312/Big5/Shift_JIS/Latin1 等多种编码
        match tokio::fs::read(&resolved_path).await {
            Ok(bytes) => {
                // 二进制文件检测：检查前 8KB 是否含 NUL 字节
                if is_binary_file(&bytes) {
                    log::warn!("read 失败: 检测为二进制文件, path={}", file_path);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("文件 {} 检测为二进制文件（含 NUL 字节），无法以文本方式读取。请使用对应的 Handler（如 docx_handler/pdf_handler）处理结构化文档。", file_path)),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }

                // 根据 encoding 标签解析编码器
                let encoding = encoding_rs::Encoding::for_label(encoding_label.as_bytes())
                    .unwrap_or(encoding_rs::UTF_8);
                // 解码字节为字符串（encoding_rs 自动处理 BOM 和无效字节）
                let (content, _actual_encoding, _had_errors) = encoding.decode(&bytes);
                let content = content.into_owned();
                let total_lines = content.lines().count();

                // 按行范围截取并添加行号
                let numbered_content = add_line_numbers(&content, start_line, end_line);
                let returned_lines = numbered_content.lines().count();

                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string();
                log::debug!(
                    "read 完成: {}, start_line={}, end_line={:?}, 返回 {} 行（总 {} 行）",
                    file_path,
                    start_line,
                    end_line,
                    returned_lines,
                    total_lines
                );
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "path": file_path,
                        "content": numbered_content,
                        "start_line": start_line,
                        "end_line": end_line.unwrap_or(total_lines),
                        "total_lines": total_lines,
                        "returned_lines": returned_lines,
                        "size": metadata.len(),
                        "extension": ext,
                        "encoding": encoding.name(),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!("读取文件失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("读取文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建内存数据库供测试使用
    fn test_db() -> Arc<Database> {
        Arc::new(Database::new(std::path::Path::new(":memory:")).unwrap())
    }

    #[test]
    fn test_resolve_path_absolute() {
        let result = resolve_path("/absolute/path/file.txt", "/workspace");
        assert_eq!(result, "/absolute/path/file.txt");
    }

    #[test]
    fn test_resolve_path_relative() {
        let result = resolve_path("relative/path/file.txt", "/workspace");
        let expected = std::path::Path::new("/workspace")
            .join("relative/path/file.txt")
            .to_string_lossy()
            .to_string();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_resolve_path_empty() {
        let result = resolve_path("", "/workspace");
        assert_eq!(result, "");
    }

    #[test]
    fn test_register_builtin_tools() {
        let mut registry = ToolRegistry::new();
        let _scratchpad_states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );

        // 验证 25 个工具都已注册（8 个原有 + 4 个阶段三新增 + 1 个 scratchpad + 2 个代码执行工具 + 3 个阶段 1 新增 edit/glob/grep + 1 个 todowrite + 1 个 source_code + 1 个 skill + 4 个阶段 4 新增 task/webfetch/websearch/question）
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 25);

        // 验证每个工具的基本属性
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"list"));
        assert!(tool_names.contains(&"search"));
        assert!(tool_names.contains(&"read"));
        assert!(tool_names.contains(&"file_info"));
        assert!(tool_names.contains(&"exists"));
        assert!(tool_names.contains(&"remove"));
        assert!(tool_names.contains(&"mkdir"));
        assert!(tool_names.contains(&"write"));
        // 阶段三 3.5 新增工具
        assert!(tool_names.contains(&"rename"));
        assert!(tool_names.contains(&"copy"));
        assert!(tool_names.contains(&"remove_dir"));
        assert!(tool_names.contains(&"hash"));
        // Scratchpad 工具
        assert!(tool_names.contains(&"scratchpad"));
        // 代码执行工具
        assert!(tool_names.contains(&"write_script"));
        assert!(tool_names.contains(&"bash"));
        // 阶段 1 编程 Agent 改造新增工具
        assert!(tool_names.contains(&"edit"));
        assert!(tool_names.contains(&"glob"));
        assert!(tool_names.contains(&"grep"));
        // TodoWrite 工具
        assert!(tool_names.contains(&"todowrite"));
        // SourceCode 工具
        assert!(tool_names.contains(&"source_code"));
        // Skill 工具
        assert!(tool_names.contains(&"skill"));
        // 阶段 4 新增工具
        assert!(tool_names.contains(&"task"));
        assert!(tool_names.contains(&"webfetch"));
        assert!(tool_names.contains(&"websearch"));
        assert!(tool_names.contains(&"question"));
    }

    #[test]
    fn test_tool_definitions_count() {
        let mut registry = ToolRegistry::new();
        let _scratchpad_states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );

        let defs = registry.tool_definitions();
        assert_eq!(defs.len(), 25);

        // 验证每个定义都有 type 和 function 字段
        for def in &defs {
            assert_eq!(def["type"], "function");
            assert!(def["function"]["name"].is_string());
            assert!(def["function"]["description"].is_string());
            assert!(def["function"]["parameters"].is_object());
        }
    }

    #[test]
    fn test_tool_info_properties() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );

        let tools = registry.list_tools();
        for tool in &tools {
            assert!(tool.is_builtin);
            assert!(tool.enabled);
            assert_eq!(tool.version, "1.0.0");
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            // 工具类别：filesystem/memory/code（阶段 1-3）、agent/web（阶段 4 新增）、skill（阶段 3）
            assert!(
                tool.category == "filesystem"
                    || tool.category == "memory"
                    || tool.category == "code"
                    || tool.category == "agent"
                    || tool.category == "web"
                    || tool.category == "skill"
            );
        }
    }

    #[tokio::test]
    async fn test_file_exists_nonexistent() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );

        let tool = registry.get_arc("exists").unwrap();
        let result = tool
            .execute(json!({
                "path": "/nonexistent/path/file.txt",
                "workspace_root": ""
            }))
            .await;

        assert!(result.success);
        assert!(result.output.is_some());
        let output = result.output.unwrap();
        assert_eq!(output["exists"], false);
    }

    #[tokio::test]
    async fn test_read_file_missing_path() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );

        let tool = registry.get_arc("read").unwrap();
        let result = tool
            .execute(json!({
                "path": "",
                "workspace_root": ""
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("缺少文件路径"));
    }

    #[tokio::test]
    async fn test_read_with_line_numbers() {
        // 验证 read 工具返回的内容带行号格式（`   N→内容`）
        use std::io::Write;
        let mut tmp_path = std::env::temp_dir();
        tmp_path.push(format!(
            "docagent_test_read_ln_{}.txt",
            uuid::Uuid::new_v4()
        ));
        {
            let mut f = std::fs::File::create(&tmp_path).unwrap();
            writeln!(f, "first line").unwrap();
            writeln!(f, "second line").unwrap();
            writeln!(f, "third line").unwrap();
        }
        let workspace_root = std::env::temp_dir().to_string_lossy().to_string();
        let file_path = tmp_path.to_string_lossy().to_string();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("read").unwrap();
        let result = tool
            .execute(json!({
                "path": file_path,
                "workspace_root": workspace_root,
            }))
            .await;

        assert!(result.success);
        let output = result.output.unwrap();
        let content = output["content"].as_str().unwrap();
        // 验证每行包含行号格式（→ 字符）
        assert!(content.contains("→first line"));
        assert!(content.contains("→second line"));
        assert!(content.contains("→third line"));
        assert_eq!(output["total_lines"], 3);
        assert_eq!(output["returned_lines"], 3);
        assert_eq!(output["start_line"], 1);
        assert_eq!(output["end_line"], 3);

        let _ = std::fs::remove_file(&tmp_path);
    }

    #[tokio::test]
    async fn test_read_line_range() {
        // 验证 read 工具按行号范围截取（start_line/end_line）
        use std::io::Write;
        let mut tmp_path = std::env::temp_dir();
        tmp_path.push(format!(
            "docagent_test_read_range_{}.txt",
            uuid::Uuid::new_v4()
        ));
        {
            let mut f = std::fs::File::create(&tmp_path).unwrap();
            for i in 1..=10 {
                writeln!(f, "line {}", i).unwrap();
            }
        }
        let workspace_root = std::env::temp_dir().to_string_lossy().to_string();
        let file_path = tmp_path.to_string_lossy().to_string();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("read").unwrap();
        // 读取第 3-5 行
        let result = tool
            .execute(json!({
                "path": file_path,
                "workspace_root": workspace_root,
                "start_line": 3,
                "end_line": 5,
            }))
            .await;

        assert!(result.success);
        let output = result.output.unwrap();
        let content = output["content"].as_str().unwrap();
        assert_eq!(output["total_lines"], 10);
        assert_eq!(output["returned_lines"], 3);
        assert_eq!(output["start_line"], 3);
        assert_eq!(output["end_line"], 5);
        // 验证内容只包含第 3-5 行
        assert!(content.contains("→line 3"));
        assert!(content.contains("→line 4"));
        assert!(content.contains("→line 5"));
        assert!(!content.contains("→line 2"));
        assert!(!content.contains("→line 6"));

        let _ = std::fs::remove_file(&tmp_path);
    }

    #[tokio::test]
    async fn test_edit_tool_create_new_file() {
        // 验证 edit 工具创建新文件（old_string 为空且文件不存在）
        let temp_dir =
            std::env::temp_dir().join(format!("docagent_edit_create_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let file_path = "new_file.txt";
        let new_content = "Hello, this is a new file.\nLine 2.";

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("edit").unwrap();
        let result = tool
            .execute(json!({
                "path": file_path,
                "old_string": "",
                "new_string": new_content,
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "创建新文件失败: {:?}", result.error);
        let output = result.output.unwrap();
        assert_eq!(output["operation"], "create");
        assert_eq!(output["bytes_written"], new_content.len());

        // 验证文件内容
        let abs_path = temp_dir.join(file_path);
        let content = std::fs::read_to_string(&abs_path).unwrap();
        assert_eq!(content, new_content);

        let _ = std::fs::remove_file(&abs_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_edit_tool_replace_unique() {
        // 验证 edit 工具唯一匹配替换
        let temp_dir =
            std::env::temp_dir().join(format!("docagent_edit_replace_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let file_path = "edit_test.txt";
        let abs_path = temp_dir.join(file_path);
        let original = "fn main() {\n    println!(\"hello\");\n}\n";
        std::fs::write(&abs_path, original).unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("edit").unwrap();
        let result = tool
            .execute(json!({
                "path": file_path,
                "old_string": "println!(\"hello\");",
                "new_string": "println!(\"world\");",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "替换失败: {:?}", result.error);
        let output = result.output.unwrap();
        assert_eq!(output["operation"], "edit");
        assert_eq!(output["matches"], 1);

        // 验证替换后内容
        let content = std::fs::read_to_string(&abs_path).unwrap();
        assert_eq!(content, "fn main() {\n    println!(\"world\");\n}\n");

        let _ = std::fs::remove_file(&abs_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_edit_tool_multiple_matches_error() {
        // 验证 edit 工具多处匹配时报错
        let temp_dir =
            std::env::temp_dir().join(format!("docagent_edit_multi_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let file_path = "multi_test.txt";
        let abs_path = temp_dir.join(file_path);
        let original = "foo\nbar\nfoo\nbaz\n";
        std::fs::write(&abs_path, original).unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("edit").unwrap();
        let result = tool
            .execute(json!({
                "path": file_path,
                "old_string": "foo",
                "new_string": "qux",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("找到 2 处匹配"));

        let _ = std::fs::remove_file(&abs_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_edit_tool_no_match_error() {
        // 验证 edit 工具 0 匹配时报错
        let temp_dir =
            std::env::temp_dir().join(format!("docagent_edit_nomatch_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let file_path = "nomatch_test.txt";
        let abs_path = temp_dir.join(file_path);
        let original = "hello world\n";
        std::fs::write(&abs_path, original).unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("edit").unwrap();
        let result = tool
            .execute(json!({
                "path": file_path,
                "old_string": "nonexistent string",
                "new_string": "replacement",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("未找到匹配"));

        let _ = std::fs::remove_file(&abs_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_glob_find_rust_files() {
        // 验证 glob 工具查找 .rs 文件
        let temp_dir = std::env::temp_dir().join(format!("docagent_glob_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        // 创建测试文件
        tokio::fs::write(temp_dir.join("main.rs"), "fn main() {}")
            .await
            .unwrap();
        tokio::fs::write(temp_dir.join("lib.rs"), "pub fn lib() {}")
            .await
            .unwrap();
        tokio::fs::write(temp_dir.join("readme.md"), "# Readme")
            .await
            .unwrap();
        // 创建子目录
        tokio::fs::create_dir_all(temp_dir.join("src"))
            .await
            .unwrap();
        tokio::fs::write(temp_dir.join("src/mod.rs"), "pub mod x;")
            .await
            .unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("glob").unwrap();
        // 用 **/*.rs 查找所有 .rs 文件
        let result = tool
            .execute(json!({
                "pattern": "**/*.rs",
                "path": ".",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "glob 失败: {:?}", result.error);
        let output = result.output.unwrap();
        let matches = output["matches"].as_array().unwrap();
        // 应该找到 3 个 .rs 文件（main.rs, lib.rs, src/mod.rs）
        assert_eq!(matches.len(), 3);
        let match_strs: Vec<String> = matches
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert!(match_strs.iter().any(|s| s.ends_with("main.rs")));
        assert!(match_strs.iter().any(|s| s.ends_with("lib.rs")));
        assert!(match_strs.iter().any(|s| s.ends_with("mod.rs")));
        // 不应包含 readme.md
        assert!(!match_strs.iter().any(|s| s.ends_with("readme.md")));

        // 清理
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_glob_with_excludes() {
        // 验证 glob 工具的 exclude_patterns 参数
        let temp_dir =
            std::env::temp_dir().join(format!("docagent_glob_exc_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        tokio::fs::write(temp_dir.join("keep.rs"), "")
            .await
            .unwrap();
        tokio::fs::create_dir_all(temp_dir.join("target"))
            .await
            .unwrap();
        tokio::fs::write(temp_dir.join("target/build.rs"), "")
            .await
            .unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("glob").unwrap();
        let result = tool
            .execute(json!({
                "pattern": "**/*.rs",
                "exclude_patterns": ["target/**"],
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "glob 失败: {:?}", result.error);
        let output = result.output.unwrap();
        let matches = output["matches"].as_array().unwrap();
        // 应该只找到 keep.rs，排除 target/build.rs
        assert_eq!(matches.len(), 1);
        assert!(matches[0].as_str().unwrap().ends_with("keep.rs"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_grep_basic_search() {
        // 验证 grep 工具基本正则搜索：搜索 "fn " 模式，应只匹配 .rs 文件中的函数定义
        let temp_dir = std::env::temp_dir().join(format!("docagent_grep_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        // 创建测试文件
        tokio::fs::write(
            temp_dir.join("main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .await
        .unwrap();
        tokio::fs::write(temp_dir.join("lib.rs"), "pub fn lib() {}\nfn helper() {}\n")
            .await
            .unwrap();
        tokio::fs::write(temp_dir.join("readme.md"), "# Readme\nnothing here\n")
            .await
            .unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("grep").unwrap();
        // 搜索 "fn " 模式
        let result = tool
            .execute(json!({
                "pattern": "fn ",
                "path": ".",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "grep 失败: {:?}", result.error);
        let output = result.output.unwrap();
        let matches = output["matches"].as_array().unwrap();
        // 应该匹配 main.rs 的 1 行（fn main）和 lib.rs 的 2 行（pub fn lib 和 fn helper）
        // readme.md 不应该匹配
        assert_eq!(matches.len(), 3);
        // 验证所有匹配都是 .rs 文件
        for m in matches {
            let path = m["path"].as_str().unwrap();
            assert!(path.ends_with(".rs"), "不应匹配非 .rs 文件: {}", path);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_grep_with_include() {
        // 验证 grep 工具的 include 参数（文件扩展名过滤）：仅搜索匹配的文件
        let temp_dir =
            std::env::temp_dir().join(format!("docagent_grep_inc_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        // 在 .rs 和 .md 文件中都写入 "fn "
        tokio::fs::write(temp_dir.join("code.rs"), "fn test() {}\n")
            .await
            .unwrap();
        tokio::fs::write(temp_dir.join("doc.md"), "fn fake\n")
            .await
            .unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("grep").unwrap();
        // 只搜索 .rs 文件
        let result = tool
            .execute(json!({
                "pattern": "fn ",
                "path": ".",
                "include": "*.rs",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "grep 失败: {:?}", result.error);
        let output = result.output.unwrap();
        let matches = output["matches"].as_array().unwrap();
        // 应该只匹配 code.rs，不匹配 doc.md
        assert_eq!(matches.len(), 1);
        assert!(matches[0]["path"].as_str().unwrap().ends_with("code.rs"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_grep_case_insensitive() {
        // 验证 grep 工具的 case_insensitive 参数：大小写不敏感匹配
        let temp_dir =
            std::env::temp_dir().join(format!("docagent_grep_ci_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        // 写入不同大小写的内容
        tokio::fs::write(
            temp_dir.join("test.rs"),
            "fn FooBar() {}\nfn foobar() {}\nfn FOOBAR() {}\n",
        )
        .await
        .unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("grep").unwrap();
        // 大小写不敏感搜索 "foobar"
        let result = tool
            .execute(json!({
                "pattern": "foobar",
                "path": ".",
                "case_insensitive": true,
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "grep 失败: {:?}", result.error);
        let output = result.output.unwrap();
        let matches = output["matches"].as_array().unwrap();
        // 应该匹配 3 行（FooBar, foobar, FOOBAR）
        assert_eq!(matches.len(), 3);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_grep_with_context() {
        // 验证 grep 工具的 context_before 和 context_after 参数：返回上下文行
        let temp_dir =
            std::env::temp_dir().join(format!("docagent_grep_ctx_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        // 写入多行内容，匹配行在中间
        let content = "line 1\nline 2\nfn target() {}\nline 4\nline 5\n";
        tokio::fs::write(temp_dir.join("ctx.rs"), content)
            .await
            .unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("grep").unwrap();
        // 搜索 "target"，前后各 1 行上下文
        let result = tool
            .execute(json!({
                "pattern": "target",
                "path": ".",
                "context_before": 1,
                "context_after": 1,
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "grep 失败: {:?}", result.error);
        let output = result.output.unwrap();
        let matches = output["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 1);
        let m = &matches[0];
        assert_eq!(m["line_number"], 3);
        assert_eq!(m["line"].as_str().unwrap(), "fn target() {}");
        // 上下文行验证：匹配行前一行
        let ctx_before = m["context_before"].as_array().unwrap();
        assert_eq!(ctx_before.len(), 1);
        assert_eq!(ctx_before[0].as_str().unwrap(), "line 2");
        // 上下文行验证：匹配行后一行
        let ctx_after = m["context_after"].as_array().unwrap();
        assert_eq!(ctx_after.len(), 1);
        assert_eq!(ctx_after[0].as_str().unwrap(), "line 4");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_create_directory_missing_path() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );

        let tool = registry.get_arc("mkdir").unwrap();
        let result = tool
            .execute(json!({
                "path": "",
                "workspace_root": ""
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("缺少目录路径"));
    }

    #[tokio::test]
    async fn test_write_text_file_missing_path() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );

        let tool = registry.get_arc("write").unwrap();
        let result = tool
            .execute(json!({
                "path": "",
                "content": "test",
                "workspace_root": ""
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("缺少文件路径"));
    }

    #[tokio::test]
    async fn test_delete_file_missing_workspace() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );

        let tool = registry.get_arc("remove").unwrap();
        let result = tool
            .execute(json!({
                "path": "test.txt",
                "workspace_root": ""
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("缺少工作区根目录路径"));
    }

    #[tokio::test]
    async fn test_search_files_empty_query_and_extensions() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );

        let tool = registry.get_arc("search").unwrap();
        let result = tool
            .execute(json!({
                "workspace_root": ""
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("不能同时为空"));
    }

    #[tokio::test]
    async fn test_file_info_missing_path() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );

        let tool = registry.get_arc("file_info").unwrap();
        let result = tool
            .execute(json!({
                "path": "",
                "workspace_root": ""
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("缺少文件路径"));
    }

    /// 测试 encoding 参数：使用 GBK 编码写入中文内容，再用 GBK 编码读取
    /// 验证 encoding_rs 集成是否正确工作
    #[tokio::test]
    async fn test_write_and_read_file_with_gbk_encoding() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );

        // 创建临时工作区目录
        let temp_dir = std::env::temp_dir().join("docagent_encoding_test");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let test_content = "你好，世界！这是 GBK 编码测试。";
        let file_path = "gbk_test.txt";

        // 使用 GBK 编码写入文件
        let write_tool = registry.get_arc("write").unwrap();
        let write_result = write_tool
            .execute(json!({
                "path": file_path,
                "content": test_content,
                "workspace_root": temp_dir.to_string_lossy(),
                "encoding": "gbk"
            }))
            .await;

        assert!(
            write_result.success,
            "GBK 编码写入失败: {:?}",
            write_result.error
        );
        let output = write_result.output.unwrap();
        // encoding_rs 返回规范化的编码名（大写）
        assert_eq!(output["encoding"], "GBK");

        // 使用 GBK 编码读取文件
        let read_tool = registry.get_arc("read").unwrap();
        let read_result = read_tool
            .execute(json!({
                "path": file_path,
                "workspace_root": temp_dir.to_string_lossy(),
                "encoding": "gbk"
            }))
            .await;

        assert!(
            read_result.success,
            "GBK 编码读取失败: {:?}",
            read_result.error
        );
        let read_output = read_result.output.unwrap();
        assert_eq!(read_output["encoding"], "GBK");
        // content 现在带行号格式（`   1→内容`），用 contains 验证原文存在
        assert!(read_output["content"]
            .as_str()
            .unwrap()
            .contains(test_content));

        // 清理临时文件
        let abs_path = temp_dir.join(file_path);
        let _ = tokio::fs::remove_file(&abs_path).await;
        let _ = tokio::fs::remove_dir(&temp_dir).await;
    }

    /// 测试 encoding 参数：UTF-8 默认编码应保持向后兼容
    #[tokio::test]
    async fn test_read_file_default_utf8_encoding() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );

        // 创建临时工作区目录
        let temp_dir = std::env::temp_dir().join("docagent_utf8_test");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let test_content = "Hello, 世界！UTF-8 默认编码测试。";
        let file_path = "utf8_test.txt";
        let abs_path = temp_dir.join(file_path);

        // 直接用 UTF-8 写入文件（模拟已存在的 UTF-8 文件）
        tokio::fs::write(&abs_path, test_content).await.unwrap();

        // 不传 encoding 参数读取（应默认 UTF-8）
        let read_tool = registry.get_arc("read").unwrap();
        let read_result = read_tool
            .execute(json!({
                "path": file_path,
                "workspace_root": temp_dir.to_string_lossy()
            }))
            .await;

        assert!(
            read_result.success,
            "UTF-8 默认读取失败: {:?}",
            read_result.error
        );
        let read_output = read_result.output.unwrap();
        // encoding_rs 返回规范化的编码名（大写）
        assert_eq!(read_output["encoding"], "UTF-8");
        // content 现在带行号格式（`   1→内容`），用 contains 验证原文存在
        assert!(read_output["content"]
            .as_str()
            .unwrap()
            .contains(test_content));

        // 清理临时文件
        let _ = tokio::fs::remove_file(&abs_path).await;
        let _ = tokio::fs::remove_dir(&temp_dir).await;
    }

    /// 测试 encoding 参数：不支持的编码标签应回退到 UTF-8
    #[tokio::test]
    async fn test_read_file_unsupported_encoding_fallback() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );

        // 创建临时工作区目录
        let temp_dir = std::env::temp_dir().join("docagent_fallback_test");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let test_content = "Fallback test 你好";
        let file_path = "fallback_test.txt";
        let abs_path = temp_dir.join(file_path);

        tokio::fs::write(&abs_path, test_content).await.unwrap();

        // 传入不支持的编码标签
        let read_tool = registry.get_arc("read").unwrap();
        let read_result = read_tool
            .execute(json!({
                "path": file_path,
                "workspace_root": temp_dir.to_string_lossy(),
                "encoding": "nonexistent-encoding"
            }))
            .await;

        assert!(
            read_result.success,
            "不支持的编码应回退到 UTF-8，但读取失败: {:?}",
            read_result.error
        );
        let read_output = read_result.output.unwrap();
        // 不支持的编码回退到 UTF-8（encoding_rs 返回大写名称）
        assert_eq!(read_output["encoding"], "UTF-8");
        // content 现在带行号格式（`   1→内容`），用 contains 验证原文存在
        assert!(read_output["content"]
            .as_str()
            .unwrap()
            .contains(test_content));

        // 清理临时文件
        let _ = tokio::fs::remove_file(&abs_path).await;
        let _ = tokio::fs::remove_dir(&temp_dir).await;
    }

    /// 测试 Scratchpad 工具的 add 操作
    #[tokio::test]
    async fn test_scratchpad_add_notes() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );

        let tool = registry.get_arc("scratchpad").unwrap();

        // 第一条笔记
        let result = tool
            .execute(json!({
                "action": "add",
                "content": "已读取 sample.docx，包含 3 个章节",
                "_session_id": "test-session-1",
                "_iteration": 1
            }))
            .await;

        assert!(result.success, "add 失败: {:?}", result.error);
        let output = result.output.unwrap();
        assert_eq!(output["action"], "add");
        assert_eq!(output["total_notes"], 1);

        // 第二条笔记
        let result2 = tool
            .execute(json!({
                "action": "add",
                "content": "识别到需要修改第 2 章的日期",
                "_session_id": "test-session-1",
                "_iteration": 2
            }))
            .await;

        assert!(result2.success);
        assert_eq!(result2.output.unwrap()["total_notes"], 2);
    }

    /// 测试 Scratchpad 工具的 read 操作
    #[tokio::test]
    async fn test_scratchpad_read_notes() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("scratchpad").unwrap();

        // 先添加两条笔记
        tool.execute(json!({
            "action": "add",
            "content": "笔记 A",
            "_session_id": "test-session-read"
        }))
        .await;
        tool.execute(json!({
            "action": "add",
            "content": "笔记 B",
            "_session_id": "test-session-read"
        }))
        .await;

        // 读取笔记
        let result = tool
            .execute(json!({
                "action": "read",
                "_session_id": "test-session-read"
            }))
            .await;

        assert!(result.success);
        let output = result.output.unwrap();
        assert_eq!(output["action"], "read");
        assert_eq!(output["total_notes"], 2);
        let notes = output["notes"].as_array().unwrap();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0], "笔记 A");
        assert_eq!(notes[1], "笔记 B");
    }

    /// 测试 Scratchpad 工具的 clear 操作
    #[tokio::test]
    async fn test_scratchpad_clear_notes() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("scratchpad").unwrap();

        // 添加笔记
        tool.execute(json!({
            "action": "add",
            "content": "待清理的笔记",
            "_session_id": "test-session-clear"
        }))
        .await;

        // 清空
        let result = tool
            .execute(json!({
                "action": "clear",
                "_session_id": "test-session-clear"
            }))
            .await;

        assert!(result.success);
        let output = result.output.unwrap();
        assert_eq!(output["action"], "clear");
        assert_eq!(output["cleared_notes"], 1);

        // 验证已清空
        let read_result = tool
            .execute(json!({
                "action": "read",
                "_session_id": "test-session-clear"
            }))
            .await;
        assert_eq!(read_result.output.unwrap()["total_notes"], 0);
    }

    /// 测试 Scratchpad 工具的会话隔离
    #[tokio::test]
    async fn test_scratchpad_session_isolation() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("scratchpad").unwrap();

        // session-A 添加笔记
        tool.execute(json!({
            "action": "add",
            "content": "会话 A 的笔记",
            "_session_id": "session-A"
        }))
        .await;

        // session-B 添加笔记
        tool.execute(json!({
            "action": "add",
            "content": "会话 B 的笔记 1",
            "_session_id": "session-B"
        }))
        .await;
        tool.execute(json!({
            "action": "add",
            "content": "会话 B 的笔记 2",
            "_session_id": "session-B"
        }))
        .await;

        // 验证 session-A 只有 1 条
        let result_a = tool
            .execute(json!({
                "action": "read",
                "_session_id": "session-A"
            }))
            .await;
        assert_eq!(result_a.output.unwrap()["total_notes"], 1);

        // 验证 session-B 有 2 条
        let result_b = tool
            .execute(json!({
                "action": "read",
                "_session_id": "session-B"
            }))
            .await;
        assert_eq!(result_b.output.unwrap()["total_notes"], 2);
    }

    /// 测试 Scratchpad 缺少 _session_id 时返回错误
    #[tokio::test]
    async fn test_scratchpad_missing_session_id() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("scratchpad").unwrap();

        let result = tool
            .execute(json!({
                "action": "add",
                "content": "测试笔记"
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("缺少会话标识"));
        assert_eq!(result.error_code, Some(crate::errors::TOOL_INVALID_PARAMS));
    }

    /// 测试 Scratchpad add 时 content 为空返回错误
    #[tokio::test]
    async fn test_scratchpad_add_empty_content() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("scratchpad").unwrap();

        let result = tool
            .execute(json!({
                "action": "add",
                "content": "",
                "_session_id": "test-session"
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("content 不能为空"));
    }

    /// 测试 Scratchpad 未知 action 返回错误
    #[tokio::test]
    async fn test_scratchpad_unknown_action() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("scratchpad").unwrap();

        let result = tool
            .execute(json!({
                "action": "delete",
                "_session_id": "test-session"
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("未知 action"));
    }

    /// 测试 Scratchpad 笔记长度限制（500 字符）
    #[tokio::test]
    async fn test_scratchpad_content_length_limit() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );
        let tool = registry.get_arc("scratchpad").unwrap();

        // 构造 1000 字符的长内容
        let long_content = "a".repeat(1000);

        let result = tool
            .execute(json!({
                "action": "add",
                "content": long_content,
                "_session_id": "test-session-limit"
            }))
            .await;

        assert!(result.success);

        // 验证存储的内容被截断到 500 字符
        let read_result = tool
            .execute(json!({
                "action": "read",
                "_session_id": "test-session-limit"
            }))
            .await;
        let binding = read_result.output.unwrap();
        let notes = binding["notes"].as_array().unwrap();
        assert_eq!(notes[0].as_str().unwrap().len(), 500);
    }

    /// 测试 format_scratchpad_summary 函数
    #[test]
    fn test_format_scratchpad_summary() {
        use std::time::SystemTime;

        let states: SharedScratchpadStates = Arc::new(RwLock::new(HashMap::new()));

        // 空状态返回 None
        assert!(format_scratchpad_summary(&states, "empty-session").is_none());

        // 添加笔记
        {
            let mut states_write = states.write().unwrap();
            states_write.insert(
                "test-session".to_string(),
                vec![
                    ScratchpadEntry {
                        content: "第一条笔记".to_string(),
                        iteration: 1,
                        timestamp_ms: SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64,
                    },
                    ScratchpadEntry {
                        content: "第二条笔记".to_string(),
                        iteration: 2,
                        timestamp_ms: SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64,
                    },
                ],
            );
        }

        let summary = format_scratchpad_summary(&states, "test-session");
        assert!(summary.is_some());
        let summary = summary.unwrap();
        assert!(summary.contains("<scratchpad>"));
        assert!(summary.contains("第一条笔记"));
        assert!(summary.contains("第二条笔记"));
        assert!(summary.contains("1. 第一条笔记"));
        assert!(summary.contains("2. 第二条笔记"));
        assert!(summary.contains("scratchpad"));
    }

    /// 测试 is_script_filename 函数：识别脚本文件扩展名
    #[test]
    fn test_is_script_filename() {
        // 脚本文件扩展名应被识别
        assert!(is_script_filename("test.py"));
        assert!(is_script_filename("script.sh"));
        assert!(is_script_filename("run.bash"));
        assert!(is_script_filename("power.ps1"));
        assert!(is_script_filename("batch.bat"));
        assert!(is_script_filename("cmd.cmd"));
        assert!(is_script_filename("ruby.rb"));
        assert!(is_script_filename("lua.lua"));
        assert!(is_script_filename("perl.pl"));

        // 大小写不敏感
        assert!(is_script_filename("TEST.PY"));
        assert!(is_script_filename("Script.SH"));

        // 包含路径的脚本文件
        assert!(is_script_filename("/tmp/test.py"));
        assert!(is_script_filename("D:\\workspace\\script.py"));
        assert!(is_script_filename("subdir/script.sh"));

        // 非脚本文件不应被识别
        assert!(!is_script_filename("readme.txt"));
        assert!(!is_script_filename("notes.md"));
        assert!(!is_script_filename("data.csv"));
        assert!(!is_script_filename("config.json"));
        assert!(!is_script_filename("document.docx"));
        assert!(!is_script_filename("image.png"));
        assert!(!is_script_filename("no_extension"));
    }

    /// 测试 is_script_leak_command 函数：检测 cp 命令将脚本复制到工作区（Windows 风格路径）
    #[test]
    fn test_is_script_leak_command_cp_windows_path() {
        let workspace_root = "D:\\DeskTop\\test";

        // 日志中的实际命令：cp 脚本到工作区（Windows 风格路径）
        let cmd = "cp \"C:/Users/a1926/AppData/Local/Temp/docagent/scripts/modify_resume_pdf.py\" \"D:/DeskTop/test/modify_resume_pdf.py\" && cd \"D:/DeskTop/test\" && python modify_resume_pdf.py 2>&1";
        assert!(
            is_script_leak_command(cmd, workspace_root),
            "Windows 风格路径的 cp 命令应被识别为脚本泄露"
        );
    }

    /// 测试 is_script_leak_command 函数：检测 cp 命令将脚本复制到工作区（Git Bash 风格路径）
    #[test]
    fn test_is_script_leak_command_cp_gitbash_path() {
        let workspace_root = "D:\\DeskTop\\test";

        // 日志中的实际命令：cp 脚本到工作区（Git Bash 风格路径 /d/DeskTop/test）
        let cmd = "cp \"C:/Users/a1926/AppData/Local/Temp/docagent/scripts/fix_resume.py\" \"/d/DeskTop/test/fix_resume.py\" && cd /d/DeskTop/test && python -u fix_resume.py 2>&1";
        assert!(
            is_script_leak_command(cmd, workspace_root),
            "Git Bash 风格路径的 cp 命令应被识别为脚本泄露"
        );
    }

    /// 测试 is_script_leak_command 函数：检测 mv 命令将脚本移动到工作区
    #[test]
    fn test_is_script_leak_command_mv_to_workspace() {
        let workspace_root = "D:\\DeskTop\\test";

        let cmd = "mv /tmp/docagent/scripts/script.py /d/DeskTop/test/script.py";
        assert!(
            is_script_leak_command(cmd, workspace_root),
            "mv 命令将脚本移动到工作区应被识别为脚本泄露"
        );
    }

    /// 测试 is_script_leak_command 函数：检测重定向将脚本写入工作区
    #[test]
    fn test_is_script_leak_command_redirect_to_workspace() {
        let workspace_root = "D:\\DeskTop\\test";

        // 使用 echo + 重定向写入脚本文件
        let cmd = "echo \"print('hello')\" > /d/DeskTop/test/hello.py";
        assert!(
            is_script_leak_command(cmd, workspace_root),
            "重定向写入脚本到工作区应被识别为脚本泄露"
        );

        // 使用 cat + 重定向
        let cmd2 = "cat > /d/DeskTop/test/script.py << EOF\nprint('hello')\nEOF";
        assert!(
            is_script_leak_command(cmd2, workspace_root),
            "cat 重定向写入脚本到工作区应被识别为脚本泄露"
        );
    }

    /// 测试 is_script_leak_command 函数：安全命令不应被误判
    #[test]
    fn test_is_script_leak_command_safe_commands() {
        let workspace_root = "D:\\DeskTop\\test";

        // 直接执行 temp 目录中的脚本（不复制到工作区）
        let cmd1 = "python \"C:/Users/a1926/AppData/Local/Temp/docagent/scripts/script.py\" 2>&1";
        assert!(
            !is_script_leak_command(cmd1, workspace_root),
            "直接执行 temp 目录脚本不应被识别为脚本泄露"
        );

        // 列出工作区文件
        let cmd2 = "ls -la /d/DeskTop/test/";
        assert!(
            !is_script_leak_command(cmd2, workspace_root),
            "ls 命令不应被识别为脚本泄露"
        );

        // 在工作区内执行 python -c 内联代码
        let cmd3 = "cd /d/DeskTop/test && python -c \"print('hello')\"";
        assert!(
            !is_script_leak_command(cmd3, workspace_root),
            "python -c 内联代码不应被识别为脚本泄露"
        );

        // 复制非脚本文件到工作区
        let cmd4 = "cp /tmp/data.csv /d/DeskTop/test/data.csv";
        assert!(
            !is_script_leak_command(cmd4, workspace_root),
            "复制非脚本文件不应被识别为脚本泄露"
        );

        // workspace_root 为空
        let cmd5 = "cp /tmp/script.py /workspace/script.py";
        assert!(
            !is_script_leak_command(cmd5, ""),
            "workspace_root 为空时不应识别为脚本泄露"
        );
    }

    /// 测试 is_script_leak_command 函数：多种脚本扩展名
    #[test]
    fn test_is_script_leak_command_various_script_extensions() {
        let workspace_root = "D:\\DeskTop\\test";

        // .sh 脚本
        assert!(is_script_leak_command(
            "cp /tmp/script.sh /d/DeskTop/test/script.sh",
            workspace_root
        ));
        // .bash 脚本
        assert!(is_script_leak_command(
            "cp /tmp/script.bash /d/DeskTop/test/script.bash",
            workspace_root
        ));
        // .ps1 脚本
        assert!(is_script_leak_command(
            "cp /tmp/script.ps1 /d/DeskTop/test/script.ps1",
            workspace_root
        ));
        // .bat 脚本
        assert!(is_script_leak_command(
            "cp /tmp/script.bat /d/DeskTop/test/script.bat",
            workspace_root
        ));
    }

    /// 集成测试：WriteTextFileTool 拒绝写入脚本文件到工作区
    /// 验证 LLM 试图通过 write_text_file 创建 .py 文件时会被拒绝
    #[tokio::test]
    async fn test_write_text_file_rejects_script_file() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::lsp::manager::LspServerManager::new(
                std::path::PathBuf::from("/tmp"),
                std::time::Duration::from_secs(30),
            )),
            std::sync::Arc::new(crate::services::lsp::router::LanguageRouter::new()),
            std::sync::Arc::new(crate::services::lsp::cache::LspResultCache::new(300, 500)),
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
            false,
        );

        let tool = registry.get_arc("write").unwrap();

        // 尝试写入 .py 脚本文件，应被拒绝
        let result = tool
            .execute(json!({
                "path": "script.py",
                "content": "print('hello')",
                "workspace_root": ""
            }))
            .await;

        assert!(!result.success, "写入 .py 文件应被拒绝");
        assert!(result.error.is_some());
        let error = result.error.unwrap();
        assert!(error.contains("脚本文件"), "错误信息应提及脚本文件");
        assert!(
            error.contains("write_script"),
            "错误信息应引导使用 write_script 工具"
        );

        // 尝试写入 .sh 脚本文件，也应被拒绝
        let result2 = tool
            .execute(json!({
                "path": "script.sh",
                "content": "echo hello",
                "workspace_root": ""
            }))
            .await;

        assert!(!result2.success, "写入 .sh 文件应被拒绝");

        // 写入普通文本文件应成功（不被拒绝）
        let tmp_dir = std::env::temp_dir().join("docagent_test_write_file");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let result3 = tool
            .execute(json!({
                "path": "readme.txt",
                "content": "hello world",
                "workspace_root": tmp_dir.to_string_lossy()
            }))
            .await;

        assert!(result3.success, "写入普通文本文件应成功");
        // 清理临时文件
        let _ = std::fs::remove_file(tmp_dir.join("readme.txt"));
    }
}

// ============================================================
// file_info - 获取文件元数据
// ============================================================

struct FileInfoTool;

#[async_trait]
impl Tool for FileInfoTool {
    fn tool_name(&self) -> &str {
        "file_info"
    }
    fn description(&self) -> &str {
        "获取文件元数据（大小、修改时间、类型等）。使用场景：在读取文件前了解文件信息、检查文件类型、确认文件是否存在且可访问。不需要读取文件内容时优先使用此工具。"
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if file_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);

        // 路径安全校验（使用统一的校验函数，包含词法归一化防线）
        if !workspace_root.is_empty() {
            if let Err(e) = validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                let is_out_of_bounds = e.contains("路径不在工作区内");
                let error_code = if is_out_of_bounds {
                    Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                } else {
                    None
                };
                log::warn!(
                    "file_info 路径校验失败: {}, path={}, workspace={}",
                    e,
                    file_path,
                    workspace_root
                );
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code,
                };
            }
        }

        if !path.exists() {
            log::error!("文件不存在: {}", file_path);
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("文件不存在: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        let metadata = match tokio::fs::metadata(&resolved_path).await {
            Ok(m) => m,
            Err(e) => {
                log::error!("获取文件信息失败: {}, 错误: {}", file_path, e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("获取文件信息失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                };
            }
        };

        let is_dir = metadata.is_dir();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let file_type = if is_dir {
            "directory"
        } else {
            match ext.as_str() {
                "docx" | "doc" => "word",
                "xlsx" | "xls" => "excel",
                "pptx" | "ppt" => "powerpoint",
                "pdf" => "pdf",
                "md" | "markdown" => "markdown",
                "txt" => "text",
                "csv" => "csv",
                "json" => "json",
                "xml" => "xml",
                "html" | "htm" => "html",
                _ => "file",
            }
        };

        ToolResult {
            success: true,
            output: Some(json!({
                "path": file_path,
                "name": path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
                "is_dir": is_dir,
                "size": metadata.len(),
                "extension": ext,
                "file_type": file_type,
                "modified": modified,
                "read_only": metadata.permissions().readonly(),
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            error_code: None,
        }
    }
}

// ============================================================
// file_exists - 检查文件或目录是否存在
// ============================================================

struct FileExistsTool;

#[async_trait]
impl Tool for FileExistsTool {
    fn tool_name(&self) -> &str {
        "exists"
    }
    fn description(&self) -> &str {
        "检查文件或目录是否存在。使用场景：在读取或修改文件前验证路径、避免对不存在的文件执行操作。比list_directory更轻量。"
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件或目录路径（相对于工作区）"
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if file_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);

        // 路径安全校验（使用统一的校验函数，包含词法归一化防线）
        // 注意：file_exists 即使路径不存在也必须先校验越界，否则攻击者可探测工作区外文件
        if !workspace_root.is_empty() {
            if let Err(e) = validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                // 路径不存在时 validate 会返回"路径不存在或无效"，但需要先检查是否越界
                // validate 内部已先做词法归一化，越界会返回"路径不在工作区内"
                let is_out_of_bounds = e.contains("路径不在工作区内");
                let error_code = if is_out_of_bounds {
                    Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                } else {
                    None
                };
                // 路径不存在但未越界时，返回 exists=false 而非错误
                if !is_out_of_bounds {
                    return ToolResult {
                        success: true,
                        output: Some(json!({
                            "path": file_path,
                            "exists": false,
                            "is_dir": false,
                            "is_file": false,
                        })),
                        error: None,
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
                log::warn!(
                    "file_exists 路径越界: {}, path={}, workspace={}",
                    e,
                    file_path,
                    workspace_root
                );
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code,
                };
            }
        }

        let exists = path.exists();
        let is_dir = exists && path.is_dir();
        let is_file = exists && path.is_file();

        ToolResult {
            success: true,
            output: Some(json!({
                "path": file_path,
                "exists": exists,
                "is_dir": is_dir,
                "is_file": is_file,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            error_code: None,
        }
    }
}

// ============================================================
// delete_file - 删除文件
// ============================================================

struct DeleteFileTool;

#[async_trait]
impl Tool for DeleteFileTool {
    fn tool_name(&self) -> &str {
        "remove"
    }
    fn description(&self) -> &str {
        "删除指定文件，删除前可选创建备份。注意：此操作不可逆，会自动触发用户确认。建议在删除前先创建版本快照。"
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要删除的文件路径（相对于工作区）"
                },
                "create_backup": {
                    "type": "boolean",
                    "description": "删除前是否创建备份文件",
                    "default": true
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if file_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        if workspace_root.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少工作区根目录路径，无法进行安全校验".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);

        // 路径安全校验（使用统一的校验函数，包含词法归一化防线）
        let canonical_file =
            match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                Ok((canonical_file, _)) => canonical_file,
                Err(e) => {
                    let is_out_of_bounds = e.contains("路径不在工作区内");
                    let error_code = if is_out_of_bounds {
                        Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                    } else {
                        None
                    };
                    log::warn!(
                        "delete_file 路径校验失败: {}, path={}, workspace={}",
                        e,
                        file_path,
                        workspace_root
                    );
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(e),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code,
                    };
                }
            };

        if !canonical_file.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是文件: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        let safe_path = canonical_file.to_string_lossy().to_string();
        let create_backup = params["create_backup"].as_bool().unwrap_or(true);
        let mut backup_path_str = String::new();

        if create_backup {
            let backup_path = format!("{}.bak", safe_path);
            match tokio::fs::copy(&safe_path, &backup_path).await {
                Ok(_) => {
                    log::info!("删除前已创建备份: {}", backup_path);
                    backup_path_str = backup_path;
                }
                Err(e) => {
                    // 备份失败时拒绝删除，避免数据丢失
                    // 用户可显式设置 create_backup=false 跳过备份后再删除
                    log::error!("创建备份失败: {}, 拒绝删除操作", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!(
                            "创建备份失败: {}。如需跳过备份强制删除，请设置 create_backup=false 后重试",
                            e
                        )),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            }
        }

        match tokio::fs::remove_file(&safe_path).await {
            Ok(_) => {
                log::info!("文件已删除: {}", safe_path);
                let mut result = json!({
                    "path": file_path,
                    "message": format!("文件已删除: {}", file_path),
                });
                if !backup_path_str.is_empty() {
                    result["backup_path"] = json!(backup_path_str);
                }
                ToolResult {
                    success: true,
                    output: Some(result),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!("删除文件失败: {}, 错误: {}", safe_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("删除文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

// ============================================================
// create_directory - 创建目录
// ============================================================

struct CreateDirectoryTool;

#[async_trait]
impl Tool for CreateDirectoryTool {
    fn tool_name(&self) -> &str {
        "mkdir"
    }
    fn description(&self) -> &str {
        "创建目录（支持递归创建）。使用场景：在写入文件前确保目标目录存在、组织文件结构。默认递归创建父目录。"
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "目录路径（相对于工作区）"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "是否递归创建父目录",
                    "default": true
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let dir_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let recursive = params["recursive"].as_bool().unwrap_or(true);

        if dir_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少目录路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(dir_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);

        // 路径安全校验：目标路径必须在工作区内
        if !workspace_root.is_empty() {
            // 对于尚不存在的路径，检查其父目录是否在工作区内
            let check_path = if path.exists() {
                match crate::utils::canonicalize(path) {
                    Ok(p) => p,
                    Err(_) => {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some(format!("路径无效: {}", dir_path)),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                }
            } else {
                // 路径不存在，检查父目录
                match path.parent() {
                    Some(parent) if parent.exists() => match crate::utils::canonicalize(parent) {
                        Ok(p) => p,
                        Err(_) => {
                            return ToolResult {
                                success: false,
                                output: None,
                                error: Some(format!("父目录路径无效: {}", dir_path)),
                                duration_ms: start.elapsed().as_millis() as u64,
                                error_code: None,
                            };
                        }
                    },
                    _ => {
                        // 如果父目录也不存在且 recursive=true，继续尝试
                        // 但仍需校验工作区根目录
                        match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                            Ok(root) => {
                                // 检查解析后的路径是否以工作区根目录开头
                                let resolved_abs = if path.is_absolute() {
                                    path.to_path_buf()
                                } else {
                                    std::path::Path::new(workspace_root).join(dir_path)
                                };
                                // 修复：使用 Path::starts_with 进行路径组件级别比较
                                // 字符串 starts_with 会将 "C:\workspace-evil" 误判为在 "C:\workspace" 内
                                if !resolved_abs.starts_with(&root) {
                                    return ToolResult {
                                        success: false,
                                        output: None,
                                        error: Some("目录路径不在工作区内，拒绝创建".to_string()),
                                        duration_ms: start.elapsed().as_millis() as u64,
                                        error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                                    };
                                }
                                // 校验通过，继续执行
                                path.to_path_buf()
                            }
                            Err(_) => {
                                return ToolResult {
                                    success: false,
                                    output: None,
                                    error: Some("工作区根目录路径无效".to_string()),
                                    duration_ms: start.elapsed().as_millis() as u64,
                                    error_code: None,
                                };
                            }
                        }
                    }
                }
            };

            let canonical_root =
                match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                    Ok(p) => p,
                    Err(_) => {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some("工作区根目录路径无效".to_string()),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                };
            if !check_path.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("目录路径不在工作区内，拒绝创建".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        // 检查目录是否已存在
        if path.exists() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("目录已存在: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        // 检查工作区根目录是否存在，防止自动重建已删除的工作区目录
        if !workspace_root.is_empty() {
            let root_path = std::path::Path::new(workspace_root);
            if !root_path.exists() {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("工作区目录已被删除，请移除该工作区后重新选择".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                };
            }
        }

        let result = if recursive {
            tokio::fs::create_dir_all(&resolved_path).await
        } else {
            tokio::fs::create_dir(&resolved_path).await
        };

        match result {
            Ok(_) => {
                log::info!("目录已创建: {}", dir_path);
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "path": dir_path,
                        "message": format!("目录已创建: {}", dir_path),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!("创建目录失败: {}, 错误: {}", dir_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("创建目录失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

// ============================================================
// write_text_file - 写入纯文本文件
// ============================================================

/// 判断文件名是否为脚本文件
/// 用于阻止通过 write_text_file 工具将脚本文件写入工作区
/// 受保护扩展名：.py/.sh/.bash/.ps1/.bat/.cmd/.rb/.lua/.pl
fn is_script_filename(path: &str) -> bool {
    let lower = path.to_lowercase();
    const SCRIPT_EXTENSIONS: &[&str] = &[
        ".py", ".sh", ".bash", ".ps1", ".bat", ".cmd", ".rb", ".lua", ".pl",
    ];
    SCRIPT_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

struct WriteTextFileTool;

#[async_trait]
impl Tool for WriteTextFileTool {
    fn tool_name(&self) -> &str {
        "write"
    }
    fn description(&self) -> &str {
        "写入纯文本文件内容（.txt/.md/.csv/.json等），不依赖Sidecar。使用场景：创建纯文本文件、修改Markdown文件、保存JSON配置。支持追加模式。注意：仅适用于纯文本，生成结构化文档请使用docx_handler/xlsx_handler/pptx_handler/pdf_handler的generate操作。禁止写入脚本文件（.py/.sh/.bash/.ps1/.bat/.cmd等），脚本文件请使用write_script工具写入系统临时目录。内容大小限制4KB（约4000字符），超出可能触发LLM响应截断。"
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "content": {
                    "type": "string",
                    "description": "文件内容"
                },
                "encoding": {
                    "type": "string",
                    "description": "文件编码，默认utf-8",
                    "default": "utf-8"
                },
                "append": {
                    "type": "boolean",
                    "description": "是否追加写入",
                    "default": false
                }
            },
            "required": ["path", "content"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let content = params["content"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let append = params["append"].as_bool().unwrap_or(false);
        // 读取 encoding 参数（默认 utf-8），支持 GBK/GB2312/Big5/Shift_JIS/Latin1 等
        let encoding_label = params["encoding"].as_str().unwrap_or("utf-8");

        if file_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 安全校验：拒绝写入脚本文件到工作区
        // 脚本文件应通过 write_script 工具创建到系统临时目录，避免污染工作区
        // 受保护扩展名：.py/.sh/.bash/.ps1/.bat/.cmd/.rb/.lua/.pl 等
        if is_script_filename(file_path) {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "不允许通过 write_text_file 写入脚本文件: {}。请改用 write_script 工具将脚本写入系统临时目录，再通过 bash 工具执行",
                    file_path
                )),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);

        // 路径安全校验
        if !workspace_root.is_empty() {
            // 如果文件已存在，直接校验
            // 如果文件不存在，校验父目录
            let check_path = if path.exists() {
                match crate::utils::canonicalize(path) {
                    Ok(p) => p,
                    Err(_) => {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some(format!("路径无效: {}", file_path)),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                }
            } else {
                // 文件不存在，校验父目录
                match path.parent() {
                    Some(parent) if parent.exists() => match crate::utils::canonicalize(parent) {
                        Ok(p) => p,
                        Err(_) => {
                            return ToolResult {
                                success: false,
                                output: None,
                                error: Some(format!("父目录路径无效: {}", file_path)),
                                duration_ms: start.elapsed().as_millis() as u64,
                                error_code: None,
                            };
                        }
                    },
                    _ => {
                        // 父目录也不存在，检查解析路径是否在工作区内
                        match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                            Ok(root) => {
                                let resolved_abs = if path.is_absolute() {
                                    path.to_path_buf()
                                } else {
                                    std::path::Path::new(workspace_root).join(file_path)
                                };
                                // 修复：使用 Path::starts_with 进行路径组件级别比较
                                // 字符串 starts_with 会将 "C:\workspace-evil" 误判为在 "C:\workspace" 内
                                if !resolved_abs.starts_with(&root) {
                                    return ToolResult {
                                        success: false,
                                        output: None,
                                        error: Some("文件路径不在工作区内，拒绝写入".to_string()),
                                        duration_ms: start.elapsed().as_millis() as u64,
                                        error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                                    };
                                }
                                path.to_path_buf()
                            }
                            Err(_) => {
                                return ToolResult {
                                    success: false,
                                    output: None,
                                    error: Some("工作区根目录路径无效".to_string()),
                                    duration_ms: start.elapsed().as_millis() as u64,
                                    error_code: None,
                                };
                            }
                        }
                    }
                }
            };

            let canonical_root =
                match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                    Ok(p) => p,
                    Err(_) => {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some("工作区根目录路径无效".to_string()),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                };
            if !check_path.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("文件路径不在工作区内，拒绝写入".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        // 确保父目录存在
        // 但如果工作区根目录已被删除，不允许自动重建，应提示用户重新选择工作区
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                // 检查工作区根目录是否存在
                if !workspace_root.is_empty() {
                    let root_path = std::path::Path::new(workspace_root);
                    if !root_path.exists() {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some("工作区目录已被删除，请移除该工作区后重新选择".to_string()),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                }
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("创建父目录失败: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
            }
        }

        // 根据 encoding 参数编码内容为字节
        // 支持 UTF-8/GBK/GB2312/Big5/Shift_JIS/Latin1 等多种编码
        let encoding = encoding_rs::Encoding::for_label(encoding_label.as_bytes())
            .unwrap_or(encoding_rs::UTF_8);
        // 编码字符串为字节（encoding_rs 自动处理无法编码的字符）
        let (encoded_bytes, _actual_encoding, _had_errors) = encoding.encode(content);
        let encoded_bytes = encoded_bytes.into_owned();

        let write_result = if append {
            // 追加模式：直接追加到目标文件（原子写入不适用于追加场景）
            match tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&resolved_path)
                .await
            {
                Ok(mut file) => {
                    tokio::io::AsyncWriteExt::write_all(&mut file, &encoded_bytes).await
                }
                Err(e) => Err(e),
            }
        } else {
            // 非追加模式：原子写入（先写临时文件，再 rename 到目标路径）
            // 防止写入过程中崩溃导致原文件损坏
            let tmp_path = format!("{}.tmp", resolved_path);
            match tokio::fs::write(&tmp_path, &encoded_bytes).await {
                Ok(_) => {
                    // rename 是原子操作（同文件系统内）
                    match tokio::fs::rename(&tmp_path, &resolved_path).await {
                        Ok(_) => Ok(()),
                        Err(rename_err) => {
                            // rename 失败，清理临时文件
                            let _ = tokio::fs::remove_file(&tmp_path).await;
                            Err(rename_err)
                        }
                    }
                }
                Err(e) => {
                    // 写入临时文件失败，清理可能残留的临时文件
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    Err(e)
                }
            }
        };

        match write_result {
            Ok(_) => {
                log::info!("文件已写入: {}, 编码: {}", file_path, encoding.name());
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "path": file_path,
                        "message": format!("文件已{}: {}", if append { "追加" } else { "写入" }, file_path),
                        "size": encoded_bytes.len(),
                        "encoding": encoding.name(),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!("写入文件失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("写入文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

// ============================================================
// 阶段三 3.5 新增工具：rename_file / copy_file / delete_directory
// / get_file_hash
// ============================================================

/// 校验已存在的路径是否在工作区内
/// 返回 Ok((canonical_path, canonical_root)) 表示通过校验
/// 返回 Err(error_message) 表示校验失败
/// 用于需要路径安全校验的工具，减少重复代码
fn validate_existing_path_in_workspace(
    resolved_path: &str,
    workspace_root: &str,
) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    if workspace_root.is_empty() {
        return Err("缺少工作区根目录路径，无法进行安全校验".to_string());
    }

    let canonical_root = crate::utils::canonicalize(std::path::Path::new(workspace_root))
        .map_err(|_| format!("工作区根目录不存在或无效: {}", workspace_root))?;

    // 安全防线 1：词法归一化检查（不依赖文件系统）
    // 即使目标文件不存在（canonicalize 会失败），也能识别 `../` 越界并拒绝
    // 避免攻击者通过路径遍历探测文件存在性
    let normalized_path = normalize_path_lexically(resolved_path, &canonical_root);
    if !normalized_path.starts_with(&canonical_root) {
        return Err(format!(
            "路径不在工作区内，拒绝访问: {} (工作区: {})",
            resolved_path,
            canonical_root.display()
        ));
    }

    // 安全防线 2：canonicalize 确认路径真实存在
    let canonical_path = crate::utils::canonicalize(std::path::Path::new(resolved_path))
        .map_err(|_| format!("路径不存在或无效: {}", resolved_path))?;

    // 安全防线 3：组件级 starts_with 比较（避免字符串前缀匹配的绕过风险）
    // 防止符号链接等文件系统层面的绕过
    if !canonical_path.starts_with(&canonical_root) {
        return Err(format!(
            "路径不在工作区内，拒绝访问: {} (工作区: {})",
            canonical_path.display(),
            canonical_root.display()
        ));
    }

    Ok((canonical_path, canonical_root))
}

/// 对路径进行词法归一化（不访问文件系统）
/// 用于在 canonicalize 失败前识别 `..` 越界，避免泄露文件存在性信息
/// 注意：这是安全防护的补充手段，不能替代 canonicalize（无法识别符号链接）
/// Rust 标准库的 Path::components() 会保留 ParentDir(`..`) 组件，
/// 因此必须手动解析 `..` 才能正确判断越界
fn normalize_path_lexically(
    resolved_path: &str,
    workspace_root: &std::path::Path,
) -> std::path::PathBuf {
    use std::path::Component;
    let path = std::path::Path::new(resolved_path);
    // 如果是相对路径，基于工作区拼接
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };

    // 手动解析 `.` 和 `..` 组件（不访问文件系统）
    let mut stack: Vec<std::path::Component<'_>> = Vec::new();
    for comp in absolute.components() {
        match comp {
            Component::CurDir => { /* `.` 忽略 */ }
            Component::ParentDir => {
                // 弹出最后一个 Normal 组件（不弹出根前缀如 Prefix/RootDir）
                if let Some(last) = stack.last() {
                    match last {
                        Component::Normal(_) => {
                            stack.pop();
                        }
                        // 根目录或前缀（如 C:\）下不能再 `..`，忽略
                        Component::RootDir | Component::Prefix(_) => {}
                        Component::ParentDir => stack.push(comp), // 连续 .. 保留
                        Component::CurDir => unreachable!(),
                    }
                }
            }
            _ => stack.push(comp),
        }
    }
    stack.iter().collect::<std::path::PathBuf>()
}

/// 校验目标路径（可能不存在）的父目录是否在工作区内
/// 用于 rename_file/copy_file 的目标路径校验（目标文件可能尚不存在）
/// 返回 Ok(canonical_root) 表示通过校验
fn validate_target_path_in_workspace(
    resolved_target: &str,
    workspace_root: &str,
) -> Result<std::path::PathBuf, String> {
    if workspace_root.is_empty() {
        return Err("缺少工作区根目录路径，无法进行安全校验".to_string());
    }

    let canonical_root = crate::utils::canonicalize(std::path::Path::new(workspace_root))
        .map_err(|_| format!("工作区根目录不存在或无效: {}", workspace_root))?;

    let target_path = std::path::Path::new(resolved_target);
    // 目标路径可能不存在，规范化父目录
    let check_path = if target_path.exists() {
        crate::utils::canonicalize(target_path)
            .map_err(|_| format!("目标路径无效: {}", resolved_target))?
    } else {
        // 父目录必须存在且在工作区内
        let parent = target_path.parent().unwrap_or(std::path::Path::new(""));
        if parent.as_os_str().is_empty() {
            // 没有父目录（如 "file.txt"），用工作区根目录
            canonical_root.clone()
        } else {
            crate::utils::canonicalize(parent)
                .map_err(|_| format!("目标路径的父目录无效: {}", parent.display()))?
        }
    };

    if !check_path.starts_with(&canonical_root) {
        return Err(format!(
            "目标路径不在工作区内，拒绝访问: {} (工作区: {})",
            resolved_target,
            canonical_root.display()
        ));
    }

    Ok(canonical_root)
}

// ============================================================
// rename_file - 重命名/移动文件
// ============================================================

struct RenameFileTool;

#[async_trait]
impl Tool for RenameFileTool {
    fn tool_name(&self) -> &str {
        "rename"
    }
    fn description(&self) -> &str {
        "重命名或移动文件。使用场景：整理文件结构、修改文件名。注意：跨文件系统移动可能失败，此操作不可逆。"
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source_path": {
                    "type": "string",
                    "description": "源文件路径（相对于工作区）"
                },
                "target_path": {
                    "type": "string",
                    "description": "目标文件路径（相对于工作区）"
                }
            },
            "required": ["source_path", "target_path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let source_path = params["source_path"].as_str().unwrap_or("");
        let target_path = params["target_path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if source_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少源文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }
        if target_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少目标文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_source = resolve_path(source_path, workspace_root);
        let resolved_target = resolve_path(target_path, workspace_root);

        // 校验源路径在工作区内
        let (canonical_source, _) =
            match validate_existing_path_in_workspace(&resolved_source, workspace_root) {
                Ok(paths) => paths,
                Err(e) => {
                    log::warn!("rename_file 源路径校验失败: {}", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(e),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                    };
                }
            };

        // 校验目标路径在工作区内
        if let Err(e) = validate_target_path_in_workspace(&resolved_target, workspace_root) {
            log::warn!("rename_file 目标路径校验失败: {}", e);
            return ToolResult {
                success: false,
                output: None,
                error: Some(e),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
            };
        }

        // 安全校验：禁止通过重命名为脚本文件绕过 write_text_file 的限制
        // 智能体可能通过 "write_text_file 写入 .txt + rename_file 改为 .py" 绕过脚本文件写入限制
        // 此检查复用 is_script_filename 函数，检测目标路径是否为脚本扩展名
        if is_script_filename(target_path) {
            log::warn!(
                "rename_file 拒绝重命名为脚本文件: {} -> {}",
                source_path,
                target_path
            );
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "不允许通过 rename_file 将文件重命名为脚本文件: {}。请改用 write_script 工具将脚本写入系统临时目录，再通过 bash 工具执行",
                    target_path
                )),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 源路径必须是文件
        if !canonical_source.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("源路径不是文件: {}", source_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        // 确保目标父目录存在
        let target_p = std::path::Path::new(&resolved_target);
        if let Some(parent) = target_p.parent() {
            if !parent.exists() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("创建目标父目录失败: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
            }
        }

        // 执行重命名
        match tokio::fs::rename(&canonical_source, &resolved_target).await {
            Ok(_) => {
                log::info!("文件已重命名: {} -> {}", source_path, target_path);
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "source_path": source_path,
                        "target_path": target_path,
                        "message": format!("文件已重命名: {} -> {}", source_path, target_path),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!(
                    "重命名文件失败: {} -> {}, 错误: {}",
                    source_path,
                    target_path,
                    e
                );
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("重命名文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

// ============================================================
// copy_file - 复制文件
// ============================================================

struct CopyFileTool;

#[async_trait]
impl Tool for CopyFileTool {
    fn tool_name(&self) -> &str {
        "copy"
    }
    fn description(&self) -> &str {
        "复制文件到新路径。使用场景：创建文件副本、备份文件、复制模板。支持二进制文件复制。"
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source_path": {
                    "type": "string",
                    "description": "源文件路径（相对于工作区）"
                },
                "target_path": {
                    "type": "string",
                    "description": "目标文件路径（相对于工作区）"
                }
            },
            "required": ["source_path", "target_path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let source_path = params["source_path"].as_str().unwrap_or("");
        let target_path = params["target_path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if source_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少源文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }
        if target_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少目标文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_source = resolve_path(source_path, workspace_root);
        let resolved_target = resolve_path(target_path, workspace_root);

        // 校验源路径在工作区内
        let (canonical_source, _) =
            match validate_existing_path_in_workspace(&resolved_source, workspace_root) {
                Ok(paths) => paths,
                Err(e) => {
                    log::warn!("copy_file 源路径校验失败: {}", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(e),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                    };
                }
            };

        // 校验目标路径在工作区内
        if let Err(e) = validate_target_path_in_workspace(&resolved_target, workspace_root) {
            log::warn!("copy_file 目标路径校验失败: {}", e);
            return ToolResult {
                success: false,
                output: None,
                error: Some(e),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
            };
        }

        // 安全校验：禁止通过复制为脚本文件绕过 write_text_file 的限制
        // 与 rename_file 相同的防护逻辑，防止智能体通过 copy_file 将 .txt 复制为 .py
        if is_script_filename(target_path) {
            log::warn!(
                "copy_file 拒绝复制为脚本文件: {} -> {}",
                source_path,
                target_path
            );
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "不允许通过 copy_file 将文件复制为脚本文件: {}。请改用 write_script 工具将脚本写入系统临时目录，再通过 bash 工具执行",
                    target_path
                )),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 源路径必须是文件
        if !canonical_source.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("源路径不是文件: {}", source_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        // 确保目标父目录存在
        let target_p = std::path::Path::new(&resolved_target);
        if let Some(parent) = target_p.parent() {
            if !parent.exists() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("创建目标父目录失败: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
            }
        }

        // 执行复制
        match tokio::fs::copy(&canonical_source, &resolved_target).await {
            Ok(bytes_copied) => {
                log::info!(
                    "文件已复制: {} -> {}, 字节数: {}",
                    source_path,
                    target_path,
                    bytes_copied
                );
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "source_path": source_path,
                        "target_path": target_path,
                        "bytes_copied": bytes_copied,
                        "message": format!("文件已复制: {} -> {}", source_path, target_path),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!(
                    "复制文件失败: {} -> {}, 错误: {}",
                    source_path,
                    target_path,
                    e
                );
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("复制文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

// ============================================================
// delete_directory - 删除目录
// ============================================================

struct DeleteDirectoryTool;

#[async_trait]
impl Tool for DeleteDirectoryTool {
    fn tool_name(&self) -> &str {
        "remove_dir"
    }
    fn description(&self) -> &str {
        "递归删除目录及其所有内容。注意：此操作不可逆，会自动触发用户确认。建议在删除前确认目录内容。"
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要删除的目录路径（相对于工作区）"
                },
                "create_backup": {
                    "type": "boolean",
                    "description": "删除前是否创建备份目录（复制到 .bak 后缀目录），默认 false（目录备份开销较大）",
                    "default": false
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let dir_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let create_backup = params["create_backup"].as_bool().unwrap_or(false);

        if dir_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少目录路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(dir_path, workspace_root);

        // 校验路径在工作区内
        let (canonical_dir, _) =
            match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                Ok(paths) => paths,
                Err(e) => {
                    log::warn!("delete_directory 路径校验失败: {}", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(e),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                    };
                }
            };

        // 必须是目录
        if !canonical_dir.is_dir() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是目录: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        // 禁止删除工作区根目录本身
        let canonical_root = crate::utils::canonicalize(std::path::Path::new(workspace_root))
            .unwrap_or_else(|_| std::path::PathBuf::from(workspace_root));
        if canonical_dir == canonical_root {
            return ToolResult {
                success: false,
                output: None,
                error: Some("禁止删除工作区根目录".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        let safe_path = canonical_dir.to_string_lossy().to_string();
        let mut backup_path_str = String::new();

        // 可选备份：复制到 .bak 目录
        if create_backup {
            let backup_path = format!("{}.bak", safe_path);
            match tokio::fs::create_dir_all(&backup_path).await {
                Ok(_) => {
                    // 递归复制目录内容到备份目录
                    if let Err(e) = copy_dir_recursive(&safe_path, &backup_path).await {
                        log::error!("创建目录备份失败: {}, 拒绝删除操作", e);
                        // 清理部分创建的备份
                        let _ = tokio::fs::remove_dir_all(&backup_path).await;
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some(format!(
                                "创建备份失败: {}。如需跳过备份强制删除，请设置 create_backup=false 后重试",
                                e
                            )),
                            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                        };
                    }
                    log::info!("删除前已创建备份: {}", backup_path);
                    backup_path_str = backup_path;
                }
                Err(e) => {
                    log::error!("创建备份目录失败: {}, 拒绝删除操作", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!(
                            "创建备份失败: {}。如需跳过备份强制删除，请设置 create_backup=false 后重试",
                            e
                        )),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            }
        }

        // 执行删除
        match tokio::fs::remove_dir_all(&safe_path).await {
            Ok(_) => {
                log::info!("目录已删除: {}", safe_path);
                let mut result = json!({
                    "path": dir_path,
                    "message": format!("目录已删除: {}", dir_path),
                });
                if !backup_path_str.is_empty() {
                    result["backup_path"] = json!(backup_path_str);
                }
                ToolResult {
                    success: true,
                    output: Some(result),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!("删除目录失败: {}, 错误: {}", safe_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("删除目录失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

/// 递归复制目录内容到目标目录
/// 用于 delete_directory 的备份功能
async fn copy_dir_recursive(src: &str, dst: &str) -> Result<(), std::io::Error> {
    tokio::task::spawn_blocking({
        let src = src.to_string();
        let dst = dst.to_string();
        move || {
            fn copy_inner(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
                if !dst.exists() {
                    std::fs::create_dir_all(dst)?;
                }
                for entry in std::fs::read_dir(src)? {
                    let entry = entry?;
                    let path = entry.path();
                    let file_name = entry.file_name();
                    let dest_path = dst.join(&file_name);
                    if path.is_dir() {
                        copy_inner(&path, &dest_path)?;
                    } else {
                        std::fs::copy(&path, &dest_path)?;
                    }
                }
                Ok(())
            }
            copy_inner(std::path::Path::new(&src), std::path::Path::new(&dst))
        }
    })
    .await
    .map_err(std::io::Error::other)?
}

// ============================================================
// get_file_hash - 计算文件哈希
// ============================================================

struct GetFileHashTool;

#[async_trait]
impl Tool for GetFileHashTool {
    fn tool_name(&self) -> &str {
        "hash"
    }
    fn description(&self) -> &str {
        "计算文件的 SHA-256 哈希值。使用场景：文件去重、完整性校验、变更检测。返回十六进制哈希字符串。"
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if file_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);

        // 校验路径在工作区内
        let (canonical_file, _) =
            match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                Ok(paths) => paths,
                Err(e) => {
                    log::warn!("get_file_hash 路径校验失败: {}", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(e),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                    };
                }
            };

        // 必须是文件
        if !canonical_file.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是文件: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        // 在 spawn_blocking 中读取文件并计算哈希（避免阻塞异步运行时）
        let hash_result = tokio::task::spawn_blocking(move || {
            use std::io::Read;
            let mut file = std::fs::File::open(&canonical_file)?;
            let mut hasher = Sha256::new();
            // 分块读取，避免大文件一次性加载到内存
            let mut buffer = [0u8; 8192];
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            let hash_bytes = hasher.finalize();
            Ok::<String, std::io::Error>(format!("{:x}", hash_bytes))
        })
        .await;

        match hash_result {
            Ok(Ok(hash)) => {
                log::info!("文件哈希计算完成: {}, sha256={}", file_path, &hash[..16]);
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "path": file_path,
                        "algorithm": "sha256",
                        "hash": hash,
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Ok(Err(e)) => {
                log::error!("计算文件哈希失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("计算文件哈希失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!("计算文件哈希任务失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("计算文件哈希任务失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

// ============================================================
// edit - 精确字符串替换工具
// ============================================================

/// 生成 unified diff 摘要，展示修改前后的内容差异
/// 使用 similar crate 计算行级差异，返回带 +/- 前缀的 diff 文本
fn format_diff_summary(old_content: &str, new_content: &str) -> String {
    use similar::{ChangeTag, TextDiff};

    let diff = TextDiff::from_lines(old_content, new_content);
    let mut result = String::new();

    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        result.push_str(sign);
        result.push_str(change.value());
    }
    result
}

struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn tool_name(&self) -> &str {
        "edit"
    }
    fn description(&self) -> &str {
        "精确字符串替换工具。oldString 必须在文件中唯一匹配（0 匹配报错，多匹配报错，除非 replace_all=true）。当 oldString 为空且文件不存在时，创建新文件。生成 diff 摘要显示修改前后内容。"
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "old_string": {
                    "type": "string",
                    "description": "要替换的原字符串（必须唯一匹配，除非 replace_all=true）。为空且文件不存在时创建新文件"
                },
                "new_string": {
                    "type": "string",
                    "description": "替换后的新字符串"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "是否替换所有匹配项（默认 false，仅替换第一个匹配）。设为 true 时替换所有匹配项，不要求唯一匹配",
                    "default": false
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let old_string = params["old_string"].as_str().unwrap_or("");
        let new_string = params["new_string"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let replace_all = params["replace_all"].as_bool().unwrap_or(false);

        if file_path.is_empty() {
            log::warn!("edit 失败: 缺少文件路径");
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);
        let file_exists = path.exists() && path.is_file();

        // 分支1：创建新文件（old_string 为空且文件不存在）
        if old_string.is_empty() && !file_exists {
            // 校验目标路径的父目录在工作区内
            if !workspace_root.is_empty() {
                if let Err(e) = validate_target_path_in_workspace(&resolved_path, workspace_root) {
                    let is_out_of_bounds = e.contains("不在工作区内");
                    let error_code = if is_out_of_bounds {
                        Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                    } else {
                        None
                    };
                    log::warn!(
                        "edit 失败: {}, path={}, workspace={}",
                        e,
                        file_path,
                        workspace_root
                    );
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(e),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code,
                    };
                }
            }

            // 写入新文件
            match tokio::fs::write(&resolved_path, new_string.as_bytes()).await {
                Ok(_) => {
                    log::info!(
                        "edit 创建新文件: {}, 字节数: {}",
                        file_path,
                        new_string.len()
                    );
                    let diff_summary = format_diff_summary("", new_string);
                    ToolResult {
                        success: true,
                        output: Some(json!({
                            "path": file_path,
                            "operation": "create",
                            "bytes_written": new_string.len(),
                            "diff": diff_summary,
                        })),
                        error: None,
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    }
                }
                Err(e) => {
                    log::error!("edit 创建文件失败: {}, 错误: {}", file_path, e);
                    ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("创建文件失败: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    }
                }
            }
        } else if !file_exists {
            // 文件不存在且 old_string 非空：无法执行替换
            log::warn!(
                "edit 失败: 文件不存在且 old_string 非空, path={}",
                file_path
            );
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "文件 {} 不存在。若要创建新文件，请将 old_string 设为空字符串。",
                    file_path
                )),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        } else {
            // 分支2：编辑已存在文件
            // 文件已存在时 old_string 不能为空（否则会产生大量匹配）
            if old_string.is_empty() {
                log::warn!(
                    "edit 失败: 文件已存在时 old_string 不能为空, path={}",
                    file_path
                );
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(
                        "文件已存在，old_string 不能为空（如需创建新文件请使用其他路径）"
                            .to_string(),
                    ),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
                };
            }

            // 路径安全校验
            if !workspace_root.is_empty() {
                if let Err(e) = validate_existing_path_in_workspace(&resolved_path, workspace_root)
                {
                    let is_out_of_bounds = e.contains("路径不在工作区内");
                    let error_code = if is_out_of_bounds {
                        Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                    } else {
                        None
                    };
                    log::warn!(
                        "edit 失败: {}, path={}, workspace={}",
                        e,
                        file_path,
                        workspace_root
                    );
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(e),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code,
                    };
                }
            }

            // 读取文件内容（UTF-8 编码）
            let old_content = match tokio::fs::read_to_string(&resolved_path).await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("edit 读取文件失败: {}, 错误: {}", file_path, e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("读取文件失败: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
            };

            // 统计 old_string 出现次数
            let match_count = old_content.matches(old_string).count();
            if match_count == 0 {
                log::warn!("edit 失败: 未找到匹配的字符串, path={}", file_path);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("未找到匹配的字符串，old_string 在文件中不存在".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                };
            }
            // 多匹配处理：replace_all=true 时替换所有，否则报错
            if match_count > 1 && !replace_all {
                log::warn!(
                    "edit 失败: 找到 {} 处匹配，需要唯一匹配（或设置 replace_all=true）, path={}",
                    match_count,
                    file_path
                );
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!(
                        "找到 {} 处匹配，需要唯一匹配。如需替换所有匹配，请设置 replace_all=true",
                        match_count
                    )),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                };
            }

            // 执行替换：replace_all=true 时替换所有匹配，否则仅替换第一个
            let new_content = if replace_all {
                old_content.replace(old_string, new_string)
            } else {
                old_content.replacen(old_string, new_string, 1)
            };
            let diff_summary = format_diff_summary(&old_content, &new_content);

            // 写回文件
            match tokio::fs::write(&resolved_path, new_content.as_bytes()).await {
                Ok(_) => {
                    let replaced_count = if replace_all { match_count } else { 1 };
                    log::info!("edit 替换成功: {}, 替换 {} 处", file_path, replaced_count);
                    ToolResult {
                        success: true,
                        output: Some(json!({
                            "path": file_path,
                            "operation": "edit",
                            "matches": match_count,
                            "replacedCount": replaced_count,
                            "diff": diff_summary,
                        })),
                        error: None,
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    }
                }
                Err(e) => {
                    log::error!("edit 写回文件失败: {}, 错误: {}", file_path, e);
                    ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("写回文件失败: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    }
                }
            }
        }
    }
}

// ============================================================
// glob - glob 模式匹配查找文件（遵循 .gitignore）
// ============================================================

struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn tool_name(&self) -> &str {
        "glob"
    }
    fn description(&self) -> &str {
        "glob 模式匹配查找文件。基于 ignore crate，遵循 .gitignore 规则。支持 **/*.rs、{a,b}/*.ts 等模式。返回相对工作区的路径列表（最多 1000 条）。"
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "glob 模式（如 **/*.rs、src/*.ts）"
                },
                "path": {
                    "type": "string",
                    "description": "搜索根目录（相对于工作区），默认 \".\"",
                    "default": "."
                },
                "exclude_patterns": {
                    "type": "array",
                    "description": "排除模式数组（如 [\"node_modules/**\", \"target/**\"]）",
                    "default": []
                }
            },
            "required": ["pattern"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let pattern = params["pattern"].as_str().unwrap_or("");
        let search_path = params["path"].as_str().unwrap_or(".");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        // 获取 exclude_patterns 数组
        let exclude_patterns: Vec<String> = params["exclude_patterns"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        if pattern.is_empty() {
            log::warn!("glob 失败: 缺少 glob 模式");
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少 glob 模式".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(search_path, workspace_root);

        // 路径安全校验
        if !workspace_root.is_empty() {
            if let Err(e) = validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                let is_out_of_bounds = e.contains("路径不在工作区内");
                let error_code = if is_out_of_bounds {
                    Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                } else {
                    None
                };
                log::warn!(
                    "glob 失败: {}, path={}, workspace={}",
                    e,
                    search_path,
                    workspace_root
                );
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code,
                };
            }
        }

        // 构建 globset 匹配器
        let glob_matcher = {
            let mut builder = globset::GlobSetBuilder::new();
            let glob = match globset::Glob::new(pattern) {
                Ok(g) => g,
                Err(e) => {
                    log::warn!("glob 失败: 无效的 glob 模式 '{}': {}", pattern, e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("无效的 glob 模式 '{}': {}", pattern, e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
                    };
                }
            };
            builder.add(glob);
            match builder.build() {
                Ok(m) => m,
                Err(e) => {
                    log::warn!("glob 失败: 构建 glob 匹配器失败: {}", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("构建 glob 匹配器失败: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
            }
        };

        // 构建排除匹配器
        let exclude_matcher = if exclude_patterns.is_empty() {
            None
        } else {
            let mut builder = globset::GlobSetBuilder::new();
            for p in &exclude_patterns {
                match globset::Glob::new(p) {
                    Ok(g) => builder.add(g),
                    Err(e) => {
                        log::warn!("glob 失败: 无效的排除模式 '{}': {}", p, e);
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some(format!("无效的排除模式 '{}': {}", p, e)),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
                        };
                    }
                };
            }
            match builder.build() {
                Ok(m) => Some(m),
                Err(e) => {
                    log::warn!("glob 失败: 构建排除匹配器失败: {}", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("构建排除匹配器失败: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
            }
        };

        // 获取 canonical_root 用于计算相对路径
        let canonical_root = if !workspace_root.is_empty() {
            crate::utils::canonicalize(std::path::Path::new(workspace_root))
                .unwrap_or_else(|_| std::path::PathBuf::from(workspace_root))
        } else {
            std::path::PathBuf::from(&resolved_path)
        };

        // 遍历目录（遵循 .gitignore）
        let walker = ignore::WalkBuilder::new(&resolved_path)
            .hidden(false) // 显示隐藏文件
            .ignore(true) // 遵循 .ignore 文件
            .git_ignore(true) // 遵循 .gitignore 文件
            .git_global(true) // 遵循全局 gitignore
            .build();

        let mut matches: Vec<String> = Vec::new();
        let mut truncated = false;

        for entry in walker.flatten() {
            let path = entry.path();
            // 跳过目录
            if path.is_dir() {
                continue;
            }
            // 将路径转为相对工作区的字符串（统一用 / 分隔符）
            let relative = path
                .strip_prefix(&canonical_root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));

            // 用 globset 匹配
            if glob_matcher.is_match(&relative) {
                // 检查排除
                if let Some(ref exc) = exclude_matcher {
                    if exc.is_match(&relative) {
                        continue;
                    }
                }
                matches.push(relative);
                if matches.len() >= 1000 {
                    truncated = true;
                    break;
                }
            }
        }

        log::debug!(
            "glob 完成: pattern={}, path={}, 匹配 {} 项",
            pattern,
            search_path,
            matches.len()
        );
        ToolResult {
            success: true,
            output: Some(json!({
                "pattern": pattern,
                "path": search_path,
                "matches": matches,
                "count": matches.len(),
                "truncated": truncated,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            error_code: None,
        }
    }
}

// ============================================================
// grep - 正则表达式搜索文件内容（遵循 .gitignore）
// ============================================================

struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn tool_name(&self) -> &str {
        "grep"
    }
    fn description(&self) -> &str {
        "正则表达式搜索文件内容。基于 ignore crate，遵循 .gitignore。支持上下文行（context_before/context_after）、文件扩展名过滤（include）、大小写不敏感。返回匹配列表（含文件路径、行号、行内容、上下文行）。"
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "正则表达式"
                },
                "path": {
                    "type": "string",
                    "description": "搜索根目录（相对于工作区），默认 \".\"",
                    "default": "."
                },
                "include": {
                    "type": "string",
                    "description": "文件扩展名 glob（如 \"*.rs\"），仅搜索匹配的文件"
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "是否大小写不敏感，默认 false",
                    "default": false
                },
                "context_before": {
                    "type": "integer",
                    "description": "匹配行前的上下文行数，默认 0",
                    "default": 0
                },
                "context_after": {
                    "type": "integer",
                    "description": "匹配行后的上下文行数，默认 0",
                    "default": 0
                },
                "max_matches": {
                    "type": "integer",
                    "description": "最大匹配数，默认 100",
                    "default": 100
                }
            },
            "required": ["pattern"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let pattern = params["pattern"].as_str().unwrap_or("");
        let search_path = params["path"].as_str().unwrap_or(".");
        let include = params["include"].as_str();
        let case_insensitive = params["case_insensitive"].as_bool().unwrap_or(false);
        let context_before = params["context_before"].as_u64().unwrap_or(0) as usize;
        let context_after = params["context_after"].as_u64().unwrap_or(0) as usize;
        let max_matches = params["max_matches"].as_u64().unwrap_or(100) as usize;
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if pattern.is_empty() {
            log::warn!("grep 失败: 缺少正则表达式");
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少正则表达式".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(search_path, workspace_root);

        // 路径安全校验
        if !workspace_root.is_empty() {
            if let Err(e) = validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                let is_out_of_bounds = e.contains("路径不在工作区内");
                let error_code = if is_out_of_bounds {
                    Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                } else {
                    None
                };
                log::warn!(
                    "grep 失败: {}, path={}, workspace={}",
                    e,
                    search_path,
                    workspace_root
                );
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code,
                };
            }
        }

        // 编译 regex（支持 (?i) 内联标志和 case_insensitive 参数）
        let re = match regex::RegexBuilder::new(pattern)
            .case_insensitive(case_insensitive)
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                log::warn!("grep 失败: 无效的正则表达式 '{}': {}", pattern, e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("无效的正则表达式 '{}': {}", pattern, e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
                };
            }
        };

        // 构建 include globset 匹配器（若提供 include）
        let include_matcher = if let Some(inc) = include {
            match globset::Glob::new(inc) {
                Ok(g) => match globset::GlobSetBuilder::new().add(g).build() {
                    Ok(m) => Some(m),
                    Err(e) => {
                        log::warn!("grep 失败: 构建 include 匹配器失败: {}", e);
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some(format!("构建 include 匹配器失败: {}", e)),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                },
                Err(e) => {
                    log::warn!("grep 失败: 无效的 include 模式 '{}': {}", inc, e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("无效的 include 模式 '{}': {}", inc, e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
                    };
                }
            }
        } else {
            None
        };

        // 获取 canonical_root 用于计算相对路径
        let canonical_root = if !workspace_root.is_empty() {
            crate::utils::canonicalize(std::path::Path::new(workspace_root))
                .unwrap_or_else(|_| std::path::PathBuf::from(workspace_root))
        } else {
            std::path::PathBuf::from(&resolved_path)
        };

        // 遍历目录（遵循 .gitignore）
        let walker = ignore::WalkBuilder::new(&resolved_path)
            .hidden(false) // 显示隐藏文件
            .ignore(true) // 遵循 .ignore 文件
            .git_ignore(true) // 遵循 .gitignore 文件
            .git_global(true) // 遵循全局 gitignore
            .build();

        let mut matches: Vec<Value> = Vec::new();
        let mut truncated = false;

        'outer: for entry in walker.flatten() {
            let path = entry.path();
            // 跳过目录
            if path.is_dir() {
                continue;
            }

            // 将路径转为相对工作区的字符串（统一用 / 分隔符）
            let relative = path
                .strip_prefix(&canonical_root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));

            // 检查 include 过滤
            if let Some(ref inc) = include_matcher {
                if !inc.is_match(&relative) {
                    continue;
                }
            }

            // 读取文件内容
            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(_) => continue,
            };

            // 跳过二进制文件（用 is_binary_file 检测前 8KB）
            if is_binary_file(&bytes) {
                continue;
            }

            // 解码为字符串（UTF-8，容错处理无效字节）
            let content = String::from_utf8_lossy(&bytes);
            let lines: Vec<&str> = content.lines().collect();

            // 逐行匹配 regex
            for (i, line) in lines.iter().enumerate() {
                if re.is_match(line) {
                    // 收集上下文行（匹配行之前的若干行）
                    let ctx_before: Vec<String> = if context_before > 0 {
                        let start_idx = i.saturating_sub(context_before);
                        lines[start_idx..i].iter().map(|s| s.to_string()).collect()
                    } else {
                        Vec::new()
                    };
                    // 收集上下文行（匹配行之后的若干行）
                    let ctx_after: Vec<String> = if context_after > 0 {
                        let end_idx = (i + 1 + context_after).min(lines.len());
                        lines[i + 1..end_idx]
                            .iter()
                            .map(|s| s.to_string())
                            .collect()
                    } else {
                        Vec::new()
                    };

                    matches.push(json!({
                        "path": relative,
                        "line_number": i + 1,
                        "line": line,
                        "match_type": "content",
                        "context_before": ctx_before,
                        "context_after": ctx_after,
                    }));

                    if matches.len() >= max_matches {
                        truncated = true;
                        break 'outer;
                    }
                }
            }
        }

        log::debug!(
            "grep 完成: pattern={}, path={}, 匹配 {} 项",
            pattern,
            search_path,
            matches.len()
        );
        ToolResult {
            success: true,
            output: Some(json!({
                "pattern": pattern,
                "path": search_path,
                "matches": matches,
                "count": matches.len(),
                "truncated": truncated,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            error_code: None,
        }
    }
}

// ============================================================
// update_notes - 智能体草稿本（Scratchpad）
// ============================================================
//
// 设计依据：Anthropic《Effective Context Engineering for AI Agents》(2025-09-29)
// 的 "Structured Note-taking" 模式。Agent 在长程任务中自主调用本工具记录关键进度、
// 决策点、待办事项，避免外部硬编码迭代元数据（如"迭代轮次 3/100"、"当前步骤"）
// 注入消息列表，从而：
//   1. 避免角色混淆（Role Confusion）——伪 user 消息注入元数据是反模式
//   2. 节省注意力预算——笔记内容是 agent 主动写的，信噪比高于外部猜测
//   3. 培养 agent 自我规划能力——由 agent 决定记录什么、何时记录
//
// 状态隔离：通过 session_id 在 HashMap 中隔离不同会话的笔记
// 注入方式：executor 每轮迭代开始时读取当前 session 的笔记，刷新到
//           AgentContext::scratchpad_summary，由 get_messages_for_iteration
//           追加到消息列表末尾（保留前缀稳定性以最大化缓存命中）

/// Scratchpad 工具：智能体草稿本
/// 持有全局共享状态 Arc，按 session_id 隔离不同会话
pub struct ScratchpadTool {
    pub states: SharedScratchpadStates,
}

/// Scratchpad 工具的 action 枚举
const ACTION_ADD: &str = "add";
const ACTION_READ: &str = "read";
const ACTION_CLEAR: &str = "clear";

#[async_trait]
impl Tool for ScratchpadTool {
    fn tool_name(&self) -> &str {
        "scratchpad"
    }

    fn description(&self) -> &str {
        "智能体草稿本：记录或读取任务笔记，用于跨迭代轮次保持上下文。\
         适用场景：复杂多步骤任务中记录关键决策、待办事项、文件路径、中间结果。\
         建议在完成关键步骤后调用 action=add 记录要点；当任务上下文变长时，\
         action=read 可回顾已有笔记；任务完成后 action=clear 清理。\
         笔记内容会在后续迭代中自动注入到你的上下文，无需重复读取。"
    }

    fn category(&self) -> &str {
        "memory"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "read", "clear"],
                    "description": "操作类型：add=追加笔记；read=读取所有笔记；clear=清空笔记",
                    "default": "add"
                },
                "content": {
                    "type": "string",
                    "description": "笔记内容（action=add 时必填）。建议简明扼要，每条不超过200字"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();

        // 从 params 中取出 _session_id（由 executor 在调用前注入）
        // _session_id 以下划线开头，表示是系统注入参数，不暴露给 LLM
        let session_id = params["_session_id"].as_str().unwrap_or("").to_string();
        if session_id.is_empty() {
            log::warn!("update_notes 调用缺少 _session_id 参数");
            return ToolResult {
                success: false,
                output: None,
                error: Some("内部错误：缺少会话标识".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let action = params["action"].as_str().unwrap_or(ACTION_ADD);
        let content = params["content"].as_str().unwrap_or("").to_string();
        let iteration = params["_iteration"].as_u64().unwrap_or(0) as u32;

        match action {
            ACTION_ADD => {
                if content.is_empty() {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some("action=add 时 content 不能为空".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
                    };
                }

                // 限制单条笔记长度，防止滥用
                let safe_content: String = content.chars().take(500).collect();
                let entry = ScratchpadEntry {
                    content: safe_content,
                    iteration,
                    timestamp_ms: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0),
                };

                let entry_count = {
                    let mut states = self.states.write().expect("scratchpad states 锁中毒");
                    let state = states.entry(session_id.clone()).or_default();
                    state.push(entry);
                    state.len()
                };

                log::info!(
                    "update_notes 追加笔记: session_id={}, 当前笔记数={}",
                    session_id,
                    entry_count
                );

                ToolResult {
                    success: true,
                    output: Some(json!({
                        "action": "add",
                        "total_notes": entry_count,
                        "message": format!("笔记已记录（共 {} 条）", entry_count),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            ACTION_READ => {
                let states = self.states.read().expect("scratchpad states 锁中毒");
                let notes: Vec<&ScratchpadEntry> = states
                    .get(&session_id)
                    .map(|s| s.iter().collect())
                    .unwrap_or_default();

                log::info!(
                    "update_notes 读取笔记: session_id={}, 笔记数={}",
                    session_id,
                    notes.len()
                );

                ToolResult {
                    success: true,
                    output: Some(json!({
                        "action": "read",
                        "total_notes": notes.len(),
                        "notes": notes.iter().map(|e| &e.content).collect::<Vec<_>>(),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            ACTION_CLEAR => {
                let cleared_count = {
                    let mut states = self.states.write().expect("scratchpad states 锁中毒");
                    states.remove(&session_id).map(|s| s.len()).unwrap_or(0)
                };

                log::info!(
                    "update_notes 清空笔记: session_id={}, 已清除 {} 条",
                    session_id,
                    cleared_count
                );

                ToolResult {
                    success: true,
                    output: Some(json!({
                        "action": "clear",
                        "cleared_notes": cleared_count,
                        "message": format!("已清空 {} 条笔记", cleared_count),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            _ => {
                log::warn!("update_notes 未知 action: {}", action);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("未知 action: {}（支持 add/read/clear）", action)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
                }
            }
        }
    }
}

/// 格式化 Scratchpad 笔记列表为摘要字符串（供 AgentContext 注入消息列表）
/// 返回 None 表示无笔记，调用方应跳过注入
pub fn format_scratchpad_summary(
    states: &SharedScratchpadStates,
    session_id: &str,
) -> Option<String> {
    let states = states.read().ok()?;
    let state = states.get(session_id)?;
    if state.is_empty() {
        return None;
    }

    let mut summary = String::from("<scratchpad>\n## 你的任务笔记\n\n以下是你之前记录的任务笔记，请基于这些笔记继续工作（无需重复读取）：\n\n");
    for (i, entry) in state.iter().enumerate() {
        summary.push_str(&format!("{}. {}\n", i + 1, entry.content));
    }
    summary.push_str("\n如需更新笔记，请调用 scratchpad 工具。\n</scratchpad>");
    Some(summary)
}

// ============================================================
// write_script - 写入脚本文件到临时目录
// ============================================================
//
// 让智能体编写 Python 或 Bash 脚本文件，存放在系统临时目录下，
// 供 bash 工具执行。脚本文件不污染工作区目录。
//
// 存放路径：<temp_dir>/docagent/scripts/<filename>
// 脚本语言：python（.py）或 bash（.sh）

/// 脚本写入工具
/// 将智能体编写的脚本内容写入系统临时目录，返回脚本绝对路径
struct WriteScriptTool;

#[async_trait]
impl Tool for WriteScriptTool {
    fn tool_name(&self) -> &str {
        "write_script"
    }

    fn description(&self) -> &str {
        "将脚本内容写入临时文件，供 bash 工具执行。\
         支持编写 Python 或 Bash 脚本解决用户问题（文档处理、数据分析、自动化任务等）。\
         脚本文件存放在系统临时目录，不污染工作区。\
         返回脚本文件的绝对路径，可在 bash 中通过 'python <path>' 或 'bash <path>' 执行。"
    }

    fn category(&self) -> &str {
        "code"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filename": {
                    "type": "string",
                    "description": "脚本文件名（含扩展名，如 'generate_report.py' 或 'process_data.sh'）"
                },
                "language": {
                    "type": "string",
                    "enum": ["python", "bash"],
                    "description": "脚本语言类型：python（.py）或 bash（.sh）。若 filename 已含扩展名，可省略此字段自动推断"
                },
                "content": {
                    "type": "string",
                    "description": "脚本文件内容（完整源代码）"
                }
            },
            "required": ["filename", "content"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();

        let filename = params["filename"].as_str().unwrap_or("").trim();
        let content = params["content"].as_str().unwrap_or("");
        let language = params["language"].as_str().unwrap_or("");

        // 参数校验
        if filename.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少文件名参数".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }
        if content.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("脚本内容不能为空".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 安全校验：禁止文件名包含路径分隔符或 .. 遍历
        if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("文件名包含非法字符: {}", filename)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 推断语言和扩展名
        let (final_filename, detected_language) = infer_script_language(filename, language);
        let _ = detected_language; // 语言仅用于日志，不强制使用

        // 构造脚本目录：<temp_dir>/docagent/scripts/
        let script_dir = std::env::temp_dir().join("docagent").join("scripts");
        if let Err(e) = std::fs::create_dir_all(&script_dir) {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("创建脚本目录失败: {}", e)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        let script_path = script_dir.join(&final_filename);

        // 写入脚本文件
        match tokio::fs::write(&script_path, content).await {
            Ok(()) => {
                let path_str = script_path.to_string_lossy().to_string();
                log::info!(
                    "write_script: 已写入脚本文件: {} (语言: {}, 大小: {} 字节)",
                    path_str,
                    detected_language,
                    content.len()
                );
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "path": path_str,
                        "filename": final_filename,
                        "language": detected_language,
                        "size": content.len(),
                        "message": format!("脚本已写入: {}", final_filename)
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => ToolResult {
                success: false,
                output: None,
                error: Some(format!("写入脚本文件失败: {}", e)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            },
        }
    }
}

/// 根据文件名和语言参数推断最终文件名和语言类型
/// 若 filename 已含扩展名，直接使用；否则根据 language 参数补充扩展名
fn infer_script_language(filename: &str, language: &str) -> (String, &'static str) {
    let lower = filename.to_lowercase();
    if lower.ends_with(".py") {
        (filename.to_string(), "python")
    } else if lower.ends_with(".sh") {
        (filename.to_string(), "bash")
    } else {
        // 无扩展名，根据 language 参数补充
        match language {
            "bash" => (format!("{}.sh", filename), "bash"),
            _ => (format!("{}.py", filename), "python"),
        }
    }
}

// ============================================================
// bash - 执行 Shell 命令（通过 Git Bash）
// ============================================================
//
// 让智能体通过 Git Bash 执行 Shell 命令，支持运行脚本、文件操作、
// 系统命令等。工作目录默认为当前工作区，可通过 working_dir 参数指定。
//
// Git Bash 路径获取优先级：
// 1. 配置中指定的 git_bash_path
// 2. 从 PATH 环境变量查找 git.exe，推断 bash.exe 位置
// 3. 从 PATH 直接查找 bash.exe

/// 检测命令是否试图将脚本文件复制/移动到工作区目录
/// 阻止以下脚本泄露途径：
/// 1. cp/mv 命令将脚本文件从临时目录复制到工作区
/// 2. 重定向（>、>>）将脚本内容写入工作区
/// 3. install 命令将脚本安装到工作区
///
/// 检测逻辑：命令同时满足以下条件时拒绝执行
/// - 包含文件复制/移动/重定向操作（cp/copy/mv/move/install/>/>>）
/// - 命令中出现脚本文件扩展名（.py/.sh/.bash/.ps1/.bat/.cmd 等）
/// - 命令中出现工作区路径（用于判断目标是否为工作区）
///
/// 路径格式兼容：
/// - Windows 风格：D:\DeskTop\test 或 D:/DeskTop/test
/// - Git Bash 风格：/d/DeskTop/test（盘符 D: 转换为 /d/）
fn is_script_leak_command(command: &str, workspace_root: &str) -> bool {
    if workspace_root.is_empty() {
        return false;
    }

    let lower = command.to_lowercase();

    // 计算工作区路径的多种格式表示，使命令中任意一种格式都能匹配
    // 1. Windows 风格（正斜杠）：D:\DeskTop\test -> d:/desktop/test
    let ws_windows = workspace_root.to_lowercase().replace('\\', "/");
    // 2. Git Bash 风格：D:/DeskTop/test -> /d/DeskTop/test
    //    将 "d:/..." 转换为 "/d/..."（移除冒号，前加 /）
    let ws_gitbash = if ws_windows.len() >= 2 && ws_windows.as_bytes()[1] == b':' {
        format!("/{}", ws_windows.replacen(':', "", 1))
    } else {
        ws_windows.clone()
    };

    // 命令中是否出现工作区路径（任意一种格式匹配即可）
    let cmd_normalized = lower.replace('\\', "/");
    let mentions_workspace =
        cmd_normalized.contains(&ws_windows) || cmd_normalized.contains(&ws_gitbash);
    if !mentions_workspace {
        return false;
    }

    // 命令中是否出现脚本文件扩展名（作为子字符串）
    // 命令中的脚本路径可能是 .py、.sh 等扩展名，需要检查多种边界情况
    const SCRIPT_EXT_TOKENS: &[&str] = &[
        ".py ", ".py\"", ".py'", ".py;", ".sh ", ".sh\"", ".sh'", ".sh;", ".bash ", ".bash\"",
        ".bash'", ".bash;", ".ps1 ", ".ps1\"", ".ps1'", ".ps1;", ".bat ", ".bat\"", ".bat'",
        ".bat;", ".cmd ", ".cmd\"", ".cmd'", ".cmd;",
    ];
    let has_script_ext = SCRIPT_EXT_TOKENS.iter().any(|tok| lower.contains(tok))
        || lower.ends_with(".py")
        || lower.ends_with(".sh")
        || lower.ends_with(".bash")
        || lower.ends_with(".ps1")
        || lower.ends_with(".bat")
        || lower.ends_with(".cmd");
    if !has_script_ext {
        return false;
    }

    // 命令中是否包含文件复制/移动/重定向操作
    // cp/copy/mv/move 命令；>、>> 重定向；install 安装命令；tee 写入命令
    lower.contains("cp ")
        || lower.contains("copy ")
        || lower.contains("mv ")
        || lower.contains("move ")
        || lower.contains("install ")
        || lower.contains("> ")
        || lower.contains(">>")
        || lower.contains("tee ")
}

/// 命令执行工具
/// 通过 Git Bash 执行 Shell 命令，捕获 stdout/stderr/exit_code
pub struct RunCommandTool {
    /// Git Bash 可执行文件路径（空字符串表示自动检测）
    pub git_bash_path: String,
}

/// 命令执行默认超时时间（秒），LLM 未传 timeout 参数时使用
const FALLBACK_COMMAND_TIMEOUT_SECS: u64 = 60;

/// 安全截断字符串到指定字节长度（不会在 UTF-8 字符中间截断）
/// 用于命令执行输出过长时的截断处理
fn truncate_safe(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        return s.to_string();
    }
    // 找到不超过 max_chars 的字符边界，避免在 UTF-8 字符中间截断导致 panic
    let mut end = max_chars;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...（截断，总 {} 字符）", &s[..end], s.chars().count())
}

#[async_trait]
impl Tool for RunCommandTool {
    fn tool_name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "通过 Git Bash 执行 Shell 命令。可用于运行脚本文件、执行系统命令、处理文件等。\
         工作目录默认为当前工作区，可通过 working_dir 参数指定其他目录。\
         命令超时默认 60 秒，可通过 timeout 参数调整（最大 300 秒）。\
         输出超过 6000 字符会被自动截断。\
         高风险命令（含 rm/del/rmdir/format/shutdown/sudo/git push --force 等）会请求用户确认。\
         返回 stdout、stderr、exit_code、success、duration_secs 字段。\
         重要：禁止通过 cp/mv/重定向等方式将脚本文件（.py/.sh/.bash等）复制到工作区目录，脚本文件应只在系统临时目录中执行。"
    }

    fn category(&self) -> &str {
        "code"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "要执行的 Shell 命令（将通过 bash -c 执行）。例如: 'python /tmp/docagent/scripts/script.py' 或 'ls -la'"
                },
                "working_dir": {
                    "type": "string",
                    "description": "命令执行的工作目录（可选，默认为当前工作区根目录）"
                },
                "timeout": {
                    "type": "integer",
                    "description": "命令超时时间（秒），默认 60，最大 300",
                    "default": 60
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();

        let command = params["command"].as_str().unwrap_or("").trim().to_string();
        let working_dir = params["working_dir"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        // 命令超时由 LLM 通过 timeout 参数决定，最大 300 秒
        // LLM 未传 timeout 时使用默认值 60 秒
        let timeout = params["timeout"]
            .as_u64()
            .unwrap_or(FALLBACK_COMMAND_TIMEOUT_SECS)
            .min(300);

        // 参数校验
        if command.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少命令参数".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 安全校验：阻止将脚本文件复制/移动到工作区目录
        // 脚本文件应只在系统临时目录中创建和执行，不允许通过 cp/mv/重定向等方式泄露到工作区
        if is_script_leak_command(&command, workspace_root) {
            log::warn!("bash: 检测到脚本泄露命令，已拒绝执行: {}", command);
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "检测到命令试图将脚本文件复制或移动到工作区目录，已拒绝执行。脚本文件应只在系统临时目录中创建和执行，请直接通过 'python <脚本路径>' 或 'bash <脚本路径>' 在临时目录中执行脚本。命令: {}",
                    command
                )),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 解析工作目录：优先使用 working_dir，其次 workspace_root
        let cwd = if !working_dir.is_empty() {
            working_dir.to_string()
        } else if !workspace_root.is_empty() {
            workspace_root.to_string()
        } else {
            String::new()
        };

        // 获取 Git Bash 可执行文件路径
        let bash_path = resolve_bash_path(&self.git_bash_path);
        let bash_path = match bash_path {
            Some(p) => p,
            None => {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(
                        "未找到 Git Bash 可执行文件。请在设置中配置 Git Bash 路径，或确保 git 已安装并添加到 PATH 环境变量".to_string()
                    ),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                };
            }
        };

        log::info!(
            "bash: 执行命令 (cwd='{}', timeout={}s): {}",
            cwd,
            timeout,
            command
        );

        // 在 spawn_blocking 中执行同步的子进程操作，避免阻塞异步运行时
        let command_for_closure = command.clone();
        let cwd_for_closure = cwd.clone();
        let bash_path_for_closure = bash_path.clone();

        let result = tokio::task::spawn_blocking(move || {
            execute_bash_command(
                &bash_path_for_closure,
                &command_for_closure,
                &cwd_for_closure,
                timeout,
            )
        })
        .await;

        match result {
            Ok(Ok(output)) => {
                log::info!(
                    "bash: 命令执行完成 (exit_code={}, stdout={} 字节, stderr={} 字节)",
                    output.exit_code,
                    output.stdout.len(),
                    output.stderr.len()
                );
                // 截断过长输出（6000 字符限制）
                const MAX_OUTPUT_CHARS: usize = 6000;
                let stdout_truncated = truncate_safe(&output.stdout, MAX_OUTPUT_CHARS);
                let stderr_truncated = truncate_safe(&output.stderr, MAX_OUTPUT_CHARS);
                let duration_secs = start.elapsed().as_secs_f64();

                ToolResult {
                    success: output.exit_code == 0,
                    output: Some(json!({
                        "stdout": stdout_truncated,
                        "stderr": stderr_truncated,
                        "exit_code": output.exit_code,
                        "success": output.exit_code == 0,
                        "duration_secs": duration_secs,
                        "command": command,
                        "working_dir": cwd,
                    })),
                    error: if output.exit_code != 0 {
                        Some(format!("命令执行失败，退出码: {}", output.exit_code))
                    } else {
                        None
                    },
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Ok(Err(e)) => {
                log::error!("bash: 命令执行错误: {}", e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("命令执行错误: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!("bash: 任务执行失败: {}", e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("任务执行失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

/// 命令执行结果
struct CommandOutput {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

/// 解析 Git Bash 可执行文件路径
/// 优先使用配置路径，否则从 PATH 环境变量自动检测
fn resolve_bash_path(configured_path: &str) -> Option<String> {
    // 1. 优先使用配置中指定的路径
    if !configured_path.is_empty() {
        let path = std::path::Path::new(configured_path);
        if path.exists() {
            log::debug!("resolve_bash_path: 使用配置路径: {}", configured_path);
            return Some(configured_path.to_string());
        }
        log::warn!(
            "resolve_bash_path: 配置的 Git Bash 路径不存在: {}",
            configured_path
        );
    }

    // 2. 从 PATH 环境变量自动检测
    find_git_bash_from_path()
}

/// 从 PATH 环境变量中查找 Git Bash 可执行文件
/// 检测策略：
///   a. 先从 PATH 中直接查找 bash.exe
///   b. 若未找到，从 PATH 中查找 git.exe，推断 bash.exe 位置（<git_root>/bin/bash.exe）
fn find_git_bash_from_path() -> Option<String> {
    let path_env = std::env::var_os("PATH")?;

    #[cfg(target_os = "windows")]
    {
        use std::path::PathBuf;

        // Windows 上 PATH 使用分号分隔
        let paths: Vec<PathBuf> = std::env::split_paths(&path_env).collect();

        // 策略 a: 从 PATH 中直接查找 bash.exe
        for dir in &paths {
            let bash_candidate = dir.join("bash.exe");
            if bash_candidate.exists() {
                log::info!(
                    "find_git_bash_from_path: 从 PATH 找到 bash.exe: {}",
                    bash_candidate.display()
                );
                return Some(bash_candidate.to_string_lossy().to_string());
            }
        }

        // 策略 b: 从 PATH 中查找 git.exe，推断 bash.exe 位置
        // Git 安装目录结构：<git_root>/cmd/git.exe，bash.exe 在 <git_root>/bin/bash.exe
        for dir in &paths {
            let git_candidate = dir.join("git.exe");
            if git_candidate.exists() {
                // dir 形如 <git_root>/cmd，bash 应在 <git_root>/bin/bash.exe
                if let Some(parent) = dir.parent() {
                    let bash_inferred = parent.join("bin").join("bash.exe");
                    if bash_inferred.exists() {
                        log::info!(
                            "find_git_bash_from_path: 从 git.exe 推断 bash.exe: {}",
                            bash_inferred.display()
                        );
                        return Some(bash_inferred.to_string_lossy().to_string());
                    }
                    // 部分安装可能在 <git_root>/usr/bin/bash.exe
                    let bash_usr = parent.join("usr").join("bin").join("bash.exe");
                    if bash_usr.exists() {
                        log::info!(
                            "find_git_bash_from_path: 从 git.exe 推断 bash.exe (usr/bin): {}",
                            bash_usr.display()
                        );
                        return Some(bash_usr.to_string_lossy().to_string());
                    }
                }
            }
        }

        log::warn!("find_git_bash_from_path: 未在 PATH 中找到 bash.exe 或 git.exe");
        None
    }

    #[cfg(not(target_os = "windows"))]
    {
        // 非 Windows 平台：直接查找 bash
        for dir in std::env::split_paths(&path_env) {
            let bash_candidate = dir.join("bash");
            if bash_candidate.exists() {
                return Some(bash_candidate.to_string_lossy().to_string());
            }
        }
        None
    }
}

/// 执行 bash 命令（同步函数，应在 spawn_blocking 中调用）
fn execute_bash_command(
    bash_path: &str,
    command: &str,
    working_dir: &str,
    timeout_secs: u64,
) -> Result<CommandOutput, String> {
    use std::process::{Command, Stdio};
    use std::time::Instant;

    #[cfg(target_os = "windows")]
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let mut cmd = Command::new(bash_path);
    cmd.arg("-c").arg(command);

    // 设置工作目录
    if !working_dir.is_empty() {
        cmd.current_dir(working_dir);
    }

    // 注入环境变量，优化 Python 脚本执行环境
    // PYTHONIOENCODING=utf-8: 强制 Python 标准输入输出使用 UTF-8 编码
    //   解决 Windows 下 Python 默认使用 GBK 编码导致输出 Unicode 字符（如 \u2022）时报错的问题
    // PYTHONUTF8=1: 启用 Python UTF-8 模式（PEP 540），使所有文件 I/O 默认使用 UTF-8
    //   进一步减少编码相关的失败，提升智能体脚本执行成功率
    cmd.env("PYTHONIOENCODING", "utf-8");
    cmd.env("PYTHONUTF8", "1");

    // 捕获 stdout 和 stderr
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let start = Instant::now();
    let mut child = cmd.spawn().map_err(|e| format!("启动子进程失败: {}", e))?;

    // 使用 tokio 的同步等待 + 超时控制
    // 由于本函数在 spawn_blocking 中调用，可以使用同步等待
    let timeout_duration = Duration::from_secs(timeout_secs);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut s| {
                        use std::io::Read;
                        let mut buf = String::new();
                        s.read_to_string(&mut buf).ok();
                        buf
                    })
                    .unwrap_or_default();
                let stderr = child
                    .stderr
                    .take()
                    .map(|mut s| {
                        use std::io::Read;
                        let mut buf = String::new();
                        s.read_to_string(&mut buf).ok();
                        buf
                    })
                    .unwrap_or_default();

                let exit_code = status.code().unwrap_or(-1);
                return Ok(CommandOutput {
                    stdout,
                    stderr,
                    exit_code,
                });
            }
            Ok(None) => {
                // 子进程仍在运行，检查超时
                if start.elapsed() >= timeout_duration {
                    log::warn!(
                        "execute_bash_command: 命令超时 ({}秒)，终止子进程",
                        timeout_secs
                    );
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("命令执行超时（{}秒），已终止", timeout_secs));
                }
                // 短暂休眠避免 CPU 空转
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return Err(format!("等待子进程失败: {}", e));
            }
        }
    }
}
