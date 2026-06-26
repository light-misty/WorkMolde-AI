// 允许在测试模块之后定义工具：项目原有结构将测试模块置于文件中部，
// WriteTextFileTool 及阶段三 3.5 新增的 5 个工具均位于测试模块之后。
// 完整重构文件结构（移动测试模块到末尾）超出当前任务范围，这里以 allow 抑制 lint。
#![allow(clippy::items_after_test_module)]

use std::time::Instant;

use async_trait::async_trait;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::models::tool::ToolResult;
use super::trait_def::Tool;
use super::registry::ToolRegistry;

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

/// 注册所有内置工具
pub fn register_builtin_tools(registry: &mut ToolRegistry) {
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
    registry.register(Box::new(ReadFileLinesTool));
    log::info!("内置工具注册完成, 共注册 13 个工具");
}

// ============================================================
// list_directory - 列出目录内容
// ============================================================

struct ListDirectoryTool;

#[async_trait]
impl Tool for ListDirectoryTool {
    fn tool_name(&self) -> &str { "list_directory" }
    fn description(&self) -> &str { "列出指定目录中的文件和子目录结构。使用场景：浏览工作区内容、查找文件位置、了解目录层级。支持深度控制和扩展名过滤。" }
    fn category(&self) -> &str { "filesystem" }
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
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        let extensions: Vec<String> = params["extensions"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let resolved_dir = resolve_path(dir_path, workspace_root);
        let dir = std::path::Path::new(&resolved_dir);
        if !dir.exists() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("目录不存在: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        if !dir.is_dir() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是目录: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            let canonical_root = match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some("工作区根目录路径无效".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            if !canonical_dir.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("目录不在工作区内，拒绝访问".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        let resolved_dir_owned = resolved_dir.clone();
        let extensions_clone = extensions.clone();

        let results = match tokio::task::spawn_blocking(move || {
            let dir = std::path::Path::new(&resolved_dir_owned);
            tool_list_dir(dir, dir, max_depth, 0, &extensions_clone)
        }).await {
            Ok(results) => results,
            Err(join_err) => {
                // spawn_blocking 任务可能因 panic 失败，不应静默吞掉
                log::error!("list_directory spawn_blocking 失败: {}", join_err);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("目录列出任务执行失败: {}", join_err)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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

        if !is_dir && !extensions.is_empty() && !extensions.iter().any(|e| e.to_lowercase() == ext) {
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
    fn tool_name(&self) -> &str { "search_files" }
    fn description(&self) -> &str { "在指定目录中搜索文件，支持按文件名或内容搜索。使用场景：按名称查找文件、按内容关键词搜索、按扩展名筛选。设置include_content=true可搜索文件内容。" }
    fn category(&self) -> &str { "filesystem" }
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
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        if query.is_empty() && extensions.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("搜索关键词和文件扩展名不能同时为空，请至少提供一项".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        let resolved_directory = resolve_path(directory, workspace_root);
        let dir_path = std::path::Path::new(&resolved_directory);
        if !dir_path.exists() || !dir_path.is_dir() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("目录不存在或不是目录: {}", directory)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            let canonical_root = match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some("工作区根目录路径无效".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            if !canonical_dir.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("搜索目录不在工作区内，拒绝访问".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        let query_lower = query.to_lowercase();
        let resolved_directory_owned = resolved_directory.clone();
        let extensions_clone = extensions.clone();

        let results = match tokio::task::spawn_blocking(move || {
            let dir_path = std::path::Path::new(&resolved_directory_owned);
            let mut results = Vec::new();
            tool_search_files(dir_path, dir_path, &query_lower, &extensions_clone, include_content, max_results, &mut results);
            results
        }).await {
            Ok(results) => results,
            Err(join_err) => {
                // spawn_blocking 任务可能因 panic 失败，不应静默吞掉
                log::error!("search_files spawn_blocking 失败: {}", join_err);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("文件搜索任务执行失败: {}", join_err)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                };
            }
        };

        log::info!("文件搜索完成: query={}, directory={}, 结果数: {}", query, directory, results.len());
        ToolResult {
            success: true,
            output: Some(json!({
                "query": query,
                "directory": directory,
                "total": results.len(),
                "results": results,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
            tool_search_files(&path, root, query, extensions, include_content, max_results, results);
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
            let text_extensions = ["txt", "md", "markdown", "csv", "json", "xml", "html", "css", "js", "ts", "py", "rs", "toml", "yaml", "yml"];
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
// read_file - 读取纯文本文件
// ============================================================

struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn tool_name(&self) -> &str { "read_file" }
    fn description(&self) -> &str { "读取纯文本文件内容（.txt/.md/.csv/.json/.xml等），不依赖Sidecar，速度更快。注意：仅适用于纯文本文件，读取Word/Excel/PPT/PDF等结构化文档请使用docx_handler/xlsx_handler/pptx_handler/pdf_handler的read操作。文件大小限制1MB。" }
    fn category(&self) -> &str { "filesystem" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "encoding": {
                    "type": "string",
                    "description": "文件编码，默认utf-8",
                    "default": "utf-8"
                },
                "max_size": {
                    "type": "integer",
                    "description": "最大读取字节数，默认1MB",
                    "default": 1048576
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let max_size = params["max_size"].as_u64().unwrap_or(1048576) as usize;
        // 读取 encoding 参数（默认 utf-8），支持 GBK/GB2312/Big5/Shift_JIS/Latin1 等
        let encoding_label = params["encoding"].as_str().unwrap_or("utf-8");

        if file_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);

        // 路径安全校验
        if !workspace_root.is_empty() {
            let canonical_file = match crate::utils::canonicalize(path) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("文件不存在或路径无效: {}", file_path)),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            let canonical_root = match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some("工作区根目录路径无效".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            if !canonical_file.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("文件路径不在工作区内，拒绝访问".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        if !path.exists() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("文件不存在: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        if !path.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是文件: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        // 检查文件大小
        let metadata = match tokio::fs::metadata(&resolved_path).await {
            Ok(m) => m,
            Err(e) => {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("获取文件信息失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                };
            }
        };

        if metadata.len() as usize > max_size {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("文件过大 ({}字节)，超过最大读取限制 ({}字节)", metadata.len(), max_size)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        // 读取文件字节，根据 encoding 参数解码
        // 支持 UTF-8/GBK/GB2312/Big5/Shift_JIS/Latin1 等多种编码
        match tokio::fs::read(&resolved_path).await {
            Ok(bytes) => {
                // 根据 encoding 标签解析编码器
                let encoding = encoding_rs::Encoding::for_label(encoding_label.as_bytes())
                    .unwrap_or(encoding_rs::UTF_8);
                // 解码字节为字符串（encoding_rs 自动处理 BOM 和无效字节）
                let (content, _actual_encoding, _had_errors) = encoding.decode(&bytes);
                let content = content.into_owned();

                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string();
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "path": file_path,
                        "content": content,
                        "size": metadata.len(),
                        "extension": ext,
                        "encoding": encoding.name(),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("读取文件失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("读取文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        register_builtin_tools(&mut registry);

        // 验证 13 个工具都已注册（8 个原有 + 5 个阶段三新增）
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 13);

        // 验证每个工具的基本属性
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"list_directory"));
        assert!(tool_names.contains(&"search_files"));
        assert!(tool_names.contains(&"read_file"));
        assert!(tool_names.contains(&"file_info"));
        assert!(tool_names.contains(&"file_exists"));
        assert!(tool_names.contains(&"delete_file"));
        assert!(tool_names.contains(&"create_directory"));
        assert!(tool_names.contains(&"write_text_file"));
        // 阶段三 3.5 新增工具
        assert!(tool_names.contains(&"rename_file"));
        assert!(tool_names.contains(&"copy_file"));
        assert!(tool_names.contains(&"delete_directory"));
        assert!(tool_names.contains(&"get_file_hash"));
        assert!(tool_names.contains(&"read_file_lines"));
    }

    #[test]
    fn test_tool_definitions_count() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        let defs = registry.tool_definitions();
        assert_eq!(defs.len(), 13);

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
        register_builtin_tools(&mut registry);

        let tools = registry.list_tools();
        for tool in &tools {
            assert!(tool.is_builtin);
            assert!(tool.enabled);
            assert_eq!(tool.version, "1.0.0");
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.category, "filesystem");
        }
    }

    #[tokio::test]
    async fn test_file_exists_nonexistent() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        let tool = registry.get_arc("file_exists").unwrap();
        let result = tool.execute(json!({
            "path": "/nonexistent/path/file.txt",
            "workspace_root": ""
        })).await;

        assert!(result.success);
        assert!(result.output.is_some());
        let output = result.output.unwrap();
        assert_eq!(output["exists"], false);
    }

    #[tokio::test]
    async fn test_read_file_missing_path() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        let tool = registry.get_arc("read_file").unwrap();
        let result = tool.execute(json!({
            "path": "",
            "workspace_root": ""
        })).await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("缺少文件路径"));
    }

    #[tokio::test]
    async fn test_create_directory_missing_path() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        let tool = registry.get_arc("create_directory").unwrap();
        let result = tool.execute(json!({
            "path": "",
            "workspace_root": ""
        })).await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("缺少目录路径"));
    }

    #[tokio::test]
    async fn test_write_text_file_missing_path() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        let tool = registry.get_arc("write_text_file").unwrap();
        let result = tool.execute(json!({
            "path": "",
            "content": "test",
            "workspace_root": ""
        })).await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("缺少文件路径"));
    }

    #[tokio::test]
    async fn test_delete_file_missing_workspace() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        let tool = registry.get_arc("delete_file").unwrap();
        let result = tool.execute(json!({
            "path": "test.txt",
            "workspace_root": ""
        })).await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("缺少工作区根目录路径"));
    }

    #[tokio::test]
    async fn test_search_files_empty_query_and_extensions() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        let tool = registry.get_arc("search_files").unwrap();
        let result = tool.execute(json!({
            "workspace_root": ""
        })).await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("不能同时为空"));
    }

    #[tokio::test]
    async fn test_file_info_missing_path() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        let tool = registry.get_arc("file_info").unwrap();
        let result = tool.execute(json!({
            "path": "",
            "workspace_root": ""
        })).await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("缺少文件路径"));
    }

    /// 测试 encoding 参数：使用 GBK 编码写入中文内容，再用 GBK 编码读取
    /// 验证 encoding_rs 集成是否正确工作
    #[tokio::test]
    async fn test_write_and_read_file_with_gbk_encoding() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        // 创建临时工作区目录
        let temp_dir = std::env::temp_dir().join("docagent_encoding_test");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let test_content = "你好，世界！这是 GBK 编码测试。";
        let file_path = "gbk_test.txt";

        // 使用 GBK 编码写入文件
        let write_tool = registry.get_arc("write_text_file").unwrap();
        let write_result = write_tool.execute(json!({
            "path": file_path,
            "content": test_content,
            "workspace_root": temp_dir.to_string_lossy(),
            "encoding": "gbk"
        })).await;

        assert!(write_result.success, "GBK 编码写入失败: {:?}", write_result.error);
        let output = write_result.output.unwrap();
        // encoding_rs 返回规范化的编码名（大写）
        assert_eq!(output["encoding"], "GBK");

        // 使用 GBK 编码读取文件
        let read_tool = registry.get_arc("read_file").unwrap();
        let read_result = read_tool.execute(json!({
            "path": file_path,
            "workspace_root": temp_dir.to_string_lossy(),
            "encoding": "gbk"
        })).await;

        assert!(read_result.success, "GBK 编码读取失败: {:?}", read_result.error);
        let read_output = read_result.output.unwrap();
        assert_eq!(read_output["encoding"], "GBK");
        assert_eq!(read_output["content"].as_str().unwrap(), test_content);

        // 清理临时文件
        let abs_path = temp_dir.join(file_path);
        let _ = tokio::fs::remove_file(&abs_path).await;
        let _ = tokio::fs::remove_dir(&temp_dir).await;
    }

    /// 测试 encoding 参数：UTF-8 默认编码应保持向后兼容
    #[tokio::test]
    async fn test_read_file_default_utf8_encoding() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        // 创建临时工作区目录
        let temp_dir = std::env::temp_dir().join("docagent_utf8_test");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let test_content = "Hello, 世界！UTF-8 默认编码测试。";
        let file_path = "utf8_test.txt";
        let abs_path = temp_dir.join(file_path);

        // 直接用 UTF-8 写入文件（模拟已存在的 UTF-8 文件）
        tokio::fs::write(&abs_path, test_content).await.unwrap();

        // 不传 encoding 参数读取（应默认 UTF-8）
        let read_tool = registry.get_arc("read_file").unwrap();
        let read_result = read_tool.execute(json!({
            "path": file_path,
            "workspace_root": temp_dir.to_string_lossy()
        })).await;

        assert!(read_result.success, "UTF-8 默认读取失败: {:?}", read_result.error);
        let read_output = read_result.output.unwrap();
        // encoding_rs 返回规范化的编码名（大写）
        assert_eq!(read_output["encoding"], "UTF-8");
        assert_eq!(read_output["content"].as_str().unwrap(), test_content);

        // 清理临时文件
        let _ = tokio::fs::remove_file(&abs_path).await;
        let _ = tokio::fs::remove_dir(&temp_dir).await;
    }

    /// 测试 encoding 参数：不支持的编码标签应回退到 UTF-8
    #[tokio::test]
    async fn test_read_file_unsupported_encoding_fallback() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        // 创建临时工作区目录
        let temp_dir = std::env::temp_dir().join("docagent_fallback_test");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let test_content = "Fallback test 你好";
        let file_path = "fallback_test.txt";
        let abs_path = temp_dir.join(file_path);

        tokio::fs::write(&abs_path, test_content).await.unwrap();

        // 传入不支持的编码标签
        let read_tool = registry.get_arc("read_file").unwrap();
        let read_result = read_tool.execute(json!({
            "path": file_path,
            "workspace_root": temp_dir.to_string_lossy(),
            "encoding": "nonexistent-encoding"
        })).await;

        assert!(read_result.success, "不支持的编码应回退到 UTF-8，但读取失败: {:?}", read_result.error);
        let read_output = read_result.output.unwrap();
        // 不支持的编码回退到 UTF-8（encoding_rs 返回大写名称）
        assert_eq!(read_output["encoding"], "UTF-8");
        assert_eq!(read_output["content"].as_str().unwrap(), test_content);

        // 清理临时文件
        let _ = tokio::fs::remove_file(&abs_path).await;
        let _ = tokio::fs::remove_dir(&temp_dir).await;
    }
}

// ============================================================
// file_info - 获取文件元数据
// ============================================================

struct FileInfoTool;

#[async_trait]
impl Tool for FileInfoTool {
    fn tool_name(&self) -> &str { "file_info" }
    fn description(&self) -> &str { "获取文件元数据（大小、修改时间、类型等）。使用场景：在读取文件前了解文件信息、检查文件类型、确认文件是否存在且可访问。不需要读取文件内容时优先使用此工具。" }
    fn category(&self) -> &str { "filesystem" }
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
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);

        // 路径安全校验
        if !workspace_root.is_empty() {
            let canonical_file = match crate::utils::canonicalize(path) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("文件不存在或路径无效: {}", file_path)),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            let canonical_root = match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some("工作区根目录路径无效".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            if !canonical_file.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("文件路径不在工作区内，拒绝访问".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        if !path.exists() {
            log::error!("文件不存在: {}", file_path);
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("文件不存在: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                };
            }
        };

        let is_dir = metadata.is_dir();
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        let modified = metadata.modified()
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
            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
        }
    }
}

// ============================================================
// file_exists - 检查文件或目录是否存在
// ============================================================

struct FileExistsTool;

#[async_trait]
impl Tool for FileExistsTool {
    fn tool_name(&self) -> &str { "file_exists" }
    fn description(&self) -> &str { "检查文件或目录是否存在。使用场景：在读取或修改文件前验证路径、避免对不存在的文件执行操作。比list_directory更轻量。" }
    fn category(&self) -> &str { "filesystem" }
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
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);

        // 路径安全校验
        if !workspace_root.is_empty() {
            if let Ok(canonical_file) = crate::utils::canonicalize(path) {
                if let Ok(canonical_root) = crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                    if !canonical_file.starts_with(&canonical_root) {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some("路径不在工作区内，拒绝访问".to_string()),
                            duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                        };
                    }
                }
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
            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
        }
    }
}

// ============================================================
// delete_file - 删除文件
// ============================================================

struct DeleteFileTool;

#[async_trait]
impl Tool for DeleteFileTool {
    fn tool_name(&self) -> &str { "delete_file" }
    fn description(&self) -> &str { "删除指定文件，删除前可选创建备份。注意：此操作不可逆，会自动触发用户确认。建议在删除前先创建版本快照。" }
    fn category(&self) -> &str { "filesystem" }
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
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        if workspace_root.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少工作区根目录路径，无法进行安全校验".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);

        // 规范化路径并校验
        let canonical_file = match crate::utils::canonicalize(std::path::Path::new(&resolved_path)) {
            Ok(p) => p,
            Err(_) => {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("文件不存在或路径无效: {}", file_path)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                };
            }
        };

        let canonical_root = match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
            Ok(p) => p,
            Err(_) => {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("工作区根目录不存在或路径无效: {}", workspace_root)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                };
            }
        };

        if !canonical_file.starts_with(&canonical_root) {
            return ToolResult {
                success: false,
                output: None,
                error: Some("文件路径不在工作区内，拒绝删除操作".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
            };
        }

        if !canonical_file.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是文件: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("删除文件失败: {}, 错误: {}", safe_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("删除文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
    fn tool_name(&self) -> &str { "create_directory" }
    fn description(&self) -> &str { "创建目录（支持递归创建）。使用场景：在写入文件前确保目标目录存在、组织文件结构。默认递归创建父目录。" }
    fn category(&self) -> &str { "filesystem" }
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
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
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
                            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                        };
                    }
                }
            } else {
                // 路径不存在，检查父目录
                match path.parent() {
                    Some(parent) if parent.exists() => {
                        match crate::utils::canonicalize(parent) {
                            Ok(p) => p,
                            Err(_) => {
                                return ToolResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!("父目录路径无效: {}", dir_path)),
                                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                                };
                            }
                        }
                    }
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
                                        duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
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
                                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                                };
                            }
                        }
                    }
                }
            };

            let canonical_root = match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some("工作区根目录路径无效".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            if !check_path.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("目录路径不在工作区内，拒绝创建".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        // 检查目录是否已存在
        if path.exists() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("目录已存在: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("创建目录失败: {}, 错误: {}", dir_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("创建目录失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
        }
    }
}

// ============================================================
// write_text_file - 写入纯文本文件
// ============================================================

struct WriteTextFileTool;

#[async_trait]
impl Tool for WriteTextFileTool {
    fn tool_name(&self) -> &str { "write_text_file" }
    fn description(&self) -> &str { "写入纯文本文件内容（.txt/.md/.csv/.json等），不依赖Sidecar。使用场景：创建纯文本文件、修改Markdown文件、保存JSON配置。支持追加模式。注意：仅适用于纯文本，生成结构化文档请使用docx_handler/xlsx_handler/pptx_handler/pdf_handler的generate操作。" }
    fn category(&self) -> &str { "filesystem" }
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
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
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
                            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                        };
                    }
                }
            } else {
                // 文件不存在，校验父目录
                match path.parent() {
                    Some(parent) if parent.exists() => {
                        match crate::utils::canonicalize(parent) {
                            Ok(p) => p,
                            Err(_) => {
                                return ToolResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!("父目录路径无效: {}", file_path)),
                                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                                };
                            }
                        }
                    }
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
                                        duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                                    };
                                }
                                path.to_path_buf()
                            }
                            Err(_) => {
                                return ToolResult {
                                    success: false,
                                    output: None,
                                    error: Some("工作区根目录路径无效".to_string()),
                                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                                };
                            }
                        }
                    }
                }
            };

            let canonical_root = match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some("工作区根目录路径无效".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            if !check_path.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("文件路径不在工作区内，拒绝写入".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
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
                            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                        };
                    }
                }
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("创建父目录失败: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
                Ok(mut file) => tokio::io::AsyncWriteExt::write_all(&mut file, &encoded_bytes).await,
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
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("写入文件失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("写入文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
        }
    }
}

// ============================================================
// 阶段三 3.5 新增工具：rename_file / copy_file / delete_directory
// / get_file_hash / read_file_lines
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

    let canonical_path = crate::utils::canonicalize(std::path::Path::new(resolved_path))
        .map_err(|_| format!("路径不存在或无效: {}", resolved_path))?;

    let canonical_root = crate::utils::canonicalize(std::path::Path::new(workspace_root))
        .map_err(|_| format!("工作区根目录不存在或无效: {}", workspace_root))?;

    // 路径组件级别的 starts_with 比较（避免字符串前缀匹配的绕过风险）
    if !canonical_path.starts_with(&canonical_root) {
        return Err(format!(
            "路径不在工作区内，拒绝访问: {} (工作区: {})",
            canonical_path.display(),
            canonical_root.display()
        ));
    }

    Ok((canonical_path, canonical_root))
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
    fn tool_name(&self) -> &str { "rename_file" }
    fn description(&self) -> &str { "重命名或移动文件。使用场景：整理文件结构、修改文件名。注意：跨文件系统移动可能失败，此操作不可逆。" }
    fn category(&self) -> &str { "filesystem" }
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
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }
        if target_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少目标文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_source = resolve_path(source_path, workspace_root);
        let resolved_target = resolve_path(target_path, workspace_root);

        // 校验源路径在工作区内
        let (canonical_source, _) = match validate_existing_path_in_workspace(&resolved_source, workspace_root) {
            Ok(paths) => paths,
            Err(e) => {
                log::warn!("rename_file 源路径校验失败: {}", e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
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
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
            };
        }

        // 源路径必须是文件
        if !canonical_source.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("源路径不是文件: {}", source_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("重命名文件失败: {} -> {}, 错误: {}", source_path, target_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("重命名文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
    fn tool_name(&self) -> &str { "copy_file" }
    fn description(&self) -> &str { "复制文件到新路径。使用场景：创建文件副本、备份文件、复制模板。支持二进制文件复制。" }
    fn category(&self) -> &str { "filesystem" }
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
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }
        if target_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少目标文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_source = resolve_path(source_path, workspace_root);
        let resolved_target = resolve_path(target_path, workspace_root);

        // 校验源路径在工作区内
        let (canonical_source, _) = match validate_existing_path_in_workspace(&resolved_source, workspace_root) {
            Ok(paths) => paths,
            Err(e) => {
                log::warn!("copy_file 源路径校验失败: {}", e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
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
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
            };
        }

        // 源路径必须是文件
        if !canonical_source.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("源路径不是文件: {}", source_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            }
        }

        // 执行复制
        match tokio::fs::copy(&canonical_source, &resolved_target).await {
            Ok(bytes_copied) => {
                log::info!("文件已复制: {} -> {}, 字节数: {}", source_path, target_path, bytes_copied);
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "source_path": source_path,
                        "target_path": target_path,
                        "bytes_copied": bytes_copied,
                        "message": format!("文件已复制: {} -> {}", source_path, target_path),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("复制文件失败: {} -> {}, 错误: {}", source_path, target_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("复制文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
    fn tool_name(&self) -> &str { "delete_directory" }
    fn description(&self) -> &str { "递归删除目录及其所有内容。注意：此操作不可逆，会自动触发用户确认。建议在删除前确认目录内容。" }
    fn category(&self) -> &str { "filesystem" }
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
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(dir_path, workspace_root);

        // 校验路径在工作区内
        let (canonical_dir, _) = match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
            Ok(paths) => paths,
            Err(e) => {
                log::warn!("delete_directory 路径校验失败: {}", e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        };

        // 必须是目录
        if !canonical_dir.is_dir() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是目录: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("删除目录失败: {}, 错误: {}", safe_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("删除目录失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
    fn tool_name(&self) -> &str { "get_file_hash" }
    fn description(&self) -> &str { "计算文件的 SHA-256 哈希值。使用场景：文件去重、完整性校验、变更检测。返回十六进制哈希字符串。" }
    fn category(&self) -> &str { "filesystem" }
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
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);

        // 校验路径在工作区内
        let (canonical_file, _) = match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
            Ok(paths) => paths,
            Err(e) => {
                log::warn!("get_file_hash 路径校验失败: {}", e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        };

        // 必须是文件
        if !canonical_file.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是文件: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
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
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Ok(Err(e)) => {
                log::error!("计算文件哈希失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("计算文件哈希失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("计算文件哈希任务失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("计算文件哈希任务失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
        }
    }
}

// ============================================================
// read_file_lines - 按行读取文件
// ============================================================

struct ReadFileLinesTool;

#[async_trait]
impl Tool for ReadFileLinesTool {
    fn tool_name(&self) -> &str { "read_file_lines" }
    fn description(&self) -> &str { "按行读取纯文本文件，支持偏移和行数限制。使用场景：读取大文件的指定部分、分页读取、查看日志文件尾部。推荐用于大文件分页读取。" }
    fn category(&self) -> &str { "filesystem" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "offset": {
                    "type": "integer",
                    "description": "起始行偏移（0-based），默认 0",
                    "default": 0
                },
                "limit": {
                    "type": "integer",
                    "description": "读取行数限制，默认 100，最大 1000",
                    "default": 100
                },
                "encoding": {
                    "type": "string",
                    "description": "文件编码，默认 utf-8。支持 gbk/gb2312/big5/shift_jis/latin1",
                    "default": "utf-8"
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let offset = params["offset"].as_u64().unwrap_or(0) as usize;
        let limit = params["limit"].as_u64().unwrap_or(100) as usize;
        let encoding_label = params["encoding"].as_str().unwrap_or("utf-8");

        if file_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 限制最大读取行数，防止 LLM 请求过大导致内存压力
        let safe_limit = limit.min(1000);

        let resolved_path = resolve_path(file_path, workspace_root);

        // 校验路径在工作区内
        let (canonical_file, _) = match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
            Ok(paths) => paths,
            Err(e) => {
                log::warn!("read_file_lines 路径校验失败: {}", e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        };

        // 必须是文件
        if !canonical_file.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是文件: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        // 在 spawn_blocking 中读取文件（避免阻塞异步运行时）
        let path_for_task = canonical_file.clone();
        let encoding_label_owned = encoding_label.to_string();
        let read_result = tokio::task::spawn_blocking(move || {
            // 读取文件字节
            let bytes = std::fs::read(&path_for_task)?;

            // 根据编码参数解码
            let encoding = encoding_rs::Encoding::for_label(encoding_label_owned.as_bytes())
                .unwrap_or(encoding_rs::UTF_8);
            let (decoded, _actual_encoding, _had_errors) = encoding.decode(&bytes);
            let content = decoded.into_owned();

            // 按行分割（兼容 \n 和 \r\n）
            let lines: Vec<&str> = content.lines().collect();
            let total_lines = lines.len();

            // 应用 offset 和 limit
            let end = offset.saturating_add(safe_limit).min(total_lines);
            let selected: Vec<String> = if offset < total_lines {
                lines[offset..end].iter().map(|s| s.to_string()).collect()
            } else {
                Vec::new()
            };

            Ok::<(Vec<String>, usize), std::io::Error>((selected, total_lines))
        })
        .await;

        match read_result {
            Ok(Ok((lines, total_lines))) => {
                let returned_lines = lines.len();
                log::debug!(
                    "按行读取文件完成: {}, offset={}, limit={}, 返回 {} 行（总 {} 行）",
                    file_path, offset, safe_limit, returned_lines, total_lines
                );
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "path": file_path,
                        "offset": offset,
                        "limit": safe_limit,
                        "total_lines": total_lines,
                        "returned_lines": returned_lines,
                        "lines": lines,
                        "has_more": offset + returned_lines < total_lines,
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Ok(Err(e)) => {
                log::error!("按行读取文件失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("按行读取文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("按行读取文件任务失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("按行读取文件任务失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
        }
    }
}
