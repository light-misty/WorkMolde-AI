# DocAgent Phase 1 端到端测试文档

## 审查结论

经过对项目代码、文档、提交历史的全面审查，**Phase 1 (MVP) 全部 5 个 Sprint 的开发任务已完成**，构建验证结果如下：

| 验证项 | 结果 |
|--------|------|
| 前端 TypeScript 类型检查 + Vite 构建 | 通过 (73 modules, 0 errors) |
| Rust 后端 cargo check | 通过 (0 errors, 0 warnings) |
| Python Sidecar 依赖 | 通过 (docx/openpyxl/pptx/fitz/reportlab 均可导入) |
| Git 提交历史 | 12 次提交，覆盖从项目初始化到功能完善全流程 |

---

## 前置条件

1. **运行环境**: Windows 11, Node.js, Python 3.12+, Rust 1.80+
2. **Python 依赖**: 已安装 `pip install -r sidecar/requirements.txt`
3. **LLM API Key**: 至少配置一个可用的 OpenAI 兼容 API（用于 Agent 交互测试）
4. **启动命令**: `npm run tauri:dev`

---

## 测试用例

### E2E-01: 应用启动与初始化

**目标**: 验证应用能正常启动，所有初始化流程正确执行

**步骤**:
1. 执行 `npm run tauri:dev` 启动应用
2. 观察应用窗口是否正常显示（标题: DocAgent - AI文档处理Agent）
3. 检查窗口尺寸是否为 1280x800（默认）
4. 检查窗口是否可调整大小

**预期结果**:
- 应用窗口正常显示，无白屏或崩溃
- 顶部栏显示应用名称和操作按钮
- 主区域显示"开始新会话"引导提示
- 右侧栏显示文件树、Agent 信息、Todo、Token 统计四个分区
- 底部输入框可正常聚焦

**验证点**:
- [ ] SQLite 数据库文件 `<app_data_dir>/docagent.db` 已创建
- [ ] 配置目录 `<app_data_dir>/config/` 已创建
- [ ] 日志文件 `log/sidecar.log` 存在（Sidecar 启动后）

---

### E2E-02: LLM Provider 配置

**目标**: 验证 LLM Provider 的增删改查和连接测试

**步骤**:
1. 点击右上角设置图标，打开设置弹窗
2. 切换到 LLM 配置标签页
3. 点击"添加 Provider"按钮
4. 填写表单:
   - 名称: `Test OpenAI`
   - 类型: `OpenAI`
   - API 地址: `https://api.openai.com/v1`
   - API Key: 有效的 OpenAI API Key
   - 模型: `gpt-4o-mini`
5. 点击"测试连接"按钮
6. 保存配置
7. 验证 Provider 列表中出现新添加的条目
8. 将该 Provider 设为默认
9. 尝试编辑 Provider 名称
10. 尝试删除 Provider

**预期结果**:
- 连接测试返回成功，显示延迟和模型信息
- Provider 列表正确显示所有 Provider
- 设为默认操作成功，默认标记正确切换
- 编辑和删除操作正常

**验证点**:
- [ ] 配置文件 `<app_data_dir>/config/llm_config.json` 正确保存
- [ ] 添加第一个 Provider 时自动设为默认
- [ ] 删除默认 Provider 后自动切换到第一个可用 Provider
- [ ] 无效 API Key 时连接测试返回明确的错误信息

---

### E2E-03: 工作区管理

**目标**: 验证工作区的添加、切换、移除功能

**步骤**:
1. 打开设置弹窗，切换到工作区标签页
2. 点击"添加工作区"
3. 选择一个本地目录路径，命名为 `测试工作区`
4. 保存后验证工作区列表中出现新条目
5. 点击"设为活动"切换到新工作区
6. 验证右侧栏文件树更新为新工作区的内容
7. 移除该工作区

**预期结果**:
- 工作区添加后列表即时更新
- 切换活动工作区后文件树刷新
- 移除工作区后列表更新，活动工作区自动回退

**验证点**:
- [ ] 配置文件 `<app_data_dir>/config/workspaces.json` 正确保存
- [ ] 文件树正确展示工作区目录结构
- [ ] 切换工作区后 Agent 的工作区路径同步更新

---

### E2E-04: Agent 对话交互（核心流程）

**目标**: 验证 Agent 完整的 Tool Calling 循环

**步骤**:
1. 确保已配置 LLM Provider 并设为默认
2. 确保已添加并激活一个工作区
3. 在输入框中输入: `帮我生成一份测试文档，标题为"项目周报"，内容包含本周工作总结和下周计划`
4. 按 Enter 发送
5. 观察工作流时间线:
   - 出现 User 节点（用户消息）
   - 出现 Thinking 节点（Agent 思考）
   - 出现 Tool 节点（调用 generate_document）
   - 出现 Result 节点（执行结果）
   - 出现 Reply 节点（Agent 最终回复）
6. 验证右侧栏 Todo 区域显示任务进度
7. 验证右侧栏 Token 统计更新

**预期结果**:
- 工作流时间线按顺序显示所有节点
- Agent 自动选择 generate_document 技能
- 文档在工作区目录中生成
- Agent 回复确认文档已生成
- Token 统计数字递增

**验证点**:
- [ ] 消息持久化到 SQLite（重启应用后可恢复）
- [ ] 流式输出过程中内容逐步显示
- [ ] Agent 执行期间输入框禁用
- [ ] Agent 执行完成后输入框恢复可用

---

### E2E-05: 文档生成（Word）

**目标**: 验证 Word 文档生成功能

**步骤**:
1. 发送消息: `生成一份 Word 文档，保存为 test_report.docx，标题是"测试报告"，内容包含三个段落`
2. 等待 Agent 执行完成
3. 在工作区目录中找到 `test_report.docx`
4. 用 Word 或 WPS 打开验证内容

**预期结果**:
- 文档成功生成
- 文档包含标题"测试报告"
- 文档包含三个段落内容
- 文档属性中作者字段正确

**验证点**:
- [ ] Sidecar 进程正常启动和通信
- [ ] 生成的 .docx 文件可正常打开
- [ ] 日志文件 `log/sidecar.log` 记录了处理过程

---

### E2E-06: 文档读取

**目标**: 验证读取已有文档内容

**步骤**:
1. 发送消息: `读取 test_report.docx 的内容`
2. 等待 Agent 执行完成
3. 观察 Agent 回复中是否包含文档内容

**预期结果**:
- Agent 调用 read_document 技能
- 返回文档的段落、表格、属性信息
- Agent 将内容整理后回复给用户

**验证点**:
- [ ] 不存在的文件路径返回明确的错误信息
- [ ] 读取结果包含段落数和表格数统计

---

### E2E-07: 文档修改

**目标**: 验证修改已有文档

**步骤**:
1. 发送消息: `修改 test_report.docx，将第一段文字替换为"这是修改后的内容"`
2. 等待 Agent 执行完成
3. 重新读取文档验证修改结果

**预期结果**:
- Agent 调用 modify_document 技能
- 由于 modify_document 是高风险操作，弹出确认对话框
- 用户确认后执行修改
- 修改结果正确反映在文档中

**验证点**:
- [ ] 高风险操作触发确认机制
- [ ] 用户拒绝时操作被取消
- [ ] 修改后文档内容正确更新

---

### E2E-08: 操作确认机制

**目标**: 验证高风险操作的用户确认流程

**步骤**:
1. 发送消息: `删除 test_report.docx`
2. 观察工作流时间线出现确认节点
3. 点击"确认执行"按钮
4. 验证文件被删除
5. 重新生成文档后，再次尝试删除，这次点击"取消操作"
6. 验证文件未被删除

**预期结果**:
- 删除操作触发确认对话框（风险等级: critical）
- 确认后文件被删除（可选创建备份）
- 取消后操作被跳过，Agent 回复"用户拒绝了操作"

**验证点**:
- [ ] 确认节点显示操作类型、描述、风险等级
- [ ] 确认超时（300秒）后自动取消
- [ ] 路径安全校验：不允许删除工作区外的文件

---

### E2E-09: 会话管理

**目标**: 验证会话的创建、切换、删除、标题更新

**步骤**:
1. 点击顶部栏"新建会话"按钮（或 Ctrl+N）
2. 验证工作流时间线清空，进入新会话
3. 发送一条消息，观察会话标题自动更新
4. 点击顶部栏"历史"按钮，打开历史面板
5. 在历史面板中切换到之前的会话
6. 关闭历史面板，验证工作流显示切换后的会话内容
7. 在历史面板中删除一个会话

**预期结果**:
- 新建会话后时间线清空
- 历史面板正确列出所有会话
- 切换会话后内容正确加载
- 删除会话后列表更新

**验证点**:
- [ ] 会话数据持久化到 SQLite
- [ ] 删除当前会话后自动切换到下一个可用会话
- [ ] 会话标题更新后历史面板同步刷新

---

### E2E-10: Agent 停止

**目标**: 验证 Agent 执行过程中可以手动停止

**步骤**:
1. 发送一个需要较长时间处理的请求
2. 在 Agent 执行期间点击"停止"按钮
3. 观察 Agent 是否中断执行

**预期结果**:
- Agent 收到停止信号后中断当前循环
- 工作流时间线显示已完成的步骤
- 已执行的消息已持久化到数据库
- 输入框恢复可用状态

**验证点**:
- [ ] 停止后 Agent 不再发送 LLM 请求
- [ ] 已完成的 Tool 结果被保留
- [ ] 停止后可以继续发送新消息

---

### E2E-11: Skill 管理

**目标**: 验证 Skill 的启用/禁用功能

**步骤**:
1. 打开设置弹窗，切换到 Skills 标签页
2. 查看已注册的 9 个内置 Skill 列表
3. 禁用 `generate_document` Skill
4. 保存设置
5. 尝试让 Agent 生成文档
6. 重新启用该 Skill

**预期结果**:
- Skill 列表正确显示 9 个内置技能
- 禁用后 Agent 无法调用该技能
- 重新启用后恢复正常

**验证点**:
- [ ] 禁用状态持久化到 `app_settings.json`
- [ ] 禁用的 Skill 不出现在 LLM 的 tool_definitions 中
- [ ] 应用重启后禁用状态保持

---

### E2E-12: 应用设置

**目标**: 验证应用设置的读取和更新

**步骤**:
1. 打开设置弹窗，切换到通用标签页
2. 修改作者名称为 `测试用户`
3. 修改确认级别为 `Always`
4. 保存设置
5. 重启应用，验证设置已持久化

**预期结果**:
- 设置修改后即时生效
- 重启后设置值保持

**验证点**:
- [ ] `app_settings.json` 文件正确保存
- [ ] 新增字段有默认值（merge_with_defaults 逻辑）
- [ ] Token 预算和版本快照设置可配置

---

### E2E-13: 文档搜索

**目标**: 验证工作区内文档搜索功能

**步骤**:
1. 发送消息: `搜索工作区中所有 .docx 文件`
2. 等待 Agent 执行完成
3. 验证返回结果包含文件名、路径、大小等信息

**预期结果**:
- Agent 调用 search_documents 技能
- 返回匹配的文件列表
- 结果按相关性排序

**验证点**:
- [ ] 搜索范围限制在工作区内
- [ ] 支持按扩展名筛选
- [ ] 支持按文件名和内容搜索

---

### E2E-14: 文档分析

**目标**: 验证文档分析功能

**步骤**:
1. 先生成一份测试文档
2. 发送消息: `分析 test_report.docx 的结构`
3. 等待 Agent 执行完成

**预期结果**:
- 返回文档统计信息（段落数、字数、标题数等）
- 返回标题层级结构
- 返回文档属性信息

**验证点**:
- [ ] 分析结果包含完整的统计信息
- [ ] 标题层级正确提取

---

### E2E-15: 错误处理与边界情况

**目标**: 验证各种错误场景的处理

**步骤**:
1. 未配置 LLM Provider 时发送消息 -> 应提示配置 Provider
2. 读取不存在的文件 -> 应返回文件未找到错误
3. 删除工作区外的文件 -> 应拒绝操作
4. LLM API Key 无效 -> 应返回认证失败错误
5. Sidecar 进程异常退出 -> 应自动重启并重试

**预期结果**:
- 每种错误场景都有明确的错误提示
- 错误信息对用户友好
- 可恢复的错误不会导致应用崩溃

**验证点**:
- [ ] 错误事件正确发射到前端
- [ ] Agent 执行失败后状态正确恢复
- [ ] Sidecar 自动重启机制有效

---

## 测试结果汇总表

| 编号 | 测试用例 | 对应 Sprint | 结果 |
|------|----------|-------------|------|
| E2E-01 | 应用启动与初始化 | Sprint 1 | |
| E2E-02 | LLM Provider 配置 | Sprint 1/2 | |
| E2E-03 | 工作区管理 | Sprint 1 | |
| E2E-04 | Agent 对话交互 | Sprint 2 | |
| E2E-05 | 文档生成（Word） | Sprint 3 | |
| E2E-06 | 文档读取 | Sprint 3 | |
| E2E-07 | 文档修改 | Sprint 3 | |
| E2E-08 | 操作确认机制 | Sprint 5 | |
| E2E-09 | 会话管理 | Sprint 5 | |
| E2E-10 | Agent 停止 | Sprint 2 | |
| E2E-11 | Skill 管理 | Sprint 3 | |
| E2E-12 | 应用设置 | Sprint 1 | |
| E2E-13 | 文档搜索 | Sprint 3 | |
| E2E-14 | 文档分析 | Sprint 3 | |
| E2E-15 | 错误处理与边界情况 | 全局 | |

---

## Phase 1 功能完成度评估

### Sprint 1: 项目搭建 (100%)
- Tauri 2 + React + TypeScript 项目结构完整
- SQLite 数据库初始化（4 张表 + 索引 + 版本记录）
- JSON 配置管理（app_settings / llm_config / workspaces）

### Sprint 2: LLM + Agent 引擎 (100%)
- LlmProvider trait 定义完整，支持 chat / chat_stream / test_connection
- OpenAI 适配器实现完整（含流式 SSE 解析、重试逻辑、错误处理）
- LlmRouter 支持默认选择和 Fallback 切换
- AgentExecutor 实现 Tool Calling 循环（最大 20 轮迭代）
- 增量持久化回调防止崩溃丢失消息
- Tauri 事件系统完整（9 个 Agent 事件 + 4 个系统事件）
- useAgent Hook 封装完整（事件监听、状态管理、组件卸载清理）

### Sprint 3: Skill 系统 (100%)
- Skill trait 定义完整（skill_name / description / parameters / execute）
- SkillRegistry 支持注册、查询、启用/禁用、工具定义生成
- 9 个内置 Skill 全部实现（generate / read / modify / delete / convert / search / analyze / list_workspace / batch_process）
- Python Sidecar 进程管理器（启动/停止/自动重启/超时/重试）
- Sidecar stdin/stdout JSON 行协议完整
- Word 文档处理器完整（generate / read / modify / convert / analyze）

### Sprint 4: 主界面 (100%)
- TopBar 组件（历史/新建/设置按钮）
- MainLayout 布局（主区域 + 右侧栏）
- WorkflowTimeline + 7 种节点类型（User / Thinking / Tool / Result / Reply / Confirm / WorkflowNode）
- InputArea 输入框（自动高度、快捷键、模板标签）
- 右侧栏四个分区（FileTree / AgentInfo / Todo / Token）
- SettingsDialog 设置弹窗（LLM / 工作区 / Skills / 通用 / 模板）

### Sprint 5: 会话与确认 (100%)
- 会话 CRUD（create / list / get / delete / update_title）
- 会话事件发射（session:updated）
- 版本快照仓库（create / list / delete）
- 历史会话面板（左侧滑出、会话切换）
- 操作确认机制（高风险 Skill 触发确认、oneshot channel 同步等待、超时处理）
- ConfirmNode 组件（确认/取消按钮、状态展示）
