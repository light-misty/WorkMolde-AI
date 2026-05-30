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
            let canonical_dir = match dir.canonicalize() {
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
            let canonical_root = match std::path::Path::new(workspace_root).canonicalize() {
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

        if is_dir && current_depth < max_depth - 1 {
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
            let canonical_dir = match dir_path.canonicalize() {
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
            let canonical_root = match std::path::Path::new(workspace_root).canonicalize() {
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
                            let start = pos.saturating_sub(30);
                            let end = (pos + query.len() + 30).min(content.len());
                            content_preview = Some(format!("...{}...", &content[start..end]));
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
    fn description(&self) -> &str { "读取纯文本文件内容（.txt/.md/.csv/.json/.xml等），不依赖Sidecar，速度更快。注意：仅适用于纯文本文件，读取Word/Excel/PPT/PDF等结构化文档请使用docx_skill/xlsx_skill/pptx_skill/pdf_skill的read操作。文件大小限制1MB。" }
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
            let canonical_file = match path.canonicalize() {
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
            let canonical_root = match std::path::Path::new(workspace_root).canonicalize() {
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

        match tokio::fs::read_to_string(&resolved_path).await {
            Ok(content) => {
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
            let canonical_file = match path.canonicalize() {
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
            let canonical_root = match std::path::Path::new(workspace_root).canonicalize() {
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
            if let Ok(canonical_file) = path.canonicalize() {
                if let Ok(canonical_root) = std::path::Path::new(workspace_root).canonicalize() {
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
        let canonical_file = match std::path::Path::new(&resolved_path).canonicalize() {
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

        let canonical_root = match std::path::Path::new(workspace_root).canonicalize() {
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
                match path.canonicalize() {
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
                        match parent.canonicalize() {
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
                        match std::path::Path::new(workspace_root).canonicalize() {
                            Ok(root) => {
                                // 检查解析后的路径是否以工作区根目录开头
                                let resolved_abs = if path.is_absolute() {
                                    path.to_path_buf()
                                } else {
                                    std::path::Path::new(workspace_root).join(dir_path)
                                };
                                // 简单前缀检查（因为路径可能不存在，无法 canonicalize）
                                let resolved_str = resolved_abs.to_string_lossy();
                                let root_str = root.to_string_lossy();
                                if !resolved_str.starts_with(root_str.as_ref()) {
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

            let canonical_root = match std::path::Path::new(workspace_root).canonicalize() {
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
    fn description(&self) -> &str { "写入纯文本文件内容（.txt/.md/.csv/.json等），不依赖Sidecar。使用场景：创建纯文本文件、修改Markdown文件、保存JSON配置。支持追加模式。注意：仅适用于纯文本，生成结构化文档请使用docx_skill/xlsx_skill/pptx_skill/pdf_skill的generate操作。" }
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
                match path.canonicalize() {
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
                        match parent.canonicalize() {
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
                        match std::path::Path::new(workspace_root).canonicalize() {
                            Ok(root) => {
                                let resolved_abs = if path.is_absolute() {
                                    path.to_path_buf()
                                } else {
                                    std::path::Path::new(workspace_root).join(file_path)
                                };
                                let resolved_str = resolved_abs.to_string_lossy();
                                let root_str = root.to_string_lossy();
                                if !resolved_str.starts_with(root_str.as_ref()) {
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

            let canonical_root = match std::path::Path::new(workspace_root).canonicalize() {
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
        if let Some(parent) = path.parent() {
            if !parent.exists() {
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

        let write_result = if append {
            // 追加模式：使用 OpenOptions
            match tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&resolved_path)
                .await
            {
                Ok(mut file) => tokio::io::AsyncWriteExt::write_all(&mut file, content.as_bytes()).await,
                Err(e) => Err(e),
            }
        } else {
            tokio::fs::write(&resolved_path, content).await
        };

        match write_result {
            Ok(_) => {
                log::info!("文件已写入: {}", file_path);
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "path": file_path,
                        "message": format!("文件已{}: {}", if append { "追加" } else { "写入" }, file_path),
                        "size": content.len(),
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
