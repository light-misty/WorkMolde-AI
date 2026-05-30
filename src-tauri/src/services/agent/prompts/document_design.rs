//! 文档设计指导模块
//! 整合自 .trae/skills/ 中的 docx/xlsx/pptx/pdf Skill 规范
//! 为 Agent System Prompt 提供专业的文档生成规范
//! 新体系按文档格式聚合，每个 Skill 统一处理该格式的所有操作

/// Word 文档设计指导
pub const WORD_DESIGN_GUIDE: &str = r#"
## Word 文档生成规范

### 页面尺寸（DXA 单位，1440 DXA = 1 inch）
- US Letter: 12240 x 15840 DXA（美国文档默认，内容宽度 9360）
- A4: 11906 x 16838 DXA（国际文档默认，内容宽度 9026）
- python-docx 中设置: section.page_width, section.page_height
- 横向布局时传入纵向尺寸，python-docx 内部自动交换宽高

### 样式覆盖规范（使用 Arial 字体）
- 默认字体: Arial 12pt (size=24, 半磅单位)
- Heading1: 16pt 粗体 (size=32), 间距前后 240 DXA, outlineLevel=0（TOC 必需）
- Heading2: 14pt 粗体 (size=28), 间距前后 180 DXA, outlineLevel=1
- 使用内置样式 ID 覆盖: id="Heading1", basedOn="Normal", quickFormat=true

### 表格规范
- 必须同时设置表格宽度和每个单元格宽度（双宽度设置）
- 表格宽度: table.columns[i].width 和 cell.width 都必须设置
- 始终使用 DXA 单位，不使用百分比（百分比在 Google Docs 中会出错）
- 边框: 单线 1pt 灰色 (#CCCCCC), BorderStyle.SINGLE
- 使用 ShadingType.CLEAR 而非 SOLID（SOLID 会导致黑色背景）
- 单元格内边距: top=80, bottom=80, left=120, right=120 (DXA)

### 列表规范
- 必须使用列表样式（WD_STYLE_PARAGRAPH.LIST_BULLET / LevelFormat.BULLET）
- 绝对禁止使用 Unicode 字符（如 \u2022 或 "•"）手动插入项目符号
- 缩进: 左 720 DXA, 悬挂 360 DXA
- 编号列表使用 LevelFormat.DECIMAL, text="%1."

### 图片规范
- 必须指定图片格式（png/jpg/jpeg/gif/bmp/svg），type 参数为必填项
- python-docx 中使用 document.add_picture() 插入图片, 需指定 width/height
- 提供 altText 三字段: title, description, name（均为必填项）

### 页眉页脚
- 使用 section.header / section.footer API
- 支持页码: 插入 PageNumber 域代码（CURRENT / TOTAL_PAGES）
- 页边距: 1440 DXA = 1 inch

### 超链接
- 外部链接: 使用 python-docx 的 OxmlElement 创建 hyperlink
- 内部链接: 使用书签（Bookmark）+ 超链接引用（anchor）

### 颜色编码标准
- 蓝色文字 (RGB: 0,0,255): 硬编码输入值，用户会修改的数字
- 黑色文字 (RGB: 0,0,0): 所有公式和计算
- 绿色文字 (RGB: 0,128,0): 跨工作表引用
- 红色文字 (RGB: 255,0,0): 外部文件链接

### 关键规则
- 始终显式设置页面尺寸（python-docx 默认 A4）
- 不使用 \n 换行，使用独立的 Paragraph 元素
- PageBreak 必须在 Paragraph 内部
- 表格必须设置 width（DXA 单位），不使用 WidthType.PERCENTAGE
"#;

/// Excel 文档设计指导
pub const EXCEL_DESIGN_GUIDE: &str = r#"
## Excel 文档生成规范

### 核心原则：使用公式而非硬编码值
- 错误: 在 Python 中计算 sum, 然后硬编码结果
- 正确: 使用 Excel 公式 =SUM(B2:B9)
- 公式单元格写入方式: ws['B10'] = '=SUM(B2:B9)'
- 增长率公式: ws['C5'] = '=(C4-C2)/C2'

### 数字格式标准
- 年份: 格式化为文本字符串（"2024" 而非 "2,024"），使用 @ 格式码
- 货币: 使用 $#,##0 格式, 标题中必须注明单位
- 零值: 使用数字格式将零显示为 "-"
- 百分比: 默认 0.0% 格式（一位小数）
- 倍数: 格式化为 0.0x
- 负数: 使用括号 (123) 而非减号 -123

### 颜色编码标准
- 蓝色文字 (RGB: 0,0,255): 硬编码输入值, 用户会修改的数字
- 黑色文字 (RGB: 0,0,0): 所有公式和计算
- 绿色文字 (RGB: 0,128,0): 跨工作表引用
- 红色文字 (RGB: 255,0,0): 外部文件链接
- 黄色背景 (RGB: 255,255,0): 关键假设

### 库选择指南
- pandas: 数据分析、批量操作、简单数据导出
- openpyxl: 复杂格式、公式、Excel 特定功能

### openpyxl 注意事项
- 单元格索引从 1 开始 (row=1, column=1 即 A1)
- data_only=True 读取计算值, 但保存会丢失公式
- 公式不会被 Python 计算, 需要 Excel 或 recalc 脚本
- 大文件使用 read_only=True 读取或 write_only=True 写入
- 指定数据类型避免推断问题: pd.read_excel('file.xlsx', dtype={'id': str})
"#;

/// PPT 文档设计指导
pub const PPT_DESIGN_GUIDE: &str = r#"
## PPT 文档生成规范

### 设计原则
- 不要创建无聊的幻灯片
- 选择大胆的、内容驱动的颜色方案
- 一种颜色占主导（60-70% 视觉权重）
- 深色背景用于标题和结论页, 浅色用于内容页
- 承诺一个视觉主题并贯穿始终

### 5种颜色方案及RGB值
| 方案 | 主色 | 辅色 | 强调色 |
|------|------|------|--------|
| Midnight Executive | #1E2761 (navy) | #CADCFC (ice blue) | #FFFFFF (white) |
| Forest & Moss | #2C5F2D (forest) | #97BC62 (moss) | #F5F5F5 (cream) |
| Coral Energy | #F96167 (coral) | #F9E795 (gold) | #2F3C7E (navy) |
| Ocean Gradient | #065A82 (deep blue) | #1C7293 (teal) | #21295C (midnight) |
| Charcoal Minimal | #36454F (charcoal) | #F2F2F2 (off-white) | #212121 (black) |

### 字体规范
| 元素 | 大小 |
|------|------|
| 幻灯片标题 | 36-44pt 粗体 |
| 节标题 | 20-24pt 粗体 |
| 正文 | 14-16pt |
| 注释 | 10-12pt 淡色 |

### 间距规范
- 最小边距: 0.5 inch
- 内容块间距: 0.3-0.5 inch
- 留白呼吸空间, 不要填满每一寸

### 避免的错误
- 不要重复相同的布局
- 不要居中正文段落
- 不要默认使用蓝色
- 不要创建纯文字幻灯片
- 不要在标题下使用强调线
"#;

/// PDF 文档设计指导
pub const PDF_DESIGN_GUIDE: &str = r#"
## PDF 文档生成规范

### 下标和上标
- 不要使用 Unicode 下标/上标字符
- reportlab 中使用 XML 标签: H<sub>2</sub>O, x<super>2</super>
- 在 Paragraph 中使用: Paragraph("H<sub>2</sub>O", style)

### Platypus 框架使用
- 使用 SimpleDocTemplate + Paragraph 创建结构化 PDF
- 基本流程: 创建 story 列表 -> 添加元素 -> doc.build(story)
- 常用元素: Paragraph, Spacer, PageBreak, Table, Image
- 样式: 使用 getSampleStyleSheet() 获取基础样式

### 高级操作（基于 pypdf）
- 合并: 使用 PdfWriter.add_page() 逐页添加
- 拆分: 每页单独保存为独立 PDF
- 旋转: page.rotate(90) 顺时针旋转
- 水印: page.merge_page(watermark) 叠加水印页
- 加密: writer.encrypt(user_pwd, owner_pwd) 设置密码保护

### 表格提取（基于 pdfplumber）
- 使用 pdfplumber 的 page.extract_tables() 提取表格
- 支持布局保留的文本提取: page.extract_text()

### reportlab 注意事项
- 特殊字符必须使用 html.escape() 转义
- 中文字体需注册后使用
- 页面尺寸: letter (612x792) 或 A4 (595x842)
"#;

/// 获取所有文档设计指导, 拼接为完整字符串
pub fn get_all_design_guides() -> String {
    format!(
        "{}\n\n{}\n\n{}\n\n{}",
        WORD_DESIGN_GUIDE,
        EXCEL_DESIGN_GUIDE,
        PPT_DESIGN_GUIDE,
        PDF_DESIGN_GUIDE,
    )
}

/// 根据文档类型获取对应的设计指导
pub fn get_design_guide_by_type(doc_type: &str) -> &'static str {
    match doc_type {
        "docx" => WORD_DESIGN_GUIDE,
        "xlsx" => EXCEL_DESIGN_GUIDE,
        "pptx" => PPT_DESIGN_GUIDE,
        "pdf" => PDF_DESIGN_GUIDE,
        _ => "",
    }
}
