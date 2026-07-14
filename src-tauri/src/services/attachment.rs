use crate::errors::CommandError;
use crate::models::llm::ContentPart;
use crate::models::message::{AttachmentMeta, AttachmentType};
use crate::services::document::DocumentService;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::json;

/// 支持的图片 MIME 类型
const SUPPORTED_IMAGE_MIME_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/jpg",
    "image/gif",
    "image/webp",
];

/// 支持的文本 MIME 类型
const SUPPORTED_TEXT_MIME_TYPES: &[&str] = &[
    "text/plain",
    "text/markdown",
    "text/csv",
    "text/html",
    "application/json",
    "text/xml",
    "application/xml",
    "text/yaml",
    "text/x-yaml",
    "text/toml",
    "text/ini",
    "text/log",
];

/// 文档 MIME 类型到 Sidecar doc_type 的映射
const DOCUMENT_MIME_TO_DOCTYPE: &[(&str, &str)] = &[
    (
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "docx",
    ),
    (
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "xlsx",
    ),
    (
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "pptx",
    ),
    ("application/pdf", "pdf"),
];

/// 文档文件扩展名到 Sidecar doc_type 的映射（用于 MIME 类型不精确时的回退推断）
const DOCUMENT_EXT_TO_DOCTYPE: &[(&str, &str)] = &[
    (".docx", "docx"),
    (".xlsx", "xlsx"),
    (".pptx", "pptx"),
    (".pdf", "pdf"),
];

/// 图片文件大小上限 (20MB)
const MAX_IMAGE_SIZE: u64 = 20 * 1024 * 1024;

/// 文本文件大小上限 (1MB)
const MAX_TEXT_SIZE: u64 = 1024 * 1024;

/// 文档文件大小上限 (10MB)
const MAX_DOCUMENT_SIZE: u64 = 10 * 1024 * 1024;

/// 单次发送附件数量上限
const MAX_ATTACHMENT_COUNT: usize = 10;

/// 文档解析后文本最大字符数（约 25K tokens）
const MAX_DOCUMENT_TEXT_LENGTH: usize = 100_000;

/// 附件解析服务：将 AttachmentMeta 解析为 LLM 可消费的 ContentPart
pub struct AttachmentService;

impl AttachmentService {
    /// 将附件列表解析为 ContentPart 列表
    /// workspace_root: 工作区根目录，用于解析相对路径
    /// doc_service: 文档服务，用于调用 Sidecar 解析文档类型附件
    pub async fn resolve_attachments(
        attachments: &[AttachmentMeta],
        workspace_root: &str,
        doc_service: &DocumentService,
    ) -> Result<Vec<ContentPart>, CommandError> {
        // 检查附件数量上限
        if attachments.len() > MAX_ATTACHMENT_COUNT {
            return Err(CommandError::doc(
                3012,
                format!(
                    "附件数量超过上限 ({}个)，最多支持 {} 个",
                    attachments.len(),
                    MAX_ATTACHMENT_COUNT
                ),
            ));
        }

        let mut parts = Vec::new();
        for attachment in attachments {
            let content_parts =
                Self::resolve_single(attachment, workspace_root, doc_service).await?;
            parts.extend(content_parts);
        }
        Ok(parts)
    }

    /// 解析单个附件
    async fn resolve_single(
        attachment: &AttachmentMeta,
        workspace_root: &str,
        doc_service: &DocumentService,
    ) -> Result<Vec<ContentPart>, CommandError> {
        match attachment.attachment_type {
            AttachmentType::Image => Self::resolve_image(attachment, workspace_root),
            AttachmentType::Document => {
                Self::resolve_document(attachment, workspace_root, doc_service).await
            }
            AttachmentType::Text => Self::resolve_text(attachment, workspace_root),
        }
    }

    /// 解析图片附件：读取文件并编码为 base64
    fn resolve_image(
        attachment: &AttachmentMeta,
        workspace_root: &str,
    ) -> Result<Vec<ContentPart>, CommandError> {
        // 检查文件大小
        if attachment.size > MAX_IMAGE_SIZE {
            return Err(CommandError::doc(
                3012,
                format!(
                    "图片文件 '{}' 过大 ({:.1}MB)，最大支持 {:.0}MB",
                    attachment.name,
                    attachment.size as f64 / (1024.0 * 1024.0),
                    MAX_IMAGE_SIZE as f64 / (1024.0 * 1024.0)
                ),
            ));
        }

        // 检查 MIME 类型
        if !SUPPORTED_IMAGE_MIME_TYPES.contains(&attachment.mime_type.as_str()) {
            return Err(CommandError::doc(
                3002,
                format!(
                    "不支持的图片格式 '{}'，支持的格式: {}",
                    attachment.mime_type,
                    SUPPORTED_IMAGE_MIME_TYPES.join(", ")
                ),
            ));
        }

        // 优先使用前端传入的 base64 数据
        let base64_data = if let Some(ref data) = attachment.data {
            data.clone()
        } else {
            // 从文件路径读取
            let file_path = Self::resolve_path(attachment, workspace_root)?;
            let data = std::fs::read(&file_path).map_err(|e| {
                CommandError::fs(
                    6006,
                    format!("读取图片文件失败 '{}': {}", attachment.name, e),
                )
            })?;
            BASE64.encode(&data)
        };

        Ok(vec![ContentPart::Image {
            mime_type: attachment.mime_type.clone(),
            data: base64_data,
        }])
    }

    /// 解析文档附件：调用 Sidecar read_document 提取文本内容
    async fn resolve_document(
        attachment: &AttachmentMeta,
        workspace_root: &str,
        doc_service: &DocumentService,
    ) -> Result<Vec<ContentPart>, CommandError> {
        // 检查文件大小
        if attachment.size > MAX_DOCUMENT_SIZE {
            return Err(CommandError::doc(
                3012,
                format!(
                    "文档文件 '{}' 过大 ({:.1}MB)，最大支持 {:.0}MB",
                    attachment.name,
                    attachment.size as f64 / (1024.0 * 1024.0),
                    MAX_DOCUMENT_SIZE as f64 / (1024.0 * 1024.0)
                ),
            ));
        }

        // 推断 Sidecar doc_type
        let doc_type = Self::infer_doc_type(&attachment.mime_type, &attachment.name);
        let Some(doc_type) = doc_type else {
            // 无法识别文档类型，降级为文本处理
            log::warn!(
                "无法识别文档类型: mime={}, name={}，降级为文本处理",
                attachment.mime_type,
                attachment.name
            );
            return Self::resolve_text(attachment, workspace_root);
        };

        // 解析文件绝对路径
        let file_path = match Self::resolve_path(attachment, workspace_root) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(e) => {
                // 路径解析失败，尝试使用前端传入的 base64 数据写入临时文件后调用 Sidecar
                if let Some(ref data) = attachment.data {
                    log::info!(
                        "文档路径解析失败，尝试将 base64 数据写入临时文件后调用 Sidecar: {}",
                        attachment.name
                    );
                    return Self::resolve_document_from_base64(
                        attachment,
                        data,
                        doc_type,
                        doc_service,
                    )
                    .await;
                }
                return Err(e);
            }
        };

        // 调用 Sidecar read 操作
        log::info!(
            "调用 Sidecar 解析文档附件: name={}, doc_type={}, path={}",
            attachment.name,
            doc_type,
            file_path
        );

        let sidecar_params = json!({
            "path": file_path,
        });

        let result = match doc_service.process("read", doc_type, sidecar_params).await {
            Ok(data) => data,
            Err(e) => {
                log::warn!(
                    "Sidecar 文档解析失败: {}，降级为文本处理: {}",
                    attachment.name,
                    e.message
                );
                // Sidecar 解析失败时降级为文本处理
                return Self::resolve_text(attachment, workspace_root);
            }
        };

        // 将 Sidecar 返回的结构化数据转换为可读文本
        let text_content = Self::format_document_content(doc_type, &result, &attachment.name);

        // 截断过长的文档内容
        let truncated_content = if text_content.len() > MAX_DOCUMENT_TEXT_LENGTH {
            log::warn!(
                "文档解析后文本过长 ({}字符)，截断至 {} 字符: {}",
                text_content.len(),
                MAX_DOCUMENT_TEXT_LENGTH,
                attachment.name
            );
            format!(
                "{}...\n\n[内容过长已截断，原始长度: {} 字符]",
                &text_content[..MAX_DOCUMENT_TEXT_LENGTH],
                text_content.len()
            )
        } else {
            text_content
        };

        // 用标签包裹，方便 LLM 理解来源
        let text_with_context = format!(
            "<attachment name=\"{}\" type=\"{}\" format=\"{}\">\n{}\n</attachment>",
            attachment.name, attachment.mime_type, doc_type, truncated_content
        );

        Ok(vec![ContentPart::Text {
            text: text_with_context,
        }])
    }

    /// 从 base64 数据解析文档附件：写入临时文件后调用 Sidecar
    async fn resolve_document_from_base64(
        attachment: &AttachmentMeta,
        base64_data: &str,
        doc_type: &str,
        doc_service: &DocumentService,
    ) -> Result<Vec<ContentPart>, CommandError> {
        // 解码 base64 数据
        let bytes = BASE64.decode(base64_data).map_err(|e| {
            CommandError::doc(
                3012,
                format!("文档 base64 解码失败 '{}': {}", attachment.name, e),
            )
        })?;

        // 写入临时文件
        let temp_dir = std::env::temp_dir().join("workmolde_attachments");
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| CommandError::fs(6006, format!("创建临时目录失败: {}", e)))?;

        // 根据文档类型确定文件扩展名
        let ext = match doc_type {
            "docx" => ".docx",
            "xlsx" => ".xlsx",
            "pptx" => ".pptx",
            "pdf" => ".pdf",
            _ => ".bin",
        };

        // 使用 UUID 生成唯一文件名，避免冲突
        let temp_file_name = format!("{}{}", uuid::Uuid::new_v4(), ext);
        let temp_file_path = temp_dir.join(&temp_file_name);

        std::fs::write(&temp_file_path, &bytes).map_err(|e| {
            CommandError::fs(
                6006,
                format!("写入临时文件失败 '{}': {}", attachment.name, e),
            )
        })?;

        log::info!(
            "已将附件 base64 数据写入临时文件: name={}, temp_path={}",
            attachment.name,
            temp_file_path.display()
        );

        // 调用 Sidecar read 操作解析临时文件
        let sidecar_params = json!({
            "path": temp_file_path.to_string_lossy().to_string(),
        });

        let result = match doc_service.process("read", doc_type, sidecar_params).await {
            Ok(data) => data,
            Err(e) => {
                // Sidecar 解析失败，清理临时文件
                let _ = std::fs::remove_file(&temp_file_path);
                return Err(e);
            }
        };

        // 清理临时文件
        let _ = std::fs::remove_file(&temp_file_path);

        // 将 Sidecar 返回的结构化数据转换为可读文本
        let text_content = Self::format_document_content(doc_type, &result, &attachment.name);

        // 截断过长的文档内容
        let truncated_content = if text_content.len() > MAX_DOCUMENT_TEXT_LENGTH {
            log::warn!(
                "文档解析后文本过长 ({}字符)，截断至 {} 字符: {}",
                text_content.len(),
                MAX_DOCUMENT_TEXT_LENGTH,
                attachment.name
            );
            format!(
                "{}...\n\n[内容过长已截断，原始长度: {} 字符]",
                &text_content[..MAX_DOCUMENT_TEXT_LENGTH],
                text_content.len()
            )
        } else {
            text_content
        };

        // 用标签包裹，方便 LLM 理解来源
        let text_with_context = format!(
            "<attachment name=\"{}\" type=\"{}\" format=\"{}\">\n{}\n</attachment>",
            attachment.name, attachment.mime_type, doc_type, truncated_content
        );

        Ok(vec![ContentPart::Text {
            text: text_with_context,
        }])
    }

    /// 解析纯文本附件：直接读取文件内容
    fn resolve_text(
        attachment: &AttachmentMeta,
        workspace_root: &str,
    ) -> Result<Vec<ContentPart>, CommandError> {
        // 检查文件大小
        if attachment.size > MAX_TEXT_SIZE {
            return Err(CommandError::doc(
                3012,
                format!(
                    "文本文件 '{}' 过大 ({:.1}MB)，最大支持 {:.0}MB",
                    attachment.name,
                    attachment.size as f64 / (1024.0 * 1024.0),
                    MAX_TEXT_SIZE as f64 / (1024.0 * 1024.0)
                ),
            ));
        }

        // 优先使用前端传入的 base64 数据
        let content = if let Some(ref data) = attachment.data {
            // 解码 base64 数据
            match BASE64.decode(data) {
                Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                Err(e) => {
                    return Err(CommandError::doc(
                        3012,
                        format!("附件 base64 解码失败 '{}': {}", attachment.name, e),
                    ));
                }
            }
        } else {
            // 从文件路径读取
            let file_path = Self::resolve_path(attachment, workspace_root)?;
            std::fs::read_to_string(&file_path).map_err(|e| {
                CommandError::fs(
                    6006,
                    format!("读取文本文件失败 '{}': {}", attachment.name, e),
                )
            })?
        };

        // 文本内容用标签包裹，方便 LLM 理解来源
        let text_with_context = format!(
            "<attachment name=\"{}\" type=\"{}\">\n{}\n</attachment>",
            attachment.name, attachment.mime_type, content
        );

        Ok(vec![ContentPart::Text {
            text: text_with_context,
        }])
    }

    /// 将 Sidecar 返回的结构化文档数据转换为可读文本
    fn format_document_content(
        doc_type: &str,
        data: &serde_json::Value,
        file_name: &str,
    ) -> String {
        match doc_type {
            "docx" => Self::format_docx_content(data),
            "xlsx" => Self::format_xlsx_content(data),
            "pptx" => Self::format_pptx_content(data),
            "pdf" => Self::format_pdf_content(data),
            _ => {
                // 未知文档类型，尝试直接提取文本
                log::warn!("未知文档类型: {}，尝试直接提取文本", doc_type);
                serde_json::to_string_pretty(data)
                    .unwrap_or_else(|_| format!("[无法格式化文档内容: {}]", file_name))
            }
        }
    }

    /// 格式化 Word 文档内容
    fn format_docx_content(data: &serde_json::Value) -> String {
        let mut text = String::new();

        // 文档属性
        if let Some(props) = data.get("properties") {
            if let Some(title) = props.get("title").and_then(|v| v.as_str()) {
                if !title.is_empty() {
                    text.push_str(&format!("标题: {}\n", title));
                }
            }
            if let Some(author) = props.get("author").and_then(|v| v.as_str()) {
                if !author.is_empty() {
                    text.push_str(&format!("作者: {}\n", author));
                }
            }
        }

        // 段落内容
        if let Some(paragraphs) = data.get("paragraphs").and_then(|v| v.as_array()) {
            for para in paragraphs {
                if let Some(para_text) = para.get("text").and_then(|v| v.as_str()) {
                    if !para_text.is_empty() {
                        // 根据样式添加标记
                        let style = para.get("style").and_then(|v| v.as_str()).unwrap_or("");
                        if style.starts_with("Heading") || style.starts_with("标题") {
                            text.push_str(&format!("## {}\n", para_text));
                        } else {
                            text.push_str(&format!("{}\n", para_text));
                        }
                    }
                }
            }
        }

        // 表格内容
        if let Some(tables) = data.get("tables").and_then(|v| v.as_array()) {
            for (i, table) in tables.iter().enumerate() {
                if let Some(rows) = table.as_array() {
                    text.push_str(&format!("\n--- 表格 {} ---\n", i + 1));
                    for row in rows {
                        if let Some(cells) = row.as_array() {
                            let cell_texts: Vec<String> = cells
                                .iter()
                                .map(|c| c.as_str().unwrap_or("").to_string())
                                .collect();
                            text.push_str(&format!("| {} |\n", cell_texts.join(" | ")));
                        }
                    }
                }
            }
        }

        if text.is_empty() {
            text.push_str("[文档为空或无可提取文本]");
        }

        text
    }

    /// 格式化 Excel 文档内容
    fn format_xlsx_content(data: &serde_json::Value) -> String {
        let mut text = String::new();

        // 工作表信息
        if let Some(sheets) = data.get("sheets").and_then(|v| v.as_array()) {
            for sheet in sheets {
                if let Some(name) = sheet.get("name").and_then(|v| v.as_str()) {
                    text.push_str(&format!("\n=== 工作表: {} ===\n", name));
                }
                // 行数据
                if let Some(rows) = sheet.get("data").and_then(|v| v.as_array()) {
                    for row in rows {
                        if let Some(cells) = row.as_array() {
                            let cell_texts: Vec<String> = cells
                                .iter()
                                .map(|c| {
                                    c.as_str()
                                        .map(|s| s.to_string())
                                        .or_else(|| c.as_f64().map(|n| n.to_string()))
                                        .or_else(|| c.as_i64().map(|n| n.to_string()))
                                        .or_else(|| c.as_bool().map(|b| b.to_string()))
                                        .unwrap_or_default()
                                })
                                .collect();
                            text.push_str(&format!("| {} |\n", cell_texts.join(" | ")));
                        }
                    }
                }
            }
        }

        // 旧格式兼容：直接 data 为行数组
        if text.is_empty() {
            if let Some(rows) = data.as_array() {
                for row in rows {
                    if let Some(cells) = row.as_array() {
                        let cell_texts: Vec<String> = cells
                            .iter()
                            .map(|c| {
                                c.as_str()
                                    .map(|s| s.to_string())
                                    .or_else(|| c.as_f64().map(|n| n.to_string()))
                                    .or_else(|| c.as_i64().map(|n| n.to_string()))
                                    .unwrap_or_default()
                            })
                            .collect();
                        text.push_str(&format!("| {} |\n", cell_texts.join(" | ")));
                    }
                }
            }
        }

        if text.is_empty() {
            text.push_str("[文档为空或无可提取数据]");
        }

        text
    }

    /// 格式化 PPT 文档内容
    fn format_pptx_content(data: &serde_json::Value) -> String {
        let mut text = String::new();

        // 幻灯片内容
        if let Some(slides) = data.get("slides").and_then(|v| v.as_array()) {
            for (i, slide) in slides.iter().enumerate() {
                text.push_str(&format!("\n=== 幻灯片 {} ===\n", i + 1));
                // 文本框内容
                if let Some(texts) = slide.get("texts").and_then(|v| v.as_array()) {
                    for t in texts {
                        if let Some(s) = t.as_str() {
                            if !s.is_empty() {
                                text.push_str(&format!("{}\n", s));
                            }
                        }
                    }
                }
                // 兼容：shapes 格式
                if let Some(shapes) = slide.get("shapes").and_then(|v| v.as_array()) {
                    for shape in shapes {
                        if let Some(shape_text) = shape.get("text").and_then(|v| v.as_str()) {
                            if !shape_text.is_empty() {
                                text.push_str(&format!("{}\n", shape_text));
                            }
                        }
                    }
                }
            }
        }

        if text.is_empty() {
            text.push_str("[文档为空或无可提取文本]");
        }

        text
    }

    /// 格式化 PDF 文档内容
    fn format_pdf_content(data: &serde_json::Value) -> String {
        let mut text = String::new();

        // PDF 文档属性
        if let Some(metadata) = data.get("metadata") {
            if let Some(title) = metadata.get("title").and_then(|v| v.as_str()) {
                if !title.is_empty() {
                    text.push_str(&format!("标题: {}\n", title));
                }
            }
            if let Some(author) = metadata.get("author").and_then(|v| v.as_str()) {
                if !author.is_empty() {
                    text.push_str(&format!("作者: {}\n", author));
                }
            }
        }

        // 页面内容
        if let Some(pages) = data.get("pages").and_then(|v| v.as_array()) {
            for (i, page) in pages.iter().enumerate() {
                if let Some(page_text) = page.get("text").and_then(|v| v.as_str()) {
                    if !page_text.is_empty() {
                        text.push_str(&format!("\n--- 第 {} 页 ---\n{}\n", i + 1, page_text));
                    }
                }
                // 兼容：content 字段
                if let Some(content) = page.get("content").and_then(|v| v.as_str()) {
                    if !content.is_empty() {
                        text.push_str(&format!("\n--- 第 {} 页 ---\n{}\n", i + 1, content));
                    }
                }
            }
        }

        // 兼容：直接 text 字段
        if text.is_empty() {
            if let Some(full_text) = data.get("text").and_then(|v| v.as_str()) {
                if !full_text.is_empty() {
                    text.push_str(full_text);
                }
            }
        }

        if text.is_empty() {
            text.push_str("[文档为空或无可提取文本]");
        }

        text
    }

    /// 根据 MIME 类型和文件名推断 Sidecar doc_type
    fn infer_doc_type(mime_type: &str, file_name: &str) -> Option<&'static str> {
        // 优先根据 MIME 类型推断
        for (mime, doc_type) in DOCUMENT_MIME_TO_DOCTYPE {
            if mime_type == *mime {
                return Some(doc_type);
            }
        }

        // 回退：根据文件扩展名推断
        let lower_name = file_name.to_lowercase();
        for (ext, doc_type) in DOCUMENT_EXT_TO_DOCTYPE {
            if lower_name.ends_with(ext) {
                return Some(doc_type);
            }
        }

        None
    }

    /// 解析附件的文件路径
    /// 优先使用绝对路径，否则拼接工作区根目录 + 相对路径
    /// 包含路径遍历安全校验，与 Tool 系统使用相同的 canonicalize + starts_with 模式
    fn resolve_path(
        attachment: &AttachmentMeta,
        workspace_root: &str,
    ) -> Result<std::path::PathBuf, CommandError> {
        let candidate = if let Some(ref abs_path) = attachment.absolute_path {
            std::path::PathBuf::from(abs_path)
        } else if let Some(ref rel_path) = attachment.path {
            std::path::PathBuf::from(workspace_root).join(rel_path)
        } else {
            return Err(CommandError::fs(
                6001,
                format!("附件缺少文件路径: {}", attachment.name),
            ));
        };

        // 路径遍历安全校验：确保解析后的路径在工作区内
        if !workspace_root.is_empty() {
            let canonical_file = match crate::utils::canonicalize(&candidate) {
                Ok(p) => p,
                Err(_) => {
                    return Err(CommandError::fs(
                        6001,
                        format!("附件文件不存在或路径无效: {}", attachment.name),
                    ));
                }
            };
            let canonical_root =
                match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                    Ok(p) => p,
                    Err(_) => {
                        return Err(CommandError::fs(6001, "工作区根目录路径无效".to_string()));
                    }
                };
            if !canonical_file.starts_with(&canonical_root) {
                return Err(CommandError::fs(
                    6001,
                    format!("附件路径不在工作区内，拒绝访问: {}", attachment.name),
                ));
            }
        }

        if !candidate.exists() {
            return Err(CommandError::fs(
                6001,
                format!("附件文件不存在: {}", attachment.name),
            ));
        }

        Ok(candidate)
    }

    /// 根据 MIME 类型推断附件类型
    pub fn infer_attachment_type(mime_type: &str) -> AttachmentType {
        if mime_type.starts_with("image/") {
            return AttachmentType::Image;
        }
        if SUPPORTED_TEXT_MIME_TYPES.contains(&mime_type) {
            return AttachmentType::Text;
        }
        // 常见文档格式
        if mime_type.contains("pdf")
            || mime_type.contains("word")
            || mime_type.contains("excel")
            || mime_type.contains("spreadsheet")
            || mime_type.contains("presentation")
            || mime_type.contains("document")
        {
            return AttachmentType::Document;
        }
        AttachmentType::Text
    }

    /// 检查附件是否包含图片
    pub fn has_image_attachments(attachments: &[AttachmentMeta]) -> bool {
        attachments
            .iter()
            .any(|a| matches!(a.attachment_type, AttachmentType::Image))
    }
}
