//! 文档设计参考模块
//! 为 Agent 通过 code_interpreter_handler 编写文档生成代码时提供设计参考
//! 包含配色方案、字体规范、页面尺寸等专业设计信息

/// Word 文档设计参考
pub const WORD_DESIGN_GUIDE: &str = r#"
## Word 文档设计参考

以下为文档设计参考，供 code_interpreter_handler 编写代码时使用。可用 helper: create_word_doc(), save_word_doc()。

### 专业配色方案
- 标题1: 深蓝色 (#1F4E79) 22pt 粗体
- 标题2: 中蓝色 (#2E75B6) 16pt 粗体
- 标题3: 浅蓝色 (#5B9BD5) 14pt 粗体
- 表头: 蓝色背景 (#D6E4F0) + 深蓝色粗体文字
- 表格交替行: 浅蓝色背景 (#EDF2F9)
- 字体: 拉丁字体 Arial，东亚字体 微软雅黑
- 页边距: 2.54cm（1 inch）四边

### 页面尺寸（DXA 单位，1440 DXA = 1 inch）
- US Letter: 12240 x 15840 DXA
- A4: 11906 x 16838 DXA（默认）

### 表格规范
- 表头应用蓝色背景和粗体样式
- 数据行应用交替行颜色
- 边框使用蓝灰色 (#B4C6E7)

### 代码块
- 使用 Consolas 等宽字体 + 浅灰色背景
"#;

/// Excel 文档设计参考
pub const EXCEL_DESIGN_GUIDE: &str = r#"
## Excel 文档设计参考

以下为文档设计参考，供 code_interpreter_handler 编写代码时使用。可用 helper: create_excel_doc(), save_excel_doc()。

### 专业配色方案
- 表头: 蓝色背景 (#D6E4F0) + 深蓝色粗体文字 (#1F4E79)
- 交替行: 浅蓝色背景 (#EDF2F9)
- 边框: 蓝灰色 (#B4C6E7)
- 标题行: 深蓝色粗体 (#1F4E79) 16pt + 浅蓝色背景 (#F2F7FB)
- 字体: 微软雅黑 11pt

### 核心原则：使用公式而非硬编码值
- 错误: 在 Python 中计算 sum, 然后硬编码结果
- 正确: 使用 Excel 公式 =SUM(B2:B9)

### 数字格式标准
- 年份: 格式化为文本字符串（"2024" 而非 "2,024"），使用 @ 格式码
- 货币: 使用 $#,##0 格式
- 零值: 使用数字格式将零显示为 "-"
- 百分比: 默认 0.0% 格式
- 负数: 使用括号 (123) 而非减号 -123

### 颜色编码标准
- 蓝色文字 (RGB: 0,0,255): 硬编码输入值
- 黑色文字 (RGB: 0,0,0): 所有公式和计算
- 绿色文字 (RGB: 0,128,0): 跨工作表引用
- 红色文字 (RGB: 255,0,0): 外部文件链接
"#;

/// PPT 文档设计参考
pub const PPT_DESIGN_GUIDE: &str = r#"
## PPT 文档设计参考

以下为文档设计参考，供 code_interpreter_handler 编写代码时使用。可用 helper: create_ppt_doc(), save_ppt_doc()。

### 专业配色方案
| 方案 | 主色 | 辅色 | 强调色 |
|------|------|------|--------|
| ocean | #065A82 (deep blue) | #1C7293 (teal) | #21295C (midnight) |
| midnight | #1E2761 (navy) | #CADCFC (ice blue) | #FFFFFF (white) |
| forest | #2C5F2D (forest) | #97BC62 (moss) | #F5F5F5 (cream) |
| coral | #F96167 (coral) | #F9E795 (gold) | #2F3C7E (navy) |
| charcoal | #36454F (charcoal) | #F2F2F2 (off-white) | #212121 (black) |

### 字体规范
- 拉丁字体: Calibri，东亚字体: 微软雅黑
- 幻灯片标题: 36-44pt 粗体
- 节标题: 20-24pt 粗体
- 正文: 14-16pt

### 间距规范
- 最小边距: 0.5 inch
- 内容块间距: 0.3-0.5 inch
"#;

/// PDF 文档设计参考
pub const PDF_DESIGN_GUIDE: &str = r#"
## PDF 文档设计参考

以下为文档设计参考，供 code_interpreter_handler 编写代码时使用。可用 helper: create_pdf_doc(), save_pdf_doc()。

### 专业配色方案
- 标题1: 深蓝色 (#1F4E79) 20pt
- 标题2: 中蓝色 (#2E75B6) 16pt
- 标题3: 浅蓝色 (#5B9BD5) 14pt
- 表头: 蓝色背景 (#2E75B6) + 白色粗体文字
- 表格交替行: 浅蓝色背景 (#D6E4F0)
- 代码块: 等宽字体 + 浅灰色背景 (#F5F5F5)
- 字体: 微软雅黑

### 表格规范
- 表头应用蓝色背景和白色粗体样式
- 数据行应用交替行颜色
"#;

/// 获取所有文档设计参考, 拼接为完整字符串
pub fn get_all_design_guides() -> String {
    format!(
        "{}\n\n{}\n\n{}\n\n{}",
        WORD_DESIGN_GUIDE,
        EXCEL_DESIGN_GUIDE,
        PPT_DESIGN_GUIDE,
        PDF_DESIGN_GUIDE,
    )
}

/// 根据文档类型获取对应的设计参考
pub fn get_design_guide_by_type(doc_type: &str) -> &'static str {
    match doc_type {
        "docx" => WORD_DESIGN_GUIDE,
        "xlsx" => EXCEL_DESIGN_GUIDE,
        "pptx" => PPT_DESIGN_GUIDE,
        "pdf" => PDF_DESIGN_GUIDE,
        _ => "",
    }
}
