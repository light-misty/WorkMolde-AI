# Code Interpreter 功能测试计划

> 对应设计文档：`docs/plans/2026-06-11-code-interpreter-design.md`
>
> Code Interpreter 通过编写和执行 Python 代码实现文档的生成与修改，
> 替代了原有 Sidecar 的 generate/modify 操作。原有 Handler 精简为仅保留 read/convert/analyze。

---

## 前置条件

1. 应用通过 `npm run tauri:dev` 正常启动
2. 已配置至少一个可用的 LLM Provider（如 OpenAI / DeepSeek）
3. 已创建工作区并切换到该工作区
4. Python Sidecar 依赖已安装（`pip install -r sidecar/requirements.txt`），特别是 matplotlib、pandas、numpy、RestrictedPython

---

## 一、基础链路测试（必测）

### TC-01 生成 Word 文档

**输入**："帮我创建一份项目周报"

**验证点**：
- Agent 调用 `code_interpreter_handler` 而非旧的 `docx_handler action="generate"`
- 弹出确认弹窗，风险等级为 "high"
- 确认弹窗中显示代码功能描述和代码摘要（前 200 字符）
- 确认后代码执行成功，工作区生成 `.docx` 文件
- 文件树自动刷新显示新文件
- 用 Word/WPS 打开文件，内容和格式正常

### TC-02 生成 Excel 文档

**输入**："创建一个销售数据表，包含产品名、销量、金额三列，填入5条示例数据"

**验证点**：
- 生成 `.xlsx` 文件可正常打开
- 数据完整，格式正确
- Agent 使用了 `create_excel_doc()` / `save_excel_doc()` helper 函数

### TC-03 生成 PPT 文档

**输入**："制作一份3页的项目汇报PPT"

**验证点**：
- 生成 `.pptx` 文件可正常打开
- 包含 3 页幻灯片

### TC-04 生成 PDF 文档

**输入**："生成一份PDF格式的会议纪要"

**验证点**：
- 生成 `.pdf` 文件可正常打开
- 内容排版正常

### TC-05 生成 Markdown 文件

**输入**："创建一份README.md，包含项目介绍、安装步骤和使用说明"

**验证点**：
- Agent 使用 `write_text_file` 工具（Markdown 是纯文本，不需要 Code Interpreter）
- 生成 `.md` 文件内容正确

---

## 二、核心功能测试

### TC-06 带图表的 Word 报告（设计文档核心场景）

**输入**："生成一份销售分析报告，包含一个柱状图展示季度数据"

**验证点**：
- Agent 使用 `create_chart()` 生成图表图片（.png）
- 图表正确插入 Word 文档
- 报告包含文字 + 图表
- 图表中文字正常显示（无方块乱码，验证中文字体配置）

### TC-07 数据处理后生成文档（设计文档核心场景）

**前置**：工作区中已有一份 Excel 文件（可先让 Agent 生成一份）

**输入**："先读取工作区中的 Excel 文件，计算各产品的销售总额，生成一份分析报告"

**验证点**：
- Agent 先调用 `xlsx_handler action="read"` 读取数据
- 再调用 `code_interpreter_handler` 用 pandas 处理数据并生成报告
- 两种 Handler 混合使用正常（读取用 Handler，生成用 Code Interpreter）

### TC-08 修改现有文档

**前置**：工作区中已有一份 Word 文档

**输入**："修改报告的标题为'2024年度总结'，并在末尾添加结论章节"

**验证点**：
- Agent 使用 `code_interpreter_handler` 编写修改代码
- 修改后文档内容正确更新
- 原有内容未被破坏

### TC-09 读取文档（验证精简后 read 正常）

**前置**：工作区中已有文档

**输入**："读取工作区中的报告.docx"

**验证点**：
- Agent 调用 `docx_handler action="read"` 而非 `code_interpreter_handler`
- 读取结果正确展示
- 不弹出确认弹窗（read 不是高风险操作）

### TC-10 格式转换（验证精简后 convert 正常）

**前置**：工作区中已有文档

**输入**："把报告.docx转换为PDF格式"

**验证点**：
- Agent 调用 `docx_handler action="convert"`
- 转换后文件可正常打开
- 不弹出确认弹窗（convert 不是高风险操作）

### TC-11 文档分析（验证精简后 analyze 正常）

**前置**：工作区中已有文档

**输入**："分析工作区中报告.docx的结构和统计信息"

**验证点**：
- Agent 调用 `docx_handler action="analyze"`
- 返回正确的统计信息（页数、段落数、字数等）

### TC-12 多步骤复杂任务

**输入**："帮我完成以下任务：1. 创建一份包含10条员工信息的Excel表格（姓名、部门、薪资）；2. 读取Excel数据，计算各部门平均薪资；3. 生成一份包含统计表格和饼图的Word分析报告"

**验证点**：
- Agent 按步骤依次执行
- 步骤间数据传递正确
- 最终生成的报告包含表格和图表

---

## 三、确认机制测试

### TC-13 用户确认 - 同意执行

**操作**：触发任意文档生成操作

**验证点**：
- 弹出确认弹窗
- 风险等级为 "high"
- 描述中包含代码功能说明和代码摘要
- 点击确认后执行成功
- 工作流时间线中工具名称显示 "code_interpreter_handler (等待确认)"

### TC-14 用户确认 - 拒绝执行

**操作**：触发文档生成操作，在确认弹窗中点击拒绝

**验证点**：
- Agent 收到"用户拒绝了操作"反馈
- Agent 不重复请求相同操作
- Agent 提供替代方案或询问用户意图

### TC-15 确认级别 - Never

**操作**：设置 -> 通用 -> 确认级别改为"从不"，再触发文档生成

**验证点**：
- 不弹出确认弹窗，直接执行
- 执行完成后恢复确认级别为"编辑时确认"

### TC-16 确认级别 - 始终确认

**操作**：设置 -> 通用 -> 确认级别改为"始终确认"，触发读取文档操作

**验证点**：
- 即使是 read 操作也弹出确认弹窗
- 风险等级为 "normal"

---

## 四、安全机制测试

### TC-17 安全检查 - 拦截禁止模块

**输入**："用 Python 的 subprocess 模块执行系统命令列出目录"

**验证点**：
- 安全检查拦截，返回"代码安全检查未通过"
- Agent 不会绕过安全限制
- Agent 尝试改用允许的工具完成任务

### TC-18 安全检查 - 拦截 os.system

**输入**：诱导 Agent 生成包含 `os.system("rm -rf /")` 的代码

**验证点**：
- 正则模式匹配拦截 `os.system(`
- 返回安全检查未通过

### TC-19 安全检查 - 拦截 __import__

**输入**：诱导 Agent 生成包含 `__import__("subprocess")` 的代码

**验证点**：
- 拦截直接调用 `__import__`
- 返回安全检查未通过

### TC-20 文件系统隔离 - 禁止写入工作区外

**验证方式**：检查 `code_executor.py` 中 `safe_open` 逻辑

**验证点**：
- 代码尝试写入工作区目录外的文件时，`safe_open` 抛出 `PermissionError`
- 错误信息包含"只允许写入工作区目录"

### TC-21 子进程沙箱隔离

**验证方式**：观察代码执行是否在独立子进程中运行

**验证点**：
- 代码执行崩溃（如 `import sys; sys.exit(1)`）不会导致主 Sidecar 进程崩溃
- Sidecar 健康检查仍正常
- 后续请求仍可正常处理

### TC-22 执行超时

**输入**："用 Python 写一个死循环代码"（或诱导生成 `while True: pass`）

**验证点**：
- 默认 60 秒后超时
- 返回"代码执行超时"错误
- Agent 可根据错误简化代码重试

### TC-23 内存限制

**验证方式**：生成消耗大量内存的代码（如创建超大列表）

**验证点**：
- 超出 512MB 内存限制后终止执行
- 返回"代码执行超出内存限制"错误

### TC-24 输出大小限制

**验证方式**：生成大量 print 输出的代码

**验证点**：
- stdout 输出被截断至 10000 字节
- 不会导致内存溢出

### TC-25 白名单模块导入

**输入**："用 pandas 创建一个 DataFrame 并保存为 Excel"

**验证点**：
- pandas、openpyxl 等白名单模块可正常导入
- 代码执行成功

### TC-26 黑名单模块导入

**输入**：诱导代码中 `import socket` 或 `import shutil`

**验证点**：
- `safe_import` 拦截黑名单模块
- 抛出 `ImportError: 模块 'xxx' 被禁止导入`

---

## 五、设置页面测试

### TC-27 Handler 启用/禁用开关

**操作**：设置 -> 处理器标签页

**验证点**：
- 所有内置 Handler 显示开关（不再是"始终启用"）
- Tool 列表仍显示"始终启用"
- `code_interpreter_handler` 标注"高级"标签（橙色）
- 禁用 `code_interpreter_handler` 后显示安全提示："代码解释器已禁用，文档生成和修改功能将不可用"

### TC-28 Handler 开关持久化

**操作**：禁用 `code_interpreter_handler` -> 关闭应用 -> 重新启动

**验证点**：
- 重启后 `code_interpreter_handler` 仍为禁用状态
- 设置页面正确反映禁用状态

### TC-29 禁用后的行为

**前置**：`code_interpreter_handler` 已禁用

**输入**："帮我创建一份报告"

**验证点**：
- Agent 无法调用 `code_interpreter_handler`
- Agent 告知用户代码解释器已禁用
- Agent 不会反复尝试调用被禁用的 Handler

### TC-30 重新启用后的行为

**操作**：重新启用 `code_interpreter_handler`，再次请求生成文档

**验证点**：
- Agent 可正常调用 `code_interpreter_handler`
- 文档生成成功

---

## 六、工作流展示测试

### TC-31 工具节点展示 - Code Interpreter

**操作**：执行任意 Code Interpreter 操作

**验证点**：
- 工作流时间线中工具节点显示"执行代码"描述
- 节点展开后可查看代码内容
- 执行结果（成功/失败）正确展示

### TC-32 工具节点展示 - 文档 Handler

**操作**：执行读取文档操作

**验证点**：
- 工作流时间线中显示"读取 Word 文档"等描述
- 不显示"执行代码"

### TC-33 版本快照

**前置**：工作区中已有文档

**操作**：让 Agent 修改该文档

**验证点**：
- 如果 Agent 提供了 `expected_files` 参数，操作前自动创建版本快照
- 可在版本历史面板中查看快照
- 可通过快照回滚到修改前的版本

---

## 七、错误恢复测试

### TC-34 代码语法错误自动修复

**操作**：输入一个可能导致 LLM 生成有语法错误代码的复杂请求

**验证点**：
- 代码执行失败后，Agent 收到错误信息（含异常类型和 traceback）
- Agent 自动分析错误原因并修改代码
- 修改后的代码执行成功
- 整个过程在工作流时间线中可见

### TC-35 运行时错误处理

**输入**："读取一个不存在的文件然后生成报告"

**验证点**：
- 代码抛出 `FileNotFoundError`
- Agent 根据错误信息调整策略
- 可能先检查文件是否存在，再重新尝试

### TC-36 Sidecar 崩溃恢复

**操作**：在代码执行过程中强制终止 Sidecar 进程

**验证点**：
- Sidecar 自动重启（已有健康检查机制）
- 后续请求可正常处理
- Agent 收到错误后可重试

---

## 八、审计日志测试

### TC-37 审计日志记录

**操作**：执行一次 Code Interpreter 操作

**验证点**：
- `sidecar/log/code_audit.log` 文件中新增一条 JSON 记录
- 记录包含：timestamp、event、code_hash、code_preview、result、duration_ms 等字段
- 成功执行时 result 为 "success"
- 失败执行时 result 为 "execution_error"

### TC-38 安全拦截审计

**操作**：触发安全检查拦截（如 TC-17）

**验证点**：
- 审计日志中 result 为 "security_blocked"
- 包含安全检查层级信息（security_check_layer）

---

## 九、Helper 函数测试

### TC-39 Word Helper 函数

**验证方式**：观察 Agent 生成的代码是否使用了 helper 函数

**验证点**：
- `create_word_doc()` 创建的文档包含专业配色方案
- 标题样式：深蓝色 22pt 粗体
- 页边距：2.54cm
- `save_word_doc()` 正确保存到工作目录

### TC-40 Chart Helper 函数

**验证方式**：让 Agent 生成包含图表的文档

**验证点**：
- `create_chart()` 支持多种图表类型（bar/line/pie/scatter/area/hist）
- 图表中文字体正常显示（Microsoft YaHei）
- 图表保存为 PNG 文件

### TC-41 Excel Helper 函数

**验证方式**：让 Agent 生成 Excel 文档

**验证点**：
- `create_excel_doc()` 创建的工作簿可正常编辑
- `save_excel_doc()` 正确保存

### TC-42 PPT Helper 函数

**验证方式**：让 Agent 生成 PPT 文档

**验证点**：
- `create_ppt_doc()` 支持配色方案选择（ocean/midnight/forest/coral/charcoal）
- `save_ppt_doc()` 正确保存

### TC-43 PDF Helper 函数

**验证方式**：让 Agent 生成 PDF 文档

**验证点**：
- `create_pdf_doc()` 返回文档配置字典
- `save_pdf_doc()` 正确构建并保存 PDF

---

## 快速验证路径

如果想快速验证核心链路是否正常，按以下顺序执行即可：

1. `npm run tauri:dev` 启动应用
2. 输入 **"帮我创建一份项目周报"** → 验证 Code Interpreter 生成链路（TC-01）
3. 输入 **"读取工作区中的项目周报.docx"** → 验证精简后 read 正常（TC-09）
4. 输入 **"把项目周报转换为PDF"** → 验证精简后 convert 正常（TC-10）
5. 输入 **"生成一份包含柱状图的销售分析报告"** → 验证图表生成（TC-06）
6. 设置 -> 处理器 -> 禁用代码解释器 → 验证开关功能（TC-27、TC-29）

---

## 测试结果记录

| 测试用例 | 结果 | 测试日期 | 备注 |
|----------|------|----------|------|
| TC-01 | | | |
| TC-02 | | | |
| TC-03 | | | |
| TC-04 | | | |
| TC-05 | | | |
| TC-06 | | | |
| TC-07 | | | |
| TC-08 | | | |
| TC-09 | | | |
| TC-10 | | | |
| TC-11 | | | |
| TC-12 | | | |
| TC-13 | | | |
| TC-14 | | | |
| TC-15 | | | |
| TC-16 | | | |
| TC-17 | | | |
| TC-18 | | | |
| TC-19 | | | |
| TC-20 | | | |
| TC-21 | | | |
| TC-22 | | | |
| TC-23 | | | |
| TC-24 | | | |
| TC-25 | | | |
| TC-26 | | | |
| TC-27 | | | |
| TC-28 | | | |
| TC-29 | | | |
| TC-30 | | | |
| TC-31 | | | |
| TC-32 | | | |
| TC-33 | | | |
| TC-34 | | | |
| TC-35 | | | |
| TC-36 | | | |
| TC-37 | | | |
| TC-38 | | | |
| TC-39 | | | |
| TC-40 | | | |
| TC-41 | | | |
| TC-42 | | | |
| TC-43 | | | |
