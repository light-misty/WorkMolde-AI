//! 文档设计参考模块
//! 为 Agent 生成文档时提供设计参考
//! 包含配色方案、字体规范、页面尺寸等专业设计信息

/// Word 文档设计参考
pub const WORD_DESIGN_GUIDE: &str = r#"
## Word 文档设计参考

以下为文档设计参考。

### 专业配色方案
- 标题1: 深蓝色 (#1F4E79) 22pt 粗体
- 标题2: 中蓝色 (#2E75B6) 16pt 粗体
- 标题3: 浅蓝色 (#5B9BD5) 14pt 粗体
- 表头: 蓝色背景 (#D6E4F0) + 深蓝色粗体文字
- 表格交替行: 浅蓝色背景 (#EDF2F9)
- 字体: 拉丁字体 Arial，东亚字体 微软雅黑
- 页边距: 2.54cm（1 inch）四边

### 页面尺寸
- US Letter: 12240 x 15840（单位 DXA，1440 DXA = 1 inch）
- A4: 11906 x 16838 DXA（默认）

**重要：不要导入 `Dxa`！** `from docx.shared import Dxa` 会触发 `ImportError`，因为 `docx.shared` 中没有 `Dxa` 类。正确导入方式：
```python
from docx.shared import Inches, Cm, Pt, Emu
```
页面尺寸直接使用 `Inches` 或 `Cm` 换算：`Inches(8.5)` = Letter 宽，`Cm(21)` = A4 宽。

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

以下为文档设计参考。

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

以下为文档设计参考。

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

**核心原则**：
- 生成新 PDF → 用 reportlab（Platypus 用于结构化文档，canvas 用于精确控制）
- 修改已有 PDF → 用 PyMuPDF(fitz) 直接修改原文件，**不要重新生成**
- 读取 PDF 视觉布局 → 用 pdf_handler 的 `include_visual` 开关

可用 helper: create_pdf_doc(), save_pdf_doc(), register_chinese_font(), register_bold_font()。
可用 PDF 库: fitz(PyMuPDF), pypdf, pdfplumber, reportlab, fpdf。

### 1. PyMuPDF 常见陷阱（重要，务必先读）

1. **insert_text 用 `fontname` 不是 `font`**：
   `page.insert_text((50,50), "文字", fontname="china-s", fontsize=12)`

2. **不能直接覆盖原文件**（报错 "save to original must be incremental"）：
   - 保存到新文件：`doc.save("output.pdf")`
   - 增量保存：`doc.saveIncr()`
   - 替换原文件：`doc.save("temp.pdf"); doc.close(); shutil.move("temp.pdf", "input.pdf")`

3. **加密时不能 incremental**：修改加密必须保存到新文件

4. **CJK 字体内置名**（区分大小写）：`china-s`(简体) / `china-t`(繁体) / `japan` / `korea`

5. **坐标系差异**：PyMuPDF 原点左上角 y 向下；reportlab 原点左下角 y 向上

### 2. 页面尺寸与坐标系
- reportlab 默认 A4 = 595.27 x 841.89 pt（宽 x 高），原点左下角，y 向上
- 常见尺寸（pt）：A4=(595,842)、A3=(842,1191)、Letter=(612,792)、Legal=(612,1008)
- 自定义页面：`SimpleDocTemplate(path, pagesize=(w, h))` 或 `canvas.Canvas(path, pagesize=(w, h))`
- 单位换算：1 inch = 72 pt = 25.4 mm

### 3. 中文字体注册（关键步骤）
reportlab 默认字体不含中文，必须先注册：
```python
# 方式一：使用内置 helper（推荐）
from handlers.font_utils import register_chinese_font, register_bold_font
register_chinese_font()   # 注册后使用 'ChineseFont' 作为字体名
register_bold_font()      # 注册后使用 'ChineseFontBold'

# 方式二：手动注册 TTF/TTC
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
pdfmetrics.registerFont(TTFont('MyChinese', 'C:/Windows/Fonts/msyh.ttc'))
# 注意：TTC 集合文件需指定 subfontIndex：TTFont('MyChinese', path, subfontIndex=0)
```

### 4. 生成 PDF（reportlab Platypus - 推荐用于结构化文档）
```python
from reportlab.lib.pagesizes import A4
from reportlab.lib.units import cm
from reportlab.lib import colors
from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, Table, TableStyle
from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle

register_chinese_font()

doc = SimpleDocTemplate("output.pdf", pagesize=A4,
                        leftMargin=2*cm, rightMargin=2*cm,
                        topMargin=2*cm, bottomMargin=2*cm)

styles = getSampleStyleSheet()
title_style = ParagraphStyle('Title', parent=styles['Title'],
                             fontName='ChineseFont', fontSize=20,
                             textColor=colors.HexColor('#1F4E79'))
body_style = ParagraphStyle('Body', parent=styles['Normal'],
                            fontName='ChineseFont', fontSize=11, leading=18)

story = [
    Paragraph("文档标题", title_style),
    Spacer(1, 0.5*cm),
    Paragraph("正文内容，支持 <b>加粗</b>、<i>斜体</i>、<br/>换行", body_style),
]

# 表格（表头蓝色背景 + 交替行颜色）
data = [['姓名', '年龄', '城市'], ['张三', '25', '北京'], ['李四', '30', '上海']]
table = Table(data, colWidths=[3*cm, 2*cm, 3*cm])
table.setStyle(TableStyle([
    ('FONTNAME', (0,0), (-1,-1), 'ChineseFont'),
    ('BACKGROUND', (0,0), (-1,0), colors.HexColor('#2E75B6')),
    ('TEXTCOLOR', (0,0), (-1,0), colors.white),
    ('ROWBACKGROUNDS', (0,1), (-1,-1), [colors.white, colors.HexColor('#D6E4F0')]),
    ('GRID', (0,0), (-1,-1), 0.5, colors.HexColor('#BFBFBF')),
]))
story.append(table)

doc.build(story)  # 自动分页
```

### 5. 生成 PDF（canvas - 精确控制坐标）
```python
from reportlab.pdfgen import canvas
from reportlab.lib.units import cm
from reportlab.lib import colors

register_chinese_font()
c = canvas.Canvas("output.pdf", pagesize=(595, 842))
c.setFont('ChineseFont', 20)
c.setFillColor(colors.HexColor('#1F4E79'))
c.drawString(2*cm, 800, "标题文字")  # 左下原点，y 向上
c.setStrokeColor(colors.HexColor('#2E75B6'))
c.setLineWidth(1.5)
c.rect(2*cm, 700, 16*cm, 60, fill=0, stroke=1)  # 矩形边框
c.line(2*cm, 690, 18*cm, 690)  # 横线
c.drawImage("image.png", 2*cm, 500, width=8*cm, height=6*cm)  # 图片
c.showPage()
c.save()
```

### 6. 页眉页脚（通过 onPage 回调）
```python
def add_header_footer(canvas_obj, doc):
    canvas_obj.saveState()
    canvas_obj.setFont('ChineseFont', 9)
    canvas_obj.setFillColor(colors.HexColor('#7F7F7F'))
    canvas_obj.drawString(2*cm, A4[1] - 1*cm, "文档标题")
    canvas_obj.drawCentredString(A4[0]/2, 1*cm, f"第 {doc.page} 页")
    canvas_obj.restoreState()

doc.build(story, onFirstPage=add_header_footer, onLaterPages=add_header_footer)
```

### 7. 修改已有 PDF（PyMuPDF）
```python
import fitz

doc = fitz.open("input.pdf")
page = doc[0]

# 修改文字（fontname 不是 font！）
page.insert_text((50, 50), "新文字", fontname="china-s", fontsize=12,
                 color=(0.12, 0.31, 0.47))  # RGB 0-1
page.insert_textbox(fitz.Rect(50, 50, 500, 150), "长文本...",
                    fontsize=11, fontname="china-s")

# 绘制矢量元素（横线/边框/矩形）
page.draw_rect(fitz.Rect(50, 50, 500, 200), color=(0.18, 0.46, 0.71),
               width=1.5, fill=None)  # 矩形边框
page.draw_rect(fitz.Rect(50, 50, 500, 80), color=None,
               fill=(0.84, 0.89, 0.94), fill_opacity=1.0)  # 填充矩形
page.draw_line(fitz.Point(50, 100), fitz.Point(500, 100),
               color=(0.5, 0.5, 0.5), width=0.5)  # 横线

# 插入图片
page.insert_image(fitz.Rect(50, 200, 250, 400), filename="image.png")

# 页面操作
doc.delete_pages([2, 3])  # 删除（0-based）
doc.rotate_page(0, 90)    # 旋转
new_doc = fitz.open()
new_doc.insert_pdf(doc, from_page=0, to_page=4)  # 提取页面

# 合并
other = fitz.open("other.pdf")
doc.insert_pdf(other)

# 加密（必须保存到新文件，不能 incremental）
doc.save("output.pdf", encryption=fitz.PDF_ENCRYPT_AES_256,
         owner_pw="owner", user_pw="user")

# 元数据
doc.set_metadata({"title": "标题", "author": "作者", "subject": "主题"})

# 书签/目录
doc.set_toc([[1, "第一章 引言", 1], [2, "1.1 背景", 2], [1, "第二章 方法", 5]])

doc.save("output.pdf")  # 保存到新文件（不能直接覆盖原文件）
doc.close()
```

### 8. 读取 PDF 的视觉级布局（pdf_handler read 操作）
让智能体像人一样看到 PDF 中的所有视觉元素，使用 pdf_handler read 并开启：
- `include_visual: true` —— 一键启用 layout + drawings + page_geometry
- `include_links: true` —— 超链接（URI / 内部跳转）
- `include_toc: true` —— 书签/大纲
- `include_fonts: true` —— 字体清单
- `include_image_data: true` —— 图片二进制 base64（默认仅元信息）
- `include_metadata_full: true` —— 完整元数据
- `include_signatures: true` —— 数字签名

返回字段说明：
- `pages[].text`: 文本内容
- `layout[].blocks[].lines[].spans[]`: 文本块→行→span，含 bbox/font/size/color
- `drawings[].drawings[]`: 矢量元素，含 rect/fill/color/width/items（op: l=线段, re=矩形, c/cu=曲线）
- `page_geometry[]`: 页面 width/height/rotation/mediabox/cropbox/orientation
- `links[].links[]`: 超链接，含 kind/uri/target_page/from
- `fonts[].fonts[]`: 字体清单，含 xref/type/basefont/name/encoding
- `toc[]`: 书签树，每项 [level, title, page]
- `metadata_full`: 文档级元数据
- `signatures[]`: 数字签名字段

**修改策略**：先 read（含 include_visual）→ 分析布局 → 用 modify 操作或 PyMuPDF 修改原文件。

### 9. 专业配色方案
- 标题1: 深蓝色 (#1F4E79) 20pt
- 标题2: 中蓝色 (#2E75B6) 16pt
- 标题3: 浅蓝色 (#5B9BD5) 14pt
- 表头: 蓝色背景 (#2E75B6) + 白色粗体文字
- 表格交替行: 浅蓝色背景 (#D6E4F0)
- 字体: 微软雅黑（注册为 ChineseFont）

### 10. 修改 PDF 的内置工具（pdf_handler modify）
pdf_handler 提供 17 个 modify 子操作，**优先使用内置操作**：
- 页面操作：rotate_pages / delete_pages / extract_pages / reorder_pages
- 合并拆分：merge / split
- 水印：add_text_watermark / add_image_watermark
- 页眉页脚：add_header_footer
- 加密解密：encrypt / decrypt
- 元数据：set_metadata
- 书签目录：add_bookmarks / set_toc
- 注释：add_annotation
- 表单：fill_form
- 压缩：compress
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
