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

### 日志文件位置

测试过程中请同步检查日志文件以验证可观测性：

| 日志文件 | 路径 | 说明 |
|---------|------|------|
| workmolde.log | `D:\DeskTop\WorkMolde-AI\log\workmolde.log` | Rust 主进程日志（Agent 迭代、Tool 执行、错误） |
| sidecar.log | `D:\DeskTop\WorkMolde-AI\src-tauri\target\debug\log\sidecar.log` | Python Sidecar 日志（Handler 执行、代码执行） |

**日志检查命令**（PowerShell）：
```powershell
# 查看最近的 WARN/ERROR
Select-String -Path 'D:\DeskTop\WorkMolde-AI\log\workmolde.log' -Pattern 'WARN|ERROR' | Select-Object -Last 20

# 查看 Agent 迭代和完成状态
Select-String -Path 'D:\DeskTop\WorkMolde-AI\log\workmolde.log' -Pattern 'Agent 迭代|Agent 执行完成|Agent 执行失败'

# 查看工具结果截断日志
Select-String -Path 'D:\DeskTop\WorkMolde-AI\log\workmolde.log' -Pattern '已截断|工具结果'
```

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
| pptx_handler | （无，PPT 转 PDF 已不再支持） |
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
- 操作被拒绝
- `error_code` 为 `9004`（TOOL_PATH_OUT_OF_BOUNDS）
- `error` 消息包含"路径不在工作区内，拒绝访问"
- **注意**：第一轮测试发现该文件因权限不足导致 canonicalize 失败，原代码返回"文件不存在或路径无效"且 error_code=None。已修复为统一使用 `validate_existing_path_in_workspace`，现在会先做词法归一化检查，正确识别绝对路径越界

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

> 注：原 8.3 PPT LibreOffice 集成测试已移除。系统不再支持 PPT 转 PDF，相关 LibreOffice 集成代码已从 ppt_handler.py 中删除。如需 PPT 转 PDF，请使用 write_script + run_command Tool 编写脚本自行实现。

---

## 九、安全防护一致性测试（第一轮发现的问题）

> 第一轮测试发现：5 个新增工具使用了带词法归一化防线的 `validate_existing_path_in_workspace`，但 4 个老工具（read_file、file_info、file_exists、delete_file）仍使用内联校验逻辑，存在安全防护不一致。已统一修复，本章验证修复效果。

### 9.1 老工具 `../` 越界防护

**测试步骤**：
1. 向 Agent 发送：`读取 ../outside.txt 文件`（相对路径越界，文件不存在）
2. 向 Agent 发送：`获取 ../outside.txt 的文件信息`
3. 向 Agent 发送：`检查 ../outside.txt 是否存在`
4. 向 Agent 发送：`删除 ../outside.txt 文件`

**预期结果**：
- 所有工具返回"路径不在工作区内，拒绝访问"（而非"文件不存在或路径无效"）
- `error_code` 为 `9004`（TOOL_PATH_OUT_OF_BOUNDS）
- **关键验证**：`../` 越界不应泄露文件存在性信息

**日志验证**：
```
Select-String -Path 'D:\DeskTop\WorkMolde-AI\log\workmolde.log' -Pattern '路径越界|路径不在工作区内'
```

### 9.2 绝对路径越界防护（canonicalize 失败场景）

**测试步骤**：
1. 向 Agent 发送：`读取 C:\Windows\System32\config\SAM 文件`（存在但无权限）
2. 向 Agent 发送：`读取 C:\Windows\System32\drivers\etc\hosts 文件`（存在且可读但越界）

**预期结果**：
- 两个文件都返回"路径不在工作区内，拒绝访问"
- `error_code` 为 `9004`
- **关键验证**：canonicalize 失败（权限不足）时，词法归一化防线仍能识别越界

**背景说明**：
- 第一轮测试中，`C:\Windows\System32\config\SAM` 因权限不足导致 canonicalize 失败，原代码返回"文件不存在或路径无效"且 error_code=None
- 修复后增加词法归一化防线，不依赖文件系统即可识别绝对路径越界

### 9.3 FileExistsTool 安全防线验证

**测试步骤**：
1. 向 Agent 发送：`检查 ../outside.txt 是否存在`（越界且不存在）
2. 向 Agent 发送：`检查 C:\Windows\System32\drivers\etc\hosts 是否存在`（越界但存在）

**预期结果**：
- 越界路径返回错误"路径不在工作区内，拒绝访问"（而非 `exists: false`）
- `error_code` 为 `9004`
- **关键验证**：FileExistsTool 不应通过 `exists: false` 绕过安全校验

**背景说明**：
- 第一轮测试发现 FileExistsTool 原逻辑：`if let Ok(canonicalize)` —— canonicalize 失败时完全跳过安全校验，直接返回 `exists: false`，攻击者可探测工作区外文件存在性
- 修复后：即使路径不存在，也先做词法归一化校验，越界路径直接拒绝

### 9.4 深层 `..` 回退验证

**测试步骤**：
1. 在工作区创建 `a/b/c/file.txt` 目录结构
2. 向 Agent 发送：`读取 a/b/../b/c/file.txt`（合法回退到工作区内，解析为 `a/b/c/file.txt`）
3. 向 Agent 发送：`读取 a/b/../../../outside.txt`（先回退再越界，解析为 `../outside.txt`）

**预期结果**：
- `a/b/../b/c/file.txt` 读取成功（合法回退，词法解析后仍在工作区内）
- `a/b/../../../outside.txt` 返回"路径不在工作区内"（越界拒绝）

**验证目的**：词法归一化能正确处理多层 `..` 回退，不会误判合法路径为越界

**路径设计说明**（第二版修正）：
- 原测试用例 `a/b/../../c/file.txt` 词法解析为 `c/file.txt`，但工作区实际文件在 `a/b/c/file.txt`，导致安全校验通过但文件不存在，无法真正验证"合法回退读取成功"
- 修正为 `a/b/../b/c/file.txt`，词法解析为 `a/b/c/file.txt`，与实际文件路径一致，能真正验证合法回退场景

---

## 十、日志可观测性测试（第一轮发现的问题）

> 第一轮测试发现：工具结果截断和 reasoning_content 压缩都执行了，但日志中无任何记录，导致智能体读大文档时中间章节丢失无法诊断。已补充日志，本章验证可观测性。

### 10.1 工具结果截断日志

**测试步骤**：
1. 创建一个超过 6000 字符的文件（如 `large_chinese.txt`，6629 字符）
2. 向 Agent 发送：`读取 large_chinese.txt 的全部内容`
3. 检查 workmolde.log

**预期结果**：
- 工具结果包含 `[已截断: 原始 N 字符，保留头部 4200 + 尾部 1800，省略中间 N 字符]`
- **日志包含 INFO 记录**：
  ```
  [INFO] 工具结果内容字段已截断, tool=read_file, 原始 6629 字符 -> 保留头部 4200 + 尾部 1800, 省略中间 629 字符
  ```

**两级截断机制说明**：
- **第一级（常规截断）**：工具结果 content 字段超过 `MAX_TOOL_RESULT_CHARS=6000` 字符时触发，按 70/30 比例保留头尾，记录 INFO 日志 `工具结果内容字段已截断`
- **第二级（字符串级安全截断）**：序列化后的工具结果整体超过 `MAX_TOOL_RESULT_CHARS * 2 = 12000` 字符时触发（极端情况，如嵌套结构、二进制数据），仅保留前 12000 字符，记录 WARN 日志 `工具结果字符串级安全截断`
- 第二级属于防御性兜底，**可能丢失关键信息**，但避免单条工具结果占用过多上下文。第二轮测试中 xlsx_handler 56538 字节被截断到 12000 字符即属于此场景

**日志验证命令**：
```powershell
# 检查常规截断（INFO）
Select-String -Path 'D:\DeskTop\WorkMolde-AI\log\workmolde.log' -Pattern '工具结果内容字段已截断'

# 检查字符串级安全截断（WARN，极端情况）
Select-String -Path 'D:\DeskTop\WorkMolde-AI\log\workmolde.log' -Pattern '工具结果字符串级安全截断'
```

### 10.2 reasoning_content 压缩日志

**测试步骤**：
1. 执行需要多轮迭代的复杂任务（如完成多个测试项）
2. 检查 workmolde.log 中的压缩记录

**预期结果**：
- 日志包含 DEBUG 记录：
  ```
  [DEBUG] 压缩早期 reasoning_content: 原始长度=N, 压缩后长度=M, 消息索引=K
  ```
- **压缩比例验证**：
  - 阈值 1200 字符（原 500，已提升）
  - 保留 500 字符（原 200，已提升）
  - 压缩比例应 ≤ 65%（第一轮测试中 2625→1046，约 60%；第二轮测试中 1533→978，约 64%；原 2625→448 约 83%）

**日志验证命令**：
```powershell
Select-String -Path 'D:\DeskTop\WorkMolde-AI\log\workmolde.log' -Pattern '压缩早期 reasoning_content' | Select-Object -Last 10
```

### 10.3 Agent 迭代次数监控

**测试步骤**：
1. 执行复杂任务（如一次性完成 5 个工具测试）
2. 检查 Agent 迭代次数

**预期结果**：
- Agent 正常完成，日志显示"Agent 执行完成, 总步骤=N"
- 总步骤 ≤ 100（MAX_ITERATIONS 统一为 100，见 `executor.rs:100` 和 project_memory.md 硬约束）
- **不应出现**："Agent 执行超过最大迭代次数"

**日志验证命令**：
```powershell
Select-String -Path 'D:\DeskTop\WorkMolde-AI\log\workmolde.log' -Pattern 'Agent 执行完成|Agent 执行失败|超过最大迭代'
```

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
| 4.2 英文截断 | | | |
| 5.1 路径越界错误码 | | | |
| 5.2 参数缺失错误码 | | | |
| 5.3 Handler 不存在错误码 | | | |
| 5.4 权限拒绝错误码 | | | |
| 6.1 Markdown 验证 | | | |
| 6.2 纯文本验证 | | | |
| 7.1 Word 格式读取 | | | |
| 7.2 Excel 扩展读取 | | | |
| 7.3 PDF 扩展读取 | | | |
| 7.4 PPT 扩展读取 | | | |
| 8.1 路径遍历防护 | | | |
| 8.3 PPT 转换 | | | |
| 9.1 老工具 ../ 越界防护 | | | |
| 9.2 绝对路径越界防护 | | | |
| 9.3 FileExistsTool 安全防线 | | | |
| 9.4 深层 .. 回退验证 | | | |
| 10.1 工具结果截断日志 | | | |
| 10.2 reasoning 压缩日志 | | | |
| 10.3 Agent 迭代次数监控 | | | |

---

## 测试方法学指导（基于第一轮测试经验）

### 1. 测试结果判定原则

**不要仅依赖智能体自述结果**。第一轮测试发现，智能体可能将测试标记为"通过"，但日志中存在 ERROR。判定测试通过前，必须检查日志：

```powershell
# 检查测试时间范围内的所有 ERROR
Select-String -Path 'D:\DeskTop\WorkMolde-AI\log\workmolde.log' -Pattern 'ERROR' | Select-Object -Last 20
```

**判定标准**：
- ✅ 通过：功能正常 + 日志无 ERROR（或 ERROR 属于预期的边界测试，如 8.1 路径遍历拒绝）
- ⚠️ 部分通过：核心功能正常但存在非预期 ERROR（如 7.2 Excel 读取时 MergedCell 错误）
- ❌ 未通过：功能失败或存在安全漏洞

### 2. 测试日期规范

**测试日期必须使用实际测试日期**，不要使用 LLM 生成的日期。第一轮测试发现智能体可能产生日期幻觉（如写入 `2024-12-06` 而实际是 `2026-06-28`）。

**推荐做法**：
- 测试前记录当前日期：`Get-Date -Format 'yyyy-MM-dd'`
- 测试后核对智能体填写的日期与实际日期是否一致

### 3. 日志 ERROR 分类处理

测试过程中遇到的 ERROR 分为三类：

| 类型 | 示例 | 处理方式 |
|------|------|---------|
| 预期 ERROR | 8.1 路径遍历被拒绝 | 正常，测试通过 |
| 边界测试 ERROR | 5.4 权限拒绝错误码 | 正常，测试通过 |
| 非预期 ERROR | 7.2 MergedCell 读取错误 | 需排查根因，可能是产品 Bug 或测试代码问题 |

### 4. 智能体困惑识别

第一轮测试发现，智能体在以下情况可能陷入困惑：

- **工具结果截断**：智能体读取大文档时，中间章节被截断丢失，智能体可能质疑"文档被截断"
- **reasoning_content 压缩**：多轮迭代后早期推理被压缩，智能体可能丢失任务上下文
- **迭代次数耗尽**：复杂任务可能超过 MAX_ITERATIONS，智能体被强制停止

**识别方法**：检查日志中的"压缩早期 reasoning_content"和"Agent 执行失败"记录

### 5. 测试反馈写入验证

第一轮测试发现，智能体声称"已写入测试结果"但用户感觉文件无变化。实际是文件已更新但变化较小（如仅更新表格一行）。

**验证方法**：
```powershell
# 检查文件修改时间
Get-Item 'D:\test_workspace\tools_handlers_validation.md' | Select-Object LastWriteTime

# 查看文件末尾的测试结果记录表格
Get-Content 'D:\test_workspace\tools_handlers_validation.md' -Tail 30
```

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
python -c "import sys; sys.path.insert(0, 'sidecar'); from handlers.word_handler import WordHandler; from handlers.excel_handler import ExcelHandler; from handlers.ppt_handler import PptHandler; from handlers.pdf_handler import PdfHandler; from handlers.markdown_handler import MarkdownHandler; print('All imports OK')"
```

### 第一轮测试后的补充验证命令

```powershell
# 验证路径安全校验一致性（第九章）
# 检查所有工具是否使用统一的 validate_existing_path_in_workspace
Select-String -Path 'D:\DeskTop\WorkMolde-AI\src-tauri\src\services\tool\builtin.rs' -Pattern 'validate_existing_path_in_workspace' | Measure-Object | Select-Object -ExpandProperty Count
# 预期：≥ 9（1 个函数定义 + 8 个调用点：read_file/file_info/file_exists/delete_file/rename_file/copy_file/delete_directory/get_file_hash/read_file_lines）

# 验证 reasoning_content 压缩阈值（第十章）
Select-String -Path 'D:\DeskTop\WorkMolde-AI\src-tauri\src\services\agent\context.rs' -Pattern 'REASONING_COMPRESS_THRESHOLD|REASONING_COMPRESS_KEEP'
# 预期：THRESHOLD=1200, KEEP=500

# 验证 MAX_ITERATIONS（第十章）
Select-String -Path 'D:\DeskTop\WorkMolde-AI\src-tauri\src\services\agent\executor.rs' -Pattern 'max_iterations'
# 预期：100（commands/agent.rs、executor.rs、context.rs 三处保持一致）

# 验证工具结果截断日志（第十章）
Select-String -Path 'D:\DeskTop\WorkMolde-AI\src-tauri\src\services\agent\executor.rs' -Pattern '工具结果内容字段已截断'
# 预期：找到 log::info! 调用
```
