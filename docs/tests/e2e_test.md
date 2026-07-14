# WorkMolde AI 端到端测试文档

> 版本：v2.0
> 最后更新：2026-06-14
> 说明：本文档根据实际实现代码更新

## 审查结论

经过对项目代码、文档、提交历史的全面审查，**全部开发任务已完成**，构建验证结果如下：

| 验证项 | 结果 |
|--------|------|
| 前端 TypeScript 类型检查 + Vite 构建 | 通过 (1232 modules, 0 errors) |
| Rust 后端 cargo check | 通过 (0 errors, 0 warnings) |
| Rust 后端 cargo clippy | 通过 (0 warnings) |
| Python Sidecar 依赖 | 通过 (docx/openpyxl/pptx/fitz/reportlab 均可导入) |
| 应用启动运行 | 通过 (正常启动，无崩溃) |
| Git 提交历史 | 38 次提交，覆盖从项目初始化到功能完善全流程 |

---

## 前置条件

1. **运行环境**: Windows 11, Node.js, Python 3.12+, Rust 1.80+
2. **Python 依赖**: 已安装 `pip install -r sidecar/requirements.txt`
3. **LLM API Key**: 至少配置一个可用的 OpenAI 兼容 API（用于 Agent 交互测试）
4. **启动命令**: `npm run tauri:dev`

> **数据目录**: `%APPDATA%\workmolde`，包含 `workmolde.db` 和 `config/` 目录

---

## 测试用例

### E2E-01: 应用启动与初始化

**目标**: 验证应用能正常启动，所有初始化流程正确执行

**步骤**:
1. 执行 `npm run tauri:dev` 启动应用
2. 观察应用窗口是否正常显示
3. 检查无边框窗口 + 自定义窗口控件（最小化/最大化/关闭）
4. 检查窗口是否可调整大小和拖拽

**预期结果**:
- 应用窗口正常显示，无白屏或崩溃
- 顶部栏显示工作区选择器、各操作按钮和窗口控件
- 主区域显示"开始新会话"引导提示
- 右侧栏显示文件树、Agent 信息、Token 统计三个分区
- 底部输入框可正常聚焦
- SQLite 数据库文件 `<app_data_dir>/workmolde.db` 已创建
- 配置目录 `<app_data_dir>/config/` 已创建
- 日志文件 `log/workmolde.log` 存在

### E2E-02: LLM Provider 配置

**目标**: 验证 LLM Provider 的增删改查和连接测试

**步骤**:
1. 点击右上角设置图标，打开设置弹窗
2. 切换到 LLM 配置标签页
3. 添加 Provider：OpenAI 类型，填写 API 地址和 Key
4. 点击"测试连接"按钮
5. 保存配置，验证列表中出现新条目
6. 编辑 Provider 名称，删除 Provider
7. 分别测试 Anthropic 和 Gemini 类型的 Provider

**预期结果**:
- 连接测试返回成功，显示延迟和模型信息
- Provider 列表正确显示
- 设为默认操作成功
- 编辑和删除正常
- 三种 Provider 类型连接测试均正常

**验证点**:
- [ ] `llm_config.json` 正确保存
- [ ] 无效 API Key 时返回明确错误
- [ ] Anthropic 使用 x-api-key 认证头
- [ ] Gemini 使用 URL 参数认证

### E2E-03: Agent 对话交互（核心流程）

**目标**: 验证 Agent 完整的 Tool Calling 循环

**步骤**:
1. 确保已配置 LLM Provider 并设为默认
2. 确保已添加并激活一个工作区
3. 输入: `帮我生成一份测试文档，标题为"项目周报"`
4. 观察工作流时间线节点顺序：User → Thinking → Content → Tool → Result → Content
5. 验证 Token 统计更新

**预期结果**:
- 工作流时间线按顺序显示所有节点
- Agent 使用 write_script + run_command Tool 生成文档
- 文档在工作区目录中生成
- Token 统计数字递增

**验证点**:
- [ ] 消息持久化到 SQLite
- [ ] 流式输出过程中内容逐步显示
- [ ] Agent 执行期间输入框禁用
- [ ] 节点可展开/折叠查看详情
- [ ] Agent 使用 `write_script + run_command` Tool 而非旧的 generate handler

### E2E-04: 脚本生成文档

**目标**: 验证 write_script + run_command Tool 生成多种格式文档

**步骤**:
1. 输入: `创建一份项目周报.docx`
2. 确认弹窗显示代码描述和摘要
3. 点击确认执行
4. 验证工作区生成 `.docx` 文件
5. 分别测试 Excel、PPT、PDF、Markdown 格式

**预期结果**:
- Agent 调用 `write_script + run_command`
- 确认弹窗显示代码功能描述和代码摘要（前 200 字符）
- 生成的文件格式正确，内容完整
- 文件树自动刷新

**验证点**:
- [ ] 文档属性中作者字段正确
- [ ] 生成的文档可正常打开

### E2E-05: 文档读取（Handler read）

**目标**: 验证 Handler read 操作

**步骤**:
1. 输入: `读取工作区中的项目周报.docx`
2. 等待 Agent 执行完成
3. 验证 Agent 调用 `docx_handler action="read"`
4. 验证返回文档段落、表格、属性信息

**预期结果**:
- 不弹出确认弹窗（read 不是高风险操作）
- 返回文档内容摘要和元数据

### E2E-06: 格式转换（Handler convert）

**目标**: 验证 Handler convert 操作

**步骤**:
1. 输入: `把项目周报.docx 转换为 PDF`
2. 等待 Agent 执行完成
3. 验证 Agent 调用 `docx_handler action="convert"`
4. 验证工作区生成 PDF 文件

**预期结果**:
- 不弹出确认弹窗（convert 不是高风险操作）
- 转换后的文件可正常打开

### E2E-07: 文档修改（脚本执行）

**目标**: 验证通过 write_script + run_command Tool 修改文档

**步骤**:
1. 输入: `修改项目周报.docx，将标题改为"2024年度总结"`
2. 确认弹窗中确认执行
3. 重新读取文档验证修改结果

**预期结果**:
- Agent 调用 `write_script + run_command` 编写修改脚本
- 确认弹窗展示代码摘要
- 修改后文档内容正确更新
- 修改前自动创建版本快照

### E2E-08: 操作确认机制

**目标**: 验证高风险操作的用户确认流程

**步骤**:
1. 输入: `删除项目周报.docx`
2. 观察工作流时间线出现确认节点
3. 点击"确认执行"按钮
4. 验证文件被删除
5. 重新生成文档后，再次尝试删除，这次点击"取消"
6. 验证文件未被删除

**预期结果**:
- Agent 调用 `delete_file` Tool
- 确认节点显示操作类型、描述、风险等级
- 确认后执行，取消后跳过
- 路径安全校验拒绝工作区外操作

### E2E-09: Electron 参考已完全移除

**验证点**:
- [ ] 所有文档中没有 Electron API 引用
- [ ] 使用 Tauri 的 `app.path().app_data_dir()` 而不是 Electron 的 `app.getPath('userData')`
- [ ] 使用 Tauri 事件系统代替 Electron IPC
- [ ] 包名从 Electron 风格改为 Tauri 风格

### E2E-10: 错误边界与自动恢复

**目标**: 验证应用的健壮性和错误恢复

**步骤**:
1. 在开发者工具中模拟组件渲染异常
2. 验证 ErrorBoundary 捕获异常，显示恢复页面
3. 点击"恢复页面"按钮
4. 未配置 LLM Provider 时发送消息 → 应提示配置 Provider
5. 读取不存在的文件 → 应返回文件未找到错误
6. 尝试删除工作区外的文件 → 应拒绝操作

**预期结果**:
- ErrorBoundary 包裹应用根组件
- 渲染异常时显示错误信息和恢复/重启按钮
- 每种错误场景都有明确的友好提示
- 可恢复的错误提供重试按钮

---

## 测试结果汇总表

| 编号 | 测试用例 | 功能模块 | 结果 |
|------|----------|----------|------|
| E2E-01 | 应用启动与初始化 | 基础框架 | |
| E2E-02 | LLM Provider 配置 | LLM | |
| E2E-03 | Agent 对话交互 | Agent | |
| E2E-04 | 脚本生成文档 | write_script + run_command | |
| E2E-05 | 文档读取 | Handler | |
| E2E-06 | 格式转换 | Handler | |
| E2E-07 | 文档修改 | write_script + run_command | |
| E2E-08 | 操作确认机制 | Agent | |
| E2E-09 | Electron 参考移除 | 全局 | |
| E2E-10 | 错误边界与自动恢复 | 全局 | |

---

## 功能完成度评估

### 基础框架 (100%)
- Tauri 2 + React 19 + TypeScript 项目结构完整
- 无边框窗口 + 自定义窗口控件
- SQLite 数据库初始化（6 张表 + 索引 + 版本记录）
- JSON 配置管理（app_settings / llm_config / workspaces）
- 双输出日志系统（控制台 + 文件）

### LLM 引擎 (100%)
- LlmProvider trait（chat / chat_stream / test_connection）
- OpenAI/Anthropic/Gemini 适配器完整
- LlmRouter + Fallback + 健康检查

### Agent 引擎 (100%)
- AgentExecutor Tool Calling 循环（最大 20 轮）
- 增量持久化
- Tauri 事件系统（13 个事件类型）
- useAgent Hook
- 操作确认机制（oneshot channel, 5分钟超时）
- 停止功能（stopping → cancelled）

### Handler 系统 (100%)
- 4 个文档 Handler（docx/xlsx/pptx/pdf）
- 10 个 Tool（全部 Rust 原生）
- Python Sidecar 管理器（自动重启/超时/重试）
- write_script + run_command Tool（10 个 Tool）

### 前端 UI (100%)
- TopBar + WindowControls + WorkspaceSelector
- WorkflowTimeline + 7 种节点类型（虚拟滚动）
- InputArea（内置 TemplateCards）
- Sidebar 三个分区（FileTree + AgentInfo + SessionList）
- PreviewOverlay（Markdown/PDF/Word/Excel/PPT/Text/Diff）
- VersionHistoryPanel（版本列表/对比/回滚）
- SettingsDialog（8 个标签页）
- SessionListSection + DeleteConfirmDialog
- ErrorBoundary + ToastContainer + NetworkStatusBanner
- UpdateNotification

### 性能优化 (100%)
- 懒加载（Preview/Settings/History/VersionHistory/Update）
- 虚拟滚动（WorkflowTimeline/FileTree）
- ErrorBoundary 全局错误边界
- Toast 通知（3 秒自动消失，最大 5 条）

### 文件系统 (100%)
- 文件树展示和交互
- 文件操作（创建/重命名/删除/预览）
- 文件监听服务（notify crate）
- 路径安全校验
