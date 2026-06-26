use std::time::Instant;

use async_trait::async_trait;
use serde_json::{json, Value};

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
    log::info!("内置工具注册完成, 共注册 8 个工具");
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
                duration_ms: start.elapsed().as_millis() as u64,
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
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        if !dir.is_dir() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是目录: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64,
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
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
            };
            if !canonical_dir.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("目录不在工作区内，拒绝访问".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        }

        let resolved_dir_owned = resolved_dir.clone();
        let extensions_clone = extensions.clone();

        let results = tokio::task::spawn_blocking(move || {
            let dir = std::path::Path::new(&resolved_dir_owned);
            tool_list_dir(dir, dir, max_depth, 0, &extensions_clone)
        }).await.unwrap_or_default();

        ToolResult {
            success: true,
            output: Some(json!({
                "path": dir_path,
                "items": results,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
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
                duration_ms: start.elapsed().as_millis() as u64,
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
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
            };
            if !canonical_dir.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("搜索目录不在工作区内，拒绝访问".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        }

        let query_lower = query.to_lowercase();
        let resolved_directory_owned = resolved_directory.clone();
        let extensions_clone = extensions.clone();

        let results = tokio::task::spawn_blocking(move || {
            let dir_path = std::path::Path::new(&resolved_directory_owned);
            let mut results = Vec::new();
            tool_search_files(dir_path, dir_path, &query_lower, &extensions_clone, include_content, max_results, &mut results);
            results
        }).await.unwrap_or_default();

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
                duration_ms: start.elapsed().as_millis() as u64,
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
                        duration_ms: start.elapsed().as_millis() as u64,
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
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
            };
            if !canonical_file.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("文件路径不在工作区内，拒绝访问".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        }

        if !path.exists() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("文件不存在: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        if !path.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是文件: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64,
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
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        if metadata.len() as usize > max_size {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("文件过大 ({}字节)，超过最大读取限制 ({}字节)", metadata.len(), max_size)),
                duration_ms: start.elapsed().as_millis() as u64,
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
                    duration_ms: start.elapsed().as_millis() as u64,
                }
            }
            Err(e) => {
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("读取文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
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

        // 验证 8 个工具都已注册
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 8);

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
    }

    #[test]
    fn test_tool_definitions_count() {
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);

        let defs = registry.tool_definitions();
        assert_eq!(defs.len(), 8);

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
                duration_ms: start.elapsed().as_millis() as u64,
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
                        duration_ms: start.elapsed().as_millis() as u64,
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
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
            };
            if !canonical_file.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("文件路径不在工作区内，拒绝访问".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        }

        if !path.exists() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("文件不存在: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        let metadata = match tokio::fs::metadata(&resolved_path).await {
            Ok(m) => m,
            Err(e) => {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("获取文件信息失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
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
            duration_ms: start.elapsed().as_millis() as u64,
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
                duration_ms: start.elapsed().as_millis() as u64,
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
                            duration_ms: start.elapsed().as_millis() as u64,
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
            duration_ms: start.elapsed().as_millis() as u64,
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
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        if workspace_root.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少工作区根目录路径，无法进行安全校验".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
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
                    duration_ms: start.elapsed().as_millis() as u64,
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
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        if !canonical_file.starts_with(&canonical_root) {
            return ToolResult {
                success: false,
                output: None,
                error: Some("文件路径不在工作区内，拒绝删除操作".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        if !canonical_file.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是文件: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64,
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
                    log::warn!("创建备份失败: {}, 继续删除操作", e);
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
                }
            }
            Err(e) => {
                log::error!("删除文件失败: {}, 错误: {}", safe_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("删除文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
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
                duration_ms: start.elapsed().as_millis() as u64,
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
                                    duration_ms: start.elapsed().as_millis() as u64,
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
                                        duration_ms: start.elapsed().as_millis() as u64,
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
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
            };
            if !check_path.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("目录路径不在工作区内，拒绝创建".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
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
                }
            }
            Err(e) => {
                log::error!("创建目录失败: {}, 错误: {}", dir_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("创建目录失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
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
                duration_ms: start.elapsed().as_millis() as u64,
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
                                    duration_ms: start.elapsed().as_millis() as u64,
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
                                        duration_ms: start.elapsed().as_millis() as u64,
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
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
            };
            if !check_path.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("文件路径不在工作区内，拒绝写入".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
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
                        };
                    }
                }
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("创建父目录失败: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
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
            // 追加模式：使用 OpenOptions
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
            tokio::fs::write(&resolved_path, &encoded_bytes).await
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
                }
            }
            Err(e) => {
                log::error!("写入文件失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("写入文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                }
            }
        }
    }
}
