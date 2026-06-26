//! 任务类型识别模块
//! 根据用户消息关键词和已调用工具推断当前任务类型，
//! 用于按需注入文档设计规范和匹配示例
//! 新体系按文档格式划分，不再区分操作类型（生成/读取/修改等）

/// 任务类型枚举
/// 按文档格式划分，每种格式对应一个 Handler（docx_handler/xlsx_handler/pptx_handler/pdf_handler）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskType {
    /// Word 文档（.docx）
    Docx,
    /// Excel 文档（.xlsx）
    Xlsx,
    /// PPT 文档（.pptx）
    Pptx,
    /// PDF 文档（.pdf）
    Pdf,
    /// Markdown 文档（.md）
    Markdown,
    /// 纯文件系统操作
    FileSystem,
    /// 通用问答
    General,
    /// 无法判断
    Unknown,
}

impl TaskType {
    /// 根据用户消息内容识别任务类型
    /// 基于文件扩展名和关键词匹配策略，只区分文档格式，不区分操作类型
    pub fn from_user_message(message: &str) -> Self {
        let msg = message.to_lowercase();

        // 文件系统操作关键词（优先匹配，避免"搜索文件"被误判为文档格式）
        if contains_any(&msg, &["列出", "搜索", "查找文件", "文件列表", "目录", "创建文件夹", "删除文件", "文件是否存在"])
        {
            return TaskType::FileSystem;
        }

        // 收集消息中所有格式标识（扩展名和关键词）及其位置
        // 当多个格式标识出现时，使用最后出现的那个（通常是目标格式）
        let format_indicators: [(&str, TaskType); 12] = [
            (".docx", TaskType::Docx),
            (".xlsx", TaskType::Xlsx),
            (".xlsm", TaskType::Xlsx),
            (".csv", TaskType::Xlsx),
            (".pptx", TaskType::Pptx),
            (".pdf", TaskType::Pdf),
            (".md", TaskType::Markdown),
            // 格式关键词（无点号前缀），用于匹配"转成PDF"等场景
            ("pdf", TaskType::Pdf),
            ("xlsx", TaskType::Xlsx),
            ("pptx", TaskType::Pptx),
            ("docx", TaskType::Docx),
            ("word", TaskType::Docx),
        ];

        let mut best_match: Option<TaskType> = None;
        let mut best_pos = 0;
        for (indicator, task_type) in &format_indicators {
            if let Some(pos) = msg.find(indicator) {
                if pos >= best_pos {
                    best_pos = pos;
                    best_match = Some(task_type.clone());
                }
            }
        }
        if let Some(task_type) = best_match {
            return task_type;
        }

        // 按中文关键词匹配文档格式（格式标识已处理英文关键词）
        // Word 文档中文关键词
        if contains_any(&msg, &["文档", "报告", "合同", "周报", "月报", "纪要", "方案", "信函", "通知"])
            && !contains_any(&msg, &["excel", "表格", "ppt", "幻灯片", "pdf"])
        {
            return TaskType::Docx;
        }

        // Excel 文档中文关键词
        if contains_any(&msg, &["电子表格", "表格", "数据表", "报表"])
        {
            return TaskType::Xlsx;
        }

        // PPT 文档中文关键词
        if contains_any(&msg, &["幻灯片", "演示文稿", "演示"])
        {
            return TaskType::Pptx;
        }

        // Markdown 文档中文关键词
        if contains_any(&msg, &["markdown", "md文件"])
        {
            return TaskType::Markdown;
        }

        TaskType::Unknown
    }

    /// 根据已调用的工具/Handler 名称推断任务类型
    /// 用于后续迭代中修正任务类型判断
    pub fn from_tool_name(tool_name: &str) -> Self {
        match tool_name {
            "docx_handler" => TaskType::Docx,
            "xlsx_handler" => TaskType::Xlsx,
            "pptx_handler" => TaskType::Pptx,
            "pdf_handler" => TaskType::Pdf,
            "list_directory" | "search_files" | "read_file" | "file_info"
            | "file_exists" | "delete_file" | "create_directory" | "write_text_file" => TaskType::FileSystem,
            _ => TaskType::Unknown,
        }
    }

    /// 根据文档格式字符串确定任务类型
    pub fn from_document_format(format: &str) -> Self {
        match format {
            "docx" => TaskType::Docx,
            "xlsx" => TaskType::Xlsx,
            "pptx" => TaskType::Pptx,
            "pdf" => TaskType::Pdf,
            "md" => TaskType::Markdown,
            _ => TaskType::Unknown,
        }
    }

    /// 获取此任务类型需要注入的文档设计规范类型列表
    /// 返回 doc_type 字符串列表，用于 get_design_guide_by_type()
    pub fn required_guide_types(&self) -> Vec<&'static str> {
        match self {
            TaskType::Docx => vec!["docx"],
            TaskType::Xlsx => vec!["xlsx"],
            TaskType::Pptx => vec!["pptx"],
            TaskType::Pdf => vec!["pdf"],
            TaskType::Markdown => vec![],
            TaskType::FileSystem => vec![],
            TaskType::General => vec![],
            TaskType::Unknown => vec![], // 未知类型不注入设计规范，避免浪费 Token 和误导 LLM
        }
    }

}

/// 检查字符串是否包含任一关键词
fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|kw| text.contains(kw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_user_message_docx() {
        assert_eq!(TaskType::from_user_message("帮我创建一份项目周报"), TaskType::Docx);
        assert_eq!(TaskType::from_user_message("生成Word文档"), TaskType::Docx);
        assert_eq!(TaskType::from_user_message("写一个合同文档"), TaskType::Docx);
        assert_eq!(TaskType::from_user_message("修改报告.docx的内容"), TaskType::Docx);
        assert_eq!(TaskType::from_user_message("读取data.docx的内容"), TaskType::Docx);
    }

    #[test]
    fn test_from_user_message_xlsx() {
        assert_eq!(TaskType::from_user_message("创建一个Excel数据表"), TaskType::Xlsx);
        assert_eq!(TaskType::from_user_message("制作报表"), TaskType::Xlsx);
        assert_eq!(TaskType::from_user_message("生成电子表格"), TaskType::Xlsx);
        assert_eq!(TaskType::from_user_message("读取data.xlsx的内容"), TaskType::Xlsx);
    }

    #[test]
    fn test_from_user_message_pptx() {
        assert_eq!(TaskType::from_user_message("创建PPT演示文稿"), TaskType::Pptx);
        assert_eq!(TaskType::from_user_message("制作幻灯片"), TaskType::Pptx);
        assert_eq!(TaskType::from_user_message("修改演示.pptx"), TaskType::Pptx);
    }

    #[test]
    fn test_from_user_message_pdf() {
        assert_eq!(TaskType::from_user_message("生成PDF文件"), TaskType::Pdf);
        assert_eq!(TaskType::from_user_message("把方案.docx转成PDF"), TaskType::Pdf);
        assert_eq!(TaskType::from_user_message("读取report.pdf"), TaskType::Pdf);
    }

    #[test]
    fn test_from_user_message_markdown() {
        assert_eq!(TaskType::from_user_message("创建markdown文件"), TaskType::Markdown);
        assert_eq!(TaskType::from_user_message("写一个.md文件"), TaskType::Markdown);
    }

    #[test]
    fn test_from_user_message_filesystem() {
        assert_eq!(TaskType::from_user_message("列出目录内容"), TaskType::FileSystem);
        assert_eq!(TaskType::from_user_message("搜索文件"), TaskType::FileSystem);
    }

    #[test]
    fn test_from_user_message_unknown() {
        assert_eq!(TaskType::from_user_message("你好"), TaskType::Unknown);
        assert_eq!(TaskType::from_user_message("什么是DocAgent"), TaskType::Unknown);
    }

    #[test]
    fn test_from_tool_name() {
        assert_eq!(TaskType::from_tool_name("docx_handler"), TaskType::Docx);
        assert_eq!(TaskType::from_tool_name("xlsx_handler"), TaskType::Xlsx);
        assert_eq!(TaskType::from_tool_name("pptx_handler"), TaskType::Pptx);
        assert_eq!(TaskType::from_tool_name("pdf_handler"), TaskType::Pdf);
        assert_eq!(TaskType::from_tool_name("list_directory"), TaskType::FileSystem);
        assert_eq!(TaskType::from_tool_name("unknown_tool"), TaskType::Unknown);
    }

    #[test]
    fn test_from_document_format() {
        assert_eq!(TaskType::from_document_format("docx"), TaskType::Docx);
        assert_eq!(TaskType::from_document_format("xlsx"), TaskType::Xlsx);
        assert_eq!(TaskType::from_document_format("pptx"), TaskType::Pptx);
        assert_eq!(TaskType::from_document_format("pdf"), TaskType::Pdf);
        assert_eq!(TaskType::from_document_format("md"), TaskType::Markdown);
        assert_eq!(TaskType::from_document_format("unknown"), TaskType::Unknown);
    }

    #[test]
    fn test_required_guide_types() {
        assert_eq!(TaskType::Docx.required_guide_types(), vec!["docx"]);
        assert_eq!(TaskType::Xlsx.required_guide_types(), vec!["xlsx"]);
        assert_eq!(TaskType::Pptx.required_guide_types(), vec!["pptx"]);
        assert_eq!(TaskType::Pdf.required_guide_types(), vec!["pdf"]);
        assert_eq!(TaskType::Markdown.required_guide_types(), Vec::<&str>::new());
        assert_eq!(TaskType::FileSystem.required_guide_types(), Vec::<&str>::new());
        assert_eq!(TaskType::Unknown.required_guide_types(), Vec::<&str>::new());
    }

    #[test]
    fn test_extension_priority_over_keywords() {
        // 文件扩展名优先于关键词匹配
        assert_eq!(TaskType::from_user_message("读取data.xlsx的内容"), TaskType::Xlsx);
        assert_eq!(TaskType::from_user_message("修改演示.pptx"), TaskType::Pptx);
    }
}
