//! 文档设计指导模块
//! 整合自 .trae/skills/ 中的 docx/xlsx/pptx/pdf Skill 规范
//! 为 Agent System Prompt 提供专业的文档生成规范
//! 新体系按文档格式聚合，每个 Skill 统一处理该格式的所有操作

/// Word 文档设计指导
pub const WORD_DESIGN_GUIDE: &str = r#"
## Word 文档生成规范

### 内容格式要求（重要）
- content 参数支持 Markdown 格式，系统会自动将 Markdown 解析为专业 Word 元素
- 推荐使用 Markdown 格式编写内容，这样可以获得最佳的排版效果
- 支持的 Markdown 语法: # 标题、**粗体**、*斜体*、`代码`、- 列表、1. 有序列表、| 表格 |、```代码块```、---分隔线
- 也可以使用结构化 JSON 格式: {"blocks": [{type, ...}]}
- 绝对不要在 content 中输出原始 Markdown 标记（如 # ** - 等）而不使用正确的格式

### 专业配色方案（已内置）
- 标题1: 深蓝色 (#1F4E79) 22pt 粗体
- 标题2: 中蓝色 (#2E75B6) 16pt 粗体
- 标题3: 浅蓝色 (#5B9BD5) 14pt 粗体
- 表头: 蓝色背景 (#D6E4F0) + 深蓝色粗体文字
- 表格交替行: 浅蓝色背景 (#EDF2F9)
- 字体: 拉丁字体 Arial，东亚字体 微软雅黑（中文不再显示为 MS Gothic/MS Mincho）
- 页边距: 2.54cm（1 inch）四边

### 页面尺寸（DXA 单位，1440 DXA = 1 inch）
- US Letter: 12240 x 15840 DXA
- A4: 11906 x 16838 DXA（默认）

### 表格规范
- 使用 Markdown 表格语法: | 列1 | 列2 | \n |---|---|
- 表头自动应用蓝色背景和粗体样式
- 数据行自动应用交替行颜色
- 边框使用蓝灰色 (#B4C6E7)

### 列表规范
- 使用 Markdown 列表语法: - 无序列表项 或 1. 有序列表项
- 系统自动使用 Word 列表样式，不会出现原始的 - 或 1. 标记

### 代码块
- 使用 Markdown 代码块语法: ```language ... ```
- 代码块自动应用 Consolas 等宽字体 + 浅灰色背景

### 页眉页脚
- header 参数设置页眉文本
- footer 参数设置页脚文本
- pageNumber 参数控制是否显示页码（默认 true）

### 关键规则
- 始终使用 Markdown 或结构化 JSON 格式编写 content，不要输出纯文本
- 表格必须使用 Markdown 表格语法或结构化 JSON，不要用纯文本描述
- 标题使用 # 语法，不要在段落中手动标注"第一章"等
"#;

/// Excel 文档设计指导
pub const EXCEL_DESIGN_GUIDE: &str = r#"
## Excel 文档生成规范

### 内容格式要求（重要）
- 推荐使用 sheets 参数提供结构化数据，这样可以获得最佳的专业样式效果
- sheets 参数格式: [{"name": "工作表名", "headers": ["列1", "列2"], "data": [[...], [...]]}]
- 系统会自动应用专业样式: 蓝色表头背景、交替行颜色、蓝灰色边框、冻结窗格
- 如果提供 title 参数，系统会自动在第一行添加合并的标题行

### 专业配色方案（已内置）
- 表头: 蓝色背景 (#D6E4F0) + 深蓝色粗体文字 (#1F4E79)
- 交替行: 浅蓝色背景 (#EDF2F9)
- 边框: 蓝灰色 (#B4C6E7)
- 标题行: 深蓝色粗体 (#1F4E79) 16pt + 浅蓝色背景 (#F2F7FB)
- 字体: 微软雅黑 11pt

### 核心原则：使用公式而非硬编码值
- 错误: 在 Python 中计算 sum, 然后硬编码结果
- 正确: 使用 Excel 公式 =SUM(B2:B9)
- 公式单元格写入方式: 在 cells 或 formulas 字段中提供 formula 值
- 增长率公式: formula: "=(C4-C2)/C2"

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

### 关键规则
- 优先使用 sheets 参数提供结构化数据，而非 content 纯文本
- 表头使用 headers 字段，数据使用 data 字段
- 公式使用 cells 或 formulas 字段中的 formula 属性
- 数字格式使用 numberFormats 参数指定范围和格式类型
"#;

/// PPT 文档设计指导
pub const PPT_DESIGN_GUIDE: &str = r#"
## PPT 文档生成规范

### 内容格式要求（重要）
- 使用 slides 参数提供结构化幻灯片数据
- slides 参数格式: [{"title": "标题", "content": "内容", "layout": "title|content|twoColumn"}]
- 系统会自动应用专业样式: 配色方案、东亚字体、页码、标题分隔线

### 专业配色方案（已内置，默认 Ocean Gradient）
| 方案 | 主色 | 辅色 | 强调色 |
|------|------|------|--------|
| ocean | #065A82 (deep blue) | #1C7293 (teal) | #21295C (midnight) |
| midnight | #1E2761 (navy) | #CADCFC (ice blue) | #FFFFFF (white) |
| forest | #2C5F2D (forest) | #97BC62 (moss) | #F5F5F5 (cream) |
| coral | #F96167 (coral) | #F9E795 (gold) | #2F3C7E (navy) |
| charcoal | #36454F (charcoal) | #F2F2F2 (off-white) | #212121 (black) |

### 字体规范（已内置东亚字体支持）
- 拉丁字体: Calibri，东亚字体: 微软雅黑
- 幻灯片标题: 36-44pt 粗体
- 节标题: 20-24pt 粗体
- 正文: 14-16pt
- 注释: 10-12pt 淡色

### 间距规范
- 最小边距: 0.5 inch
- 内容块间距: 0.3-0.5 inch
- 留白呼吸空间, 不要填满每一寸

### 自动功能
- 每张幻灯片自动添加页码（右下角）
- 内容页标题下方自动添加彩色分隔线
- 标题页支持 subtitle 字段显示副标题

### 关键规则
- 使用 colorScheme 参数选择配色方案（推荐 "ocean"）
- 每张幻灯片内容不宜过多，保持简洁
- 标题页使用 layout: "title"，内容页使用 layout: "content"
"#;

/// PDF 文档设计指导
pub const PDF_DESIGN_GUIDE: &str = r#"
## PDF 文档生成规范

### 内容格式要求（重要）
- content 参数支持 Markdown 格式，系统会自动将 Markdown 解析为专业 PDF 元素
- 推荐使用 Markdown 格式编写内容，这样可以获得最佳的排版效果
- 支持的 Markdown 语法: # 标题、**粗体**、*斜体*、`代码`、- 列表、1. 有序列表、| 表格 |、```代码块```、---分隔线
- 也可以使用结构化 JSON 格式
- 绝对不要在 content 中输出原始 Markdown 标记而不使用正确的格式

### 专业配色方案（已内置）
- 标题1: 深蓝色 (#1F4E79) 20pt
- 标题2: 中蓝色 (#2E75B6) 16pt
- 标题3: 浅蓝色 (#5B9BD5) 14pt
- 表头: 蓝色背景 (#2E75B6) + 白色粗体文字
- 表格交替行: 浅蓝色背景 (#D6E4F0)
- 代码块: 等宽字体 + 浅灰色背景 (#F5F5F5)
- 字体: 微软雅黑（已自动注册中文字体）

### 表格规范
- 使用 Markdown 表格语法: | 列1 | 列2 | \n |---|---|
- 表头自动应用蓝色背景和白色粗体样式
- 数据行自动应用交替行颜色
- 单元格自动应用内边距

### 关键规则
- 始终使用 Markdown 或结构化 JSON 格式编写 content
- 表格必须使用 Markdown 表格语法或结构化 JSON
- 特殊字符会自动转义，无需手动处理
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
