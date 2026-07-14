# 上下文窗口管理功能完善 - 开发计划

> **注意**: 本文档中提到的 "Skill" 已重命名为 "Handler"，相关工具名如 `docx_skill` 已更改为 `docx_handler`。

**目标**: 将上下文窗口大小与主流大模型实际能力匹配，修复现有死代码问题，并在右侧栏添加上下文占用可视化 UI

**架构**: 后端新增模型上下文窗口预设表 + Tauri 命令/事件推送 Token 用量，前端新增 ContextWindowSection 组件以横条可视化展示各部分占比

**技术栈**: Rust (Tauri 命令/事件) + React/TypeScript (Zustand store + CSS-in-JS)

---

## 一、主流大模型上下文窗口大小数据表

以下数据基于 2025-2026 年 5 月公开信息整理，用于构建模型预设表。

> **准确性声明**: 以下数据基于公开信息整理，部分 2026 年发布模型的数据可能为推测值或早期公告值，实际上下文窗口大小以各 Provider 官方文档为准。实现时应以保守值为准，避免超出模型实际能力。

### 国际模型

| Provider | 模型 | 上下文窗口 (tokens) | 备注 |
|----------|------|---------------------|------|
| OpenAI | gpt-5.5 | 1,000,000 (1M) | 2026 旗舰 |
| OpenAI | gpt-5.4 | 272,000 (272K) / 1M | 可扩展至 1M |
| OpenAI | gpt-5.4-mini | 400,000 (400K) | 轻量版 |
| OpenAI | gpt-5.4-nano | 400,000 (400K) | 最轻量版 |
| OpenAI | gpt-4.1 | 1,000,000 (1M) | 2025.4 发布 |
| OpenAI | gpt-4.1-mini | 1,000,000 (1M) | 2025.4 发布 |
| OpenAI | gpt-4.1-nano | 1,000,000 (1M) | 2025.4 发布 |
| OpenAI | gpt-4o | 128,000 (128K) | 2024.5 发布 |
| OpenAI | gpt-4o-mini | 128,000 (128K) | 2024.7 发布 |
| OpenAI | gpt-4-turbo | 128,000 (128K) | 2023.11 发布 |
| OpenAI | gpt-3.5-turbo | 16,385 (16K) | 旧版模型 |
| OpenAI | o3 | 200,000 (200K) | 推理模型 |
| OpenAI | o4-mini | 200,000 (200K) | 推理模型 |
| Anthropic | claude-opus-4-7 | 2,000,000 (2M) | 2026 发布，全球最长上下文 |
| Anthropic | claude-opus-4-6 | 1,000,000 (1M) | 2026.2 发布，3 月标准定价 |
| Anthropic | claude-sonnet-4-6 | 1,000,000 (1M) | 2026.2 发布，3 月标准定价 |
| Anthropic | claude-haiku-4-5 | 200,000 (200K) | 2025.11 发布 |
| Anthropic | claude-3-7-sonnet | 200,000 (200K) | 2025.2 发布 |
| Anthropic | claude-3-5-sonnet | 200,000 (200K) | 2024.10 发布 |
| Anthropic | claude-3-5-haiku | 200,000 (200K) | 2024.10 发布 |
| Anthropic | claude-3-opus | 200,000 (200K) | 2024.3 发布 |
| Anthropic | claude-3-haiku | 200,000 (200K) | 2024.3 发布 |
| Google | gemini-3.1-pro | 1,000,000 (1M) | 2026.2 发布 |
| Google | gemini-3.5-flash | 128,000 (128K) | 2026 发布，极速版 |
| Google | gemini-2.5-pro | 1,000,000 (1M) | 可扩展至 2M |
| Google | gemini-2.5-flash | 1,000,000 (1M) | 2025 发布 |
| Google | gemini-2.5-flash-lite | 1,000,000 (1M) | 轻量版 |
| Google | gemini-2.0-flash | 1,000,000 (1M) | 2024.12 发布 |
| Google | gemini-1.5-pro | 2,000,000 (2M) | 2024.2 发布，最长上下文 |
| Google | gemini-1.5-flash | 1,000,000 (1M) | 2024.5 发布 |
| DeepSeek | deepseek-v4-pro | 1,000,000 (1M) | 2026.4 发布，1.6T 参数 MoE |
| DeepSeek | deepseek-v4-flash | 1,000,000 (1M) | 2026.4 发布，284B 参数 MoE |
| DeepSeek | deepseek-r2 | 1,000,000 (1M) | 2026.5 发布，开源旗舰 |
| DeepSeek | deepseek-v3 | 128,000 (128K) | 2024.12 发布 |
| DeepSeek | deepseek-r1 | 128,000 (128K) | 2025.1 发布 |
| Meta | llama-4-scout | 10,000,000 (10M) | 2025.4 发布，109B MoE，开源最长 |
| Meta | llama-4-maverick | 1,000,000 (1M) | 2025.4 发布，400B MoE |
| Meta | llama-3.3-70b | 128,000 (128K) | 2024.12 发布 |
| Meta | llama-3.1-405b | 128,000 (128K) | 2024.7 发布 |
| Meta | llama-3.1-70b | 128,000 (128K) | 2024.7 发布 |
| Meta | llama-3.1-8b | 128,000 (128K) | 2024.7 发布 |
| Mistral | mistral-large-3 | 128,000 (128K) | 2025 发布，675B MoE |
| Mistral | mistral-small-4 | 128,000 (128K) | 2025 发布 |
| Mistral | magistral | 128,000 (128K) | 2025 发布 |
| xAI | grok-4.20 | 未公开 | 2026 发布，数学推理强 |
| Magic.dev | ltm-2-mini | 100,000,000 (100M) | 实验性，实际使用情况不明 |

### 国内模型

| Provider | 模型 | 上下文窗口 (tokens) | 备注 |
|----------|------|---------------------|------|
| 阿里云 | qwen3.7-max | 1,000,000 (1M) | 2026.5 发布，Agent 原生 |
| 阿里云 | qwen3.6-max | 1,000,000 (1M) | 2026.4 发布 |
| 阿里云 | qwen3.6-plus | 1,000,000 (1M) | 2026.4 发布，编程 Agent |
| 阿里云 | qwen3-235b-a22b | 262,144 (262K) | 2025.4 发布，开源 MoE |
| 阿里云 | qwen3-30b-a3b | 262,144 (262K) | 可扩展至 1M |
| 阿里云 | qwen3-14b | 131,072 (128K) | 实测可达 131K |
| 阿里云 | qwen2.5-1m | 1,000,000 (1M) | 开源版 |
| 阿里云 | qwen-max | 1,000,000 (1M) | 通义千问旗舰 |
| 阿里云 | qwen-plus | 131,072 (128K) | 通义千问增强 |
| 阿里云 | qwen-turbo | 1,000,000 (1M) | 通义千问快速 |
| 月之暗面 | kimi-k2-6 | 262,144 (262K) | 2026.4 发布，12 小时连续编码 |
| 月之暗面 | kimi-k2-5 | 256,000 (256K) | 2026.1 发布，Agent Swarm |
| 月之暗面 | kimi-k2 | 128,000 (128K) | 2025.7 发布，开源 |
| 月之暗面 | moonshot-v1-128k | 128,000 (128K) | Kimi 早期版 |
| 月之暗面 | moonshot-v1-32k | 32,000 (32K) | Kimi 早期版 |
| 智谱AI | glm-5.1 | 200,000 (200K) | 2026 发布，企业级 |
| 智谱AI | glm-4 | 128,000 (128K) | ChatGLM |
| 智谱AI | glm-4-flash | 128,000 (128K) | ChatGLM 快速版 |
| 百度 | ernie-5.1 | 128,000 (128K) | 2026 发布 |
| 百度 | ernie-4.0 | 128,000 (128K) | 文心一言 |
| 百度 | ernie-3.5 | 32,000 (32K) | 文心一言 |
| 字节跳动 | seed-2.0-pro | 128,000 (128K) | 2026 发布，豆包旗舰 |
| 字节跳动 | doubao-1.5-pro | 128,000 (128K) | 豆包 |
| 字节跳动 | doubao-1.5-lite | 128,000 (128K) | 豆包轻量版 |
| MiniMax | minimax-m2.7 | 200,000 (200K) | 2026 发布，全球最低价 |
| 腾讯 | hunyuan-3-preview | 未公开 | 2026 发布 |
| 零一万物 | yi-large | 200,000 (200K) | 旗舰版 |
| 零一万物 | yi-lightning | 16,000 (16K) | 快速版 |
| 百川 | baichuan-4 | 128,000 (128K) | 2024 发布 |
| 讯飞 | spark-v4 | 32,000 (32K) | 星火 |

### 数据更新说明

1. **上下文窗口快速增长**: 2024 年主流为 128K，2025 年旗舰模型普遍达到 1M，2026 年出现 2M-10M 的超长上下文模型
2. **MoE 架构普及**: DeepSeek V4、Llama 4、Qwen3 等主流模型均采用 MoE 架构，大幅降低推理成本
3. **开源模型追赶闭源**: DeepSeek V4、Llama 4 Scout 等开源模型在上下文长度上已超越部分闭源模型
4. **国产模型崛起**: Qwen3.7-Max、Kimi K2.6、GLM-5.1 等国产模型在上下文窗口和 Agent 能力上达到国际一流水平

---

## 二、现有问题清单

| 编号 | 问题 | 严重程度 | 说明 |
|------|------|----------|------|
| P1 | 上下文窗口大小硬编码为 128K | 高 | `AgentContext::new()` 调用 `TokenBudgetManager::default_context()`，固定 128K，不随模型变化 |
| P2 | 未与 LLM Provider 关联 | 高 | `LlmProvider`（`config/llm_config.rs`）中没有 `context_window` 字段。`AdvancedConfig` 已有 `max_tokens: u32`（默认 4096），但这是 LLM 响应的最大输出 token 数，不是上下文窗口大小 |
| P3 | `should_inject_guides()` 从未被调用 | 中 | 定义了根据 Token 预算决定是否注入规范层的方法，但系统提示词构建时始终无条件注入 |
| P4 | `calculate_window_size()` 从未被调用 | 中 | 定义了动态计算滑动窗口大小的方法，但实际使用固定值 `keep_recent_rounds * 4` |
| P5 | `available_conversation_tokens()` 从未被调用 | 低 | 定义了计算剩余 Token 空间的方法，但生产代码中未使用 |
| P6 | 前端无配置入口 | 中 | 设置页面没有上下文窗口大小的配置 UI |
| P7 | Token 估算过于粗糙 | 中 | 使用 `chars().count()` 近似，中文偏保守、英文偏激进 |
| P8 | `ModelInfo.max_tokens` 含义混淆 | 中 | 连接测试返回的 `ModelInfo.max_tokens`（`models/llm.rs`）是 LLM 最大输出 token 数（如 4096），不是上下文窗口大小。部分 API（如 OpenAI）可通过 `model` 端点获取上下文窗口，但当前未实现 |
| P9 | 无上下文占用可视化 | 中 | 用户无法直观感知当前上下文使用情况 |
| P10 | 前端 `LLMProviderType` 缺少 `"gemini"` | 低 | Rust 端 `ProviderType` 枚举包含 `Gemini`，但前端类型定义为 `"openai" \| "anthropic" \| "ollama" \| "custom"`，缺少 `"gemini"`，前后端类型不一致 |

---

## 三、设计方案

### 3.1 后端：模型上下文窗口预设表

在 `src-tauri/src/services/llm/` 下新增 `context_presets.rs`，内置上述数据表，支持按模型名称模糊匹配上下文窗口大小。

**核心结构**:

```rust
/// 模型上下文窗口预设项
pub struct ContextPreset {
    /// 模型名称关键词（用于模糊匹配）
    pub model_pattern: &'static str,
    /// 上下文窗口大小 (tokens)
    pub context_window: usize,
    /// Provider 类型（可选，用于精确匹配）
    pub provider_type: Option<&'static str>,
}
```

**匹配策略**:

许多模型通过 OpenAI 兼容 API 访问（如 DeepSeek、Qwen、Kimi、Llama 等），其 `provider_type` 为 `"openai"` 或 `"custom"`。因此 `provider_type` 匹配仅对原生 API 的 Provider 有效，不能单独作为区分依据。

匹配优先级:
1. **精确 provider_type + model_pattern**: 如 `provider_type = "openai"` + `model_pattern = "gpt-4.1"` 匹配 OpenAI 官方模型
2. **仅 model_pattern 精确匹配**: 如模型名含 `"gpt-4.1"` 精确匹配 1M
3. **model_pattern 模糊匹配**: 如模型名含 `"gpt-4"` 匹配 128K
4. **兜底默认值 128K**

**OpenAI 兼容 API 的特殊处理**: 对于 `provider_type = "openai"` 的 Provider，需要额外检查模型名称是否为 OpenAI 官方模型（如以 `gpt-` 或 `o3`/`o4` 开头），避免将 DeepSeek 的 `deepseek-v3` 误匹配为 OpenAI 模型。对于 `provider_type = "custom"` 的 Provider，完全依赖模型名称匹配。

### 3.2 后端：LlmProvider 增加 context_window 字段

在 `LlmProvider` 的 `AdvancedConfig` 中新增 `context_window` 字段（`Option<usize>`），用户可手动覆盖自动检测值。

**数据流**:
1. 用户添加 Provider 时，根据模型名称自动从预设表推断 `context_window`
2. 用户可在高级配置中手动修改
3. 注意: `ModelInfo.max_tokens` 是 LLM 最大输出 token 数（如 4096），不是上下文窗口大小，不能用于推断上下文窗口。未来可通过各 Provider 的模型详情 API 获取真实上下文窗口大小

**Ollama Provider 的特殊处理**:
- 当 `provider_type` 为 `"ollama"` 时，`resolve_context_window()` 返回保守默认值 8192（Ollama 默认 `num_ctx` 为 2048，但大多数现代模型支持 8192+）
- 在 `ProviderFormDialog` 中，当用户选择 Ollama 类型时，上下文窗口输入框显示提示文字"Ollama 模型的上下文窗口取决于模型配置，建议手动设置"
- 未来可通过 Ollama API (`/api/show`) 查询模型的 `num_ctx` 参数

### 3.3 后端：TokenBudgetManager 动态化

修改 `AgentContext::new()` 接收 `context_window` 参数，不再硬编码。

**数据来源路径**（重要实现细节）:
1. `run_agent()` 函数（`commands/agent.rs`）接收 `config: &Arc<Mutex<ConfigManager>>`
2. 在创建 `AgentContext` 前，通过 `config.lock().await.load_llm_config()` 获取 `LlmConfig`
3. 从 `LlmConfig` 中找到默认 Provider（`get_default_provider()`），读取其 `advanced.context_window`
4. 如果 `context_window` 为 `None`，调用 `resolve_context_window(model_name, provider_type)` 从预设表推断
5. 将解析后的 `context_window: usize` 传入 `AgentContext::new(session_id, system_prompt, context_window)`

**context_window 在 Agent 运行期间不变**: `context_window` 在 `run_agent()` 中解析一次后传入 `AgentContext::new()`，Agent 运行期间不再读取 Provider 配置。理由:
1. `AgentContext` 的 `token_budget` 在 `new()` 时初始化，运行中不更新
2. 运行中改变 context_window 会导致压缩阈值突变，可能产生不可预期行为
3. 与 `max_iterations` 等参数一致，均在 Agent 启动时确定

**优化点**：当前 `run_agent()` 第 551 行先调用 `build_system_prompt(workspace_path)` 构建默认 prompt 作为 `AgentContext::new()` 的参数，随后第 564-570 行又用 `build_system_prompt_with_task()` 覆盖 `ctx.system_prompt`。接入动态化后可重构为：
- 先解析 `context_window` 和 `task_type`
- 再一次性构建系统提示词带任务类型
- 最后将结果传入 `AgentContext::new()`，消除冗余的初始调用

### 3.4 后端：接入死代码

- `should_inject_guides()` -> 在 `build_system_prompt_with_task()` 中调用，Token 预算不足时跳过规范层和示例层
- `calculate_window_size()` -> 在 `compress_history_if_needed()` 中替代固定值 `keep_recent_rounds * 4`
- `available_conversation_tokens()` -> 在压缩日志中输出剩余空间信息

### 3.5 后端：新增 Tauri 命令和事件

**新增命令**:
- `get_context_usage(session_id: String) -> ContextUsageInfo`: 获取当前会话的上下文占用详情

**新增事件**:
- `agent:context_update`: Agent 每次迭代后推送上下文占用快照（确保横条实时更新）

**ContextUsageInfo 结构**:
```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContextUsageInfo {
    /// 模型上下文窗口总大小
    pub context_window: usize,
    /// 系统提示词占用 tokens
    pub system_prompt_tokens: usize,
    /// 函数定义占用 tokens（包含 Tool + Handler 两部分）
    pub function_definitions_tokens: usize,
    /// 对话历史占用 tokens
    pub conversation_tokens: usize,
    /// LLM 响应占用 tokens（当前轮，迭代完成后估算）
    pub response_tokens: usize,
    /// 总已用 tokens
    pub total_used_tokens: usize,
    /// 压缩状态: "normal" | "compressed" | "critical"
    pub compression_status: String,
    /// 当前模型名称
    pub model_name: String,
    /// 对话历史消息总数（压缩前）
    pub total_message_count: usize,
    /// 压缩后保留的消息数
    pub retained_message_count: usize,
}
```

**字段命名说明**: `function_definitions_tokens`（而非 `tool_definitions_tokens`），因为 Agent 发送给 LLM 的工具定义包含 Tool（8 个基础工具）和 Handler（6 个内置处理器 + 自定义处理器）两部分，executor 中通过 `[tool_defs, handler_defs].concat()` 合并。"function" 更准确反映 OpenAI API 中 `functions` / `tools` 的概念。

**function_definitions_tokens 的计算来源**: `AgentContext` 没有访问 tool definitions 的途径（tool definitions 在 executor 的局部变量中构建），因此:
1. `AgentContext` 新增 `pub function_definitions_tokens: usize` 字段（默认 0）
2. executor 在 `execute()` 方法中构建 tool definitions 后，估算其 token 数并设置 `ctx.function_definitions_tokens = estimated_tokens`
3. `get_context_usage()` 直接读取 `self.function_definitions_tokens`

**response_tokens 的计算时机**: 在每次迭代完成后计算（即 `agent:context_update` 事件发射时），使用 `estimate_tokens()` 估算当前轮 assistant 消息的 token 数。不进行流式实时估算（复杂度高、收益低）。

**事件 Payload 定义**:

`agent:context_update` 事件:
```rust
pub const AGENT_CONTEXT_UPDATE: &str = "agent:context_update";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContextUpdatePayload {
    pub session_id: String,
    pub context_usage: ContextUsageInfo,
}
```

**agent:context_update 事件发射时机**: executor 的 `execute()` 方法中有多处迭代结束点，需在以下位置发射:
1. **有 tool_calls 的迭代**: 在 `persist_new_messages()` + `mark_persisted()` 之后、`continue` 之前发射
2. **正常完成**: 在 `emit_done()` 之前发射
3. **响应截断/空响应继续**: 不发射（这些是异常恢复场景，不是有意义的迭代完成）
4. **被用户停止**: 在 `emit_stopped()` 之前发射，提供最终快照

**自动压缩检测**: 当前 `compress_history_if_needed()` 是 `&self` 方法，返回压缩后的消息列表但不通知外部。需要增加返回值标识是否发生了压缩:
```rust
pub struct CompressionResult {
    pub messages: Vec<ChatMessage>,
    pub was_compressed: bool,
    pub before_count: usize,
    pub after_count: usize,
    pub before_tokens: usize,
    pub after_tokens: usize,
}
```
executor 在 `get_messages_for_iteration()` 返回后检查 `was_compressed`，如果为 true 则发射 `agent:context_update` 事件。

### 3.6 前端：ContextWindowSection 组件

在右侧栏新增 `ContextWindowSection` 组件，使用 `SidebarSection` 包裹。

**UI 设计**:

```
+------------------------------------------+
|  上下文窗口                               |
+------------------------------------------+
|  窗口: 128K tokens                        |
|                                           |
|  [=====系统====|==工具==|=====历史=====|..] |
|  15%         10%       50%              |
|                                           |
|  系统提示词  19.2K / 19.2K    [===] 100%  |
|  工具定义    12.8K / 12.8K    [===] 100%  |
|  对话历史    32.1K / 64.0K    [==  ]  50%  |
|  LLM 响应   8.5K / 32.0K     [=   ]  27%  |
|                                           |
|  总计: 72.6K / 128K (56.7%)              |
+------------------------------------------+
```

**横条颜色方案**（使用 CSS 变量，支持 dark mode）:

```css
:root {
  --color-context-system: #6366f1;
  --color-context-functions: #f59e0b;
  --color-context-history: #3b82f6;
  --color-context-response: #10b981;
  --color-context-idle: var(--color-border-light);
}
[data-theme="dark"] {
  --color-context-system: #818cf8;
  --color-context-functions: #fbbf24;
  --color-context-history: #60a5fa;
  --color-context-response: #34d399;
}
```

- 系统提示词: `var(--color-context-system)` (靛蓝色)
- 函数定义: `var(--color-context-functions)` (琥珀色)
- 对话历史: `var(--color-context-history)` (蓝色)
- LLM 响应: `var(--color-context-response)` (翠绿色)
- 空闲空间: `var(--color-context-idle)` (浅灰)

**前端实现要点**:

1. **实时更新机制**:
   - Agent 每次迭代结束后，后端自动发射 `agent:context_update` 事件
   - 前端通过 `onContextUpdate()` 监听器接收事件，更新 `contextUsage` 状态
   - React 组件通过 Zustand store 订阅 `contextUsage`，状态变化自动触发重新渲染
   - 更新频率: 与 Agent 迭代频率一致（每次 LLM 调用 + Tool 执行后），通常 2-10 秒一次
   - 横条宽度变化使用 CSS transition（0.5s ease），避免突变闪烁
   - 如果 3 秒内收到多次更新，合并为最后一次（防抖）

2. **交互**:
   - 横条 hover 显示各部分 tooltip（名称 + token 数 + 百分比）
   - 压缩状态为 "compressed" 时，对话历史行显示压缩标记（小图标 + "已压缩" 文字）
   - 压缩状态为 "critical" 时，总占用行文字变红
   - Agent 运行时实时更新占用数据

3. **Agent 未运行时的上下文信息获取**:
   - **Agent 运行时**: 通过 `agent:context_update` 事件获取实时数据
   - **Agent 未运行时**: 从 `useSettingsStore` 中的 `providers` 列表获取当前默认 Provider 的 `contextWindow` 字段，显示静态信息（模型名 + 窗口大小，无占用数据）

### 3.7 前端：Zustand Store 扩展

上下文占用数据与 Agent 执行生命周期紧密相关（只在 Agent 运行时有意义），应放在 `useWorkflowStore` 中（而非 `useSettingsStore`），与 `executionStatus`、`nodes` 等运行时状态放在一起。`useSettingsStore` 管理的是应用级持久化设置，上下文占用是运行时临时状态。

在 `useWorkflowStore` 中新增:
- `contextUsage: ContextUsageInfo | null`
- `loadContextUsage(sessionId: string): Promise<void>`

在 `useWorkflowStore` 中监听 `agent:context_update` 事件，自动更新上下文占用数据。

### 3.8 Token 估算优化

将 `estimate_tokens()` 从简单的 `chars().count()` 改进为基于字符类型的分段估算（逐字符遍历，无需分词）：
- CJK 统一汉字 (U+4E00-U+9FFF): 1 字符 = 1.5 Token
- ASCII 字母/数字: 1 字符 = 0.25 Token（约 4 字符 = 1 Token）
- 空白/标点: 1 字符 = 0.5 Token
- 其他 Unicode: 1 字符 = 1 Token

**实现要点**:
- 逐字符遍历，根据 Unicode 范围分类，无需分词器
- 性能: 对 100K 字符文本约 1ms，可接受
- 不引入外部 tokenizer（如 tiktoken），因为精确计数需要网络请求或 WASM 模块，开销过大
- UI 上标注"估算值"，避免用户误认为是精确计数

---

## 四、任务分解

### Task 1: 新增模型上下文窗口预设表

**Files:**
- Create: `src-tauri/src/services/llm/context_presets.rs`
- Modify: `src-tauri/src/services/llm/mod.rs`

**实现内容**:
1. 定义 `ContextPreset` 结构体
2. 内置上述数据表为 `const` 数组
3. 实现 `fn resolve_context_window(model_name: &str, provider_type: Option<&str>) -> usize` 匹配函数
4. 匹配优先级: 精确 provider_type + model_pattern > 仅 model_pattern 精确匹配 > model_pattern 模糊匹配 > 兜底 128K
5. OpenAI 兼容 API 的特殊处理: `provider_type = "openai"` 时需检查模型名是否为 OpenAI 官方模型（以 `gpt-`/`o3`/`o4` 开头），避免误匹配
6. Ollama 特殊处理: `provider_type = "ollama"` 时返回保守默认值 8192
7. 编写单元测试覆盖精确匹配、模糊匹配、OpenAI 兼容 API 区分、Ollama 默认值、兜底默认值

**验证**: `cargo test context_presets`

---

### Task 2: LlmProvider 增加 context_window 字段

**Files:**
- Modify: `src-tauri/src/config/llm_config.rs` (LlmProvider 新增 context_window)
- Modify: `src-tauri/src/models/llm.rs` (ProviderInfo 新增 contextWindow 透传)
- Modify: `src/types/settings.ts` (前端 ProviderConfig/ProviderInfo 类型对齐 + LLMProviderType 扩展)

**实现内容**:
1. `LlmProvider`（`config/llm_config.rs`）新增 `context_window: Option<usize>` 字段，`#[serde(default)]` 默认 None（使用自动推断）。这是持久化字段，保存在 `llm_config.json` 中
2. `ProviderInfo`（`models/llm.rs`）新增 `context_window: usize` 字段（运行时计算后的最终值），用于向前端透传
3. 添加/更新 Provider 时，如果用户未指定 context_window，自动从预设表推断并填充到 `ProviderInfo`
4. 注意: `AdvancedConfig.max_tokens`（默认 4096）是 LLM 最大输出 token 数，与 `context_window` 无关，不要混淆
5. 前端 `ProviderConfig` 和 `ProviderInfo` 类型同步新增 `contextWindow` 字段
6. 前端 `LLMProviderType` 扩展为 `"openai" | "anthropic" | "ollama" | "gemini" | "custom"`，与 Rust 端对齐（修复已有的类型不一致问题）

**验证**: `cargo test llm_config` + `npx tsc -b`

---

### Task 3: TokenBudgetManager 动态化

**Files:**
- Modify: `src-tauri/src/services/agent/context.rs`
- Modify: `src-tauri/src/commands/agent.rs`

**实现内容**:
1. `AgentContext::new()` 改为接收 `context_window: usize` 参数
2. 在 `run_agent()` 中，从当前活跃 Provider 获取 `context_window`，传入 `AgentContext::new()`
3. 如果 Provider 未配置 `context_window`，调用 `resolve_context_window()` 推断
4. 保留 `default_context()` 作为 fallback（测试用）
5. **context_window 在 Agent 启动时确定，运行期间不变**: `context_window` 在 `run_agent()` 中解析一次后传入 `AgentContext::new()`，Agent 运行期间不再读取 Provider 配置

**验证**: `cargo test agent_context`

---

### Task 4: 接入死代码 - should_inject_guides

**Files:**
- Modify: `src-tauri/src/services/agent/context.rs`

**架构注意事项**（重要）:
`build_system_prompt_with_task()` 当前是 **static 方法**（`AgentContext::build_system_prompt_with_task(...)`），而 `TokenBudgetManager::should_inject_guides()` 需要实例方法调用。需要以下改造之一：

**方案 A（推荐，改动最小）**：给 `build_system_prompt_with_task()` 增加 `context_window: Option<usize>` 参数
```rust
pub fn build_system_prompt_with_task(
    workspace_path: &str,
    task_type: &TaskType,
    tool_count: usize,
    handler_count: usize,
    context_window: Option<usize>,  // 新增: None 表示不做预算检查（保持旧行为）
) -> String
```
- 当 `context_window` 为 `Some(window)` 时，创建临时 `TokenBudgetManager::new(window)`
- 构建各层后估算当前 Token 数，调用 `should_inject_guides()` 判断
- 预算不足时跳过 Layer 6 和 Layer 7

**方案 B**：将构建逻辑改为非静态方法，但会改动调用链路（`run_agent()` 中的调用顺序），不建议

**实现内容**:
1. 采用方案 A，在 `build_system_prompt_with_task()` 中增加 `context_window: Option<usize>` 参数
2. 当传递了 `context_window` 时，创建临时 `TokenBudgetManager`，估算已构建部分 Token 数
3. 调用 `should_inject_guides()` 判断 Layer 6+7 是否注入
4. 添加日志记录跳过原因
5. 更新所有调用点（`build_system_prompt()` 默认传 `None`，`run_agent()` 传解析后的值）

**验证**: `cargo test build_system_prompt`

---

### Task 5: 接入死代码 - calculate_window_size

**Files:**
- Modify: `src-tauri/src/services/agent/context.rs`

**实现内容**:
1. 在 `compress_history_if_needed()` 中，用 `calculate_window_size()` 替代固定值 `keep_recent_rounds * 4`
2. 需要估算每轮平均 Token 数（可从已有消息计算）
3. 在压缩日志中输出 `available_conversation_tokens()` 的值
4. 更新 `compress_history_if_needed()` 返回类型为 `CompressionResult`（包含 `was_compressed` 标志），使 executor 能检测自动压缩事件
5. 更新相关单元测试

**验证**: `cargo test compress_history`

---

### Task 6: 新增 ContextUsageInfo 结构和 Tauri 命令

**Files:**
- Create: `src-tauri/src/models/context_usage.rs`（注意: 与已有的 `context_memory.rs` 区分，后者是跨会话摘要，本文件是运行时上下文占用）
- Modify: `src-tauri/src/models/mod.rs`
- Modify: `src-tauri/src/commands/agent.rs`
- Modify: `src-tauri/src/lib.rs` (注册新命令)
- Modify: `src-tauri/src/events/types.rs` (新增事件常量和 Payload)
- Modify: `src-tauri/src/events/emitter.rs` (新增发射函数)

**实现内容**:
1. 定义 `ContextUsageInfo` 结构体（使用 `function_definitions_tokens` 字段名，含 `total_message_count` 和 `retained_message_count`），实现 `Serialize`/`Deserialize`/`Clone`
2. `AgentContext` 新增 `pub function_definitions_tokens: usize` 字段（默认 0），由 executor 在构建 tool definitions 后设置
3. 在 `AgentContext` 中新增 `fn get_context_usage(&self, model_name: &str) -> ContextUsageInfo` 方法
4. 在 `events/types.rs` 中新增 `AGENT_CONTEXT_UPDATE` 常量和 `ContextUpdatePayload` 结构体
5. 新增 Tauri 命令 `get_context_usage`
6. 新增事件 `agent:context_update`，在 Agent 每次迭代后发射（发射时机见 3.5 节）
7. 前端类型同步

**验证**: `cargo test context_usage` + `cargo build -p workmolde_lib`

---

### Task 7: Token 估算优化

**Files:**
- Modify: `src-tauri/src/services/agent/prompts/token_budget.rs`

**实现内容**:
1. 改进 `estimate_tokens()` 方法，基于字符类型分段估算（逐字符遍历）
2. 权重: CJK 统一汉字 (U+4E00-U+9FFF) = 1.5x, ASCII 字母/数字 = 0.25x, 空白/标点 = 0.5x, 其他 Unicode = 1x
3. 更新单元测试覆盖各类字符组合
4. 旧方法 `chars().count()` 过于简单（仅一行），无需保留为独立方法，直接在注释中说明即可

**验证**: `cargo test estimate_tokens`

---

### Task 8: 前端 - ContextWindowSection 组件

**Files:**
- Create: `src/components/sidebar/ContextWindowSection.tsx`
- Modify: `src/components/layout/Sidebar.tsx` (在 ContextWindowSection 中渲染)

**实现内容**:
1. 创建 `ContextWindowSection` 组件，使用 `SidebarSection` 包裹
2. 实现横条可视化（总览横条 + 各部分独立进度条）
3. 实现颜色方案（使用 CSS 变量，支持 dark mode，见 3.6 节颜色方案）
4. 实现 hover tooltip
5. 实现压缩状态标记（compressed/critical）
6. Agent 未运行时显示模型信息和默认窗口大小（从 `useSettingsStore` 的 providers 列表获取当前默认 Provider 的 contextWindow）
7. Agent 运行时实时更新占用数据
8. **实时更新**: 订阅 `useWorkflowStore` 中的 `contextUsage` 状态
   - 横条宽度使用 CSS transition (0.5s ease) 平滑过渡
   - 百分比数字变化使用动画效果
   - 3 秒防抖合并多次更新

**验证**: `npm run dev` 手动验证 UI

---

### Task 9: 前端 - Store 和事件监听

**Files:**
- Modify: `src/stores/useWorkflowStore.ts` (新增 contextUsage 状态和事件监听)
- Modify: `src/services/tauri.ts` (新增 get_context_usage 命令)
- Modify: `src/services/event.ts` (新增 onContextUpdate 监听)
- Modify: `src/types/` (新增 ContextUsageInfo 类型)

**实现内容**:
1. 在 `useWorkflowStore` 中新增 `contextUsage: ContextUsageInfo | null` 状态和 `loadContextUsage` 方法
2. 在 `tauri.ts` 中封装 `getContextUsage()` 命令
3. 在 `event.ts` 中新增 `onContextUpdate()` 事件监听
4. 在 `types/` 中定义 `ContextUsageInfo` TypeScript 类型
5. Agent 启动时加载初始上下文占用，运行中通过事件实时更新
6. **实时更新**: 在 `useWorkflowStore` 中注册 `agent:context_update` 事件监听
   - 收到事件后更新 `contextUsage` 状态
   - 使用防抖（3 秒）避免频繁重渲染
   - Agent 执行完成（`agent:done`）时最后一次更新

**验证**: `npx tsc -b` + `npm run dev`

---

### Task 10: 前端 - 设置页面增加上下文窗口配置

**Files:**
- Modify: `src/components/settings/ProviderFormDialog.tsx` (上下文窗口输入框)

**实现内容**:
1. 在 Provider 编辑弹窗的高级配置区域新增"上下文窗口大小"输入框
2. 默认显示自动推断值，用户可手动修改
3. 输入框旁边显示"自动检测"按钮，点击后从预设表重新推断
4. 保存时校验范围（4K - 10M）
5. Ollama Provider 特殊处理: 当用户选择 Ollama 类型时，上下文窗口输入框显示提示文字"Ollama 模型的上下文窗口取决于模型配置，建议手动设置"

**验证**: `npm run dev` 手动验证

---

### Task 11: 集成测试和边界情况

**Files:**
- Modify: `src-tauri/src/services/agent/context.rs` (补充测试)
- Modify: `src-tauri/src/services/agent/prompts/token_budget.rs` (补充测试)

**实现内容**:
1. 测试不同模型切换时上下文窗口正确更新
2. 测试超长对话历史的压缩行为
3. 测试 context_window 为最小值 (4096) 时的边界情况
4. 测试手动覆盖 context_window 的优先级
5. 测试前端组件在无 Agent 运行时的展示

**验证**: `cargo test` + 手动验证

---

## 五、实施顺序

```
Task 1 (预设表) ──> Task 2 (Provider 字段) ──> Task 3 (动态化)
                                                      |
Task 4 (接入 guides) ──> Task 5 (接入 window_size) ──>|
                                                      |
Task 7 (Token 估算) ────────────────────────────────>|
                                                      v
                                              Task 6 (命令+事件)
                                                      |
                                    +-----------------+------------------+
                                    v                                    v
                            Task 8 (UI 组件)                    Task 10 (设置页面)
                                    |                                    |
                                    v                                    |
                            Task 9 (Store+事件) <───────────────────────+
                                    |
                                    v
                            Task 11 (集成测试)
```

**关键依赖**:
- Task 6 依赖 Task 1-5 和 Task 7（需要后端数据完整才能提供 API）
- Task 8-9 依赖 Task 6（前端需要后端命令和事件）
- Task 10 依赖 Task 2（需要 Provider 配置字段）
- Task 11 依赖所有其他 Task

---

## 六、风险和注意事项

1. **预设表维护**: 模型更新频繁，预设表需要定期更新。建议在代码中添加注释说明更新方式，并考虑未来支持从远程配置更新
2. **Token 估算精度**: 近似估算与实际 Token 数可能偏差 10-30%，UI 上应标注"估算值"而非精确值
3. **性能影响**: 每次迭代发射 `agent:context_update` 事件会增加少量开销，Token 估算应尽量轻量
4. **向后兼容**: `AdvancedConfig` 新增 `context_window` 字段使用 `Option<usize>` + `serde(default)`，旧配置文件自动兼容
5. **Ollama 模型**: Ollama 运行的本地模型上下文窗口大小不确定，使用保守默认值 8192，并在 UI 中引导用户手动设置。未来可通过 Ollama API (`/api/show`) 查询模型的 `num_ctx` 参数
6. **custom Provider**: 自定义 Provider 的模型名称不在预设表中，需要用户手动设置或使用默认 128K
7. **实时更新的渲染性能**: 频繁的状态更新可能导致横条组件频繁重渲染。使用 CSS transition 而非 JS 动画，配合防抖减少状态更新频率
8. **API 实际 Token 用量的反馈闭环（建议二期优化）**: 当前 LLM 适配器已从 API 响应中解析 `ChatUsage`（`prompt_tokens`/`completion_tokens`/`total_tokens`），但 `AgentExecutor` 和路由层完全忽略了这些数据。本计划一期不涉及该优化，但建议作为二期任务将此真实 Token 数据反馈到 `TokenBudgetManager`，替代字符级估算，实现更精确的上下文管理
9. **context_window 运行时不变**: Agent 启动时确定 context_window，运行期间即使 Provider 切换也不更新，避免压缩阈值突变产生不可预期行为
