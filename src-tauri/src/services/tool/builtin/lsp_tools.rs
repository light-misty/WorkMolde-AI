//! LSP 工具:暴露 LSP 能力给 Agent(单一工具,通过 operation 参数路由)
//! 参照 OpenCode 架构,通过 operation 参数支持 8 种操作:
//! definition/references/hover/diagnostics/document_symbol/workspace_symbol/implementation/call_hierarchy
//! 含优雅降级机制:LSP 不可用时返回 fallback 标记的空结果

use crate::errors::{CommandError, FS_IO_ERROR, TOOL_INVALID_PARAMS};
use crate::models::tool::ToolResult;
use crate::services::lsp::cache::LspResultCache;
use crate::services::lsp::manager::LspServerManager;
use crate::services::lsp::router::LanguageRouter;
use crate::services::tool::trait_def::Tool;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// lsp 工具:单一入口,通过 operation 参数路由不同 LSP 操作(实验性)
pub struct LspTool {
    /// LSP 服务器管理器
    manager: Arc<LspServerManager>,
    /// 语言路由器
    router: Arc<LanguageRouter>,
    /// 结果缓存
    cache: Arc<LspResultCache>,
}

impl LspTool {
    pub fn new(
        manager: Arc<LspServerManager>,
        router: Arc<LanguageRouter>,
        cache: Arc<LspResultCache>,
    ) -> Self {
        Self {
            manager,
            router,
            cache,
        }
    }
}

#[async_trait]
impl Tool for LspTool {
    fn tool_name(&self) -> &str {
        "lsp"
    }

    fn description(&self) -> &str {
        "LSP code intelligence tool (experimental). Specify the operation type via the operation parameter:\n\
         - definition: Go to symbol definition\n\
         - references: Find symbol references\n\
         - hover: Get hover information\n\
         - diagnostics: Get file diagnostics\n\
         - document_symbol: Get document symbol list\n\
         - workspace_symbol: Search workspace symbols\n\
         - implementation: Go to implementation\n\
         - call_hierarchy: Get call hierarchy (direction=incoming|outgoing)"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["definition", "references", "hover", "diagnostics",
                             "document_symbol", "workspace_symbol", "implementation", "call_hierarchy"],
                    "description": "LSP operation type"
                },
                "file_path": {
                    "type": "string",
                    "description": "File path (absolute or relative to workspace)"
                },
                "line": {
                    "type": "integer",
                    "description": "Cursor line number (0-based)"
                },
                "character": {
                    "type": "integer",
                    "description": "Cursor character number (0-based)"
                },
                "direction": {
                    "type": "string",
                    "enum": ["incoming", "outgoing"],
                    "description": "call_hierarchy direction (incoming=who calls this, outgoing=who this calls)"
                },
                "query": {
                    "type": "string",
                    "description": "workspace_symbol search query"
                },
                "include_declaration": {
                    "type": "boolean",
                    "default": true,
                    "description": "Whether references include the declaration location"
                }
            },
            "required": ["operation"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();

        // 从 params 读取 workspace_root
        let workspace_root = params
            .get("workspace_root")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // 读取 operation 参数
        let operation = match params.get("operation").and_then(|v| v.as_str()) {
            Some(op) => op.to_string(),
            None => {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("Missing operation parameter".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(TOOL_INVALID_PARAMS),
                }
            }
        };

        // 路由到对应的处理方法
        let result: Result<Value, CommandError> = match operation.as_str() {
            "definition" => self.try_lsp_definition(&params, workspace_root).await,
            "references" => self.try_lsp_references(&params, workspace_root).await,
            "hover" => self.try_lsp_hover(&params, workspace_root).await,
            "diagnostics" => self.try_lsp_diagnostics(&params, workspace_root).await,
            "document_symbol" => self.try_lsp_document_symbol(&params, workspace_root).await,
            "workspace_symbol" => self.try_lsp_workspace_symbol(&params, workspace_root).await,
            "implementation" => self.try_lsp_implementation(&params, workspace_root).await,
            "call_hierarchy" => self.try_lsp_call_hierarchy(&params, workspace_root).await,
            _ => Err(CommandError::tool(
                TOOL_INVALID_PARAMS,
                format!("Unknown operation: {}", operation),
            )),
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        // 统一转换为 ToolResult
        match result {
            Ok(output) => ToolResult {
                success: true,
                output: Some(output),
                error: None,
                duration_ms,
                error_code: None,
            },
            Err(e) => ToolResult {
                success: false,
                output: None,
                error: Some(e.message),
                duration_ms,
                error_code: Some(e.code),
            },
        }
    }
}

impl LspTool {
    /// 解析文件路径为绝对路径(相对工作区根目录时拼接)
    /// 含路径遍历校验:相对路径检查 .. 组件是否会逃逸 workspace_root
    fn resolve_path(file_path_str: &str, workspace_root: &str) -> Result<PathBuf, CommandError> {
        let file_path = PathBuf::from(file_path_str);
        if file_path.is_absolute() {
            // 绝对路径直接使用(LSP 服务器需要绝对路径,可能访问工作区外依赖库源码)
            Ok(file_path)
        } else {
            // 相对路径:校验 .. 遍历攻击
            if !Self::is_relative_path_safe(&file_path) {
                return Err(CommandError::tool(
                    TOOL_INVALID_PARAMS,
                    format!(
                        "Path traversal validation failed: relative path contains '..' components that escape the workspace root: file_path={}",
                        file_path_str
                    ),
                ));
            }
            // 拼接 workspace_root
            Ok(PathBuf::from(workspace_root).join(&file_path))
        }
    }

    /// 词法校验相对路径是否安全(不包含逃逸基目录的 .. 组件)
    /// 通过模拟路径规范化,检查 .. 组件数量是否超过前置的正常组件数量
    fn is_relative_path_safe(rel_path: &std::path::Path) -> bool {
        use std::path::Component;
        let mut depth = 0i32;
        for component in rel_path.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    depth -= 1;
                    if depth < 0 {
                        return false;
                    }
                }
                Component::Normal(_) => {
                    depth += 1;
                }
                Component::RootDir | Component::Prefix(_) => {
                    // 相对路径不应包含根目录或前缀组件
                    return false;
                }
            }
        }
        true
    }

    /// definition 操作:跳转到符号定义
    async fn try_lsp_definition(
        &self,
        params: &Value,
        workspace_root: &str,
    ) -> Result<Value, CommandError> {
        let file_path_str = params
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                CommandError::tool(TOOL_INVALID_PARAMS, "Missing file_path parameter")
            })?;

        let line = params.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let character = params
            .get("character")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let abs_path = Self::resolve_path(file_path_str, workspace_root)?;

        // 检查缓存
        if let Some(cached) = self
            .cache
            .get_definition(&abs_path.to_string_lossy(), line, character)
            .await
        {
            return Ok(json!({
                "locations": cached,
                "total": cached.len(),
                "cached": true,
            }));
        }

        // 检测语言(无法识别时降级)
        let language = match self.router.detect_language(&abs_path) {
            Some(lang) => lang,
            None => {
                return Ok(json!({
                    "fallback": true,
                    "message": format!("Cannot detect file language: {}", file_path_str),
                    "locations": [],
                    "total": 0,
                }))
            }
        };

        // 获取或启动 LSP 服务器(失败时降级)
        let client = match self.manager.get_or_start(&language).await {
            Ok(c) => c,
            Err(e) => {
                log::warn!("LSP definition 服务器不可用: {}, 降级返回空结果", e.message);
                return Ok(json!({
                    "fallback": true,
                    "message": format!("LSP server unavailable: {}", e.message),
                    "locations": [],
                    "total": 0,
                    "language": language,
                }));
            }
        };

        // 读取文件内容并发送 didOpen
        let content = std::fs::read_to_string(&abs_path)
            .map_err(|e| CommandError::fs(FS_IO_ERROR, format!("Failed to read file: {}", e)))?;
        let language_id = self.router.get_language_id(&language);
        client.did_open(&abs_path, language_id, &content).await?;

        // 请求 definition
        let locations = client.goto_definition(&abs_path, line, character).await?;

        // 缓存结果
        self.cache
            .set_definition(
                &abs_path.to_string_lossy(),
                line,
                character,
                locations.clone(),
            )
            .await;

        Ok(json!({
            "locations": locations,
            "total": locations.len(),
            "language": language,
        }))
    }

    /// references 操作:查找符号的所有引用
    async fn try_lsp_references(
        &self,
        params: &Value,
        workspace_root: &str,
    ) -> Result<Value, CommandError> {
        let file_path_str = params
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                CommandError::tool(TOOL_INVALID_PARAMS, "Missing file_path parameter")
            })?;

        let line = params.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let character = params
            .get("character")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let include_declaration = params
            .get("include_declaration")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let abs_path = Self::resolve_path(file_path_str, workspace_root)?;

        let language = match self.router.detect_language(&abs_path) {
            Some(lang) => lang,
            None => {
                return Ok(json!({
                    "fallback": true,
                    "message": format!("Cannot detect file language: {}", file_path_str),
                    "references": [],
                    "total": 0,
                }))
            }
        };

        let client = match self.manager.get_or_start(&language).await {
            Ok(c) => c,
            Err(e) => {
                log::warn!("LSP references 服务器不可用: {}, 降级返回空结果", e.message);
                return Ok(json!({
                    "fallback": true,
                    "message": format!("LSP server unavailable: {}", e.message),
                    "references": [],
                    "total": 0,
                    "language": language,
                }));
            }
        };

        let content = std::fs::read_to_string(&abs_path)
            .map_err(|e| CommandError::fs(FS_IO_ERROR, format!("Failed to read file: {}", e)))?;
        let language_id = self.router.get_language_id(&language);
        client.did_open(&abs_path, language_id, &content).await?;

        let locations = client
            .find_references(&abs_path, line, character, include_declaration)
            .await?;

        Ok(json!({
            "references": locations,
            "total": locations.len(),
            "language": language,
        }))
    }

    /// hover 操作:获取符号的悬停信息(类型、文档)
    async fn try_lsp_hover(
        &self,
        params: &Value,
        workspace_root: &str,
    ) -> Result<Value, CommandError> {
        let file_path_str = params
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                CommandError::tool(TOOL_INVALID_PARAMS, "Missing file_path parameter")
            })?;

        let line = params.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let character = params
            .get("character")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let abs_path = Self::resolve_path(file_path_str, workspace_root)?;

        // 检查缓存
        if let Some(cached) = self
            .cache
            .get_hover(&abs_path.to_string_lossy(), line, character)
            .await
        {
            return Ok(json!({
                "hover": cached,
                "cached": true,
            }));
        }

        let language = match self.router.detect_language(&abs_path) {
            Some(lang) => lang,
            None => {
                return Ok(json!({
                    "fallback": true,
                    "message": format!("Cannot detect file language: {}", file_path_str),
                    "hover": null,
                }))
            }
        };

        let client = match self.manager.get_or_start(&language).await {
            Ok(c) => c,
            Err(e) => {
                log::warn!("LSP hover 服务器不可用: {}, 降级返回空结果", e.message);
                return Ok(json!({
                    "fallback": true,
                    "message": format!("LSP server unavailable: {}", e.message),
                    "hover": null,
                    "language": language,
                }));
            }
        };

        let content = std::fs::read_to_string(&abs_path)
            .map_err(|e| CommandError::fs(FS_IO_ERROR, format!("Failed to read file: {}", e)))?;
        let language_id = self.router.get_language_id(&language);
        client.did_open(&abs_path, language_id, &content).await?;

        let hover = client.hover(&abs_path, line, character).await?;

        // 缓存结果
        self.cache
            .set_hover(&abs_path.to_string_lossy(), line, character, hover.clone())
            .await;

        Ok(json!({
            "hover": hover,
            "language": language,
        }))
    }

    /// diagnostics 操作:获取文件诊断信息(错误、警告)
    async fn try_lsp_diagnostics(
        &self,
        params: &Value,
        workspace_root: &str,
    ) -> Result<Value, CommandError> {
        let file_path_str = params
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                CommandError::tool(TOOL_INVALID_PARAMS, "Missing file_path parameter")
            })?;

        let abs_path = Self::resolve_path(file_path_str, workspace_root)?;

        let language = match self.router.detect_language(&abs_path) {
            Some(lang) => lang,
            None => {
                return Ok(json!({
                    "fallback": true,
                    "message": format!("Cannot detect file language: {}", file_path_str),
                    "diagnostics": [],
                    "total": 0,
                }))
            }
        };

        let client = match self.manager.get_or_start(&language).await {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "LSP diagnostics 服务器不可用: {}, 降级返回空结果",
                    e.message
                );
                return Ok(json!({
                    "fallback": true,
                    "message": format!("LSP server unavailable: {}", e.message),
                    "diagnostics": [],
                    "total": 0,
                    "language": language,
                }));
            }
        };

        let content = std::fs::read_to_string(&abs_path)
            .map_err(|e| CommandError::fs(FS_IO_ERROR, format!("Failed to read file: {}", e)))?;
        let language_id = self.router.get_language_id(&language);
        client.did_open(&abs_path, language_id, &content).await?;

        let diagnostics = client.diagnostics(&abs_path).await?;

        let error_count = diagnostics.iter().filter(|d| d.severity == 1).count();
        let warning_count = diagnostics.iter().filter(|d| d.severity == 2).count();
        let info_count = diagnostics.iter().filter(|d| d.severity == 3).count();

        Ok(json!({
            "diagnostics": diagnostics,
            "total": diagnostics.len(),
            "errors": error_count,
            "warnings": warning_count,
            "informations": info_count,
            "language": language,
        }))
    }

    /// document_symbol 操作:获取文档符号列表
    async fn try_lsp_document_symbol(
        &self,
        params: &Value,
        workspace_root: &str,
    ) -> Result<Value, CommandError> {
        let file_path_str = params
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                CommandError::tool(TOOL_INVALID_PARAMS, "Missing file_path parameter")
            })?;

        let abs_path = Self::resolve_path(file_path_str, workspace_root)?;

        let language = match self.router.detect_language(&abs_path) {
            Some(lang) => lang,
            None => {
                return Ok(json!({
                    "fallback": true,
                    "message": format!("Cannot detect file language: {}", file_path_str),
                    "symbols": [],
                    "total": 0,
                }))
            }
        };

        let client = match self.manager.get_or_start(&language).await {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "LSP document_symbol 服务器不可用: {}, 降级返回空结果",
                    e.message
                );
                return Ok(json!({
                    "fallback": true,
                    "message": format!("LSP server unavailable: {}", e.message),
                    "symbols": [],
                    "total": 0,
                    "language": language,
                }));
            }
        };

        let content = std::fs::read_to_string(&abs_path)
            .map_err(|e| CommandError::fs(FS_IO_ERROR, format!("Failed to read file: {}", e)))?;
        let language_id = self.router.get_language_id(&language);
        client.did_open(&abs_path, language_id, &content).await?;

        let symbols = client.document_symbol(&abs_path).await?;

        Ok(json!({
            "symbols": symbols,
            "total": symbols.len(),
            "language": language,
        }))
    }

    /// workspace_symbol 操作:搜索工作区符号
    async fn try_lsp_workspace_symbol(
        &self,
        params: &Value,
        workspace_root: &str,
    ) -> Result<Value, CommandError> {
        let query = params
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::tool(TOOL_INVALID_PARAMS, "Missing query parameter"))?;

        // workspace_symbol 不绑定特定文件,遍历所有已启动的 LSP 服务器
        let symbols = self.manager.workspace_symbol(query).await?;

        Ok(json!({
            "symbols": symbols,
            "total": symbols.len(),
            "query": query,
            "workspaceRoot": workspace_root,
        }))
    }

    /// implementation 操作:跳转到实现
    async fn try_lsp_implementation(
        &self,
        params: &Value,
        workspace_root: &str,
    ) -> Result<Value, CommandError> {
        let file_path_str = params
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                CommandError::tool(TOOL_INVALID_PARAMS, "Missing file_path parameter")
            })?;

        let line = params.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let character = params
            .get("character")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let abs_path = Self::resolve_path(file_path_str, workspace_root)?;

        let language = match self.router.detect_language(&abs_path) {
            Some(lang) => lang,
            None => {
                return Ok(json!({
                    "fallback": true,
                    "message": format!("Cannot detect file language: {}", file_path_str),
                    "locations": [],
                    "total": 0,
                }))
            }
        };

        let client = match self.manager.get_or_start(&language).await {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "LSP implementation 服务器不可用: {}, 降级返回空结果",
                    e.message
                );
                return Ok(json!({
                    "fallback": true,
                    "message": format!("LSP server unavailable: {}", e.message),
                    "locations": [],
                    "total": 0,
                    "language": language,
                }));
            }
        };

        let content = std::fs::read_to_string(&abs_path)
            .map_err(|e| CommandError::fs(FS_IO_ERROR, format!("Failed to read file: {}", e)))?;
        let language_id = self.router.get_language_id(&language);
        client.did_open(&abs_path, language_id, &content).await?;

        let locations = client
            .goto_implementation(&abs_path, line, character)
            .await?;

        Ok(json!({
            "locations": locations,
            "total": locations.len(),
            "language": language,
        }))
    }

    /// call_hierarchy 操作:获取调用层级
    async fn try_lsp_call_hierarchy(
        &self,
        params: &Value,
        workspace_root: &str,
    ) -> Result<Value, CommandError> {
        let file_path_str = params
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                CommandError::tool(TOOL_INVALID_PARAMS, "Missing file_path parameter")
            })?;

        let line = params.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let character = params
            .get("character")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let direction = params
            .get("direction")
            .and_then(|v| v.as_str())
            .unwrap_or("incoming");

        let abs_path = Self::resolve_path(file_path_str, workspace_root)?;

        let language = match self.router.detect_language(&abs_path) {
            Some(lang) => lang,
            None => {
                return Ok(json!({
                    "fallback": true,
                    "message": format!("Cannot detect file language: {}", file_path_str),
                    "calls": [],
                    "total": 0,
                }))
            }
        };

        let client = match self.manager.get_or_start(&language).await {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "LSP call_hierarchy 服务器不可用: {}, 降级返回空结果",
                    e.message
                );
                return Ok(json!({
                    "fallback": true,
                    "message": format!("LSP server unavailable: {}", e.message),
                    "calls": [],
                    "total": 0,
                    "language": language,
                }));
            }
        };

        let content = std::fs::read_to_string(&abs_path)
            .map_err(|e| CommandError::fs(FS_IO_ERROR, format!("Failed to read file: {}", e)))?;
        let language_id = self.router.get_language_id(&language);
        client.did_open(&abs_path, language_id, &content).await?;

        // 准备调用层级
        let items = client
            .prepare_call_hierarchy(&abs_path, line, character)
            .await?;

        // 根据方向查询 incoming 或 outgoing
        let calls = match direction {
            "incoming" => client.incoming_calls(&items).await?,
            "outgoing" => client.outgoing_calls(&items).await?,
            _ => {
                return Err(CommandError::tool(
                    TOOL_INVALID_PARAMS,
                    format!("Unknown direction: {}", direction),
                ))
            }
        };

        Ok(json!({
            "calls": calls,
            "total": calls.len(),
            "direction": direction,
            "language": language,
        }))
    }
}
