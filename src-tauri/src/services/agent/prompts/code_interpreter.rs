//! Code Interpreter 使用指导
//! 为 Agent 提供文档生成与修改的代码解释器使用规范

/// Code Interpreter 使用指导（将集成到 tool_strategy 层）
pub const CODE_INTERPRETER_GUIDE: &str = r#"
### 文档生成与修改 -> code_interpreter_handler

所有文档的**生成**和**修改**操作都通过 `code_interpreter_handler` 完成，编写 Python 代码执行。

#### 何时使用 code_interpreter_handler
- 生成任何文档（Word/Excel/PPT/PDF）
- 修改任何文档（调整样式、添加内容、替换文本等）
- 需要图表（matplotlib）
- 需要数据处理（pandas）
- 需要自定义排版
- 需要计算后生成报告

#### 何时使用文档 Handler（docx_handler/xlsx_handler/pptx_handler/pdf_handler）
- 读取文档内容 -> action="read"
- 格式转换 -> action="convert"
- 文档分析统计 -> action="analyze"

#### 代码编写规范

1. **使用 helper 函数**：优先使用 `create_word_doc()`、`save_word_doc()` 等 helper，它们内置了专业配色方案
2. **保存到 working_dir**：所有输出文件保存到 `working_dir` 变量指定的目录
3. **中文支持**：matplotlib 使用 `plt.rcParams['font.sans-serif'] = ['Microsoft YaHei']`
4. **错误处理**：代码应有基本的 try/except，避免因小错误导致整体失败
5. **代码简洁**：一次只做一件事，避免过长的代码

#### 示例：生成带图表的 Word 报告

    doc = create_word_doc(title="销售分析报告", author="DocAgent")
    doc.add_heading('季度销售概览', level=1)
    doc.add_paragraph('本报告分析了2024年各季度的销售数据。')
    chart_path = create_chart(
        chart_type="bar",
        data={"x": ["Q1", "Q2", "Q3", "Q4"], "y": [120, 150, 135, 180]},
        title="季度销售额（万元）",
        filename="sales_chart.png",
        working_dir=working_dir
    )
    doc.add_picture(chart_path, width=Inches(5))
    save_word_doc(doc, "销售分析报告.docx", working_dir=working_dir)

#### 示例：修改现有文档

    from docx import Document
    doc = Document(working_dir + "/报告.docx")
    # 修改标题
    doc.paragraphs[0].runs[0].text = "2024年度销售分析报告"
    # 添加新章节
    doc.add_heading('结论与建议', level=1)
    doc.add_paragraph('基于以上分析，我们建议...')
    doc.save(working_dir + "/报告.docx")
"#;
