use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::models::skill::SkillResult;
use crate::services::document::DocumentService;
use super::registry::Skill;

/// 注册所有内置技能
pub fn register_builtin_skills(
    registry: &mut super::registry::SkillRegistry,
    doc_service: Arc<DocumentService>,
) {
    log::info!("开始注册内置技能");
    registry.register(Box::new(GenerateDocumentSkill::new(doc_service.clone())));
    registry.register(Box::new(ReadDocumentSkill::new(doc_service.clone())));
    registry.register(Box::new(ModifyDocumentSkill::new(doc_service.clone())));
    registry.register(Box::new(DeleteDocumentSkill::new(doc_service.clone())));
    registry.register(Box::new(ConvertFormatSkill::new(doc_service.clone())));
    registry.register(Box::new(SearchDocumentsSkill::new(doc_service.clone())));
    registry.register(Box::new(AnalyzeDocumentSkill::new(doc_service.clone())));
    registry.register(Box::new(ListWorkspaceSkill::new(doc_service.clone())));
    registry.register(Box::new(BatchProcessSkill::new(doc_service)));
    log::info!("内置技能注册完成, 共注册 9 个技能");
}

/// 生成文档技能
struct GenerateDocumentSkill {
    doc_service: Arc<DocumentService>,
}

impl GenerateDocumentSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for GenerateDocumentSkill {
    fn skill_name(&self) -> &str { "generate_document" }
    fn description(&self) -> &str { "生成新的文档，支持 Word、Excel、PPT、PDF、Markdown 格式" }
    fn category(&self) -> &str { "document" }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into(), "xlsx".into(), "pptx".into(), "pdf".into(), "md".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "enum": ["docx", "xlsx", "pptx", "pdf", "md"],
                    "description": "文档格式"
                },
                "path": {
                    "type": "string",
                    "description": "输出文件路径（相对于工作区）"
                },
                "title": {
                    "type": "string",
                    "description": "文档标题"
                },
                "content": {
                    "type": "string",
                    "description": "文档内容（纯文本或结构化 JSON）"
                },
                "template": {
                    "type": "string",
                    "description": "模板文件路径（可选）"
                }
            },
            "required": ["format", "path", "content"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let doc_type = params["format"].as_str().unwrap_or("docx");
        let output_path = params["path"].as_str().unwrap_or("");
        let title = params["title"].as_str().unwrap_or("");
        let content = params["content"].as_str().unwrap_or("");

        let sidecar_params = json!({
            "output_path": output_path,
            "title": title,
            "content": content,
            "variables": {
                "title": title,
            },
        });

        match self.doc_service.process("generate", doc_type, sidecar_params).await {
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
}

/// 读取文档技能
struct ReadDocumentSkill {
    doc_service: Arc<DocumentService>,
}

impl ReadDocumentSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for ReadDocumentSkill {
    fn skill_name(&self) -> &str { "read_document" }
    fn description(&self) -> &str { "读取文档内容，支持提取文本、表格、属性等信息" }
    fn category(&self) -> &str { "document" }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into(), "xlsx".into(), "pptx".into(), "pdf".into(), "md".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "include_formatting": {
                    "type": "boolean",
                    "description": "是否包含格式信息",
                    "default": false
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("docx");
        let doc_type = match extension {
            "docx" => "docx",
            "xlsx" => "xlsx",
            "pptx" => "pptx",
            "pdf" => "pdf",
            "md" | "markdown" => "md",
            _ => "docx",
        };

        let sidecar_params = json!({
            "input_path": file_path,
            "options": {
                "include_formatting": params["include_formatting"].as_bool().unwrap_or(false),
            },
        });

        match self.doc_service.process("read", doc_type, sidecar_params).await {
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
}

/// 修改文档技能
struct ModifyDocumentSkill {
    doc_service: Arc<DocumentService>,
}

impl ModifyDocumentSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for ModifyDocumentSkill {
    fn skill_name(&self) -> &str { "modify_document" }
    fn description(&self) -> &str { "修改已有文档，支持文本替换、添加段落、添加表格等操作" }
    fn category(&self) -> &str { "document" }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into(), "xlsx".into(), "pptx".into(), "md".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "operations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": {
                                "type": "string",
                                "enum": ["replace", "add_paragraph", "add_heading", "add_table", "set_cell", "append", "prepend"],
                                "description": "操作类型"
                            }
                        }
                    },
                    "description": "修改操作列表"
                }
            },
            "required": ["path", "operations"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("docx");
        let doc_type = match extension {
            "docx" => "docx",
            "xlsx" => "xlsx",
            "pptx" => "pptx",
            "md" | "markdown" => "md",
            _ => "docx",
        };

        let sidecar_params = json!({
            "input_path": file_path,
            "output_path": file_path,
            "operations": params["operations"],
        });

        match self.doc_service.process("modify", doc_type, sidecar_params).await {
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
}

/// 删除文档技能
struct DeleteDocumentSkill {
    doc_service: Arc<DocumentService>,
}

impl DeleteDocumentSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for DeleteDocumentSkill {
    fn skill_name(&self) -> &str { "delete_document" }
    fn description(&self) -> &str { "删除指定文档文件" }
    fn category(&self) -> &str { "document" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "create_snapshot": {
                    "type": "boolean",
                    "description": "删除前是否创建快照",
                    "default": true
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");

        let sidecar_params = json!({
            "input_path": file_path,
            "create_snapshot": params["create_snapshot"].as_bool().unwrap_or(true),
        });

        match self.doc_service.process("delete", "docx", sidecar_params).await {
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
}

/// 格式转换技能
struct ConvertFormatSkill {
    doc_service: Arc<DocumentService>,
}

impl ConvertFormatSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for ConvertFormatSkill {
    fn skill_name(&self) -> &str { "convert_format" }
    fn description(&self) -> &str { "文档格式转换，如 Word 转 PDF、Markdown 转 Word 等" }
    fn category(&self) -> &str { "document" }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into(), "xlsx".into(), "pptx".into(), "pdf".into(), "md".into()]
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source_path": {
                    "type": "string",
                    "description": "源文件路径"
                },
                "target_format": {
                    "type": "string",
                    "enum": ["docx", "xlsx", "pptx", "pdf", "md", "txt"],
                    "description": "目标格式"
                },
                "output_path": {
                    "type": "string",
                    "description": "输出文件路径（可选，默认自动生成）"
                }
            },
            "required": ["source_path", "target_format"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let source_path = params["source_path"].as_str().unwrap_or("");
        let target_format = params["target_format"].as_str().unwrap_or("pdf");
        let output_path = params["output_path"].as_str().unwrap_or("");

        let source_extension = std::path::Path::new(source_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("docx");

        let output_path = if output_path.is_empty() {
            let stem = std::path::Path::new(source_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            format!("{}.{}", stem, target_format)
        } else {
            output_path.to_string()
        };

        let sidecar_params = json!({
            "input_path": source_path,
            "output_path": output_path,
            "source_type": source_extension,
            "options": {},
        });

        match self.doc_service.process("convert", target_format, sidecar_params).await {
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
}

/// 搜索文档技能
struct SearchDocumentsSkill {
    doc_service: Arc<DocumentService>,
}

impl SearchDocumentsSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for SearchDocumentsSkill {
    fn skill_name(&self) -> &str { "search_documents" }
    fn description(&self) -> &str { "在工作区中搜索文档，支持按文件名或内容搜索" }
    fn category(&self) -> &str { "workspace" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "搜索关键词"
                },
                "extensions": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "限定文件扩展名"
                },
                "include_content": {
                    "type": "boolean",
                    "description": "是否搜索文件内容",
                    "default": false
                },
                "max_results": {
                    "type": "integer",
                    "description": "最大结果数",
                    "default": 50
                }
            },
            "required": ["query"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();

        let sidecar_params = json!({
            "query": params["query"],
            "extensions": params["extensions"],
            "include_content": params["include_content"].as_bool().unwrap_or(false),
            "max_results": params["max_results"].as_u64().unwrap_or(50),
        });

        match self.doc_service.process("search", "docx", sidecar_params).await {
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
}

/// 分析文档技能
struct AnalyzeDocumentSkill {
    doc_service: Arc<DocumentService>,
}

impl AnalyzeDocumentSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for AnalyzeDocumentSkill {
    fn skill_name(&self) -> &str { "analyze_document" }
    fn description(&self) -> &str { "分析文档结构和统计信息，如字数、段落数、标题层级等" }
    fn category(&self) -> &str { "document" }
    fn supported_types(&self) -> Vec<String> {
        vec!["docx".into(), "xlsx".into(), "pptx".into(), "pdf".into(), "md".into()]
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
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("docx");
        let doc_type = match extension {
            "docx" => "docx",
            "xlsx" => "xlsx",
            "pptx" => "pptx",
            "pdf" => "pdf",
            "md" | "markdown" => "md",
            _ => "docx",
        };

        let sidecar_params = json!({
            "input_path": file_path,
        });

        match self.doc_service.process("analyze", doc_type, sidecar_params).await {
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
}

/// 列出工作区文件技能
struct ListWorkspaceSkill {
    doc_service: Arc<DocumentService>,
}

impl ListWorkspaceSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for ListWorkspaceSkill {
    fn skill_name(&self) -> &str { "list_workspace" }
    fn description(&self) -> &str { "列出工作区中的文件和目录结构" }
    fn category(&self) -> &str { "workspace" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "目录路径（相对于工作区根目录，默认为根目录）"
                },
                "depth": {
                    "type": "integer",
                    "description": "遍历深度，默认1",
                    "default": 1
                },
                "extensions": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "筛选文件扩展名"
                }
            }
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();

        let sidecar_params = json!({
            "path": params["path"].as_str().unwrap_or("."),
            "depth": params["depth"].as_u64().unwrap_or(1),
            "extensions": params["extensions"],
        });

        match self.doc_service.process("list", "docx", sidecar_params).await {
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
}

/// 批量处理技能
struct BatchProcessSkill {
    doc_service: Arc<DocumentService>,
}

impl BatchProcessSkill {
    fn new(doc_service: Arc<DocumentService>) -> Self {
        Self { doc_service }
    }
}

#[async_trait]
impl Skill for BatchProcessSkill {
    fn skill_name(&self) -> &str { "batch_process" }
    fn description(&self) -> &str { "批量处理多个文档，支持批量转换、修改、分析等操作" }
    fn category(&self) -> &str { "document" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["convert", "modify", "analyze"],
                    "description": "批量操作类型"
                },
                "paths": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "文件路径列表"
                },
                "params": {
                    "type": "object",
                    "description": "操作参数"
                }
            },
            "required": ["operation", "paths"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let operation = params["operation"].as_str().unwrap_or("analyze");
        let paths = params["paths"].as_array().cloned().unwrap_or_default();
        let op_params = params["params"].clone();

        let mut results = Vec::new();
        let mut all_success = true;

        for path_val in paths {
            let path_str = path_val.as_str().unwrap_or("");
            if path_str.is_empty() {
                continue;
            }

            let extension = std::path::Path::new(path_str)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("docx");
            let doc_type = match extension {
                "docx" => "docx",
                "xlsx" => "xlsx",
                "pptx" => "pptx",
                "pdf" => "pdf",
                "md" | "markdown" => "md",
                _ => "docx",
            };

            let sidecar_params = match operation {
                "convert" => json!({
                    "input_path": path_str,
                    "output_path": op_params["output_path"],
                    "source_type": extension,
                    "options": {},
                }),
                "modify" => json!({
                    "input_path": path_str,
                    "output_path": path_str,
                    "operations": op_params["operations"],
                }),
                _ => json!({
                    "input_path": path_str,
                }),
            };

            let action = match operation {
                "convert" => "convert",
                "modify" => "modify",
                _ => "analyze",
            };

            match self.doc_service.process(action, doc_type, sidecar_params).await {
                Ok(data) => results.push(json!({
                    "path": path_str,
                    "success": true,
                    "data": data,
                })),
                Err(e) => {
                    all_success = false;
                    results.push(json!({
                        "path": path_str,
                        "success": false,
                        "error": e.message,
                    }));
                }
            }
        }

        SkillResult {
            success: all_success,
            output: Some(json!({
                "operation": operation,
                "total": results.len(),
                "results": results,
            })),
            error: if all_success { None } else { Some("部分文件处理失败".to_string()) },
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }
}
