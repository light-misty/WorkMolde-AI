use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::models::skill::SkillResult;
use crate::services::document::DocumentService;
use super::registry::Skill;

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

/// 解析 operations 数组中的路径字段
/// 遍历每个操作，对其中的路径相关字段（files/image/outputPath 等）进行相对路径到绝对路径的转换
fn resolve_operation_paths(operations: &Value, workspace_root: &str) -> Value {
    let ops = match operations.as_array() {
        Some(arr) => arr,
        None => return operations.clone(),
    };

    let resolved: Vec<Value> = ops.iter().map(|op| {
        let mut resolved_op = op.clone();
        let op_type = op["type"].as_str().unwrap_or("");

        match op_type {
            "merge" => {
                // 合并操作: 解析 files 数组中的路径和 outputPath
                if let Some(files) = op["files"].as_array() {
                    let resolved_files: Vec<Value> = files.iter().map(|f| {
                        let f_str = f.as_str().unwrap_or("");
                        json!(resolve_path(f_str, workspace_root))
                    }).collect();
                    resolved_op["files"] = json!(resolved_files);
                }
                if let Some(output) = op["outputPath"].as_str() {
                    resolved_op["outputPath"] = json!(resolve_path(output, workspace_root));
                }
            }
            "split" => {
                // 拆分操作: 解析 outputDir
                if let Some(dir) = op["outputDir"].as_str() {
                    resolved_op["outputDir"] = json!(resolve_path(dir, workspace_root));
                }
            }
            "rotate" => {
                // 旋转操作: 解析 outputPath
                if let Some(output) = op["outputPath"].as_str() {
                    resolved_op["outputPath"] = json!(resolve_path(output, workspace_root));
                }
            }
            "addWatermark" => {
                // 水印操作: 解析 image 和 outputPath
                if let Some(image) = op["image"].as_str() {
                    resolved_op["image"] = json!(resolve_path(image, workspace_root));
                }
                if let Some(output) = op["outputPath"].as_str() {
                    resolved_op["outputPath"] = json!(resolve_path(output, workspace_root));
                }
            }
            "encrypt" => {
                // 加密操作: 解析 outputPath
                if let Some(output) = op["outputPath"].as_str() {
                    resolved_op["outputPath"] = json!(resolve_path(output, workspace_root));
                }
            }
            _ => {}
        }

        resolved_op
    }).collect();

    json!(resolved)
}

/// 从 params 中提取 content 字段，支持字符串和结构化 JSON
fn extract_content(params: &Value) -> String {
    match params["content"].as_str() {
        Some(s) => s.to_string(),
        None => {
            if !params["content"].is_null() {
                serde_json::to_string(&params["content"]).unwrap_or_default()
            } else {
                String::new()
            }
        }
    }
}

/// 执行 generate 操作的通用逻辑
/// 构造 sidecar_params 并调用 doc_service.process("generate", doc_type, ...)
async fn execute_generate(
    doc_service: &DocumentService,
    doc_type: &str,
    params: Value,
) -> SkillResult {
    let start = Instant::now();
    let output_path = params["path"].as_str().unwrap_or("");
    let title = params["title"].as_str().unwrap_or("");
    let workspace_root = params["workspace_root"].as_str().unwrap_or("");

    let resolved_path = resolve_path(output_path, workspace_root);
    let content = extract_content(&params);

    let mut sidecar_params = json!({
        "path": resolved_path,
        "title": title,
        "content": content,
    });

    // 传递模板参数
    if let Some(template) = params["template"].as_str() {
        if !template.is_empty() {
            sidecar_params["template"] = json!(template);
        }
    }

    // Word 专用参数
    if let Some(page_size) = params["pageSize"].as_str() {
        sidecar_params["pageSize"] = json!(page_size);
    }
    if let Some(header) = params["header"].as_str() {
        sidecar_params["header"] = json!(header);
    }
    if let Some(footer) = params["footer"].as_str() {
        sidecar_params["footer"] = json!(footer);
    }
    if !params["pageNumber"].is_null() {
        sidecar_params["pageNumber"] = json!(params["pageNumber"].as_bool().unwrap_or(true));
    }
    if !params["includeToc"].is_null() {
        sidecar_params["includeToc"] = json!(params["includeToc"].as_bool().unwrap_or(false));
    }
    if !params["colorCoding"].is_null() {
        sidecar_params["colorCoding"] = json!(params["colorCoding"].as_bool().unwrap_or(true));
    }
    if !params["bookmarks"].is_null() {
        sidecar_params["bookmarks"] = params["bookmarks"].clone();
    }
    if !params["hyperlinks"].is_null() {
        sidecar_params["hyperlinks"] = params["hyperlinks"].clone();
    }

    // Excel 专用参数
    if !params["sheets"].is_null() {
        sidecar_params["sheets"] = params["sheets"].clone();
    }
    if !params["useFormulas"].is_null() {
        sidecar_params["useFormulas"] = json!(params["useFormulas"].as_bool().unwrap_or(true));
    }
    if !params["numberFormats"].is_null() {
        sidecar_params["numberFormats"] = params["numberFormats"].clone();
    }
    if !params["conditionalFormats"].is_null() {
        sidecar_params["conditionalFormats"] = params["conditionalFormats"].clone();
    }

    // PPT 专用参数
    if !params["slides"].is_null() {
        sidecar_params["slides"] = params["slides"].clone();
    }
    if let Some(color_scheme) = params["colorScheme"].as_str() {
        sidecar_params["colorScheme"] = json!(color_scheme);
    }
    if !params["fonts"].is_null() {
        sidecar_params["fonts"] = params["fonts"].clone();
    }
    if !params["margins"].is_null() {
        sidecar_params["margins"] = params["margins"].clone();
    }

    // PDF 专用参数
    if !params["subscripts"].is_null() {
        sidecar_params["subscripts"] = params["subscripts"].clone();
    }
    if !params["superscripts"].is_null() {
        sidecar_params["superscripts"] = params["superscripts"].clone();
    }

    match doc_service.process("generate", doc_type, sidecar_params).await {
        Ok(data) => {
            // 生成成功后执行文档验证（可选，默认关闭）
            let enable_validation = params["validate"].as_bool().unwrap_or(false);
            let mut output = data;
            if enable_validation {
                if let Some(path) = output.get("path").and_then(|p| p.as_str()) {
                    let validate_params = json!({
                        "path": path,
                    });
                    match doc_service.process("validate", doc_type, validate_params).await {
                        Ok(validation_data) => {
                            output["validation"] = validation_data;
                        }
                        Err(_) => {
                            // 验证失败不影响主流程
                        }
                    }
                }
            }
            SkillResult {
                success: true,
                output: Some(output),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
            }
        }
        Err(e) => SkillResult {
            success: false,
            output: None,
            error: Some(e.message),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    }
}

/// 执行 read 操作的通用逻辑
async fn execute_read(
    doc_service: &DocumentService,
    doc_type: &str,
    params: Value,
) -> SkillResult {
    let start = Instant::now();
    let file_path = params["path"].as_str().unwrap_or("");
    let workspace_root = params["workspace_root"].as_str().unwrap_or("");
    let resolved_path = resolve_path(file_path, workspace_root);

    let mut sidecar_params = json!({
        "path": resolved_path,
    });

    // read 操作的通用参数
    if !params["include_formatting"].is_null() {
        sidecar_params["include_formatting"] = json!(params["include_formatting"].as_bool().unwrap_or(false));
    }

    // Excel read 专用参数
    if let Some(sheet) = params["sheet"].as_str() {
        sidecar_params["sheet"] = json!(sheet);
    }
    if let Some(range) = params["range"].as_str() {
        sidecar_params["range"] = json!(range);
    }

    match doc_service.process("read", doc_type, sidecar_params).await {
        Ok(data) => SkillResult {
            success: true,
            output: Some(data),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => SkillResult {
            success: false,
            output: None,
            error: Some(e.message),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    }
}

/// 执行 modify 操作的通用逻辑
async fn execute_modify(
    doc_service: &DocumentService,
    doc_type: &str,
    params: Value,
) -> SkillResult {
    let start = Instant::now();
    let file_path = params["path"].as_str().unwrap_or("");
    let workspace_root = params["workspace_root"].as_str().unwrap_or("");
    let resolved_path = resolve_path(file_path, workspace_root);

    // 对 operations 数组中的路径字段进行解析（将相对路径转为绝对路径）
    let resolved_operations = resolve_operation_paths(&params["operations"], workspace_root);

    let sidecar_params = json!({
        "path": resolved_path,
        "operations": resolved_operations,
    });

    match doc_service.process("modify", doc_type, sidecar_params).await {
        Ok(data) => SkillResult {
            success: true,
            output: Some(data),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => SkillResult {
            success: false,
            output: None,
            error: Some(e.message),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    }
}

/// 执行 convert 操作的通用逻辑
async fn execute_convert(
    doc_service: &DocumentService,
    doc_type: &str,
    params: Value,
) -> SkillResult {
    let start = Instant::now();
    let file_path = params["path"].as_str().unwrap_or("");
    let target_format = params["target_format"].as_str().unwrap_or("pdf");
    let output_path = params["output_path"].as_str().unwrap_or("");
    let workspace_root = params["workspace_root"].as_str().unwrap_or("");

    let resolved_source = resolve_path(file_path, workspace_root);

    let output_path = if output_path.is_empty() {
        let stem = std::path::Path::new(&resolved_source)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        format!("{}.{}", stem, target_format)
    } else {
        resolve_path(output_path, workspace_root)
    };

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
        Ok(data) => SkillResult {
            success: true,
            output: Some(data),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => SkillResult {
            success: false,
            output: None,
            error: Some(e.message),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    }
}

/// 执行 analyze 操作的通用逻辑
async fn execute_analyze(
    doc_service: &DocumentService,
    doc_type: &str,
    params: Value,
) -> SkillResult {
    let start = Instant::now();
    let file_path = params["path"].as_str().unwrap_or("");
    let workspace_root = params["workspace_root"].as_str().unwrap_or("");
    let resolved_path = resolve_path(file_path, workspace_root);

    let sidecar_params = json!({
        "path": resolved_path,
    });

    match doc_service.process("analyze", doc_type, sidecar_params).await {
        Ok(data) => SkillResult {
            success: true,
            output: Some(data),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => SkillResult {
            success: false,
            output: None,
            error: Some(e.message),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    }
}

/// 注册所有内置技能
pub fn register_builtin_skills(
    registry: &mut super::registry::SkillRegistry,
    doc_service: Arc<DocumentService>,
) {
    log::info!("开始注册内置技能");
    registry.register_builtin(Box::new(DocxSkill::new(doc_service.clone())));
    registry.register_builtin(Box::new(XlsxSkill::new(doc_service.clone())));
    registry.register_builtin(Box::new(PptxSkill::new(doc_service.clone())));
    registry.register_builtin(Box::new(PdfSkill::new(doc_service)));
    log::info!("内置技能注册完成, 共注册 4 个技能");
}

// ============================================================================
// DocxSkill - Word 文档技能
// ============================================================================

/// Word 文档技能
/// 聚合 generate/read/modify/convert/analyze 五种操作
struct DocxSkill {
    doc_service: Arc<DocumentService>,
}

impl DocxSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for DocxSkill {
    fn skill_name(&self) -> &str { "docx_skill" }
    fn description(&self) -> &str {
        "Word文档(.docx)处理技能，支持生成、读取、修改、格式转换、分析五种操作。生成支持页面设置、目录、页眉页脚、书签、超链接；修改支持替换/添加段落/添加标题/添加表格/添加页眉页脚/添加书签/添加超链接/设置页面尺寸/添加目录等操作；转换支持docx/pdf/md/txt/html等格式。"
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
                    "enum": ["generate", "read", "modify", "convert", "analyze"],
                    "description": "操作类型: generate=生成文档, read=读取文档, modify=修改文档, convert=格式转换, analyze=分析文档"
                },
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）。generate 时为输出路径，其他操作为输入路径"
                },
                "content": {
                    "type": "string",
                    "description": "[generate] 文档内容（纯文本或结构化 JSON）"
                },
                "title": {
                    "type": "string",
                    "description": "[generate] 文档标题"
                },
                "pageSize": {
                    "type": "string",
                    "enum": ["letter", "a4"],
                    "description": "[generate] 页面尺寸（letter=US Letter, a4=A4，默认 a4）"
                },
                "includeToc": {
                    "type": "boolean",
                    "description": "[generate] 是否包含目录，默认 false",
                    "default": false
                },
                "header": {
                    "type": "string",
                    "description": "[generate] 页眉文本"
                },
                "footer": {
                    "type": "string",
                    "description": "[generate] 页脚文本"
                },
                "pageNumber": {
                    "type": "boolean",
                    "description": "[generate] 是否显示页码，默认 true",
                    "default": true
                },
                "colorCoding": {
                    "type": "boolean",
                    "description": "[generate] 是否启用颜色编码（蓝色=输入值、黑色=公式、绿色=跨表引用、红色=外部链接），默认 true",
                    "default": true
                },
                "bookmarks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string", "description": "书签 ID" },
                            "text": { "type": "string", "description": "书签文本" }
                        }
                    },
                    "description": "[generate] 书签列表"
                },
                "hyperlinks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "text": { "type": "string", "description": "链接显示文本" },
                            "url": { "type": "string", "description": "外部链接 URL" },
                            "anchor": { "type": "string", "description": "内部书签锚点" }
                        }
                    },
                    "description": "[generate] 超链接列表"
                },
                "template": {
                    "type": "string",
                    "description": "[generate] 模板文件路径（可选）"
                },
                "validate": {
                    "type": "boolean",
                    "description": "[generate] 生成后是否执行文档质量验证，默认 false",
                    "default": false
                },
                "include_formatting": {
                    "type": "boolean",
                    "description": "[read] 是否包含格式信息，默认 false",
                    "default": false
                },
                "operations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": {
                                "type": "string",
                                "enum": [
                                    "replace", "add_paragraph", "add_heading", "add_table",
                                    "addHeader", "addFooter", "addBookmark", "addHyperlink",
                                    "setPageSize", "addToc"
                                ],
                                "description": "操作类型"
                            },
                            "index": {
                                "type": "integer",
                                "description": "段落索引（从0开始），用于 replace 操作按索引替换整段内容"
                            },
                            "text": {
                                "type": "string",
                                "description": "文本内容，用于多种操作"
                            },
                            "old": {
                                "type": "string",
                                "description": "要替换的旧文本，用于 replace 操作的全文搜索替换模式"
                            },
                            "new": {
                                "type": "string",
                                "description": "替换后的新文本，用于 replace 操作的全文搜索替换模式"
                            },
                            "level": {
                                "type": "integer",
                                "description": "标题级别（1-6），用于 add_heading 操作"
                            },
                            "pageNumber": {
                                "type": "boolean",
                                "description": "是否显示页码，用于 addFooter 操作"
                            },
                            "id": {
                                "type": "string",
                                "description": "书签 ID，用于 addBookmark 操作"
                            },
                            "url": {
                                "type": "string",
                                "description": "外部链接 URL，用于 addHyperlink 操作"
                            },
                            "anchor": {
                                "type": "string",
                                "description": "内部书签锚点，用于 addHyperlink 操作"
                            },
                            "size": {
                                "type": "string",
                                "description": "页面尺寸 (letter/a4)，用于 setPageSize 操作"
                            }
                        },
                        "required": ["type"]
                    },
                    "description": "[modify] 修改操作列表"
                },
                "target_format": {
                    "type": "string",
                    "enum": ["docx", "xlsx", "pptx", "pdf", "md", "txt", "csv", "html"],
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
    async fn execute(&self, params: Value) -> SkillResult {
        let action = params["action"].as_str().unwrap_or("");
        match action {
            "generate" => execute_generate(&self.doc_service, "docx", params).await,
            "read" => execute_read(&self.doc_service, "docx", params).await,
            "modify" => execute_modify(&self.doc_service, "docx", params).await,
            "convert" => execute_convert(&self.doc_service, "docx", params).await,
            "analyze" => execute_analyze(&self.doc_service, "docx", params).await,
            _ => SkillResult {
                success: false,
                output: None,
                error: Some(format!("DocxSkill 不支持的操作类型: {}", action)),
                duration_ms: 0,
            },
        }
    }
}

// ============================================================================
// XlsxSkill - Excel 文档技能
// ============================================================================

/// Excel 文档技能
/// 聚合 generate/read/modify/convert/analyze 五种操作
struct XlsxSkill {
    doc_service: Arc<DocumentService>,
}

impl XlsxSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for XlsxSkill {
    fn skill_name(&self) -> &str { "xlsx_skill" }
    fn description(&self) -> &str {
        "Excel文档(.xlsx)处理技能，支持生成、读取、修改、格式转换、分析五种操作。生成支持公式、数字格式、条件格式、颜色编码；修改支持设置单元格/添加工作表/删除工作表/设置范围/设置公式/设置格式/设置颜色编码/添加条件格式等操作；转换支持xlsx/pdf/csv/html等格式。"
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
                    "enum": ["generate", "read", "modify", "convert", "analyze"],
                    "description": "操作类型: generate=生成文档, read=读取文档, modify=修改文档, convert=格式转换, analyze=分析文档"
                },
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）。generate 时为输出路径，其他操作为输入路径"
                },
                "content": {
                    "type": "string",
                    "description": "[generate] 文档内容（纯文本或结构化 JSON）"
                },
                "title": {
                    "type": "string",
                    "description": "[generate] 文档标题"
                },
                "sheets": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "工作表名称" },
                            "data": { "type": "array", "description": "行数据（二维数组）" },
                            "headers": { "type": "array", "description": "表头行" },
                            "cells": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "row": { "type": "integer", "description": "行号" },
                                        "col": { "type": "integer", "description": "列号" },
                                        "value": { "description": "单元格值" },
                                        "formula": { "type": "string", "description": "Excel 公式" }
                                    }
                                },
                                "description": "单元格列表（支持公式）"
                            },
                            "formulas": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "row": { "type": "integer", "description": "行号" },
                                        "col": { "type": "integer", "description": "列号" },
                                        "formula": { "type": "string", "description": "Excel 公式" }
                                    }
                                },
                                "description": "公式列表"
                            }
                        }
                    },
                    "description": "[generate] 工作表列表（结构化数据，优先于 content）"
                },
                "useFormulas": {
                    "type": "boolean",
                    "description": "[generate] 是否使用公式而非硬编码值，默认 true",
                    "default": true
                },
                "numberFormats": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "range": { "type": "string", "description": "单元格范围，如 B2:B10" },
                            "format": { "type": "string", "description": "格式类型: currency/percent/text/number/zero_dash/custom" }
                        }
                    },
                    "description": "[generate] 数字格式列表"
                },
                "conditionalFormats": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "range": { "type": "string", "description": "单元格范围" },
                            "rule": { "type": "string", "description": "规则: greaterThan/lessThan/equal/between 等" },
                            "value": { "type": "string", "description": "规则值" },
                            "color": { "type": "string", "description": "高亮颜色（十六进制）" }
                        }
                    },
                    "description": "[generate] 条件格式列表"
                },
                "colorCoding": {
                    "type": "boolean",
                    "description": "[generate] 是否启用颜色编码（蓝色=输入值、黑色=公式、绿色=跨表引用、红色=外部链接），默认 true",
                    "default": true
                },
                "template": {
                    "type": "string",
                    "description": "[generate] 模板文件路径（可选）"
                },
                "validate": {
                    "type": "boolean",
                    "description": "[generate] 生成后是否执行文档质量验证，默认 false",
                    "default": false
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
                    "description": "[read] 是否包含格式信息，默认 false",
                    "default": false
                },
                "operations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": {
                                "type": "string",
                                "enum": [
                                    "set_cell", "add_sheet", "delete_sheet", "set_range",
                                    "setFormula", "setFormat", "setColorCoding", "addConditionalFormat"
                                ],
                                "description": "操作类型"
                            },
                            "sheet": {
                                "type": "string",
                                "description": "工作表名称"
                            },
                            "row": {
                                "type": "integer",
                                "description": "行号（从1开始）"
                            },
                            "col": {
                                "type": "integer",
                                "description": "列号（从1开始）"
                            },
                            "value": {
                                "description": "单元格值或规则值"
                            },
                            "formula": {
                                "type": "string",
                                "description": "Excel 公式，用于 setFormula 操作"
                            },
                            "range": {
                                "type": "string",
                                "description": "单元格范围，用于 setFormat/setColorCoding/addConditionalFormat 操作"
                            },
                            "format": {
                                "type": "string",
                                "description": "数字格式类型，用于 setFormat 操作"
                            },
                            "colorType": {
                                "type": "string",
                                "description": "颜色编码类型 (input/formula/cross_ref/external/assumption)，用于 setColorCoding 操作"
                            },
                            "rule": {
                                "type": "string",
                                "description": "条件格式规则，用于 addConditionalFormat 操作"
                            },
                            "color": {
                                "type": "string",
                                "description": "颜色值（十六进制），用于多种操作"
                            },
                            "name": {
                                "type": "string",
                                "description": "工作表名称，用于 add_sheet/delete_sheet 操作"
                            },
                            "data": {
                                "type": "array",
                                "description": "数据行，用于 set_range 操作"
                            }
                        },
                        "required": ["type"]
                    },
                    "description": "[modify] 修改操作列表"
                },
                "target_format": {
                    "type": "string",
                    "enum": ["docx", "xlsx", "pptx", "pdf", "md", "txt", "csv", "html"],
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
    async fn execute(&self, params: Value) -> SkillResult {
        let action = params["action"].as_str().unwrap_or("");
        match action {
            "generate" => execute_generate(&self.doc_service, "xlsx", params).await,
            "read" => execute_read(&self.doc_service, "xlsx", params).await,
            "modify" => execute_modify(&self.doc_service, "xlsx", params).await,
            "convert" => execute_convert(&self.doc_service, "xlsx", params).await,
            "analyze" => execute_analyze(&self.doc_service, "xlsx", params).await,
            _ => SkillResult {
                success: false,
                output: None,
                error: Some(format!("XlsxSkill 不支持的操作类型: {}", action)),
                duration_ms: 0,
            },
        }
    }
}

// ============================================================================
// PptxSkill - PPT 文档技能
// ============================================================================

/// PPT 文档技能
/// 聚合 generate/read/modify/convert/analyze 五种操作
struct PptxSkill {
    doc_service: Arc<DocumentService>,
}

impl PptxSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for PptxSkill {
    fn skill_name(&self) -> &str { "pptx_skill" }
    fn description(&self) -> &str {
        "PPT演示文稿(.pptx)处理技能，支持生成、读取、修改、格式转换、分析五种操作。生成支持颜色方案、字体配置、边距设置；修改支持添加幻灯片/替换文本/应用颜色方案/设置字体/设置边距/设置幻灯片背景等操作；转换支持pptx/pdf等格式。"
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
                    "enum": ["generate", "read", "modify", "convert", "analyze"],
                    "description": "操作类型: generate=生成文档, read=读取文档, modify=修改文档, convert=格式转换, analyze=分析文档"
                },
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）。generate 时为输出路径，其他操作为输入路径"
                },
                "content": {
                    "type": "string",
                    "description": "[generate] 文档内容（纯文本或结构化 JSON）"
                },
                "title": {
                    "type": "string",
                    "description": "[generate] 文档标题"
                },
                "slides": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string", "description": "幻灯片标题" },
                            "content": { "type": "string", "description": "幻灯片内容" },
                            "layout": { "type": "string", "description": "布局名称" }
                        }
                    },
                    "description": "[generate] 幻灯片列表（结构化数据，优先于 content）"
                },
                "colorScheme": {
                    "type": "string",
                    "enum": ["midnight", "forest", "coral", "ocean", "charcoal"],
                    "description": "[generate] 颜色方案: midnight(深蓝)/forest(森林)/coral(珊瑚)/ocean(海洋)/charcoal(炭灰)"
                },
                "fonts": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string", "description": "标题字体" },
                        "body": { "type": "string", "description": "正文字体" }
                    },
                    "description": "[generate] 字体配置"
                },
                "margins": {
                    "type": "object",
                    "properties": {
                        "top": { "type": "number", "description": "上边距（inch）" },
                        "right": { "type": "number", "description": "右边距（inch）" },
                        "bottom": { "type": "number", "description": "下边距（inch）" },
                        "left": { "type": "number", "description": "左边距（inch）" }
                    },
                    "description": "[generate] 边距配置（单位: inch，默认 0.5）"
                },
                "template": {
                    "type": "string",
                    "description": "[generate] 模板文件路径（可选）"
                },
                "validate": {
                    "type": "boolean",
                    "description": "[generate] 生成后是否执行文档质量验证，默认 false",
                    "default": false
                },
                "operations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": {
                                "type": "string",
                                "enum": [
                                    "add_slide", "replace_text", "applyColorScheme",
                                    "setFont", "setMargins", "setSlideBackground"
                                ],
                                "description": "操作类型"
                            },
                            "title": {
                                "type": "string",
                                "description": "幻灯片标题，用于 add_slide 操作"
                            },
                            "content": {
                                "type": "string",
                                "description": "幻灯片内容，用于 add_slide 操作"
                            },
                            "layout": {
                                "type": "string",
                                "description": "布局名称，用于 add_slide 操作"
                            },
                            "old": {
                                "type": "string",
                                "description": "要替换的旧文本，用于 replace_text 操作"
                            },
                            "new": {
                                "type": "string",
                                "description": "替换后的新文本，用于 replace_text 操作"
                            },
                            "scheme": {
                                "type": "string",
                                "description": "颜色方案名称 (midnight/forest/coral/ocean/charcoal)，用于 applyColorScheme 操作"
                            },
                            "element": {
                                "type": "string",
                                "description": "字体元素类型 (title/body/all)，用于 setFont 操作"
                            },
                            "font": {
                                "type": "string",
                                "description": "字体名称，用于 setFont 操作"
                            },
                            "fontSize": {
                                "type": "integer",
                                "description": "字体大小（pt），用于 setFont 操作"
                            },
                            "top": {
                                "type": "number",
                                "description": "上边距（inch），用于 setMargins 操作"
                            },
                            "right": {
                                "type": "number",
                                "description": "右边距（inch），用于 setMargins 操作"
                            },
                            "bottom": {
                                "type": "number",
                                "description": "下边距（inch），用于 setMargins 操作"
                            },
                            "left": {
                                "type": "number",
                                "description": "左边距（inch），用于 setMargins 操作"
                            },
                            "slideIndex": {
                                "type": "integer",
                                "description": "幻灯片索引（从0开始），用于 setSlideBackground 操作"
                            },
                            "color": {
                                "type": "string",
                                "description": "颜色值（十六进制），用于 setSlideBackground 操作"
                            }
                        },
                        "required": ["type"]
                    },
                    "description": "[modify] 修改操作列表"
                },
                "target_format": {
                    "type": "string",
                    "enum": ["docx", "xlsx", "pptx", "pdf", "md", "txt", "csv", "html"],
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
    async fn execute(&self, params: Value) -> SkillResult {
        let action = params["action"].as_str().unwrap_or("");
        match action {
            "generate" => execute_generate(&self.doc_service, "pptx", params).await,
            "read" => execute_read(&self.doc_service, "pptx", params).await,
            "modify" => execute_modify(&self.doc_service, "pptx", params).await,
            "convert" => execute_convert(&self.doc_service, "pptx", params).await,
            "analyze" => execute_analyze(&self.doc_service, "pptx", params).await,
            _ => SkillResult {
                success: false,
                output: None,
                error: Some(format!("PptxSkill 不支持的操作类型: {}", action)),
                duration_ms: 0,
            },
        }
    }
}

// ============================================================================
// PdfSkill - PDF 文档技能
// ============================================================================

/// PDF 文档技能
/// 聚合 generate/read/modify/convert/analyze 五种操作
struct PdfSkill {
    doc_service: Arc<DocumentService>,
}

impl PdfSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for PdfSkill {
    fn skill_name(&self) -> &str { "pdf_skill" }
    fn description(&self) -> &str {
        "PDF文档(.pdf)处理技能，支持生成、读取、修改、格式转换、分析五种操作。生成支持下标上标、页面设置；修改支持合并/拆分/旋转/添加水印/加密等操作；转换支持pdf/txt/md/html等格式。"
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
                    "enum": ["generate", "read", "modify", "convert", "analyze"],
                    "description": "操作类型: generate=生成文档, read=读取文档, modify=修改文档, convert=格式转换, analyze=分析文档"
                },
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）。generate 时为输出路径，其他操作为输入路径"
                },
                "content": {
                    "type": "string",
                    "description": "[generate] 文档内容（纯文本或结构化 JSON）"
                },
                "title": {
                    "type": "string",
                    "description": "[generate] 文档标题"
                },
                "pageSize": {
                    "type": "string",
                    "enum": ["letter", "a4"],
                    "description": "[generate] 页面尺寸（letter=US Letter, a4=A4，默认 a4）"
                },
                "subscripts": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "text": { "type": "string", "description": "下标文本" },
                            "position": { "type": "integer", "description": "插入位置" }
                        }
                    },
                    "description": "[generate] 下标列表，使用 <sub> XML 标签"
                },
                "superscripts": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "text": { "type": "string", "description": "上标文本" },
                            "position": { "type": "integer", "description": "插入位置" }
                        }
                    },
                    "description": "[generate] 上标列表，使用 <super> XML 标签"
                },
                "template": {
                    "type": "string",
                    "description": "[generate] 模板文件路径（可选）"
                },
                "validate": {
                    "type": "boolean",
                    "description": "[generate] 生成后是否执行文档质量验证，默认 false",
                    "default": false
                },
                "operations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": {
                                "type": "string",
                                "enum": ["merge", "split", "rotate", "addWatermark", "encrypt"],
                                "description": "操作类型"
                            },
                            "files": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "要合并的 PDF 文件路径列表，用于 merge 操作"
                            },
                            "ranges": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "页码范围列表（如 ['1-5', '6-10']），用于 split 操作"
                            },
                            "outputDir": {
                                "type": "string",
                                "description": "拆分输出目录，用于 split 操作"
                            },
                            "pages": {
                                "type": "array",
                                "items": { "type": "integer" },
                                "description": "要旋转的页码列表，用于 rotate 操作"
                            },
                            "angle": {
                                "type": "integer",
                                "description": "旋转角度，用于 rotate 操作"
                            },
                            "image": {
                                "type": "string",
                                "description": "水印图片路径，用于 addWatermark 操作"
                            },
                            "text": {
                                "type": "string",
                                "description": "水印文本，用于 addWatermark 操作"
                            },
                            "userPassword": {
                                "type": "string",
                                "description": "用户密码，用于 encrypt 操作"
                            },
                            "ownerPassword": {
                                "type": "string",
                                "description": "所有者密码，用于 encrypt 操作"
                            },
                            "outputPath": {
                                "type": "string",
                                "description": "输出文件路径（可选），用于 PDF 高级操作"
                            }
                        },
                        "required": ["type"]
                    },
                    "description": "[modify] 修改操作列表"
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
    async fn execute(&self, params: Value) -> SkillResult {
        let action = params["action"].as_str().unwrap_or("");
        match action {
            "generate" => execute_generate(&self.doc_service, "pdf", params).await,
            "read" => execute_read(&self.doc_service, "pdf", params).await,
            "modify" => execute_modify(&self.doc_service, "pdf", params).await,
            "convert" => execute_convert(&self.doc_service, "pdf", params).await,
            "analyze" => execute_analyze(&self.doc_service, "pdf", params).await,
            _ => SkillResult {
                success: false,
                output: None,
                error: Some(format!("PdfSkill 不支持的操作类型: {}", action)),
                duration_ms: 0,
            },
        }
    }
}
