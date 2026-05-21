use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::models::skill::SkillResult;
use crate::services::document::DocumentService;
use super::registry::Skill;

/// 将相对路径解析为绝对路径
/// 如果路径已经是绝对路径，直接返回；否则拼接 workspace_root
fn resolve_path(path: &str, workspace_root: &str) -> String {
    if path.is_empty() {
        return path.to_string();
    }
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        return path.to_string();
    }
    // 相对路径：拼接工作区根目录
    let root = std::path::Path::new(workspace_root);
    root.join(path).to_string_lossy().to_string()
}

/// 注册所有内置技能
pub fn register_builtin_skills(
    registry: &mut super::registry::SkillRegistry,
    doc_service: Arc<DocumentService>,
) {
    log::info!("开始注册内置技能");
    registry.register(Box::new(GenerateDocumentSkill::new(doc_service.clone())));
    registry.register(Box::new(ReadDocumentSkill::new(doc_service.clone())));
    registry.register(Box::new(ModifyDocumentSkill::new(doc_service.clone())));
    registry.register(Box::new(DeleteDocumentSkill::new()));
    registry.register(Box::new(ConvertFormatSkill::new(doc_service.clone())));
    registry.register(Box::new(SearchDocumentsSkill::new()));
    registry.register(Box::new(AnalyzeDocumentSkill::new(doc_service.clone())));
    registry.register(Box::new(ListWorkspaceSkill::new()));
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
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        // 将相对路径解析为绝对路径，确保 Sidecar 在正确目录生成文件
        let resolved_path = resolve_path(output_path, workspace_root);

        // content 参数支持纯文本或结构化 JSON
        // 如果是字符串直接使用，如果是 JSON 对象/数组则序列化为字符串传递
        let content = match params["content"].as_str() {
            Some(s) => s.to_string(),
            None => {
                if !params["content"].is_null() {
                    serde_json::to_string(&params["content"]).unwrap_or_default()
                } else {
                    String::new()
                }
            }
        };

        let mut sidecar_params = json!({
            "path": resolved_path,
            "title": title,
            "content": content,
        });

        // 如果提供了模板参数，传递给 Sidecar
        if let Some(template) = params["template"].as_str() {
            if !template.is_empty() {
                sidecar_params["template"] = json!(template);
            }
        }

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
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let resolved_path = resolve_path(file_path, workspace_root);
        let extension = std::path::Path::new(&resolved_path)
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
            "path": resolved_path,
            "include_formatting": params["include_formatting"].as_bool().unwrap_or(false),
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
                            },
                            "index": {
                                "type": "integer",
                                "description": "段落索引（从0开始），用于 replace 操作按索引替换整段内容"
                            },
                            "text": {
                                "type": "string",
                                "description": "新文本内容，用于 replace（按索引替换）或 add_paragraph/add_heading 操作"
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
                            }
                        },
                        "required": ["type"]
                    },
                    "description": "修改操作列表。replace 操作支持两种模式：1) 按索引替换整段：提供 index 和 text；2) 全文搜索替换：提供 old 和 new"
                }
            },
            "required": ["path", "operations"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let resolved_path = resolve_path(file_path, workspace_root);
        let extension = std::path::Path::new(&resolved_path)
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
            "path": resolved_path,
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

/// 删除文档技能（Rust 原生实现，不走 Sidecar）
struct DeleteDocumentSkill;

impl DeleteDocumentSkill {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Skill for DeleteDocumentSkill {
    fn skill_name(&self) -> &str { "delete_document" }
    fn description(&self) -> &str { "删除指定文档文件，删除前可选创建备份" }
    fn category(&self) -> &str { "document" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要删除的文件路径"
                },
                "workspace_root": {
                    "type": "string",
                    "description": "工作区根目录路径，用于安全校验，文件路径必须在该目录下"
                },
                "create_backup": {
                    "type": "boolean",
                    "description": "删除前是否创建备份文件",
                    "default": true
                }
            },
            "required": ["path", "workspace_root"]
        })
    }
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        // workspace_root 优先使用 executor 注入的值（来自应用配置），
        // 而非 LLM 提供的值，防止 LLM 提供恶意路径绕过校验
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if file_path.is_empty() {
            return SkillResult {
                success: false,
                output: None,
                error: Some("缺少文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        // 安全校验：必须提供工作区根目录路径
        if workspace_root.is_empty() {
            return SkillResult {
                success: false,
                output: None,
                error: Some("缺少工作区根目录路径，无法进行安全校验".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        // 将相对路径解析为绝对路径后再做安全校验
        let resolved_path = resolve_path(file_path, workspace_root);

        // 规范化路径并校验文件是否在工作区内，防止路径遍历攻击（如 ../）
        let canonical_file = match std::path::Path::new(&resolved_path).canonicalize() {
            Ok(p) => p,
            Err(_) => {
                return SkillResult {
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
                return SkillResult {
                    success: false,
                    output: None,
                    error: Some(format!("工作区根目录不存在或路径无效: {}", workspace_root)),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        if !canonical_file.starts_with(&canonical_root) {
            return SkillResult {
                success: false,
                output: None,
                error: Some("文件路径不在工作区内，拒绝删除操作".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        if !canonical_file.is_file() {
            return SkillResult {
                success: false,
                output: None,
                error: Some(format!("路径不是文件: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        // 使用规范化后的安全路径继续操作
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
                SkillResult {
                    success: true,
                    output: Some(result),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                }
            }
            Err(e) => {
                log::error!("删除文件失败: {}, 错误: {}", safe_path, e);
                SkillResult {
                    success: false,
                    output: None,
                    error: Some(format!("删除文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                }
            }
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
                    "enum": ["docx", "xlsx", "pptx", "pdf", "md", "txt", "csv", "html"],
                    "description": "目标格式（docx/xlsx/pptx/pdf/md/txt/csv/html）"
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
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        // 解析源文件路径
        let resolved_source = resolve_path(source_path, workspace_root);

        let output_path = if output_path.is_empty() {
            let stem = std::path::Path::new(&resolved_source)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            format!("{}.{}", stem, target_format)
        } else {
            // 解析输出文件路径
            resolve_path(output_path, workspace_root)
        };

        // 根据源文件扩展名确定 doc_type，确保调用正确的处理器
        // 例如：.docx 转 .pdf 时，应调用 Word 处理器的 convert 方法（它知道如何读取 .docx）
        let source_extension = std::path::Path::new(&resolved_source)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("docx");
        let source_doc_type = match source_extension {
            "docx" => "docx",
            "xlsx" => "xlsx",
            "pptx" => "pptx",
            "pdf" => "pdf",
            "md" | "markdown" => "md",
            _ => "docx",
        };

        let sidecar_params = json!({
            "path": resolved_source,
            "output_path": output_path,
            "format": target_format,
        });

        match self.doc_service.process("convert", source_doc_type, sidecar_params).await {
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

/// 搜索文档技能（Rust 原生实现，不走 Sidecar）
struct SearchDocumentsSkill;

impl SearchDocumentsSkill {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Skill for SearchDocumentsSkill {
    fn skill_name(&self) -> &str { "search_documents" }
    fn description(&self) -> &str { "在指定目录中搜索文档，支持按文件名或内容搜索" }
    fn category(&self) -> &str { "workspace" }
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
    async fn execute(&self, params: Value) -> SkillResult {
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
            return SkillResult {
                success: false,
                output: None,
                error: Some("搜索关键词和文件扩展名不能同时为空，请至少提供一项".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        // 路径安全校验：搜索目录必须在工作区内
        // 先将相对路径解析为绝对路径
        let resolved_directory = resolve_path(directory, workspace_root);
        let dir_path = std::path::Path::new(&resolved_directory);
        if !dir_path.exists() || !dir_path.is_dir() {
            return SkillResult {
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
                    return SkillResult {
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
                    return SkillResult {
                        success: false,
                        output: None,
                        error: Some("工作区根目录路径无效".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
            };
            if !canonical_dir.starts_with(&canonical_root) {
                return SkillResult {
                    success: false,
                    output: None,
                    error: Some("搜索目录不在工作区内，拒绝访问".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        }

        let query_lower = query.to_lowercase();
        let directory_owned = directory.to_string();
        let extensions_clone = extensions.clone();

        // 使用 spawn_blocking 避免同步文件IO阻塞异步运行时
        let results = tokio::task::spawn_blocking(move || {
            let dir_path = std::path::Path::new(&directory_owned);
            let mut results = Vec::new();
            skill_search_files(dir_path, dir_path, &query_lower, &extensions_clone, include_content, max_results, &mut results);
            results
        }).await.unwrap_or_default();

        SkillResult {
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

/// 递归搜索文件（Skill 内部使用）
fn skill_search_files(
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
            skill_search_files(&path, root, query, extensions, include_content, max_results, results);
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
        // 当 query 为空时，跳过名称匹配（仅按扩展名过滤）
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

        // 根据匹配方式确定 match_type：内容匹配 > 名称匹配 > 扩展名过滤
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
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let resolved_path = resolve_path(file_path, workspace_root);
        let extension = std::path::Path::new(&resolved_path)
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
            "path": resolved_path,
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

/// 列出工作区文件技能（Rust 原生实现，不走 Sidecar）
struct ListWorkspaceSkill;

impl ListWorkspaceSkill {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Skill for ListWorkspaceSkill {
    fn skill_name(&self) -> &str { "list_workspace" }
    fn description(&self) -> &str { "列出指定目录中的文件和子目录结构" }
    fn category(&self) -> &str { "workspace" }
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
    async fn execute(&self, params: Value) -> SkillResult {
        let start = Instant::now();
        let dir_path = params["path"].as_str().unwrap_or(".");
        let max_depth = params["depth"].as_u64().unwrap_or(1) as u32;
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        let extensions: Vec<String> = params["extensions"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        // 将相对路径解析为绝对路径
        let resolved_dir = resolve_path(dir_path, workspace_root);
        let dir = std::path::Path::new(&resolved_dir);
        if !dir.exists() {
            return SkillResult {
                success: false,
                output: None,
                error: Some(format!("目录不存在: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        if !dir.is_dir() {
            return SkillResult {
                success: false,
                output: None,
                error: Some(format!("路径不是目录: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }

        // 路径安全校验：列出目录必须在工作区内
        if !workspace_root.is_empty() {
            let canonical_dir = match dir.canonicalize() {
                Ok(p) => p,
                Err(_) => {
                    return SkillResult {
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
                    return SkillResult {
                        success: false,
                        output: None,
                        error: Some("工作区根目录路径无效".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
            };
            if !canonical_dir.starts_with(&canonical_root) {
                return SkillResult {
                    success: false,
                    output: None,
                    error: Some("目录不在工作区内，拒绝访问".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        }

        let dir_path_owned = dir_path.to_string();
        let extensions_clone = extensions.clone();

        // 使用 spawn_blocking 避免同步文件IO阻塞异步运行时
        let results = tokio::task::spawn_blocking(move || {
            let dir = std::path::Path::new(&dir_path_owned);
            skill_list_dir(dir, dir, max_depth, 0, &extensions_clone)
        }).await.unwrap_or_default();

        SkillResult {
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

/// 递归列出目录内容（Skill 内部使用）
fn skill_list_dir(
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
            let children = skill_list_dir(&path, root, max_depth, current_depth + 1, extensions);
            node["children"] = json!(children);
        }

        nodes.push(node);
    }

    nodes
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
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        let mut results = Vec::new();
        let mut all_success = true;

        for path_val in paths {
            let path_str = path_val.as_str().unwrap_or("");
            if path_str.is_empty() {
                continue;
            }

            // 解析相对路径为绝对路径
            let resolved_path = resolve_path(path_str, workspace_root);

            let extension = std::path::Path::new(&resolved_path)
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
                    "path": resolved_path,
                    "output_path": op_params["output_path"],
                    "format": op_params.get("target_format").and_then(|v| v.as_str()).unwrap_or(extension),
                }),
                "modify" => json!({
                    "path": resolved_path,
                    "operations": op_params["operations"],
                }),
                _ => json!({
                    "path": resolved_path,
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
