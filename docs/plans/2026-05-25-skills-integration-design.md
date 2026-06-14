# Handlers 整合开发计划：提升文档生成专业性

> **注意**: 本文档中提到的 "Skill" 已重命名为 "Handler"，相关工具名如 `docx_skill` 已更改为 `docx_handler`。

## 一、现状分析

### 1.1 当前系统 Sidecar Handlers 实现

当前 DocAgent 的文档处理通过 Python Sidecar 实现，包含四个 Handler：

| Handler | 基础库 | 功能范围 | 专业程度 |
|---------|--------|----------|----------|
| WordHandler | python-docx | 基础生成、读取、修改、转换 | 低 - 缺少格式规范、样式指南 |
| ExcelHandler | openpyxl | 基础生成、读取、修改、转换 | 低 - 缺少公式使用、格式标准 |
| PptHandler | python-pptx | 基础生成、读取、修改、转换 | 低 - 缺少设计指导、颜色方案 |
| PdfHandler | reportlab + PyMuPDF | 基础生成、读取、转换 | 中 - 功能较完整但缺少高级操作 |
| MarkdownHandler | 纯 Python | 基础生成、读取、修改、转换 | 低 - 纯文本处理，无格式规范 |

### 1.2 当前实现的核心问题

1. **缺少设计指导**：LLM 生成的文档内容混乱、格式不专业，因为系统没有提供专业的设计规范
2. **缺少最佳实践**：没有颜色编码标准、字体规范、间距标准等
3. **缺少公式支持**：Excel 生成时硬编码计算值，而非使用 Excel 公式
4. **缺少高级功能**：Word 缺少页眉页脚、目录、书签；PPT 缺少设计模板；PDF 缺少合并、水印、加密等
5. **缺少 QA 流程**：生成后没有验证机制，无法确保文档质量

### 1.3 新 Handlers 的优势

用户提供的四个 Handler 文件包含丰富的专业指南：

#### docx Handler 优势
- **页面尺寸规范**：US Letter vs A4，DXA 单位换算
- **样式系统**：标题层级、字体选择（Arial）、间距规范
- **列表规范**：使用 LevelFormat.BULLET 而非 Unicode 字符
- **表格规范**：双宽度设置、ShadingType.CLEAR
- **图片规范**：type 参数必填、altText 三字段
- **页眉页脚**：Header/Footer 实现
- **超链接**：外部链接和内部书签链接
- **颜色编码标准**：蓝色输入、黑色公式、绿色跨表、红色外部
- **编辑方法**：unpack → edit XML → pack 流程

#### xlsx Handler 优势
- **公式优先原则**：使用 Excel 公式而非 Python 硬编码
- **数据分析**：pandas + openpyxl 组合使用
- **颜色编码标准**：蓝色输入、黑色公式、绿色跨表引用、红色外部链接、黄色假设
- **数字格式标准**：年份文本化、货币格式、零值显示、百分比、负数括号
- **库选择指南**：pandas 用于数据分析，openpyxl 用于格式和公式

#### pptx Handler 优势
- **设计理念**：避免无聊幻灯片，内容驱动的颜色方案
- **颜色方案库**：Midnight Executive、Forest & Moss、Coral Energy 等
- **字体规范**：标题 36-44pt，正文 14-16pt
- **间距规范**：0.5" 边距，留白呼吸空间
- **避免错误**：不重复布局、不居中正文、不默认蓝色
- **QA 流程**：使用 python-pptx 读取生成内容检查 + PDF 转图片视觉检查

#### pdf Handler 优势
- **多库覆盖**：pypdf（基础操作）、pdfplumber（表格提取）、reportlab（创建）
- **高级操作**：合并、拆分、旋转、水印、加密、OCR
- **命令行工具**：pdftotext、qpdf
- **下标上标**：XML 标签而非 Unicode

---

## 二、整合方案

### 2.1 方案概述

由于 DocAgent 的 Sidecar 是 Python 进程，无法直接执行 JavaScript（docx-js、pptxgenjs），因此采用以下整合策略：

1. **系统提示词整合**：将新 Handlers 的设计指导、最佳实践、颜色标准等整合到 Agent 的 System Prompt 中
2. **Python Handler 增强**：根据新 Handlers 的规范，增强现有 Python Handler 的功能
3. **参数 Schema 扩展**：扩展 Handler 的参数定义，支持更多专业选项
4. **QA 验证流程**：在 Handler 执行后添加验证步骤

### 2.2 整合架构

```
┌─────────────────────────────────────────────────────────────────┐
│                     Agent System Prompt                          │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  文档设计指导（从新 Handlers 整合）                            ││
│  │  - Word: 页面尺寸、样式、颜色编码、表格规范                  ││
│  │  - Excel: 公式优先、数字格式、颜色编码                       ││
│  │  - PPT: 设计理念、颜色方案、字体规范、避免错误               ││
│  │  - PDF: 高级操作、下标上标                                   ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                     Handler 参数定义                               │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  扩展参数支持：                                              ││
│  │  - generate_document: pageSize, colorScheme, font, QA       ││
│  │  - modify_document: formula, format, validation             ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                     Python Sidecar Handlers                      │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  功能增强：                                                  ││
│  │  - WordHandler: 页眉页脚、目录、书签、超链接                  ││
│  │  - ExcelHandler: 公式写入、格式设置、颜色编码                ││
│  │  - PptHandler: 设计模板、颜色方案应用                        ││
│  │  - PdfHandler: 合并、拆分、水印、加密                        ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

---

## 三、详细实施计划

### 阶段一：系统提示词整合（预计 1-2 天）

#### 任务 1.1：创建文档设计指导模块

**文件**：
- 创建：`src-tauri/src/services/agent/prompts/document_design.rs`
- 修改：`src-tauri/src/services/agent/mod.rs`（新增 `pub mod prompts;` 导出）

**内容**：
- Word 设计指导（页面尺寸、样式、颜色编码、表格规范）
- Excel 设计指导（公式优先、数字格式、颜色编码）
- PPT 设计指导（设计理念、颜色方案、字体规范）
- PDF 设计指导（高级操作、下标上标）

#### 任务 1.2：整合到 System Prompt

**文件**：
- 修改：`src-tauri/src/services/agent/context.rs`

**内容**：
- 在 `build_system_prompt()` 中引用文档设计指导
- 根据用户请求的文档类型，动态注入相关指导

#### 任务 1.3：创建设计指导常量

**示例 System Prompt 结构**：

```rust
/// Word 文档设计指导
const WORD_DESIGN_GUIDE: &str = r#"
## Word 文档生成规范

### 页面尺寸
- US Letter: 12240 x 15840 DXA（美国文档默认）
- A4: 11906 x 16838 DXA（国际文档默认）
- 1 inch = 1440 DXA

### 样式规范
- 默认字体: Arial 12pt
- 标题1: 16pt 粗体
- 标题2: 14pt 粗体
- 正文: 12pt，行间距 1.5

### 表格规范
- 必须设置表格宽度（DXA 单位）
- 同时设置列宽和每个单元格的宽度
- python-docx 中使用 table.columns[i].width 和 cell.width 设置宽度
- 边框: 单线 1pt 灰色 (#CCCCCC)

### 列表规范
- 使用 python-docx 的列表样式（WD_STYLE_PARAGRAPH.LIST_BULLET）而非 Unicode 字符（•）
- 缩进: 左 720 DXA，悬挂 360 DXA

### 图片规范
- 必须指定图片格式（png/jpg/jpeg/gif/bmp/svg）
- python-docx 中使用 document.add_picture() 插入图片，需指定 width/height
"#;

/// Excel 文档设计指导
const EXCEL_DESIGN_GUIDE: &str = r#"
## Excel 文档生成规范

### 核心原则：使用公式而非硬编码值
- 错误: 在 Python 中计算 sum，然后硬编码结果
- 正确: 使用 Excel 公式 =SUM(B2:B9)

### 数字格式标准
- 年份: 格式化为文本字符串（"2024" 而非 "2,024"）
- 货币: 使用 $#,##0 格式，标题中必须注明单位
- 零值: 使用数字格式将零显示为 "-"
- 百分比: 默认 0.0% 格式（一位小数）
- 倍数: 格式化为 0.0x
- 负数: 使用括号 (123) 而非减号 -123

### 颜色编码标准
- 蓝色文字: 硬编码输入值
- 黑色文字: 所有公式和计算
- 绿色文字: 跨工作表引用
- 红色文字: 外部文件链接
- 黄色背景: 关键假设

### 库选择
- pandas: 数据分析、批量操作、简单数据导出
- openpyxl: 复杂格式、公式、Excel 特定功能
"#;

/// PPT 文档设计指导
const PPT_DESIGN_GUIDE: &str = r#"
## PPT 文档生成规范

### 设计原则
- 不要创建无聊的幻灯片
- 选择大胆的、内容驱动的颜色方案
- 一种颜色占主导（60-70% 视觉权重）
- 深色背景用于标题和结论页，浅色用于内容页

### 颜色方案库
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
- 留白呼吸空间，不要填满每一寸

### 避免的错误
- 不要重复相同的布局
- 不要居中正文段落
- 不要默认使用蓝色
- 不要创建纯文字幻灯片
- 不要在标题下使用强调线
"#;

/// PDF 文档设计指导
const PDF_DESIGN_GUIDE: &str = r#"
## PDF 文档生成规范

### 下标和上标
- 不要使用 Unicode 下标/上标字符
- reportlab 中使用 XML 标签: H<sub>2</sub>O, x<super>2</super>

### 高级操作（需新增依赖）
- 合并: 需安装 pypdf，使用 PdfWriter.add_page()
- 拆分: 每页单独保存
- 旋转: page.rotate(90)
- 水印: page.merge_page(watermark)
- 加密: writer.encrypt(user_pwd, owner_pwd)
- OCR: 需安装 pytesseract + pdf2image

### 表格提取（需新增依赖）
- 需安装 pdfplumber，使用 pdfplumber.extract_tables()

### 注意
- 当前 requirements.txt 仅包含 PyMuPDF 和 pdfminer.six
- 实现高级操作前需先在 requirements.txt 中添加 pypdf、pdfplumber 等依赖
- pdftotext、qpdf 等命令行工具需要用户自行安装，不建议作为默认依赖
"#;
```

### 阶段二：Python Handler 功能增强（预计 3-4 天）

#### 任务 2.1：增强 WordHandler

**文件**：
- 修改：`sidecar/handlers/word_handler.py`

**新增功能**：
1. **页眉页脚支持**
   - 参数：`header`、`footer`、`pageNumberFormat`
   - 实现：使用 python-docx 的 Header/Footer API

2. **目录支持**
   - 参数：`includeToc: boolean`
   - 实现：添加 TOC 字段

3. **书签和超链接**
   - 参数：`bookmarks: [{id, text}]`、`hyperlinks: [{text, url, anchor}]`
   - 实现：使用 python-docx 的 Hyperlink API

4. **颜色编码**
   - 参数：`colorCoding: boolean`（默认 true）
   - 实现：根据内容类型自动应用颜色

5. **页面尺寸**
   - 参数：`pageSize: "letter" | "a4"`
   - 实现：设置文档页面尺寸

#### 任务 2.2：增强 ExcelHandler

**文件**：
- 修改：`sidecar/handlers/excel_handler.py`

**新增功能**：
1. **公式支持**
   - 参数：`cells: [{row, col, formula}]`（新增 formula 字段）
   - 实现：写入公式而非硬编码值

2. **数字格式**
   - 参数：`formats: [{range, format}]`
   - 支持格式：currency、percent、text、custom
   - 实现：使用 openpyxl 的 number_format

3. **颜色编码**
   - 参数：`colorCoding: boolean`
   - 实现：根据单元格类型自动应用颜色

4. **条件格式**
   - 参数：`conditionalFormats: [{range, rule}]`
   - 实现：使用 openpyxl 的 ConditionalFormatting

#### 任务 2.3：增强 PptHandler

**文件**：
- 修改：`sidecar/handlers/ppt_handler.py`

**新增功能**：
1. **颜色方案**
   - 参数：`colorScheme: "midnight" | "forest" | "coral" | "ocean" | "charcoal"`
   - 实现：预设颜色方案，自动应用到幻灯片

2. **设计模板**
   - 参数：`template: "minimal" | "corporate" | "creative"`
   - 实现：预设布局模板

3. **字体规范**
   - 参数：`fonts: {title, body}`
   - 实现：统一字体设置

4. **间距控制**
   - 参数：`margins: {top, right, bottom, left}`
   - 实现：设置幻灯片边距

#### 任务 2.4：增强 PdfHandler

**文件**：
- 修改：`sidecar/handlers/pdf_handler.py`

**新增功能**：
1. **合并 PDF**
   - 参数：`merge: {files: [...], output: "..."}`
   - 实现：使用 pypdf 合合多个 PDF

2. **拆分 PDF**
   - 参数：`split: {ranges: ["1-5", "6-10"]}`
   - 实现：按页码范围拆分

3. **水印**
   - 参数：`watermark: {text, image, position}`
   - 实现：叠加水印

4. **加密**
   - 参数：`encrypt: {userPassword, ownerPassword}`
   - 实现：PDF 加密

5. **下标上标**
   - 参数：`subscripts: [{text, position}]`、`superscripts: [{text, position}]`
   - 实现：使用 reportlab XML 标签

### 阶段三：Handler 参数 Schema 扩展（预计 1-2 天）

#### 任务 3.1：扩展 generate_document Handler 参数

**文件**：
- 修改：`src-tauri/src/services/handler/builtin.rs`

**新增参数**：

```json
{
  "generate_document": {
    "parameters": {
      "format": "docx|xlsx|pptx|pdf|md",
      "path": "输出路径",
      "title": "文档标题",
      "content": "文档内容",
      "template": "模板文件路径（已有参数，保持不变）",
      
      // Word 专用
      "pageSize": "letter|a4",
      "colorCoding": true,
      "includeToc": false,
      "header": "页眉文本",
      "footer": "页脚文本",
      "pageNumber": true,
      
      // Excel 专用
      "useFormulas": true,
      "numberFormats": [{ "range": "B2:B10", "format": "currency" }],
      "conditionalFormats": [{ "range": "C2:C10", "rule": "greaterThan", "value": 100 }],
      
      // PPT 专用
      "colorScheme": "midnight|forest|coral|ocean|charcoal",
      "fonts": { "title": "Arial Black", "body": "Arial" },
      "margins": { "top": 0.5, "right": 0.5, "bottom": 0.5, "left": 0.5 },
      
      // PDF 专用
      "subscripts": [{ "text": "2", "position": 1 }],
      "superscripts": [{ "text": "2", "position": 3 }]
    }
  }
}
```

#### 任务 3.2：扩展 modify_document Handler 参数

**新增参数**：

```json
{
  "modify_document": {
    "parameters": {
      "path": "文件路径",
      "operations": [
        // Word 操作
        { "type": "addHeader", "text": "页眉文本" },
        { "type": "addFooter", "text": "页脚文本", "pageNumber": true },
        { "type": "addBookmark", "id": "chapter1", "text": "第一章" },
        { "type": "addHyperlink", "text": "点击跳转", "url": "https://example.com" },
        { "type": "setPageSize", "size": "a4" },
        
        // Excel 操作
        { "type": "setFormula", "sheet": "Sheet1", "row": 10, "col": 2, "formula": "=SUM(B2:B9)" },
        { "type": "setFormat", "sheet": "Sheet1", "range": "A1:A10", "format": "currency" },
        { "type": "setColorCoding", "sheet": "Sheet1", "range": "B2", "colorType": "input" },
        
        // PPT 操作
        { "type": "applyColorScheme", "scheme": "midnight" },
        { "type": "setFont", "element": "title", "font": "Arial Black", "size": 36 },
        
        // PDF 操作
        { "type": "merge", "files": ["doc1.pdf", "doc2.pdf"] },
        { "type": "split", "ranges": ["1-5", "6-10"] },
        { "type": "addWatermark", "text": "CONFIDENTIAL" }
      ]
    }
  }
}
```

### 阶段四：QA 验证流程（预计 1 天）

#### 任务 4.1：创建文档验证模块

**文件**：
- 创建：`sidecar/handlers/validator.py`

**功能**：
1. **内容验证**：检查缺失内容、拼写错误、顺序问题
2. **格式验证**：检查表格宽度、图片 altText、颜色编码
3. **公式验证**：检查 Excel 公式是否正确
4. **设计验证**：检查 PPT 是否遵循设计规范

#### 任务 4.2：集成到 Handler 执行流程

**文件**：
- 修改：`src-tauri/src/services/handler/builtin.rs`

**流程**：
```
1. Handler 执行（生成/修改文档）
2. 调用 validator.validate()
3. 返回验证结果
4. 如果验证失败，返回警告信息给 LLM
```

---

## 四、关键设计决策

### 4.1 为什么不直接使用 JavaScript（docx-js、pptxgenjs）？

DocAgent 的 Sidecar 是 Python 进程，通过 stdin/stdout JSON 协议与 Rust 后端通信。引入 JavaScript 会需要：

1. 新增 Node.js Sidecar 进程
2. 新增进程间通信协议
3. 增加系统复杂度和维护成本

因此选择增强现有 Python Handler，而非引入新的技术栈。

### 4.2 如何确保 LLM 遵循设计规范？

通过 System Prompt 整合设计指导，让 LLM 在生成文档参数时自动遵循规范。例如：

- LLM 在生成 Excel 时会优先使用公式而非硬编码值
- LLM 在生成 PPT 时会选择合适的颜色方案
- LLM 在生成 Word 时会设置正确的页面尺寸

### 4.3 验证流程的作用

验证流程在 Handler 执行后检查文档质量，如果发现问题则返回警告给 LLM，LLM 可以决定是否重新生成或修改。这形成了一个闭环的质量保证机制。

---

## 五、验收标准

1. System Prompt 包含完整的文档设计指导（Word/Excel/PPT/PDF）
2. WordHandler 支持页眉页脚、目录、书签、超链接、颜色编码、页面尺寸
3. ExcelHandler 支持公式写入、数字格式、颜色编码、条件格式
4. PptHandler 支持颜色方案、设计模板、字体规范、间距控制
5. PdfHandler 支持合并、拆分、水印、加密、下标上标
6. generate_document Handler 参数 Schema 包含所有新增参数
7. modify_document Handler 参数 Schema 包含所有新增操作
8. 文档验证模块能检测常见质量问题
9. 生成的文档遵循颜色编码标准
10. 生成的 Excel 使用公式而非硬编码值
11. 生成的 PPT 使用预设颜色方案而非默认蓝色

---

## 六、风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| LLM 不遵循设计规范 | 生成的文档仍不专业 | 强化 System Prompt + 验证流程反馈 |
| Python Handler 功能有限 | 无法实现某些高级功能 | 优先实现核心功能，高级功能后续迭代 |
| 参数 Schema 过于复杂 | LLM 难以正确使用 | 提供示例和默认值，简化必填参数 |
| 验证流程增加延迟 | 用户等待时间变长 | 验证可选，默认关闭，用户可启用 |
| PDF 高级操作依赖未安装的库 | 功能无法实现 | 实现前先在 requirements.txt 中添加 pypdf、pdfplumber 等依赖 |

---

## 七、与其他计划的依赖关系

本计划与《Handlers 与 Tools 分离重构开发计划》存在执行顺序依赖：

1. **应先执行 Tools 分离重构**：该计划将 3 个 Rust 原生 Handler 迁移为 Tool，精简 HandlerRegistry
2. **再执行本计划**：在精简后的 Handler 基础上扩展参数和增强 Handler
3. **原因**：如果先扩展参数再分离，迁移时需要重新调整已扩展的参数 Schema；先分离后扩展可以避免重复工作
4. **系统提示词冲突**：两份计划都修改 `context.rs` 的系统提示词，需合并处理