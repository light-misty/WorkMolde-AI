# LLM 缓存命中率优化设计文档

> 日期：2026-06-14
> 状态：设计稿
> 涉及组件：Rust 后端 (models/llm, services/llm/*_adapter, services/agent/{executor,context})、TypeScript 前端 (ContextWindowSection, workflow store, settings types)

---

## 一、背景与问题分析

### 1.1 当前缓存命中率低的原因

当前 DeepSeek 缓存命中率约 30%，远低于预期。原因如下：

| 原因 | 说明 | 影响程度 |
|------|------|----------|
| **未跟踪 API 返回的缓存字段** | `ChatUsage` 仅解析了 `prompt_tokens` / `completion_tokens` / `total_tokens`，忽略了 `prompt_cache_hit_tokens` / `prompt_cache_miss_tokens`（OpenAI 兼容格式）和 Anthropic 的 `cache_creation_input_tokens` / `cache_read_input_tokens` | 高 |
| **系统提示词每轮迭代变化** | `get_messages_for_iteration()` 在迭代 > 1 时向系统提示词追加迭代上下文（`build_iteration_context`），改变了 prompt 前缀，导致 DeepSeek 前缀缓存失效 | 高 |
| **Token 计数纯启发式估算** | 使用 `estimate_tokens()`（字符计数法）而非 API 返回的真实值，无法准确体现缓存效果 | 中 |
| **消息顺序不确定** | 工具执行结果按执行顺序插入消息列表，不同工具调用顺序导致消息前缀不同 | 中 |
| **无缓存感知的消息构建** | 系统提示词、工具定义、对话历史三者混合，没有将稳定部分与动态部分分离 | 高 |
| **DeepSeek 前缀缓存机制特性** | 需要从 token 0 开始的完全匹配前缀；以 64 token 为存储单元（实际需 ≥1024 tokens 才能稳定命中）；缓存构建需要时间 | — |

### 1.2 DeepSeek 上下文缓存原理

DeepSeek 的磁盘缓存机制核心要点：

- **自动启用**：所有用户默认开启，无需代码更改
- **前缀匹配**：仅从 token 0 开始的完全匹配前缀才算命中。中间部分匹配不会触发缓存
- **三类持久化**：
  - 请求边界持久化：每个请求在用户输入末尾和模型输出末尾产生缓存前缀单元
  - 公共前缀检测持久化：系统检测多个请求间的公共前缀并持久化为独立缓存单元
  - 固定 token 间隔持久化：避免长前缀因一直未到达末尾位置而完全无法缓存
- **返回字段**：`usage.prompt_cache_hit_tokens`（命中）和 `usage.prompt_cache_miss_tokens`（未命中）
- **价格差异**：命中 ~$0.0028/百万 tokens vs 未命中 ~$0.14/百万 tokens（50 倍差距）
- **存储单元**：64 tokens 为最小单位，V4 实际需要 ≥1024 tokens 前缀长度才能稳定命中
- **尽力而为**：不保证 100% 命中率，缓存构建需要数秒，空闲后数小时至数天自动清除

### 1.3 主流 LLM 提供商的缓存机制对比

| 提供商 | 缓存类型 | 触发方式 | API 返回字段 | 特化优化策略 |
|--------|---------|---------|-------------|------------|
| **DeepSeek** | 磁盘前缀缓存（自动） | 从 token 0 完全匹配 | `prompt_cache_hit_tokens`, `prompt_cache_miss_tokens` | 保持系统提示词 + 工具定义前缀完全不变；分离稳定/动态内容 |
| **Anthropic** | Prompt Caching（显式标记） | `cache_control` 标记 | `cache_creation_input_tokens`, `cache_read_input_tokens` | 标记系统提示词和最近历史为可缓存 |
| **Gemini** | Context Caching（独立资源） | 创建缓存资源后 `cachedContent` | `cachedContentTokenCount` | 适合固定上下文 + 多轮查询场景 |
| **OpenAI** | 无自动服务器缓存 | — | 无 | 无特定优化 |

---

## 二、设计目标

1. **跟踪真实的缓存命中/未命中 Token 数**：从各 Provider API 响应中解析缓存相关字段
2. **实时显示缓存命中率**：在右侧栏 `ContextWindowSection` 中以 `缓存命中率：90%` 格式展示
3. **优化 prompt 前缀稳定性**：针对 DeepSeek 优化消息构建，最大化前缀缓存命中
4. **支持多 Provider**：DeepSeek 作为主要优化目标，同时适配 Anthropic 和 Gemini
5. **成本可视化**：让用户清晰看到缓存带来的实际节省

---

## 三、详细设计方案

### 3.1 数据模型扩展

#### 3.1.1 Rust 后端 — `ChatUsage` 扩展

文件：`src-tauri/src/models/llm.rs`

```rust
/// Token 用量（含缓存信息）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,

    // --- 新增缓存字段 ---

    /// DeepSeek: 命中缓存的输入 tokens
    #[serde(default)]
    pub prompt_cache_hit_tokens: u64,

    /// DeepSeek: 未命中缓存的输入 tokens
    #[serde(default)]
    pub prompt_cache_miss_tokens: u64,

    /// Anthropic: 缓存创建消耗的 tokens
    #[serde(default)]
    pub cache_creation_input_tokens: u64,

    /// Anthropic: 缓存读取消耗的 tokens
    #[serde(default)]
    pub cache_read_input_tokens: u64,

    /// Gemini: 缓存命中 tokens
    #[serde(default)]
    pub cached_content_token_count: u64,
}
```

#### 3.1.2 Rust 后端 — `ContextUsageInfo` 扩展

文件：`src-tauri/src/models/llm.rs`

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContextUsageInfo {
    // ... 现有字段保持不变 ...
    pub context_window: usize,
    pub system_prompt_tokens: usize,
    pub function_definitions_tokens: usize,
    pub conversation_tokens: usize,
    pub response_tokens: usize,
    pub total_used_tokens: usize,
    pub compression_status: String,
    pub model_name: String,
    pub total_message_count: usize,
    pub retained_message_count: usize,

    // --- 新增缓存统计字段 ---

    /// 本轮请求的缓存命中 tokens（来自 API 响应）
    pub cache_hit_tokens: u64,
    /// 本轮请求的缓存未命中 tokens（来自 API 响应）
    pub cache_miss_tokens: u64,
    /// 本轮请求的缓存创建 tokens（Anthropic）
    pub cache_creation_tokens: u64,
    /// 生命周期累计缓存命中 tokens
    pub lifetime_cache_hit_tokens: u64,
    /// 生命周期累计缓存未命中 tokens
    pub lifetime_cache_miss_tokens: u64,
    /// 缓存命中率（0.0 - 1.0），实时计算
    pub cache_hit_rate: f64,
    /// Provider 缓存类型标识
    pub provider_cache_type: String,  // "deepseek" | "anthropic" | "gemini" | "none"
}
```

#### 3.1.3 Rust 后端 — `StreamChunk` 扩展

```rust
/// 流式响应块（携带可选 usage，仅在最后一个 chunk 中存在）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StreamChunk {
    pub id: String,
    pub choices: Vec<StreamChoice>,
    #[serde(default)]
    pub usage: Option<ChatUsage>,  // 新增
}
```

#### 3.1.4 TypeScript 前端同步

文件：`src/types/settings.ts`

```typescript
export interface ContextUsageInfo {
  contextWindow: number;
  systemPromptTokens: number;
  functionDefinitionsTokens: number;
  conversationTokens: number;
  responseTokens: number;
  totalUsedTokens: number;
  compressionStatus: string;
  modelName: string;
  totalMessageCount: number;
  retainedMessageCount: number;

  // 新增缓存字段
  cacheHitTokens: number;
  cacheMissTokens: number;
  cacheCreationTokens: number;
  lifetimeCacheHitTokens: number;
  lifetimeCacheMissTokens: number;
  cacheHitRate: number;
  providerCacheType: "deepseek" | "anthropic" | "gemini" | "none";
}
```

文件：`src/services/event.ts`

```typescript
// StreamChunk 类型同步
export interface StreamChunk {
  id: string;
  choices: StreamChoice[];
  usage?: ChatUsage;  // 新增
}
```

### 3.2 Provider 适配器修改

#### 3.2.1 OpenAI 适配器（兼容 DeepSeek）

文件：`src-tauri/src/services/llm/openai_adapter.rs`

```rust
// parse_response() 中扩展 usage 解析
fn parse_response(&self, value: Value) -> Result<ChatResponse, CommandError> {
    // ... 现有代码 ...

    let usage = value["usage"].as_object().map(|u| ChatUsage {
        prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0),
        completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0),
        total_tokens: u["total_tokens"].as_u64().unwrap_or(0),

        // DeepSeek 缓存字段（也兼容 OpenAI 格式的其他 Provider）
        prompt_cache_hit_tokens: u["prompt_cache_hit_tokens"].as_u64().unwrap_or(0),
        prompt_cache_miss_tokens: u["prompt_cache_miss_tokens"].as_u64().unwrap_or(0),

        // Anthropic 字段在 OpenAI 兼容模式下不使用
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
        cached_content_token_count: 0,
    });

    // ...
}
```

SSE 流式解析中，在最后一个 chunk（含 `finish_reason`）提取 usage：

```rust
// 流式 SSE 循环中解析每个 chunk 时增加 usage 提取
let usage = value.get("usage").map(|u| ChatUsage {
    prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0),
    completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0),
    total_tokens: u["total_tokens"].as_u64().unwrap_or(0),
    prompt_cache_hit_tokens: u["prompt_cache_hit_tokens"].as_u64().unwrap_or(0),
    prompt_cache_miss_tokens: u["prompt_cache_miss_tokens"].as_u64().unwrap_or(0),
    ..Default::default()
});

let chunk = StreamChunk {
    id,
    choices,
    usage,  // 填入 usage
};
```

#### 3.2.2 Anthropic 适配器

文件：`src-tauri/src/services/llm/anthropic_adapter.rs`

```rust
// parse_response() 中 Anthropic 的 usage 结构:
// { "input_tokens": N, "output_tokens": N,
//   "cache_creation_input_tokens": N,  // 可选
//   "cache_read_input_tokens": N       // 可选
// }

let usage = value["usage"].as_object().map(|u| ChatUsage {
    prompt_tokens: u["input_tokens"].as_u64().unwrap_or(0),
    completion_tokens: u["output_tokens"].as_u64().unwrap_or(0),
    total_tokens: u["input_tokens"].as_u64().unwrap_or(0)
        + u["output_tokens"].as_u64().unwrap_or(0),

    prompt_cache_hit_tokens: u["cache_read_input_tokens"].as_u64().unwrap_or(0),
    prompt_cache_miss_tokens: u["input_tokens"].as_u64().unwrap_or(0)
        .saturating_sub(u["cache_read_input_tokens"].as_u64().unwrap_or(0)),
    cache_creation_input_tokens: u["cache_creation_input_tokens"].as_u64().unwrap_or(0),
    cache_read_input_tokens: u["cache_read_input_tokens"].as_u64().unwrap_or(0),
    cached_content_token_count: 0,
});
```

**Anthropic 显式 `cache_control` 标记**（额外优化）：

```rust
// build_request_body() 中，为系统提示词和第一组对话添加 cache_control:
// 系统提示词 -> cache_control: { "type": "ephemeral" }
// 历史消息中的第一轮 user + assistant -> cache_control: { "type": "ephemeral" }
// 后续消息不加 cache_control 标记，保持动态

// 请求体示例:
// {
//   "model": "claude-sonnet-4-20250514",
//   "system": [
//     {
//       "type": "text",
//       "text": "...系统提示词...",
//       "cache_control": { "type": "ephemeral" }
//     }
//   ],
//   "messages": [
//     {
//       "role": "user",
//       "content": [
//         { "type": "text", "text": "...第一条历史消息...",
//           "cache_control": { "type": "ephemeral" } }
//       ]
//     },
//     { "role": "assistant", "content": "..." },
//     // 后续消息不加 cache_control
//   ]
// }
```

#### 3.2.3 Gemini 适配器

文件：`src-tauri/src/services/llm/gemini_adapter.rs`

```rust
// Gemini 的 usageMetadata 包含 cachedContentTokenCount
let usage = value["usageMetadata"].as_object().map(|u| ChatUsage {
    prompt_tokens: u["promptTokenCount"].as_u64().unwrap_or(0),
    completion_tokens: u["candidatesTokenCount"].as_u64().unwrap_or(0),
    total_tokens: u["totalTokenCount"].as_u64().unwrap_or(0),

    prompt_cache_hit_tokens: u["cachedContentTokenCount"].as_u64().unwrap_or(0),
    prompt_cache_miss_tokens: u["promptTokenCount"].as_u64().unwrap_or(0)
        .saturating_sub(u["cachedContentTokenCount"].as_u64().unwrap_or(0)),

    cache_creation_input_tokens: 0,
    cache_read_input_tokens: 0,
    cached_content_token_count: u["cachedContentTokenCount"].as_u64().unwrap_or(0),
});
```

### 3.3 Agent Executor 修改

核心改动：将 LLM 响应中的 `ChatUsage` 传递给上下文使用信息计算。

#### 3.3.1 流结束后提取 usage

流式响应结束后，从最后一个包含 usage 的 StreamChunk 提取缓存信息：

```rust
// src-tauri/src/services/agent/executor.rs

// 在流式循环结束后，检查是否从最后一个 chunk 中获得了 usage
let mut final_usage: Option<ChatUsage> = None;
// ... 流式循环中将最后一个 chunk 的 usage 保存到 final_usage ...

// 调用 emit_context_usage 传递 usage
let response_tokens = if let Some(ref usage) = final_usage {
    usage.completion_tokens as usize  // 使用 API 返回的真实值
} else {
    TokenBudgetManager::estimate_tokens(&assistant_content)  // fallback
};

self.emit_context_usage(ctx, response_tokens, final_usage.as_ref()).await;
```

#### 3.3.2 `emit_context_usage` 接收真实 usage

```rust
// 修改签名，接收 Option<&ChatUsage>
async fn emit_context_usage(
    &self,
    ctx: &AgentContext,
    response_tokens: usize,
    usage: Option<&ChatUsage>,  // 新增：来自 API 的真实 token 用量
) {
    let model_name = self.router.current_model_name();

    let usage_info = ctx.calculate_context_usage(response_tokens, model_name, usage);

    // 持久化到数据库
    if let Some(ref persist_fn) = self.context_usage_persist_fn {
        persist_fn(&ctx.session_id, &usage_info);
    }

    self.emitter.emit_context_usage(ContextUsagePayload {
        session_id: ctx.session_id.clone(),
        context_usage: usage_info,
    }).ok();
}
```

#### 3.3.3 AgentContext 添加累计缓存统计

```rust
// src-tauri/src/services/agent/context.rs

pub struct AgentContext {
    // ... 现有字段 ...

    // 新增：生命周期累计缓存统计（持久化到 SQLite 并在会话恢复时加载）
    pub lifetime_cache_hit_tokens: u64,
    pub lifetime_cache_miss_tokens: u64,
}

impl AgentContext {
    pub fn new(session_id: String, system_prompt: String, context_window: usize) -> Self {
        Self {
            // ... 现有初始化 ...
            lifetime_cache_hit_tokens: 0,
            lifetime_cache_miss_tokens: 0,
        }
    }

    /// 从历史会话恢复累计缓存统计
    pub fn restore_cache_stats(&mut self, hit: u64, miss: u64) {
        self.lifetime_cache_hit_tokens = hit;
        self.lifetime_cache_miss_tokens = miss;
    }

    pub fn calculate_context_usage(
        &self,
        response_tokens: usize,
        model_name: String,
        usage: Option<&ChatUsage>,
    ) -> ContextUsageInfo {
        let system_prompt_tokens = TokenBudgetManager::estimate_tokens(&self.system_prompt);
        let conversation_tokens = TokenBudgetManager::estimate_tokens(
            &self.messages.iter().map(|m| m.content.as_str()).collect::<String>()
        );
        let function_definitions_tokens = self.function_definitions_tokens;
        let total_used_tokens = system_prompt_tokens + function_definitions_tokens
            + conversation_tokens + response_tokens;

        let usage_percentage = if self.token_budget.context_window() > 0 {
            (total_used_tokens as f64 / self.token_budget.context_window() as f64).min(1.0)
        } else {
            0.0
        };

        let is_over_budget = self.token_budget.is_conversation_over_budget(conversation_tokens);
        let compression_status = if usage_percentage >= 0.95 {
            "critical".to_string()
        } else if is_over_budget {
            "compressed".to_string()
        } else {
            "normal".to_string()
        };

        // --- 缓存统计 ---
        let (cache_hit_tokens, cache_miss_tokens, cache_creation_tokens) = match usage {
            Some(u) => (u.prompt_cache_hit_tokens, u.prompt_cache_miss_tokens, u.cache_creation_input_tokens),
            None => (0, 0, 0),
        };

        let lifetime_hit = self.lifetime_cache_hit_tokens + cache_hit_tokens;
        let lifetime_miss = self.lifetime_cache_miss_tokens + cache_miss_tokens;
        let total_lifetime = lifetime_hit + lifetime_miss;
        let cache_hit_rate = if total_lifetime > 0 {
            lifetime_hit as f64 / total_lifetime as f64
        } else {
            0.0
        };

        ContextUsageInfo {
            context_window: self.token_budget.context_window(),
            system_prompt_tokens,
            function_definitions_tokens,
            conversation_tokens,
            response_tokens,
            total_used_tokens,
            compression_status,
            model_name,
            total_message_count: self.messages.len(),
            retained_message_count: self.messages.len().min(self.calculate_keep_message_count()),

            // 缓存统计
            cache_hit_tokens,
            cache_miss_tokens,
            cache_creation_tokens,
            lifetime_cache_hit_tokens: lifetime_hit,
            lifetime_cache_miss_tokens: lifetime_miss,
            cache_hit_rate,
            provider_cache_type: self.detect_cache_type(),
        }
    }

    /// 根据当前 Provider 类型检测缓存类型
    fn detect_cache_type(&self) -> String {
        // 实际实现中可以通过 router 或 provider meta 获取
        // 这里简化为占位，运行时由 executor 注入 model_name 或 provider_type
        // 通过在 build_system_prompt 或 executor 中传递 provider_type 信息实现
        "none".to_string()
    }
}
```

#### 3.3.4 Provider 缓存类型检测

文件：`src-tauri/src/services/llm/router.rs`

```rust
impl LlmRouter {
    /// 获取当前默认 Provider 的缓存类型
    pub fn current_cache_type(&self) -> &str {
        let default_id = self.default_id.lock().unwrap();
        if let Some(meta) = self.meta.get(&*default_id) {
            match meta.provider_type.as_str() {
                "openai" | "custom" => {
                    // DeepSeek 使用 OpenAI 兼容接口，通过模型名判断
                    if meta.model.to_lowercase().contains("deepseek") {
                        "deepseek"
                    } else {
                        "none"
                    }
                }
                "anthropic" => "anthropic",
                "gemini" => "gemini",
                "ollama" => "none",
                _ => "none",
            }
        } else {
            "none"
        }
    }
}
```

### 3.4 Prompt 前缀稳定性优化（DeepSeek 核心优化）

这是提升 DeepSeek 缓存命中率最关键的改造。当前问题在于 `get_messages_for_iteration()` 每次迭代都在修改系统提示词前缀。

#### 3.4.1 分离系统提示词与迭代上下文

文件：`src-tauri/src/services/agent/context.rs`

```rust
/// 当前方案（缓存不友好）：
/// 迭代 > 1 时系统提示词被修改：
///   system_content = format!("{}\n\n{}", self.system_prompt, iteration_context)
/// 结果：每次迭代 system message 内容变化，前缀缓存失效

/// 优化方案（缓存友好）：
/// 迭代上下文作为独立的 user 消息追加，不修改 system prompt：
/// 消息序列变为：
///   [system: 固定系统提示词（从不变化）]
///   [user: <iteration_context>（仅在迭代 > 1 时，独立消息）]
///   [user: 原始用户消息]
///   [assistant: LLM 响应]
///   [tool: 工具结果]
///   ...

pub fn get_messages_for_iteration(&self, current_iteration: u32) -> Vec<ChatMessage> {
    // 系统提示词始终为原始内容，不附加任何迭代上下文
    let mut all = vec![ChatMessage {
        role: "system".to_string(),
        content: self.system_prompt.clone(),  // 从不变化
        content_parts: None,
        tool_calls: None,
        tool_call_id: None,
        reasoning_content: None,
        attachments: None,
    }];

    // 迭代上下文作为独立 user 消息（不修改 system prompt）
    if current_iteration > 1 {
        all.push(ChatMessage {
            role: "user".to_string(),
            content: format!(
                "<iteration_context>\n## 当前执行进度\n\n迭代轮次: {}/{}\n{}当前步骤: [进行中] {}\n\n请基于以上进度继续执行，不要重复已完成的步骤。\n</iteration_context>",
                current_iteration,
                self.max_iterations,
                self.format_completed_steps(),
                self.current_step,
            ),
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            attachments: None,
        });
    }

    // 对话历史压缩处理
    let compression_result = self.compress_history_if_needed();
    let processed_messages = compression_result.messages;

    // 遍历消息，压缩早期 reasoning_content
    let last_reasoning_idx = processed_messages.iter().rposition(|m| {
        m.role == "assistant" && m.reasoning_content.is_some()
    });

    for (i, msg) in processed_messages.iter().enumerate() {
        let mut compressed_msg = msg.clone();
        if let Some(rc) = &msg.reasoning_content {
            let is_latest = last_reasoning_idx.is_none_or(|idx| i == idx);
            if !is_latest && rc.len() > REASONING_COMPRESS_THRESHOLD {
                let kept = rc.chars().take(REASONING_COMPRESS_KEEP).collect::<String>();
                compressed_msg.reasoning_content = Some(format!("{}...(已省略)", kept));
            }
        }
        all.push(compressed_msg);
    }

    all
}

fn format_completed_steps(&self) -> String {
    if self.completed_steps.is_empty() {
        return String::new();
    }
    let mut s = "已完成步骤:\n".to_string();
    for (i, step) in self.completed_steps.iter().enumerate() {
        s.push_str(&format!("  {}. [已完成] {}\n", i + 1, step));
    }
    s
}
```

#### 3.4.2 工具定义稳定排序

文件：`src-tauri/src/services/agent/executor.rs`

```rust
// 在构建 tool_defs_json 时确保稳定排序
let tool_defs_json = {
    let tool_defs = self.tool_registry.tool_definitions();
    let handler_defs = {
        let reg = self.registry.lock().await;
        reg.tool_definitions()
    };
    let mut all = [tool_defs, handler_defs].concat();
    // 按 "function.name" 字母序稳定排序，确保相同工具集产生相同 JSON 序列化
    all.sort_by(|a, b| {
        let name_a = a["function"]["name"].as_str().unwrap_or("");
        let name_b = b["function"]["name"].as_str().unwrap_or("");
        name_a.cmp(name_b)
    });
    all
};
```

#### 3.4.3 完整请求体的前缀稳定性

最终的 API 请求体结构为：

```json
{
  "model": "deepseek-chat",
  "messages": [
    {"role": "system", "content": "固定系统提示词"},
    // ... 后续消息
  ],
  "tools": [...]  // 稳定排序的工具定义
}
```

优化策略的最终目标是：**从 token 0 开始，请求体的前 N 个 token（系统提示词 + 工具定义 + 第一条用户消息的起始部分）在多次请求之间保持完全一致**。

### 3.5 ContextWindowSection 前端修改

#### 3.5.1 缓存命中率 UI 展示

文件：`src/components/sidebar/ContextWindowSection.tsx`

```tsx
function CacheHitRateBar({
  hitRate,
  cacheType,
  hitTokens,
  missTokens,
}: {
  hitRate: number;
  cacheType: string;
  hitTokens: number;
  missTokens: number;
}) {
  const { t } = useTranslation();
  const percent = Math.round(hitRate * 100);
  const color =
    percent >= 70
      ? "var(--color-success)"
      : percent >= 40
        ? "var(--color-warning)"
        : "var(--color-error)";

  const cacheTypeLabel = {
    deepseek: "DeepSeek 磁盘缓存",
    anthropic: "Anthropic Prompt Caching",
    gemini: "Gemini Context Caching",
    none: "",
  }[cacheType] ?? "";

  return (
    <div className="cw-cache-section">
      {/* 命中率标题行 */}
      <div className="cw-cache-header">
        <span className="cw-cache-label">
          {t('contextWindow.cacheHitRate')}
        </span>
        <span className="cw-cache-rate" style={{ color }}>
          {percent}%
        </span>
      </div>

      {/* 命中率横条 */}
      {hitRate > 0 && (
        <div className="cw-cache-bar-track">
          <div
            className="cw-cache-bar-hit"
            style={{ width: `${percent}%`, background: color }}
            title={`${t('contextWindow.cacheHit')}: ${formatTokens(hitTokens)}`}
          />
          {percent < 100 && (
            <div
              className="cw-cache-bar-miss"
              style={{ width: `${100 - percent}%` }}
              title={`${t('contextWindow.cacheMiss')}: ${formatTokens(missTokens)}`}
            />
          )}
        </div>
      )}

      {/* Provider 缓存类型标签 */}
      {cacheType !== "none" && (
        <span className="cw-cache-provider-tag" data-type={cacheType}>
          {cacheTypeLabel}
        </span>
      )}

      {/* Token 明细 */}
      {hitRate > 0 && (
        <div className="cw-cache-detail">
          {t('contextWindow.cacheHitTokenDetail', {
            hit: formatTokens(hitTokens),
            miss: formatTokens(missTokens),
          })}
        </div>
      )}
    </div>
  );
}
```

在 `ContextWindowSection` 主组件中，现有 bar 下方插入缓存命中率：

```tsx
// 有实时数据时，在 sections 之后插入
{contextUsage.providerCacheType !== "none" && (
  <CacheHitRateBar
    hitRate={contextUsage.cacheHitRate}
    cacheType={contextUsage.providerCacheType}
    hitTokens={contextUsage.lifetimeCacheHitTokens}
    missTokens={contextUsage.lifetimeCacheMissTokens}
  />
)}
```

#### 3.5.2 新增样式

```tsx
function CWStyles() {
  return (
    <style>{`
      /* ... 现有样式 ... */

      /* 缓存命中率区域 */
      .cw-cache-section {
        margin-top: 6px;
        padding-top: 6px;
        border-top: 1px solid var(--color-border-secondary);
      }
      .cw-cache-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 3px;
      }
      .cw-cache-label {
        font-size: 11px;
        font-weight: 600;
        color: var(--color-text-tertiary);
      }
      .cw-cache-rate {
        font-size: 14px;
        font-weight: 700;
        font-variant-numeric: tabular-nums;
      }
      .cw-cache-bar-track {
        height: 4px;
        background: var(--color-context-idle);
        border-radius: 2px;
        overflow: hidden;
        display: flex;
      }
      .cw-cache-bar-hit {
        height: 100%;
        transition: width 0.5s ease;
        min-width: 0;
      }
      .cw-cache-bar-miss {
        height: 100%;
        min-width: 0;
        background: var(--color-context-idle);
        opacity: 0.5;
      }
      .cw-cache-provider-tag {
        display: inline-block;
        padding: 1px 6px;
        border-radius: 3px;
        font-size: 9px;
        font-weight: 500;
        margin-top: 3px;
      }
      .cw-cache-provider-tag[data-type="deepseek"] {
        background: #4D6BFE20;
        color: #4D6BFE;
      }
      .cw-cache-provider-tag[data-type="anthropic"] {
        background: #D4895720;
        color: #D48957;
      }
      .cw-cache-provider-tag[data-type="gemini"] {
        background: #4285F420;
        color: #4285F4;
      }
      .cw-cache-detail {
        font-size: 9px;
        color: var(--color-text-quaternary);
        margin-top: 2px;
      }
    `}</style>
  );
}
```

### 3.6 国际化 i18n 添加

文件：`src/i18n/locales/zh-CN.json`

```json
{
  "contextWindow": {
    "cacheHitRate": "缓存命中率",
    "cacheHit": "缓存命中",
    "cacheMiss": "缓存未命中",
    "cacheHitTokenDetail": "命中 {{hit}} / 未命中 {{miss}}"
  }
}
```

文件：`src/i18n/locales/en-US.json`

```json
{
  "contextWindow": {
    "cacheHitRate": "Cache Hit Rate",
    "cacheHit": "Cache Hit",
    "cacheMiss": "Cache Miss",
    "cacheHitTokenDetail": "Hit {{hit}} / Miss {{miss}}"
  }
}
```

---

## 四、事件与数据流

### 4.1 数据流图

```
LLM API 响应（含缓存字段）
  │
  ▼
Provider Adapter (parse_response / SSE 解析)
  │ 提取 prompt_cache_hit_tokens / prompt_cache_miss_tokens
  ▼
ChatUsage { prompt_cache_hit_tokens, prompt_cache_miss_tokens, ... }
  │
  ▼
AgentExecutor 流结束 → 从最后的 StreamChunk.usage 获取
  │
  ▼
emit_context_usage(ctx, response_tokens, Some(usage))
  │
  ▼
AgentContext.calculate_context_usage()
  │ 合并到 lifetime_cache_hit_tokens / lifetime_cache_miss_tokens
  │ 计算 cache_hit_rate = hit / (hit + miss)
  │ detect_cache_type() 判断 Provider 缓存类型
  ▼
ContextUsageInfo { cacheHitRate, lifetimeCacheHitTokens, providerCacheType, ... }
  │
  ├─▶ 持久化到 SQLite（sessions.context_usage_json）
  │
  ▼
agent:context_update 事件 → 前端 onAgentContextUpdate()
  │
  ▼
useWorkflowStore.set({ contextUsage })
  │
  ▼
ContextWindowSection → CacheHitRateBar 渲染缓存命中率
```

### 4.2 累计统计的持久化与恢复

```rust
// context_usage_json 已持久化完整 ContextUsageInfo（含新增缓存字段）
// 历史会话恢复流程：

// 1. get_context_usage Tauri 命令加载 SQLite 中的 JSON
// 2. 反序列化为 ContextUsageInfo（lifetime_cache_* 字段自动恢复）
// 3. 前端加载并显示缓存统计

// 新会话开始时：
// lifetime_cache_hit_tokens = 0
// lifetime_cache_miss_tokens = 0
```

---

## 五、实施计划

### 阶段一：缓存跟踪核心（预估 3-4 天）

| 任务 | 文件 | 工作量 |
|------|------|--------|
| 1.1 `ChatUsage` 扩展 - 增加缓存字段 | `models/llm.rs` | 小 |
| 1.2 `StreamChunk` 扩展 - 增加 `usage` 字段 | `models/llm.rs` | 小 |
| 1.3 OpenAI 适配器 - 非流式 + 流式解析缓存字段 | `openai_adapter.rs` | 中 |
| 1.4 Anthropic 适配器 - 非流式 + 流式解析缓存字段 | `anthropic_adapter.rs` | 中 |
| 1.5 Gemini 适配器 - 非流式 + 流式解析缓存字段 | `gemini_adapter.rs` | 小 |
| 1.6 Agent Executor - 流结束后提取 usage 传递到 `emit_context_usage` | `executor.rs` | 中 |
| 1.7 `AgentContext` - 累计缓存统计 + `calculate_context_usage` 扩展示例 | `context.rs` | 中 |
| 1.8 `ContextUsageInfo` 前后端类型同步 | `models/llm.rs`, `types/settings.ts`, `services/event.ts` | 小 |

### 阶段二：缓存优化（预估 2-3 天）

| 任务 | 文件 | 工作量 |
|------|------|--------|
| 2.1 分离系统提示词与迭代上下文（独立 user 消息） | `context.rs` | 中 |
| 2.2 工具定义稳定排序（按名称字母序） | `executor.rs` | 小 |
| 2.3 Anthropic `cache_control` 标记注入 | `anthropic_adapter.rs` | 中 |
| 2.4 Provider 缓存类型检测（`current_cache_type`） | `router.rs`, `context.rs` | 小 |

### 阶段三：前端 UI（预估 2 天）

| 任务 | 文件 | 工作量 |
|------|------|--------|
| 3.1 `ContextUsageInfo` 前端类型同步 | `types/settings.ts` | 小 |
| 3.2 `CacheHitRateBar` 组件开发 | `ContextWindowSection.tsx` | 中 |
| 3.3 i18n 翻译键添加 | `zh-CN.json`, `en-US.json` | 小 |
| 3.4 样式编写 | `ContextWindowSection.tsx` | 小 |

### 总计：约 7-9 天

---

## 六、风险与注意事项

1. **DeepSeek 缓存特性**：前缀必须从 token 0 完全匹配；缓存构建需要数秒；V4 实际需 ≥1024 tokens 前缀长度才稳定命中；不保证 100% 命中率；存储单元为 64 tokens
2. **Anthropic 缓存特性**：显式 `cache_control` 需要额外设计消息结构；缓存有效期约 5 分钟；缓存创建本身消耗 tokens
3. **竞争条件**：流式响应中 usage 可能延迟到最后一个 chunk 才出现，需确保 stream 关闭前捕获
4. **向后兼容**：所有新增字段均 `#[serde(default)]`，确保旧版 JSON 可正常反序列化
5. **Provider 自适应**：对无缓存支持的 Provider（如 OpenAI 标准版），所有缓存字段为 0，前端不显示缓存区域
6. **历史会话**：切换回历史会话时，lifetime 缓存统计从 SQLite `context_usage_json` 自动恢复
7. **缓存不累加重复 token**：缓存命中的 token 不计入 `prompt_tokens`，只占 billing 计数，因此 `prompt_cache_hit_tokens + prompt_cache_miss_tokens` 不一定等于 `prompt_tokens`

---

## 七、验证方法

### 7.1 API 响应验证

添加 `deepseek-chat` / `deepseek-reasoner` Provider，发送多条具有相同前缀（相同系统提示词 + 工具定义）的请求：

```
请求 1: 系统提示词(500t) + 工具定义(500t) + "你好" → miss
请求 2: 系统提示词(500t) + 工具定义(500t) + "请继续" → hit (前缀 1000t 命中)
```

期望结果：`prompt_cache_hit_tokens` > 0，且随时间推移命中率逐渐提升。

### 7.2 累计统计验证

多轮对话后：

- `lifetime_cache_hit_tokens` 正确累计每轮命中
- `lifetime_cache_miss_tokens` 正确累计每轮未命中
- `cache_hit_rate = lifetime_hit / (lifetime_hit + lifetime_miss)` 计算正确

### 7.3 UI 验证

- Agent 执行期间，缓存命中率实时更新
- 百分比显示正确，横条颜色随命中率变化（绿 ≥70% / 黄 ≥40% / 红 <40%）
- Provider 缓存类型标签显示正确（DeepSeek / Anthropic / Gemini）

### 7.4 回归验证

- 切换 Provider 为 OpenAI 标准版：缓存区域不显示
- 切换回 DeepSeek：缓存区域恢复显示
- 历史会话切换：缓存统计从 DB 恢复

### 7.5 前缀优化对比验证

相同对话场景下，对比优化前后 DeepSeek API 返回的 `prompt_cache_hit_tokens` 占比：

- 优化前：~30% 命中率
- 优化后：期望 ≥70% 命中率
