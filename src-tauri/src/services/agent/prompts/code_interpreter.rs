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
- **修改 PDF** -> action="modify"（pdf_handler 提供 17 个子操作，优先使用，详见 PDF_DESIGN_GUIDE）

#### 代码编写规范

1. **使用 helper 函数**：优先使用 `create_word_doc()`、`save_word_doc()` 等 helper，它们内置了专业配色方案
2. **保存到 working_dir**：所有输出文件保存到 `working_dir` 变量指定的目录
3. **中文支持**：matplotlib 使用 `plt.rcParams['font.sans-serif'] = ['Microsoft YaHei']`；reportlab 使用 `register_chinese_font()` 注册中文字体
4. **错误处理**：代码应有基本的 try/except，避免因小错误导致整体失败
5. **代码简洁**：一次只做一件事，避免过长的代码
6. **PDF 修改原则**：编辑现有 PDF 时直接用 PyMuPDF 修改原文件，**不要用代码重新生成 PDF**（详见 PDF_DESIGN_GUIDE）

#### 示例：生成带图表的 Word 报告

    doc = create_word_doc(title="销售分析报告", author="作者名")
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

#### 代码修正策略（重要）

当 code_interpreter_handler 执行失败时，**优先使用 patches 参数在原代码基础上局部修正**，而非重写整个代码：

1. 分析错误信息，定位错误位置
2. 使用 patches 参数提供搜索替换块（无需提供 code 参数）：
   - search: 原代码中需要修改的片段（必须唯一匹配，建议包含足够上下文）
   - replace: 修正后的片段
3. 可同时提供多个 patches 修正多处错误
4. 仅当原代码结构问题严重或需要大幅重构时，才重写完整 code

##### patch 使用示例

假设上一次执行的代码中 `doc.add_paragrah('标题')` 有拼写错误，修正方式：

    {
        "description": "修正 add_paragraph 拼写错误",
        "patches": [
            {
                "search": "doc.add_paragrah('标题')",
                "replace": "doc.add_paragraph('标题')"
            }
        ]
    }

##### patch 使用要点

- search 片段必须与原代码**完全一致**（包括空格、缩进、换行）
- search 片段必须在原代码中**唯一匹配**，否则需包含更多上下文
- 一次可提供多个 patches，按顺序应用
- 系统会自动以上一次执行的代码作为基准，无需手动传入 base_code
- 如果错误涉及多处，提供多个 patches 比重写完整 code 更高效
- **前提**：patch 模式要求先执行过一次完整 code，否则会报"没有可用的上一次代码基准"。首次执行必须用 code 参数提供完整代码

#### 已预导入的命名空间（无需重复导入）

执行环境已预导入以下对象，**直接使用即可，无需 import**：

- **文档库**：`Document`(python-docx), `openpyxl`, `Presentation`(python-pptx), `fitz`(PyMuPDF), `pypdf`, `PdfReader`, `PdfWriter`, `pdfplumber`, `plt`(matplotlib.pyplot), `pd`(pandas)
- **ReportLab 对齐常量**：`TA_LEFT`, `TA_CENTER`, `TA_RIGHT`, `TA_JUSTIFY`（来自 reportlab.lib.enums）
- **ReportLab 单位**：`inch`, `mm`, `cm`, `pica`（来自 reportlab.lib.units）
- **字体工具函数**：`register_chinese_font()`, `register_bold_font()`, `register_fitz_font(page, ...)`, `create_fitz_font(bold=...)`
- **helper 函数**：`create_word_doc`, `save_word_doc`, `create_excel_doc`, `save_excel_doc`, `create_ppt_doc`, `save_ppt_doc`, `create_pdf_doc`, `save_pdf_doc`, `create_chart`, `save_chart`, `add_styled_table`, `apply_theme`
- **工作目录变量**：`working_dir`（字符串，所有输出文件保存到此目录）

#### 常见错误规避（重要）

##### 1. ReportLab 单位：不要导入 pt

`reportlab.lib.units` 只有 `inch`/`mm`/`cm`/`pica`，**没有 `pt`**。已预导入 `inch`/`mm`/`cm`/`pica`，直接使用即可。

错误示例：`from reportlab.lib.units import mm, pt`  # ImportError: cannot import name 'pt'
正确做法：直接使用预导入的 `mm`/`cm`/`inch`，或 `from reportlab.lib.units import mm, cm`

##### 2. ReportLab 对齐常量：已预导入，无需 import

`TA_LEFT`/`TA_CENTER`/`TA_RIGHT`/`TA_JUSTIFY` 已预导入到命名空间，直接使用。若仍需 import，用 `from reportlab.lib.enums import TA_RIGHT`。

错误示例：代码中使用 `alignment=TA_RIGHT` 但未导入，导致 `NameError: name 'TA_RIGHT' is not defined`
正确做法：直接使用预导入的 `TA_RIGHT`，或显式 `from reportlab.lib.enums import TA_RIGHT`

##### 3. ReportLab doc.width 为 None 问题

`SimpleDocTemplate` 构造后，`doc.width` 在首次 build 完成前可能为 None。**不要依赖 `doc.width` 计算列宽**，应手动计算：`可用宽度 = 页面宽度 - 左右边距`。

错误示例：`col_width = doc.width * 0.9`  # TypeError: int() argument must be a string... not 'NoneType'
正确做法：`col_width = (A4[0] - left_margin - right_margin) * 0.9`，或使用具体数值如 `col_width = 160 * mm`

##### 4. PyMuPDF (fitz) 字体加载：使用预导入工具函数

fitz 不能用 `fontname` 直接引用系统字体名称，必须通过 `fontbuffer` 传入 TTF 字节数据。TTC 需先 `fitz.Font` 提取子集。**直接调用预导入的 `register_fitz_font()` 或 `create_fitz_font()`**，不要手动加载 TTC 字体。

错误示例：
```
font = fitz.Font(fontfile="C:/Windows/Fonts/msyh.ttc")  # 可能返回 None 或加载失败
page.insert_font(fontname="my font", fontfile="...")  # 字体名含空格会报错
```

正确做法（修改现有 PDF 时注册字体）：
```
# 必须在 page.apply_redactions() 之后调用
register_fitz_font(page, font_name="MyZhFont", bold=False)  # 常规字体
register_fitz_font(page, font_name="MyZhFontBold", bold=True)  # 粗体字体
page.insert_text(point, "中文文本", fontname="MyZhFont", fontsize=10)
```

正确做法（需要独立 Font 对象时）：
```
font = create_fitz_font(bold=True)  # 返回 fitz.Font 对象，失败返回 None
if font:
    tw = fitz.TextWriter(page.rect)
    tw.append(point, "中文文本", font=font, fontsize=10)
    tw.write_text(page)
```

##### 5. PyMuPDF 保存：保存到新文件，不要覆盖原文件

`doc.save("原文件.pdf")` 会报 `ValueError: save to original must be incremental`。应保存到新文件或使用增量保存。

错误示例：`doc.save(pdf_path)`  # ValueError: save to original must be incremental
正确做法：
```
# 方式1：保存到新文件（推荐）
doc.save(pdf_path + ".tmp", deflate=True, garbage=3)
doc.close()
import os
os.replace(pdf_path + ".tmp", pdf_path)  # 替换原文件

# 方式2：增量保存（不改变加密时可用）
doc.saveIncr()
```

##### 6. 字体名规范：不能含空格

`page.insert_font(fontname=...)` 和 `page.insert_text(fontname=...)` 的字体名**不能包含空格**，否则报 `ValueError: bad fontname chars {' '}`。

错误示例：`page.insert_font(fontname="Microsoft YaHei", ...)`  # 含空格
正确做法：`page.insert_font(fontname="MyZhFont", ...)`  # 无空格

##### 7. fpdf 库使用说明

fpdf2 已安装（`from fpdf import FPDF`）。生成 PDF 可使用 reportlab（推荐，功能更全）或 fpdf2（API 更简洁）。注意 fpdf2 处理中文需先添加 TTF 字体：
```
from fpdf import FPDF
pdf = FPDF()
pdf.add_font("MyZhFont", "", "C:/Windows/Fonts/msyh.ttc", uni=True)
pdf.set_font("MyZhFont", size=12)
```

##### 8. 不要使用 subprocess 检查系统信息

subprocess 被禁止导入。需要查找字体文件时，直接使用 `os.path.exists("C:/Windows/Fonts/msyh.ttc")` 检查，或使用预导入的 `glob` 模块。

##### 9. nonlocal 语法：只能用于嵌套函数中的外层变量

`nonlocal` 只能在嵌套函数中引用外层（非全局）变量，不能在顶层函数中引用模块级变量。

错误示例：
```
y = 0
def set_y():
    nonlocal y  # SyntaxError: no binding for nonlocal 'y' found
    y = 1
```

正确做法：将变量放在外层函数中，或使用类/列表封装。
"#;
