# Tools & Handlers 全面评估 - 功能测试文档

> 本文档覆盖 tools_handlers_comprehensive_evaluation 项目的功能验证，包含手工测试步骤、预期结果和边界场景。

## 测试环境准备

1. 启动应用：`npm run tauri:dev`
2. 在设置中配置至少一个 LLM Provider（推荐 OpenAI 兼容）
3. 创建测试工作区，例如 `D:\test_workspace`
4. 准备测试文件：
   - `sample.docx` - 包含标题、正文、表格的 Word 文档
   - `sample.xlsx` - 包含表头和数据的 Excel 文件
   - `large_file.txt` - 超过 6000 字符的文本文件（用于截断测试）
   - `中文内容.md` - 包含中文的 Markdown 文件

---

## 一、阶段三 3.5：5 个新增 Tool 测试

### 1.1 rename_file - 重命名文件

**测试步骤**：
1. 在工作区创建 `test_rename.txt`，内容随意
2. 向 Agent 发送：`将 test_rename.txt 重命名为 renamed.txt`
3. 观察文件树变化

**预期结果**：
- 文件 `test_rename.txt` 消失
- 文件 `renamed.txt` 出现
- 工作流时间线显示 rename_file 工具调用成功

**边界测试**：
- 源文件不存在 → 应返回 `TOOL_INVALID_PARAMS` 或文件不存在错误
- 目标路径在工作区外（如 `../renamed.txt`）→ 应返回 `TOOL_PATH_OUT_OF_BOUNDS`，拒绝操作
- 目标父目录不存在 → 应自动创建父目录

### 1.2 copy_file - 复制文件

**测试步骤**：
1. 向 Agent 发送：`复制 sample.docx 到 backup.docx`
2. 观察文件树

**预期结果**：
- `sample.docx` 保留
- `backup.docx` 出现，内容与原文件一致
- 工具结果包含 `bytes_copied` 字段

**边界测试**：
- 跨格式复制（如 docx → txt）→ 应仅复制文件，不转换格式
- 二进制文件复制 → 完整保留二进制内容

### 1.3 delete_directory - 删除目录

**测试步骤**：
1. 在工作区创建 `test_dir/subdir/file.txt` 目录结构
2. 向 Agent 发送：`删除 test_dir 目录`
3. 确认删除操作（弹窗）

**预期结果**：
- `test_dir` 及其所有子内容被删除
- 工作流显示 delete_directory 工具调用成功

**安全测试**：
- 尝试删除工作区根目录 → 应返回错误"禁止删除工作区根目录"
- 路径越界（如 `../other_dir`）→ 应返回 `TOOL_PATH_OUT_OF_BOUNDS`

**备份测试**：
1. 向 Agent 发送：`删除 test_dir 目录，并创建备份`
2. 检查是否生成 `test_dir.bak` 目录
3. 验证备份内容与原目录一致

### 1.4 get_file_hash - 计算文件哈希

**测试步骤**：
1. 向 Agent 发送：`计算 sample.docx 的 SHA-256 哈希`
2. 记录返回的哈希值
3. 修改文件内容后再次计算

**预期结果**：
- 返回 64 位十六进制哈希字符串
- 修改后哈希值发生变化
- 相同文件多次计算返回相同哈希

**边界测试**：
- 大文件（>100MB）→ 应分块读取，不占用过多内存
- 空文件 → 应返回空文件的哈希 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`

### 1.5 read_file_lines - 按行读取文件

**测试步骤**：
1. 创建 200 行的文本文件 `lines.txt`
2. 向 Agent 发送：`读取 lines.txt 的第 10-20 行`
3. 向 Agent 发送：`读取 lines.txt 的第 50 行起 5 行`

**预期结果**：
- 返回 `offset`、`limit`、`total_lines`、`returned_lines`、`lines`、`has_more` 字段
- `lines` 数组长度与请求一致
- `has_more` 正确反映是否还有更多行

**边界测试**：
- offset 超过总行数 → `lines` 为空数组，`has_more` 为 false
- limit 超过 1000 → 应被限制为 1000
- GBK 编码文件 → 指定 `encoding=gbk` 应正确解码

---

## 二、阶段四 4.2：apply_theme 主题应用测试

### 2.1 Word 文档主题应用

**测试步骤**：
1. 向 Agent 发送：`生成一份项目报告 Word 文档，包含标题、正文、表格`
2. 生成完成后，下载并用 Word 打开

**预期结果**：
- 标题 1（Heading 1）颜色为 `#1F4E79`（深蓝），字号 16pt，粗体
- 标题 2（Heading 2）颜色为 `#2E75B6`（中蓝），字号 14pt，粗体
- 标题 3（Heading 3）颜色为 `#5B9BD5`（浅蓝），字号 12pt，粗体
- 正文（Normal）颜色为 `#262626`，字号 11pt
- 表格表头行背景色为 `#D6E4F0`

### 2.2 PPT 配色方案

**测试步骤**：
1. 向 Agent 发送：`生成一份项目汇报 PPT，使用 forest 配色方案`
2. 生成完成后下载

**预期结果**：
- PPT 的 `core_properties.comments` 包含 `color_scheme:forest`
- LLM 在添加形状时使用 `get_ppt_color_scheme(prs)` 获取的绿色系配色
- 无效配色方案名（如 `invalid`）→ 回退到 `ocean` 方案

**验证方法**：
```python
from pptx import Presentation
prs = Presentation("项目汇报.pptx")
print(prs.core_properties.comments)  # 应输出 color_scheme:forest
```

### 2.3 Excel 表头样式

**测试步骤**：
1. 向 Agent 发送：`生成一份销售数据 Excel，包含表头行`
2. 生成完成后用 Excel 打开

**预期结果**：
- 表头行（第 1 行）背景色为 `#D6E4F0`（浅蓝）
- 表头字体为微软雅黑、粗体、颜色 `#1F4E79`、字号 11
- 表头单元格有细边框（`#B4C6E7`）
- 表头居中对齐
- 工作表标签颜色为 `#1F4E79`

---

## 三、阶段五 P4-3：Handler Schema 一致性测试

### 3.1 target_format enum 验证

**测试步骤**：
1. 向 Agent 发送：`将 sample.docx 转换为 md 格式`
2. 向 Agent 发送：`将 sample.docx 转换为 pdf 格式`
3. 尝试：`将 sample.docx 转换为 jpg 格式`（不支持格式）

**预期结果**：
- md/pdf 转换成功
- jpg 转换应被 LLM 拒绝（schema enum 不包含 jpg），或返回格式不支持错误

**各 Handler 支持格式**：
| Handler | target_format enum |
|---------|-------------------|
| docx_handler | md, txt, pdf |
| xlsx_handler | csv, pdf, html, txt |
| pptx_handler | pdf |
| pdf_handler | md, txt, docx, html |

---

## 四、阶段五 P4-4：UTF-8 截断修复测试

### 4.1 中文内容截断

**测试步骤**：
1. 创建一个超过 6000 字符的中文文本文件 `large_chinese.txt`
2. 向 Agent 发送：`读取 large_chinese.txt 的全部内容`
3. 观察工具结果

**预期结果**：
- 工具调用成功，不 panic
- 结果包含截断提示：`...[已截断: 原始 N 字符，保留头部 X + 尾部 Y，省略中间 Z 字符]...`
- 头部 70%（4200 字符）+ 尾部 30%（1800 字符）= 6000 字符
- 中文内容完整显示，无乱码

### 4.2 英文内容截断

**测试步骤**：
1. 创建超过 6000 字符的英文文件
2. 读取并观察截断

**预期结果**：
- 同上，截断提示正确显示字符数

---

## 五、阶段五 P4-6：错误码字段测试

### 5.1 路径越界错误码

**测试步骤**：
1. 向 Agent 发送：`读取 ../../../etc/passwd 文件`（路径遍历攻击）
2. 观察工具结果中的 error_code 字段

**预期结果**：
- 操作被拒绝
- `error_code` 为 `9004`（TOOL_PATH_OUT_OF_BOUNDS）
- `error` 消息包含"路径不在工作区内"

### 5.2 参数缺失错误码

**测试步骤**：
1. 模拟 LLM 调用 read_file 但不提供 path 参数
2. 观察工具结果

**预期结果**：
- `error_code` 为 `9002`（TOOL_INVALID_PARAMS）
- `error` 消息包含"缺少文件路径"

### 5.3 Handler 不存在错误码

**测试步骤**：
1. 模拟调用不存在的 handler（如 `nonexistent_handler`）
2. 观察 executor 返回

**预期结果**：
- `error_code` 为 `2006`（AGENT_HANDLER_NOT_FOUND）

### 5.4 权限拒绝错误码

**测试步骤**：
1. 向 Agent 发送：`读取 C:\Windows\system32\config\SAM 文件`（工作区外）
2. 观察工具结果

**预期结果**：
- `error_code` 为 `3011`（DOC_PERMISSION_DENIED）

---

## 六、阶段五 P4-10：Validator 补充测试

### 6.1 Markdown 验证

**测试步骤**：
1. 创建以下问题的 Markdown 文件：
   - 未闭合代码块（`` ``` `` 单独出现）
   - 标题层级跳跃（H1 直接到 H3）
   - 行尾空白
   - 连续空行超过 3 行
2. 向 Agent 发送：`验证 problem.md 文件`

**预期结果**：
- 返回 warnings 列表，包含以下类型：
  - `UNCLOSED_CODE_BLOCK` - 未闭合代码块
  - `HEADING_LEVEL_SKIP` - 标题层级跳跃
  - `TRAILING_WHITESPACE` - 行尾空白
  - `EXCESSIVE_BLANK_LINES` - 连续空行过多
- 返回 stats 统计信息（链接数、图片数、表格数）

### 6.2 纯文本验证

**测试步骤**：
1. 创建以下问题的 txt 文件：
   - CRLF 和 LF 混用
   - 制表符和空格混用缩进
   - 单行超过 500 字符
   - 连续空行超过 5 行
2. 向 Agent 发送：`验证 problem.txt 文件`

**预期结果**：
- 返回 warnings 列表，包含：
  - `MIXED_LINE_ENDINGS` - 混用换行符
  - `MIXED_INDENT` - 混用缩进
  - `LONG_LINES` - 单行过长
  - `EXCESSIVE_BLANK_LINES` - 连续空行过多

---

## 七、阶段二 P1：文档读取扩展测试

### 7.1 Word 字符级格式读取

**测试步骤**：
1. 创建包含不同字体、字号、颜色的 Word 文档
2. 向 Agent 发送：`读取 sample.docx，包含格式信息`

**预期结果**：
- 返回 Run 级字符属性：字体名、字号、粗体、斜体、下划线、颜色
- 表格结构详细信息（合并单元格、边框）
- 节信息（页面尺寸、边距）
- 页眉页脚内容

### 7.2 Excel 扩展读取

**测试步骤**：
1. 创建包含公式、图表、合并单元格、批注的 Excel 文件
2. 向 Agent 发送：`读取 sample.xlsx，包含公式和图表信息`

**预期结果**：
- 返回公式内容（如 `=SUM(A1:A10)`）
- 图表信息（类型、数据范围、标题）
- 合并单元格区域列表
- 批注内容和作者

### 7.3 PDF 扩展读取

**测试步骤**：
1. 创建包含表单、注释、表格的 PDF 文件
2. 向 Agent 发送：`读取 sample.pdf，包含表单和注释`

**预期结果**：
- 表单字段信息（字段名、类型、值）
- 注释内容（高亮、批注）
- 表格提取结果（表格行列数据）
- 布局信息（页面尺寸、文本位置）

### 7.4 PPT 扩展读取

**测试步骤**：
1. 创建包含备注、形状详细信息的 PPT
2. 向 Agent 发送：`读取 sample.pptx，包含备注和形状详情`

**预期结果**：
- 幻灯片备注内容
- 形状详细信息（类型、位置、大小、文本）
- 母版信息

---

## 八、阶段一 P0：关键修复测试

### 8.1 路径遍历防护

**测试步骤**：
1. 尝试多种路径遍历攻击：
   - `../../../etc/passwd`
   - `..\..\Windows\System32`
   - 绝对路径 `/etc/passwd` 或 `C:\Windows\System32`

**预期结果**：
- 所有越界路径被拒绝
- 返回路径安全校验失败错误
- 日志记录警告信息

### 8.2 UTF-8 安全切片

**测试步骤**：
1. 搜索包含中文的文件内容
2. 读取包含中文的文件

**预期结果**：
- 不出现 panic
- 中文内容正确显示，无乱码

### 8.3 PPT LibreOffice 集成

**测试步骤**：
1. 向 Agent 发送：`将 sample.pptx 转换为 pdf`
2. 确认系统已安装 LibreOffice

**预期结果**：
- 转换成功生成 PDF 文件
- PDF 内容与 PPT 一致

---

## 测试结果记录模板

| 测试项 | 测试日期 | 测试结果 | 备注 |
|--------|----------|----------|------|
| 1.1 rename_file | | | |
| 1.2 copy_file | | | |
| 1.3 delete_directory | | | |
| 1.4 get_file_hash | | | |
| 1.5 read_file_lines | | | |
| 2.1 Word 主题 | | | |
| 2.2 PPT 配色 | | | |
| 2.3 Excel 表头 | | | |
| 3.1 Schema 一致性 | | | |
| 4.1 中文截断 | | | |
| 5.1 路径越界错误码 | | | |
| 5.2 参数缺失错误码 | | | |
| 6.1 Markdown 验证 | | | |
| 6.2 纯文本验证 | | | |
| 7.1 Word 格式读取 | | | |
| 7.2 Excel 扩展读取 | | | |
| 7.3 PDF 扩展读取 | | | |
| 7.4 PPT 扩展读取 | | | |
| 8.1 路径遍历防护 | | | |
| 8.3 PPT 转换 | | | |

---

## 自动化测试

以下命令可用于自动化验证：

```powershell
# Rust 编译和测试
cd src-tauri
cargo check
cargo clippy --all-targets
cargo test

# Python 语法验证
python -c "import ast, os; [ast.parse(open(os.path.join(r,f),encoding='utf-8').read()) for r,_,fs in os.walk('sidecar') for f in fs if f.endswith('.py')]; print('OK')"

# Python 导入和功能测试
python -c "import sys; sys.path.insert(0, 'sidecar'); from handlers.doc_helpers.common import apply_theme, THEME_COLORS; from handlers.doc_helpers.ppt_helpers import create_ppt_doc, get_ppt_color_scheme; from handlers.doc_helpers.excel_helpers import create_excel_doc, apply_excel_header_style, THEME; print('All imports OK')"
```
