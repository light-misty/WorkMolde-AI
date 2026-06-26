use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::models::handler::HandlerResult;
use crate::services::document::DocumentService;
use super::registry::Handler;

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

/// 验证路径是否在工作区内
/// 防止路径遍历攻击：LLM 可能构造绝对路径或 ..\..\ 越界路径读取/覆盖工作区外文件
/// 返回 Ok(()) 表示通过校验，Err(error_message) 表示路径越权
/// 对于不存在的路径（如 convert 的 output_path），规范化父目录后校验
fn validate_workspace_path(resolved_path: &str, workspace_root: &str) -> Result<(), String> {
    if workspace_root.is_empty() {
        return Err("workspace_root 为空，无法校验路径边界".to_string());
    }
    if resolved_path.is_empty() {
        return Err("待校验路径为空".to_string());
    }

    // 规范化工作区根目录（必须存在）
    let canonical_root = crate::utils::canonicalize(workspace_root)
        .map_err(|e| format!("工作区根目录无效: {} ({})", workspace_root, e))?;

    // 尝试规范化待校验路径
    // 路径可能不存在（如 convert 的 output_path），此时规范化父目录
    let canonical_path = match crate::utils::canonicalize(resolved_path) {
        Ok(p) => p,
        Err(_) => {
            // 路径不存在，规范化父目录后拼接文件名
            let path = std::path::Path::new(resolved_path);
            let parent = path.parent().unwrap_or(std::path::Path::new(""));
            if parent.as_os_str().is_empty() {
                // 没有父目录（如 "file.txt"），直接用工作区根目录
                canonical_root.join(path.file_name().unwrap_or_default())
            } else {
                let canonical_parent = crate::utils::canonicalize(parent)
                    .map_err(|e| format!("路径父目录无效: {} ({})", parent.display(), e))?;
                canonical_parent.join(path.file_name().unwrap_or_default())
            }
        }
    };

    // 路径组件级别的 starts_with 比较（避免字符串前缀匹配的绕过风险）
    if !canonical_path.starts_with(&canonical_root) {
        return Err(format!(
            "路径不在工作区内，拒绝访问: {} (工作区: {})",
            canonical_path.display(),
            canonical_root.display()
        ));
    }

    Ok(())
}

/// 执行 read 操作的通用逻辑
async fn execute_read(
    doc_service: &DocumentService,
    doc_type: &str,
    params: Value,
) -> HandlerResult {
    let start = Instant::now();
    let file_path = params["path"].as_str().unwrap_or("");
    let workspace_root = params["workspace_root"].as_str().unwrap_or("");
    let resolved_path = resolve_path(file_path, workspace_root);

    // 路径安全校验：防止 LLM 构造越界路径读取工作区外文件
    if let Err(e) = validate_workspace_path(&resolved_path, workspace_root) {
        log::warn!("Handler read 操作路径校验失败: {}", e);
        return HandlerResult {
            success: false,
            output: None,
            error: Some(e),
            duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::DOC_PERMISSION_DENIED),
        };
    }

    let mut sidecar_params = json!({
        "path": resolved_path,
    });

    // read 操作的通用参数
    if !params["include_formatting"].is_null() {
        sidecar_params["include_formatting"] = json!(params["include_formatting"].as_bool().unwrap_or(false));
    }

    // Word read 专用参数（include_formatting=true 时等价于 include_runs=true）
    if !params["include_runs"].is_null() {
        sidecar_params["include_runs"] = json!(params["include_runs"].as_bool().unwrap_or(false));
    }
    if !params["include_tables_detailed"].is_null() {
        sidecar_params["include_tables_detailed"] = json!(params["include_tables_detailed"].as_bool().unwrap_or(false));
    }
    if !params["include_sections"].is_null() {
        sidecar_params["include_sections"] = json!(params["include_sections"].as_bool().unwrap_or(false));
    }
    if !params["include_headers_footers"].is_null() {
        sidecar_params["include_headers_footers"] = json!(params["include_headers_footers"].as_bool().unwrap_or(false));
    }

    // Excel read 专用参数
    if let Some(sheet) = params["sheet"].as_str() {
        sidecar_params["sheet"] = json!(sheet);
    }
    if let Some(range) = params["range"].as_str() {
        sidecar_params["range"] = json!(range);
    }

    // PDF read 专用参数
    if let Some(pages) = params["pages"].as_str() {
        sidecar_params["pages"] = json!(pages);
    }
    if !params["include_layout"].is_null() {
        sidecar_params["include_layout"] = json!(params["include_layout"].as_bool().unwrap_or(false));
    }
    if !params["include_forms"].is_null() {
        sidecar_params["include_forms"] = json!(params["include_forms"].as_bool().unwrap_or(false));
    }
    if !params["include_annotations"].is_null() {
        sidecar_params["include_annotations"] = json!(params["include_annotations"].as_bool().unwrap_or(false));
    }
    if !params["extract_tables"].is_null() {
        sidecar_params["extract_tables"] = json!(params["extract_tables"].as_bool().unwrap_or(false));
    }
    if !params["include_images"].is_null() {
        sidecar_params["include_images"] = json!(params["include_images"].as_bool().unwrap_or(false));
    }

    // PPT read 专用参数
    if !params["include_notes"].is_null() {
        sidecar_params["include_notes"] = json!(params["include_notes"].as_bool().unwrap_or(false));
    }
    if !params["include_shapes_detailed"].is_null() {
        sidecar_params["include_shapes_detailed"] = json!(params["include_shapes_detailed"].as_bool().unwrap_or(false));
    }

    // Excel 扩展 read 专用参数（P1-2）
    if !params["include_formulas"].is_null() {
        sidecar_params["include_formulas"] = json!(params["include_formulas"].as_bool().unwrap_or(false));
    }
    if !params["include_charts"].is_null() {
        sidecar_params["include_charts"] = json!(params["include_charts"].as_bool().unwrap_or(false));
    }
    if !params["include_merged_cells"].is_null() {
        sidecar_params["include_merged_cells"] = json!(params["include_merged_cells"].as_bool().unwrap_or(false));
    }
    if !params["include_comments"].is_null() {
        sidecar_params["include_comments"] = json!(params["include_comments"].as_bool().unwrap_or(false));
    }

    match doc_service.process("read", doc_type, sidecar_params).await {
        Ok(data) => HandlerResult {
            success: true,
            output: Some(data),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
        },
        Err(e) => HandlerResult {
            success: false,
            output: None,
            error: Some(e.message),
            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
        },
    }
}

/// 执行 convert 操作的通用逻辑
async fn execute_convert(
    doc_service: &DocumentService,
    doc_type: &str,
    params: Value,
) -> HandlerResult {
    let start = Instant::now();
    let file_path = params["path"].as_str().unwrap_or("");
    let target_format = params["target_format"].as_str().unwrap_or("pdf");
    let output_path = params["output_path"].as_str().unwrap_or("");
    let workspace_root = params["workspace_root"].as_str().unwrap_or("");

    let resolved_source = resolve_path(file_path, workspace_root);

    // 路径安全校验：源文件必须在工作区内
    if let Err(e) = validate_workspace_path(&resolved_source, workspace_root) {
        log::warn!("Handler convert 操作源路径校验失败: {}", e);
        return HandlerResult {
            success: false,
            output: None,
            error: Some(e),
            duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::DOC_PERMISSION_DENIED),
        };
    }

    let output_path = if output_path.is_empty() {
        // 自动生成输出路径：与源文件同目录（源文件已通过 resolve_path 解析为工作区内的绝对路径）
        let source_path = std::path::Path::new(&resolved_source);
        let stem = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let new_filename = format!("{}.{}", stem, target_format);
        source_path
            .parent()
            .map(|p| p.join(&new_filename).to_string_lossy().to_string())
            .unwrap_or(new_filename)
    } else {
        resolve_path(output_path, workspace_root)
    };

    // 路径安全校验：输出路径必须在工作区内（防止 LLM 覆盖工作区外文件）
    if let Err(e) = validate_workspace_path(&output_path, workspace_root) {
        log::warn!("Handler convert 操作输出路径校验失败: {}", e);
        return HandlerResult {
            success: false,
            output: None,
            error: Some(e),
            duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::DOC_PERMISSION_DENIED),
        };
    }

    let mut sidecar_params = json!({
        "path": resolved_source,
        "output_path": output_path,
        "format": target_format,
    });

    // Excel convert 专用参数
    if let Some(sheet) = params["sheet"].as_str() {
        sidecar_params["sheet"] = json!(sheet);
    }

    match doc_service.process("convert", doc_type, sidecar_params).await {
        Ok(data) => HandlerResult {
            success: true,
            output: Some(data),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
        },
        Err(e) => HandlerResult {
            success: false,
            output: None,
            error: Some(e.message),
            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
        },
    }
}

/// 执行 analyze 操作的通用逻辑
async fn execute_analyze(
    doc_service: &DocumentService,
    doc_type: &str,
    params: Value,
) -> HandlerResult {
    let start = Instant::now();
    let file_path = params["path"].as_str().unwrap_or("");
    let workspace_root = params["workspace_root"].as_str().unwrap_or("");
    let resolved_path = resolve_path(file_path, workspace_root);

    // 路径安全校验：防止 LLM 构造越界路径读取工作区外文件
    if let Err(e) = validate_workspace_path(&resolved_path, workspace_root) {
        log::warn!("Handler analyze 操作路径校验失败: {}", e);
        return HandlerResult {
            success: false,
            output: None,
            error: Some(e),
            duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::DOC_PERMISSION_DENIED),
        };
    }

    let sidecar_params = json!({
        "path": resolved_path,
    });

    match doc_service.process("analyze", doc_type, sidecar_params).await {
        Ok(data) => HandlerResult {
            success: true,
            output: Some(data),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
        },
        Err(e) => HandlerResult {
            success: false,
            output: None,
            error: Some(e.message),
            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
        },
    }
}

/// 注册所有内置处理器
pub fn register_builtin_handlers(
    registry: &mut super::registry::HandlerRegistry,
    doc_service: Arc<DocumentService>,
) {
    log::info!("开始注册内置处理器");
    registry.register(Box::new(DocxHandler::new(doc_service.clone())));
    registry.register(Box::new(XlsxHandler::new(doc_service.clone())));
    registry.register(Box::new(PptxHandler::new(doc_service.clone())));
    registry.register(Box::new(PdfHandler::new(doc_service.clone())));
    registry.register(Box::new(CodeInterpreterHandler::new(doc_service)));
    log::info!("内置处理器注册完成, 共注册 5 个处理器");
}

// ============================================================================
// DocxHandler - Word 文档处理器
// ============================================================================

/// Word 文档处理器
/// 聚合 read/convert/analyze 三种操作
struct DocxHandler {
    doc_service: Arc<DocumentService>,
}

impl DocxHandler {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Handler for DocxHandler {
    fn handler_name(&self) -> &str { "docx_handler" }
    fn description(&self) -> &str {
        "Word文档(.docx)处理器，支持读取、格式转换、分析三种操作。转换支持 md/txt/pdf 格式（与 sidecar word_handler.convert 实际支持一致）。"
    }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["read", "convert", "analyze"],
                    "description": "操作类型: read=读取文档, convert=格式转换, analyze=分析文档"
                },
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "include_formatting": {
                    "type": "boolean",
                    "description": "[read] 是否包含格式信息（等价于 include_runs=true），默认 false",
                    "default": false
                },
                "include_runs": {
                    "type": "boolean",
                    "description": "[read] 是否提取 Run 级字符属性（字体名/字号/粗体/斜体/下划线/颜色），默认 false",
                    "default": false
                },
                "include_tables_detailed": {
                    "type": "boolean",
                    "description": "[read] 是否提取表格详细结构（合并单元格/列宽/行高/表格样式），默认 false",
                    "default": false
                },
                "include_sections": {
                    "type": "boolean",
                    "description": "[read] 是否提取节信息（页面尺寸/方向/边距），默认 false",
                    "default": false
                },
                "include_headers_footers": {
                    "type": "boolean",
                    "description": "[read] 是否提取页眉页脚内容，默认 false",
                    "default": false
                },
                "target_format": {
                    "type": "string",
                    "enum": ["md", "txt", "pdf"],
                    "description": "[convert] 目标格式（与 sidecar word_handler.convert 实际支持格式一致）"
                },
                "output_path": {
                    "type": "string",
                    "description": "[convert] 输出文件路径（可选，默认自动生成）"
                }
            },
            "required": ["action", "path"]
        })
    }
    async fn execute(&self, params: Value) -> HandlerResult {
        let action = params["action"].as_str().unwrap_or("");
        match action {
            "read" => execute_read(&self.doc_service, "docx", params).await,
            "convert" => execute_convert(&self.doc_service, "docx", params).await,
            "analyze" => execute_analyze(&self.doc_service, "docx", params).await,
            _ => HandlerResult {
                success: false,
                output: None,
                error: Some(format!("DocxHandler 不支持的操作类型: {}", action)),
                duration_ms: 0, error_code: None,
            },
        }
    }
}

// ============================================================================
// XlsxHandler - Excel 文档处理器
// ============================================================================

/// Excel 文档处理器
/// 聚合 read/convert/analyze 三种操作
struct XlsxHandler {
    doc_service: Arc<DocumentService>,
}

impl XlsxHandler {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Handler for XlsxHandler {
    fn handler_name(&self) -> &str { "xlsx_handler" }
    fn description(&self) -> &str {
        "Excel文档(.xlsx)处理器，支持读取、格式转换、分析三种操作。转换支持 csv/pdf/html/txt 格式（与 sidecar excel_handler.convert 实际支持一致）。"
    }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
    fn supported_types(&self) -> Vec<String> {
        vec!["xlsx".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["read", "convert", "analyze"],
                    "description": "操作类型: read=读取文档, convert=格式转换, analyze=分析文档"
                },
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "sheet": {
                    "type": "string",
                    "description": "[read/convert] 工作表名称"
                },
                "range": {
                    "type": "string",
                    "description": "[read] 单元格范围，如 A1:D10"
                },
                "include_formatting": {
                    "type": "boolean",
                    "description": "[read] 是否提取单元格格式（字体/填充/边框/对齐/数字格式），默认 false",
                    "default": false
                },
                "include_formulas": {
                    "type": "boolean",
                    "description": "[read] 是否分离公式与计算结果值（同时加载 data_only=True 工作簿），默认 false",
                    "default": false
                },
                "include_charts": {
                    "type": "boolean",
                    "description": "[read] 是否提取图表信息（类型/标题/数据范围），默认 false",
                    "default": false
                },
                "include_merged_cells": {
                    "type": "boolean",
                    "description": "[read] 是否提取合并单元格范围列表，默认 false",
                    "default": false
                },
                "include_comments": {
                    "type": "boolean",
                    "description": "[read] 是否提取单元格批注，默认 false",
                    "default": false
                },
                "target_format": {
                    "type": "string",
                    "enum": ["csv", "pdf", "html", "txt"],
                    "description": "[convert] 目标格式（与 sidecar excel_handler.convert 实际支持格式一致）"
                },
                "output_path": {
                    "type": "string",
                    "description": "[convert] 输出文件路径（可选，默认自动生成）"
                }
            },
            "required": ["action", "path"]
        })
    }
    async fn execute(&self, params: Value) -> HandlerResult {
        let action = params["action"].as_str().unwrap_or("");
        match action {
            "read" => execute_read(&self.doc_service, "xlsx", params).await,
            "convert" => execute_convert(&self.doc_service, "xlsx", params).await,
            "analyze" => execute_analyze(&self.doc_service, "xlsx", params).await,
            _ => HandlerResult {
                success: false,
                output: None,
                error: Some(format!("XlsxHandler 不支持的操作类型: {}", action)),
                duration_ms: 0, error_code: None,
            },
        }
    }
}

// ============================================================================
// PptxHandler - PPT 文档处理器
// ============================================================================

/// PPT 文档处理器
/// 聚合 read/convert/analyze 三种操作
struct PptxHandler {
    doc_service: Arc<DocumentService>,
}

impl PptxHandler {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Handler for PptxHandler {
    fn handler_name(&self) -> &str { "pptx_handler" }
    fn description(&self) -> &str {
        "PPT演示文稿(.pptx)处理器，支持读取、格式转换、分析三种操作。转换支持 pdf 格式（需 LibreOffice headless 模式）。"
    }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
    fn supported_types(&self) -> Vec<String> {
        vec!["pptx".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["read", "convert", "analyze"],
                    "description": "操作类型: read=读取文档, convert=格式转换, analyze=分析文档"
                },
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "include_notes": {
                    "type": "boolean",
                    "description": "[read] 是否提取幻灯片备注内容，默认 false",
                    "default": false
                },
                "include_shapes_detailed": {
                    "type": "boolean",
                    "description": "[read] 是否提取形状详细信息（位置/尺寸/填充/边框/版式/表格/图表识别），默认 false",
                    "default": false
                },
                "target_format": {
                    "type": "string",
                    "enum": ["pdf"],
                    "description": "[convert] 目标格式（仅支持 pdf，需 LibreOffice headless 模式）"
                },
                "output_path": {
                    "type": "string",
                    "description": "[convert] 输出文件路径（可选，默认自动生成）"
                }
            },
            "required": ["action", "path"]
        })
    }
    async fn execute(&self, params: Value) -> HandlerResult {
        let action = params["action"].as_str().unwrap_or("");
        match action {
            "read" => execute_read(&self.doc_service, "pptx", params).await,
            "convert" => execute_convert(&self.doc_service, "pptx", params).await,
            "analyze" => execute_analyze(&self.doc_service, "pptx", params).await,
            _ => HandlerResult {
                success: false,
                output: None,
                error: Some(format!("PptxHandler 不支持的操作类型: {}", action)),
                duration_ms: 0, error_code: None,
            },
        }
    }
}

// ============================================================================
// PdfHandler - PDF 文档处理器
// ============================================================================

/// PDF 文档处理器
/// 聚合 read/convert/analyze 三种操作
struct PdfHandler {
    doc_service: Arc<DocumentService>,
}

impl PdfHandler {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Handler for PdfHandler {
    fn handler_name(&self) -> &str { "pdf_handler" }
    fn description(&self) -> &str {
        "PDF文档(.pdf)处理器，支持读取、格式转换、分析三种操作。转换支持 txt/md/html 格式（与 sidecar pdf_handler.convert 实际支持一致）。"
    }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
    fn supported_types(&self) -> Vec<String> {
        vec!["pdf".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["read", "convert", "analyze"],
                    "description": "操作类型: read=读取文档, convert=格式转换, analyze=分析文档"
                },
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "pages": {
                    "type": "string",
                    "description": "[read] 页码范围，如 \"1-5,8,10-12\"，默认读取所有页"
                },
                "include_layout": {
                    "type": "boolean",
                    "description": "[read] 是否提取文本位置和样式（字号/字体/颜色），使用 PyMuPDF get_text(\"dict\")，默认 false",
                    "default": false
                },
                "include_forms": {
                    "type": "boolean",
                    "description": "[read] 是否提取表单字段（AcroForm），使用 pypdf，默认 false",
                    "default": false
                },
                "include_annotations": {
                    "type": "boolean",
                    "description": "[read] 是否提取注释（高亮/批注/签名等），使用 pypdf，默认 false",
                    "default": false
                },
                "extract_tables": {
                    "type": "boolean",
                    "description": "[read] 是否提取表格结构，使用 pdfplumber extract_tables()，默认 false",
                    "default": false
                },
                "include_images": {
                    "type": "boolean",
                    "description": "[read] 是否提取图片信息（数量/位置/尺寸），使用 PyMuPDF，默认 false",
                    "default": false
                },
                "target_format": {
                    "type": "string",
                    "enum": ["txt", "md", "html"],
                    "description": "[convert] 目标格式"
                },
                "output_path": {
                    "type": "string",
                    "description": "[convert] 输出文件路径（可选，默认自动生成）"
                }
            },
            "required": ["action", "path"]
        })
    }
    async fn execute(&self, params: Value) -> HandlerResult {
        let action = params["action"].as_str().unwrap_or("");
        match action {
            "read" => execute_read(&self.doc_service, "pdf", params).await,
            "convert" => execute_convert(&self.doc_service, "pdf", params).await,
            "analyze" => execute_analyze(&self.doc_service, "pdf", params).await,
            _ => HandlerResult {
                success: false,
                output: None,
                error: Some(format!("PdfHandler 不支持的操作类型: {}", action)),
                duration_ms: 0, error_code: None,
            },
        }
    }
}

// ============================================================================
// CodeInterpreterHandler - 代码解释器处理器
// ============================================================================

/// 代码解释器处理器
/// 让 Agent 自由编写 Python 代码生成/修改文档
/// 承担原有 generate 和 modify 操作的全部职责
struct CodeInterpreterHandler {
    doc_service: Arc<DocumentService>,
}

impl CodeInterpreterHandler {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Handler for CodeInterpreterHandler {
    fn handler_name(&self) -> &str { "code_interpreter_handler" }
    fn description(&self) -> &str {
        "代码解释器，通过编写和执行 Python 代码生成和修改文档。所有文档生成和修改操作都通过此处理器完成。可用库: python-docx, openpyxl, python-pptx, reportlab, matplotlib, pandas, numpy, Pillow。可用 helper: create_word_doc(), save_word_doc() 等。"
    }
    fn category(&self) -> &str { "document" }
    fn is_builtin(&self) -> bool { true }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into(), "xlsx".into(), "pptx".into(), "pdf".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "要执行的 Python 代码。可用库: python-docx, openpyxl, python-pptx, reportlab, matplotlib, pandas, numpy, Pillow。可用 helper: create_word_doc(), save_word_doc(), create_excel_doc(), save_excel_doc(), create_ppt_doc(), save_ppt_doc(), create_pdf_doc(), save_pdf_doc(), create_chart(), save_chart()。工作目录变量: working_dir"
                },
                "patches": {
                    "type": "array",
                    "description": "搜索替换块列表。提供此字段时，将基于上一次执行的代码应用这些替换得到完整代码，用于在原代码基础上做局部修正而非重写。每个 search 必须在原代码中唯一匹配。与 code 字段二选一，patches 优先。",
                    "items": {
                        "type": "object",
                        "properties": {
                            "search": {
                                "type": "string",
                                "description": "原代码中需要被替换的片段（必须唯一匹配，建议包含足够上下文）"
                            },
                            "replace": {
                                "type": "string",
                                "description": "替换后的片段"
                            }
                        },
                        "required": ["search", "replace"]
                    }
                },
                "description": {
                    "type": "string",
                    "description": "代码功能的简要描述，用于用户确认时展示"
                },
                "timeout": {
                    "type": "integer",
                    "description": "执行超时时间（秒），默认 60，最大 120",
                    "default": 60
                },
                "expected_files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "预期生成的文件名列表（如 [\"报告.docx\", \"chart.png\"]）"
                }
            },
            "required": ["description"]
        })
    }
    async fn execute(&self, params: Value) -> HandlerResult {
        let start = Instant::now();
        let description = params["description"].as_str().unwrap_or("");
        let timeout = params["timeout"].as_u64().unwrap_or(60).min(120);
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        // 判断模式：patches 优先于 code
        // patch 模式：基于 base_code（由 executor 注入上一次代码）应用搜索替换块
        // 完整代码模式：直接使用 code 字段（向后兼容）
        let final_code = if let Some(patches) = params.get("patches").and_then(|p| p.as_array()) {
            if patches.is_empty() {
                return HandlerResult {
                    success: false,
                    output: None,
                    error: Some("patches 数组不能为空".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                };
            }
            // patch 模式：从 base_code 获取基准（由 executor 注入）
            let base_code = params["base_code"].as_str().unwrap_or("");
            if base_code.is_empty() {
                return HandlerResult {
                    success: false,
                    output: None,
                    error: Some("使用 patch 模式但没有可用的 base_code（上一次代码不存在或为空）".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                };
            }
            // 应用所有搜索替换块
            match apply_patches(base_code, patches) {
                Ok(merged) => merged,
                Err(e) => return HandlerResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                },
            }
        } else {
            // 完整代码模式（向后兼容）
            let code = params["code"].as_str().unwrap_or("").to_string();
            if code.is_empty() {
                return HandlerResult {
                    success: false,
                    output: None,
                    error: Some("缺少代码内容".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                };
            }
            code
        };

        // 调用 Sidecar：action="execute", type="code"
        // Sidecar handle_request() 通过 getattr(handler, action) 路由
        // CodeHandler 实现了 execute() 方法
        let sidecar_params = json!({
            "code": final_code,
            "working_dir": workspace_root,
            "timeout": timeout,
        });

        match self.doc_service.process("execute", "code", sidecar_params).await {
            Ok(data) => {
                let mut output = data;
                output["description"] = json!(description);
                // 在 output 中附带最终执行的代码，供 executor 保存为 last_code
                output["_executed_code"] = json!(final_code);
                HandlerResult {
                    success: true,
                    output: Some(output),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => HandlerResult {
                success: false,
                // 关键：失败时也返回完整代码，供 executor 保存为 last_code 和构造错误反馈
                output: Some(json!({ "_executed_code": final_code })),
                error: Some(e.message),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            },
        }
    }
}

/// 应用搜索替换块到基准代码上
/// 返回 Ok(merged_code) 或 Err(error_message)
/// 每个 patch 的 search 必须在 base_code 中唯一匹配，否则返回错误
fn apply_patches(base_code: &str, patches: &[serde_json::Value]) -> Result<String, String> {
    let mut result = base_code.to_string();
    for (i, patch) in patches.iter().enumerate() {
        let search = patch["search"].as_str().unwrap_or("");
        let replace = patch["replace"].as_str().unwrap_or("");
        if search.is_empty() {
            return Err(format!("第 {} 个 patch 的 search 字段不能为空", i + 1));
        }
        // 统计匹配次数，要求唯一匹配
        let match_count = result.matches(search).count();
        match match_count {
            0 => return Err(format!(
                "第 {} 个 patch 的 search 片段在原代码中未找到匹配。请确认 search 片段与原代码完全一致（包括空格、缩进、换行）。\n未匹配的 search 片段:\n{}",
                i + 1, search
            )),
            1 => {
                result = result.replacen(search, replace, 1);
            }
            _ => return Err(format!(
                "第 {} 个 patch 的 search 片段在原代码中匹配了 {} 次，要求唯一匹配。请在 search 中包含更多上下文以精确定位。",
                i + 1, match_count
            )),
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：构造 search/replace patch 的 JSON Value
    fn make_patch(search: &str, replace: &str) -> Value {
        json!({ "search": search, "replace": replace })
    }

    /// 测试单个 patch 成功应用：修正拼写错误
    #[test]
    fn test_apply_patches_single_patch_success() {
        let base = "doc.add_paragrah('标题')\nprint('done')";
        let patches = vec![make_patch("add_paragrah", "add_paragraph")];
        let result = apply_patches(base, &patches);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "doc.add_paragraph('标题')\nprint('done')");
    }

    /// 测试多个 patch 顺序应用：修正两处错误
    #[test]
    fn test_apply_patches_multiple_patches_success() {
        let base = "doc.add_paragrah('标题')\ndoc.savee('file.docx')";
        let patches = vec![
            make_patch("add_paragrah", "add_paragraph"),
            make_patch("savee", "save"),
        ];
        let result = apply_patches(base, &patches);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "doc.add_paragraph('标题')\ndoc.save('file.docx')");
    }

    /// 测试 search 未匹配：返回明确错误，包含未匹配的 search 片段
    #[test]
    fn test_apply_patches_search_not_found() {
        let base = "print('hello')";
        let patches = vec![make_patch("nonexistent_code", "replacement")];
        let result = apply_patches(base, &patches);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("第 1 个 patch"));
        assert!(err.contains("未找到匹配"));
        assert!(err.contains("nonexistent_code"));
    }

    /// 测试 search 多次匹配：返回明确错误，提示要求唯一匹配
    #[test]
    fn test_apply_patches_search_multiple_matches() {
        let base = "x = 1\nx = 2\nx = 3";
        // "x = " 在原代码中匹配 3 次
        let patches = vec![make_patch("x = ", "y = ")];
        let result = apply_patches(base, &patches);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("第 1 个 patch"));
        assert!(err.contains("匹配了 3 次"));
        assert!(err.contains("唯一匹配"));
    }

    /// 测试空 patches 数组：apply_patches 本身允许空数组（返回原代码）
    /// 注：Handler execute() 层会拦截空 patches，apply_patches 层只负责算法
    #[test]
    fn test_apply_patches_empty_patches_returns_base() {
        let base = "original code";
        let patches: Vec<Value> = vec![];
        let result = apply_patches(base, &patches);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "original code");
    }

    /// 测试空 search 字段：返回明确错误
    #[test]
    fn test_apply_patches_empty_search_field() {
        let base = "some code";
        let patches = vec![make_patch("", "replacement")];
        let result = apply_patches(base, &patches);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("第 1 个 patch"));
        assert!(err.contains("search 字段不能为空"));
    }

    /// 测试 search 字段缺失（非字符串）：等同于空字符串处理
    #[test]
    fn test_apply_patches_missing_search_field() {
        let base = "some code";
        // search 字段缺失，unwrap_or("") 返回空字符串
        let patches = vec![json!({ "replace": "replacement" })];
        let result = apply_patches(base, &patches);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("search 字段不能为空"));
    }

    /// 测试空白字符敏感：search 必须与原代码完全一致（tab 与 space 不互通）
    #[test]
    fn test_apply_patches_whitespace_sensitive() {
        // base 使用 4 个空格缩进，search 使用 tab 缩进 -> 应匹配失败
        let base = "    doc.add_paragraph('标题')\n";
        let patches = vec![make_patch("\tdoc.add_paragraph", "doc.add_heading")];
        let result = apply_patches(base, &patches);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("未找到匹配"));

        // 缩进完全一致（4 个空格），应匹配成功
        let patches2 = vec![make_patch("    doc.add_paragraph", "    doc.add_heading")];
        let result2 = apply_patches(base, &patches2);
        assert!(result2.is_ok());
        assert_eq!(result2.unwrap(), "    doc.add_heading('标题')\n");
    }

    /// 测试多行 search/replace：支持跨行片段替换
    #[test]
    fn test_apply_patches_multiline_search_replace() {
        let base = "for i in range(10):\n    print(i)\n";
        let search = "for i in range(10):\n    print(i)";
        let replace = "for j in range(5):\n    print(j * 2)";
        let patches = vec![make_patch(search, replace)];
        let result = apply_patches(base, &patches);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "for j in range(5):\n    print(j * 2)\n");
    }

    /// 测试 replace 为空字符串：等同于删除 search 片段
    #[test]
    fn test_apply_patches_empty_replace_deletes_search() {
        let base = "line1\nunnecessary_line\nline3";
        let patches = vec![make_patch("unnecessary_line\n", "")];
        let result = apply_patches(base, &patches);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "line1\nline3");
    }

    /// 测试多个 patch 时第一个失败：应立即返回错误，不应用后续 patch
    #[test]
    fn test_apply_patches_first_patch_fails_stops_early() {
        let base = "original";
        let patches = vec![
            make_patch("nonexistent", "replacement1"),
            make_patch("original", "replacement2"),  // 这个本应匹配，但因第一个失败不会执行
        ];
        let result = apply_patches(base, &patches);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("第 1 个 patch"));
        // 原代码未被修改
    }

    /// 测试多个 patch 时第二个失败：第一个已应用，第二个返回错误
    #[test]
    fn test_apply_patches_second_patch_fails() {
        let base = "foo\nbar";
        let patches = vec![
            make_patch("foo", "FOO"),  // 第一个成功
            make_patch("nonexistent", "BAZ"),  // 第二个失败
        ];
        let result = apply_patches(base, &patches);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("第 2 个 patch"));
        assert!(err.contains("未找到匹配"));
    }

    /// 测试 patch 顺序应用：第一个 patch 的 replace 结果会影响第二个 patch 的 search 匹配
    #[test]
    fn test_apply_patches_sequential_application() {
        let base = "a = 1";
        let patches = vec![
            make_patch("a = 1", "b = 2"),
            make_patch("b = 2", "c = 3"),  // 在第一个 patch 应用后的结果中匹配
        ];
        let result = apply_patches(base, &patches);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "c = 3");
    }

    /// 测试中文字符在 search/replace 中的正确处理
    #[test]
    fn test_apply_patches_chinese_characters() {
        let base = "标题 = '测试内容'\nprint(标题)";
        let patches = vec![make_patch("测试内容", "修正后的内容")];
        let result = apply_patches(base, &patches);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "标题 = '修正后的内容'\nprint(标题)");
    }

    /// 测试空 base_code：任何非空 search 都无法匹配
    #[test]
    fn test_apply_patches_empty_base_code() {
        let base = "";
        let patches = vec![make_patch("something", "replacement")];
        let result = apply_patches(base, &patches);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("未找到匹配"));
    }

    /// 测试空 base_code 加空 patches：返回空字符串
    #[test]
    fn test_apply_patches_empty_base_empty_patches() {
        let base = "";
        let patches: Vec<Value> = vec![];
        let result = apply_patches(base, &patches);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }
}
