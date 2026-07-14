# 阶段 4:子 Agent 与高级工具 详细改造文档

> **文档版本**:v1.1(2026-07-08 修订:子 Agent 继承 Document 模式,工具列表按模式过滤)
> **创建日期**:2026-07-08
> **阶段目标**:实现 Task 工具(子 Agent 委托)、WebFetch 工具(URL 内容获取)、WebSearch 工具(网络搜索),扩展 Agent 的任务分解与信息获取能力
> **依赖阶段**:[阶段 1:核心架构与工具链](./2026-07-08-coding-agent-refactor-phase1-core.md)、[阶段 2:权限系统与 Agent 模式](./2026-07-08-coding-agent-refactor-phase2-permission.md)、[阶段 3:Skill 系统与上下文管理](./2026-07-08-coding-agent-refactor-phase3-skill-context.md)
> **预计任务数**:19 个(T4.01-T4.19,含 question 工具)
> **v1.1 修订**:子 Agent 继承父 Agent 的 AgentMode(含 Document),工具列表需按模式过滤(非 Document 模式下文档 Handler 不对子 Agent 可见)

---

## 一、阶段概述

### 1.1 改造背景

OpenCode 通过 Task 工具、WebFetch 工具、WebSearch 工具,构建了强大的任务分解与信息获取能力:

1. **Task 工具(子 Agent)**:允许主 Agent 委托复杂子任务给独立的子 Agent 执行。子 Agent 继承父 Agent 的部分上下文(系统提示词、权限规则),独立完成子任务后返回结果。支持并行执行多个子 Agent,大幅提升复杂任务的处理效率。

2. **WebFetch 工具**:获取指定 URL 的网页内容,自动转换为 markdown 格式,支持内容截断和字符数限制。让 Agent 能够读取在线文档、API 参考等资源。

3. **WebSearch 工具**:执行网络搜索,返回相关结果列表(标题、URL、摘要)。让 Agent 能够主动发现信息,而非被动等待用户提供。

### 1.2 WorkMolde AI 现状

- **无子 Agent 能力**:所有任务在单一 Agent 上下文中执行,复杂任务容易耗尽 token
- **无网络访问能力**:Agent 无法获取 URL 内容或执行搜索,信息获取完全依赖用户输入
- **Tauri CSP 限制**:当前 CSP 仅允许 `http://localhost:*` 和 `http://127.0.0.1:*`,需调整以支持外部 URL 访问

### 1.3 改造目标

1. **实现 Task 工具**:支持委托子任务给子 Agent,继承父 Agent 上下文,独立执行后返回结果
2. **实现 WebFetch 工具**:获取 URL 内容,转换为 markdown,支持截断
3. **实现 WebSearch 工具**:执行网络搜索,返回结果列表
4. **调整 Tauri CSP**:允许 Agent 访问外部 URL(用户可配置白名单)
5. **权限系统集成**:Task/WebFetch/WebSearch 工具受权限系统控制

### 1.4 设计原则

- **隔离性**:子 Agent 拥有独立的上下文窗口,不影响父 Agent
- **继承性**:子 Agent 继承父 Agent 的系统提示词、权限规则、工作区配置、AgentMode(v1.1:含 Document 模式)
- **可控性**:子 Agent 的最大迭代次数、超时时间可配置
- **安全性**:WebFetch/WebSearch 受权限系统控制,防止访问恶意 URL
- **可观测性**:子 Agent 的执行过程通过事件推送到前端
- **v1.1 模式一致性**:子 Agent 的工具列表按继承的 AgentMode 过滤,非 Document 模式下文档 Handler 不对子 Agent 可见

---

## 二、任务依赖图

```
T4.01 (TaskConfig) ── T4.02 (SubAgentExecutor) ── T4.03 (TaskTool)
                                │
                                ├── T4.04 (子 Agent 事件隔离)
                                │
                                └── T4.05 (并行子 Agent)

T4.06 (WebFetch 依赖) ── T4.07 (URL 验证) ── T4.08 (HTML 转 Markdown) ── T4.09 (WebFetchTool)
                                                                            │
                                                                            └── T4.10 (WebFetch 权限)

T4.11 (WebSearch 配置) ── T4.12 (搜索引擎适配) ── T4.13 (WebSearchTool)
                                                        │
                                                        └── T4.14 (WebSearch 权限)

T4.15 (CSP 调整) ── T4.16 (前端子 Agent 展示) ── T4.17 (集成测试) ── T4.18 (文档更新)
```

---

## 三、任务清单

### T4.01:定义子 Agent 配置与类型

**文件**:
- 创建:`src-tauri/src/models/sub_agent.rs`
- 修改:`src-tauri/src/models/mod.rs`(添加 `pub mod sub_agent;`)

**实施内容**:
```rust
//! 子 Agent 模型定义
//! 子 Agent 是主 Agent 委托的独立执行单元,拥有独立上下文但继承父 Agent 配置

use serde::{Deserialize, Serialize};

/// 子 Agent 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentConfig {
    /// 子 Agent 唯一 ID
    pub agent_id: String,
    /// 父 Agent 的会话 ID
    pub parent_session_id: String,
    /// 子任务描述
    pub task_description: String,
    /// 子 Agent 的系统提示词(继承自父 Agent,可追加)
    pub system_prompt: String,
    /// 工作区根目录(继承自父 Agent)
    pub workspace_root: String,
    /// 最大迭代次数(默认 10)
    pub max_iterations: u32,
    /// 超时时间(秒,默认 300)
    pub timeout_seconds: u64,
    /// 是否允许子 Agent 调用 Task 工具(默认 false,防止递归)
    pub allow_nested_task: bool,
    /// 可用工具列表(空表示继承所有工具)
    pub allowed_tools: Vec<String>,
    /// Agent 模式(继承自父 Agent)
    /// v1.1: 取值为 "plan" / "build" / "document",子 Agent 必须与父 Agent 模式一致
    /// Document 模式下子 Agent 也能看到文档 Handler(docx/xlsx/pptx/pdf)
    pub agent_mode: String,
}

impl Default for SubAgentConfig {
    fn default() -> Self {
        Self {
            agent_id: String::new(),
            parent_session_id: String::new(),
            task_description: String::new(),
            system_prompt: String::new(),
            workspace_root: String::new(),
            max_iterations: 10,
            timeout_seconds: 300,
            allow_nested_task: false,
            allowed_tools: Vec::new(),
            agent_mode: "build".to_string(),
        }
    }
}

/// 子 Agent 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentResult {
    /// 子 Agent ID
    pub agent_id: String,
    /// 是否成功
    pub success: bool,
    /// 执行结果文本
    pub result: String,
    /// 错误信息(失败时)
    pub error: Option<String>,
    /// 执行迭代次数
    pub iterations: u32,
    /// 执行耗时(毫秒)
    pub duration_ms: u64,
    /// 使用的工具调用次数
    pub tool_calls: u32,
}

/// 子 Agent 状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SubAgentStatus {
    /// 待执行
    Pending,
    /// 执行中
    Running,
    /// 已完成
    Completed,
    /// 已失败
    Failed,
    /// 已取消
    Cancelled,
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功

---

### T4.02:实现 SubAgentExecutor

**文件**:
- 创建:`src-tauri/src/services/agent/sub_executor.rs`
- 修改:`src-tauri/src/services/agent/mod.rs`(添加 `pub mod sub_executor;`)

**实施内容**:
```rust
//! 子 Agent 执行器
//! 独立执行子任务,继承父 Agent 的上下文配置
//! 与主 AgentExecutor 共享 LLM Router、Tool Registry 等基础设施

use crate::models::sub_agent::{SubAgentConfig, SubAgentResult, SubAgentStatus};
use crate::models::llm::{ChatMessage, MessageRole};
use crate::services::agent::context::AgentContext;
use crate::services::agent::executor::AgentExecutor;
use crate::services::llm::router::LlmRouter;
use crate::services::tool::registry::ToolRegistry;
use crate::services::permission::registry::PermissionRegistry;
use crate::services::permission::session_whitelist::SessionWhitelist;
use std::sync::Arc;
use std::time::Instant;
use tokio::time::Duration;

/// 子 Agent 执行器
pub struct SubAgentExecutor {
    /// LLM Router(共享)
    llm_router: Arc<RwLock<Arc<LlmRouter>>>,
    /// Tool Registry(共享)
    tool_registry: Arc<ToolRegistry>,
    /// Permission Registry(共享)
    permission_registry: Arc<PermissionRegistry>,
    /// Session Whitelist(共享)
    session_whitelist: Arc<SessionWhitelist>,
    /// Agent Context(独立,每个子 Agent 一个)
    context: Arc<AgentContext>,
}

impl SubAgentExecutor {
    /// 创建子 Agent 执行器
    pub fn new(
        llm_router: Arc<RwLock<Arc<LlmRouter>>>,
        tool_registry: Arc<ToolRegistry>,
        permission_registry: Arc<PermissionRegistry>,
        session_whitelist: Arc<SessionWhitelist>,
    ) -> Self {
        Self {
            llm_router,
            tool_registry,
            permission_registry,
            session_whitelist,
            context: Arc::new(AgentContext::new()),
        }
    }

    /// 列出指定配置下可用的工具定义列表(用于测试和调试)
    pub fn list_tools_for_config(&self, config: &SubAgentConfig) -> Vec<ToolDefinition> {
        let document_handler_names = ["docx", "xlsx", "pptx", "pdf"];
        let mode = config.agent_mode.as_str();
        let is_document_mode = mode == "document";

        self.tool_registry.list_tool_definitions()
            .into_iter()
            .filter(|t| {
                if !is_document_mode && document_handler_names.contains(&t.name.as_str()) {
                    return false;
                }
                true
            })
            .filter(|t| {
                if config.allowed_tools.is_empty() {
                    true
                } else {
                    config.allowed_tools.contains(&t.name)
                }
            })
            .collect()
    }

    /// 执行子 Agent
    /// 1. 初始化子 Agent 上下文(继承父 Agent 配置)
    /// 2. 构建子任务消息
    /// 3. 循环执行 LLM 调用 + 工具调用
    /// 4. 返回最终结果
    pub async fn execute(&self, config: SubAgentConfig) -> SubAgentResult {
        let start_time = Instant::now();
        let agent_id = config.agent_id.clone();
        let max_iterations = config.max_iterations;
        let timeout = Duration::from_secs(config.timeout_seconds);

        log::info!(
            "子 Agent {} 开始执行任务: {}",
            agent_id,
            config.task_description
        );

        // 发射子 Agent 开始事件
        self.emit_sub_agent_event(&agent_id, &config.parent_session_id, SubAgentStatus::Running, None);

        // 使用 tokio::time::timeout 控制超时
        let result = tokio::time::timeout(
            timeout,
            self.execute_inner(config.clone()),
        )
        .await;

        match result {
            Ok(Ok(exec_result)) => {
                let duration_ms = start_time.elapsed().as_millis() as u64;
                let result = SubAgentResult {
                    agent_id: agent_id.clone(),
                    success: true,
                    result: exec_result.final_message,
                    error: None,
                    iterations: exec_result.iterations,
                    duration_ms,
                    tool_calls: exec_result.tool_calls,
                };

                self.emit_sub_agent_event(
                    &agent_id,
                    &config.parent_session_id,
                    SubAgentStatus::Completed,
                    Some(result.result.clone()),
                );

                log::info!(
                    "子 Agent {} 执行完成,迭代 {} 次,耗时 {}ms",
                    agent_id,
                    result.iterations,
                    duration_ms
                );

                result
            }
            Ok(Err(e)) => {
                let duration_ms = start_time.elapsed().as_millis() as u64;
                let error_msg = e.to_string();
                log::error!("子 Agent {} 执行失败: {}", agent_id, error_msg);

                let result = SubAgentResult {
                    agent_id: agent_id.clone(),
                    success: false,
                    result: String::new(),
                    error: Some(error_msg.clone()),
                    iterations: 0,
                    duration_ms,
                    tool_calls: 0,
                };

                self.emit_sub_agent_event(
                    &agent_id,
                    &config.parent_session_id,
                    SubAgentStatus::Failed,
                    Some(error_msg),
                );

                result
            }
            Err(_) => {
                let duration_ms = start_time.elapsed().as_millis() as u64;
                log::error!("子 Agent {} 执行超时({}秒)", agent_id, timeout.as_secs());

                let result = SubAgentResult {
                    agent_id: agent_id.clone(),
                    success: false,
                    result: String::new(),
                    error: Some(format!("执行超时({}秒)", timeout.as_secs())),
                    iterations: 0,
                    duration_ms,
                    tool_calls: 0,
                };

                self.emit_sub_agent_event(
                    &agent_id,
                    &config.parent_session_id,
                    SubAgentStatus::Failed,
                    Some("执行超时".to_string()),
                );

                result
            }
        }
    }

    /// 子 Agent 执行内部逻辑
    async fn execute_inner(&self, config: SubAgentConfig) -> Result<ExecResult, crate::errors::CommandError> {
        let mut messages: Vec<ChatMessage> = Vec::new();

        // 添加系统提示词
        messages.push(ChatMessage {
            role: MessageRole::System,
            content: config.system_prompt.clone(),
            ..Default::default()
        });

        // 添加子任务描述作为用户消息
        messages.push(ChatMessage {
            role: MessageRole::User,
            content: format!(
                "请执行以下子任务:\n\n{}\n\n完成后给出最终结果摘要。",
                config.task_description
            ),
            ..Default::default()
        });

        let mut iterations = 0u32;
        let mut tool_calls = 0u32;

        // 获取 LLM Provider
        let llm_router = self.llm_router.read().await.clone();
        let provider = llm_router.get_provider().await?;

        // 工具定义(过滤可用工具)
        // v1.1: 先按 AgentMode 过滤(非 Document 模式下文档 Handler 不可见),
        //       再按 allowed_tools 过滤(若指定),与主 executor 的 build_tool_definitions 行为一致
        let tools = {
            let mode = config.agent_mode.as_str();
            let is_document_mode = mode == "document";
            let document_handler_names = ["docx", "xlsx", "pptx", "pdf"];

            let mode_filtered = self.tool_registry.list_tool_definitions()
                .into_iter()
                .filter(|t| {
                    // 非 Document 模式下过滤掉文档 Handler
                    if !is_document_mode && document_handler_names.contains(&t.name.as_str()) {
                        return false;
                    }
                    true
                });

            if config.allowed_tools.is_empty() {
                mode_filtered.collect::<Vec<_>>()
            } else {
                mode_filtered
                    .filter(|t| config.allowed_tools.contains(&t.name))
                    .collect()
            }
        };

        // 迭代执行
        while iterations < config.max_iterations {
            iterations += 1;

            // 调用 LLM
            let response = provider.chat(&messages, Some(&tools), None).await?;

            // 检查是否有工具调用
            if let Some(tool_call) = &response.tool_calls.first() {
                tool_calls += 1;

                // 添加 assistant 消息
                messages.push(ChatMessage {
                    role: MessageRole::Assistant,
                    content: response.content.clone(),
                    tool_calls: response.tool_calls.clone(),
                    ..Default::default()
                });

                // 执行工具
                let tool_result = self.execute_tool(tool_call, &config).await?;

                // 添加工具结果消息
                messages.push(ChatMessage {
                    role: MessageRole::Tool,
                    content: tool_result,
                    tool_call_id: Some(tool_call.id.clone()),
                    ..Default::default()
                });

                // 继续下一轮迭代
                continue;
            }

            // 无工具调用,视为任务完成
            return Ok(ExecResult {
                final_message: response.content,
                iterations,
                tool_calls,
            });
        }

        // 达到最大迭代次数,返回最后的状态
        Ok(ExecResult {
            final_message: format!(
                "子 Agent 达到最大迭代次数({}),未完成任务。最后状态: {}",
                config.max_iterations,
                messages.last()
                    .map(|m| m.content.clone())
                    .unwrap_or_default()
            ),
            iterations,
            tool_calls,
        })
    }

    /// 执行工具调用(子 Agent 上下文)
    async fn execute_tool(
        &self,
        tool_call: &crate::models::llm::LlmToolCall,
        config: &SubAgentConfig,
    ) -> Result<String, crate::errors::CommandError> {
        let tool = self.tool_registry.get(&tool_call.function.name)
            .ok_or_else(|| crate::errors::CommandError::tool(
                crate::errors::TOOL_NOT_FOUND,
                format!("工具 {} 不存在", tool_call.function.name),
            ))?;

        // 权限检查(继承父 Agent 权限)
        // ... 权限检查逻辑(复用主 AgentExecutor 的 check_permission)

        // 执行工具
        let result = tool.execute(
            tool_call.function.arguments.clone(),
            &config.workspace_root,
        ).await?;

        // 序列化结果
        Ok(serde_json::to_string(&result)?)
    }

    /// 发射子 Agent 事件
    fn emit_sub_agent_event(
        &self,
        agent_id: &str,
        parent_session_id: &str,
        status: SubAgentStatus,
        message: Option<String>,
    ) {
        // 通过 Tauri 事件系统推送到前端
        // 事件名: agent:sub_agent_status
        // Payload: { agentId, parentSessionId, status, message }
        log::debug!(
            "子 Agent 事件: agent_id={}, status={:?}, message={:?}",
            agent_id,
            status,
            message
        );
    }
}

/// 执行结果内部结构
struct ExecResult {
    final_message: String,
    iterations: u32,
    tool_calls: u32,
}

// 引入需要的 trait
use tokio::sync::RwLock;
use crate::services::tool::trait_def::Tool;
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 单元测试:创建 SubAgentConfig,执行简单子任务

---

### T4.03:实现 Task 工具

**文件**:
- 创建:`src-tauri/src/services/tool/builtin/task.rs`
- 修改:`src-tauri/src/services/tool/builtin.rs`(注册 TaskTool)

**实施内容**:
```rust
//! Task 工具:委托子任务给子 Agent
//! 主 Agent 通过此工具将复杂子任务委托给独立的子 Agent 执行
//! 子 Agent 继承父 Agent 的上下文,独立完成后返回结果

use async_trait::async_trait;
use crate::services::tool::trait_def::Tool;
use crate::services::agent::sub_executor::SubAgentExecutor;
use crate::models::sub_agent::SubAgentConfig;
use crate::models::tool::ToolResult;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

pub struct TaskTool {
    /// 子 Agent 执行器
    sub_executor: Arc<SubAgentExecutor>,
    /// 父 Agent 的系统提示词(由 AgentContext 注入)
    parent_system_prompt: Arc<RwLock<String>>,
    /// 当前 Agent 模式
    /// v1.1: 取值为 "plan" / "build" / "document",由 executor 每轮迭代更新
    agent_mode: Arc<RwLock<String>>,
}

impl TaskTool {
    pub fn new(sub_executor: Arc<SubAgentExecutor>) -> Self {
        Self {
            sub_executor,
            parent_system_prompt: Arc::new(RwLock::new(String::new())),
            agent_mode: Arc::new(RwLock::new("build".to_string())),
        }
    }

    /// 更新父 Agent 系统提示词(每轮迭代时调用)
    pub async fn update_parent_prompt(&self, prompt: String) {
        let mut p = self.parent_system_prompt.write().await;
        *p = prompt;
    }

    /// 更新 Agent 模式
    pub async fn update_agent_mode(&self, mode: String) {
        let mut m = self.agent_mode.write().await;
        *m = mode;
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn tool_name(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "委托子任务给独立的子 Agent 执行。子 Agent 拥有独立上下文窗口,继承当前 Agent 的系统提示词和权限配置。适用于将复杂任务分解为子任务并行执行的场景。子 Agent 默认最多迭代 10 次,超时 300 秒。"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "子任务描述(详细说明子 Agent 需要完成的工作)"
                },
                "maxIterations": {
                    "type": "integer",
                    "default": 10,
                    "description": "子 Agent 最大迭代次数(1-50)"
                },
                "timeoutSeconds": {
                    "type": "integer",
                    "default": 300,
                    "description": "子 Agent 超时时间(秒,最大 600)"
                },
                "allowedTools": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "子 Agent 可用的工具列表(空表示继承所有工具,不包括 task 工具本身)"
                }
            },
            "required": ["description"]
        })
    }

    async fn execute(
        &self,
        params: Value,
        workspace_root: &str,
    ) -> Result<ToolResult, crate::errors::CommandError> {
        let description = params.get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                "缺少 description 参数",
            ))?;

        let max_iterations = params.get("maxIterations")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .min(50) as u32;

        let timeout_seconds = params.get("timeoutSeconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(300)
            .min(600);

        let allowed_tools: Vec<String> = params.get("allowedTools")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect())
            .unwrap_or_default();

        // 从 params 中获取 session_id(由 executor 注入)
        let session_id = params.get("_session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        // 构建子 Agent 配置
        let parent_prompt = self.parent_system_prompt.read().await.clone();
        let agent_mode = self.agent_mode.read().await.clone();

        let config = SubAgentConfig {
            agent_id: Uuid::new_v4().to_string(),
            parent_session_id: session_id.to_string(),
            task_description: description.to_string(),
            system_prompt: parent_prompt,
            workspace_root: workspace_root.to_string(),
            max_iterations,
            timeout_seconds,
            allow_nested_task: false, // 默认禁止嵌套
            allowed_tools,
            agent_mode,
        };

        log::info!(
            "主 Agent 委托子任务: agent_id={}, description={}",
            config.agent_id,
            description
        );

        // 执行子 Agent
        let result = self.sub_executor.execute(config).await;

        // 构建工具结果
        let result_json = json!({
            "agentId": result.agent_id,
            "success": result.success,
            "result": result.result,
            "error": result.error,
            "iterations": result.iterations,
            "durationMs": result.duration_ms,
            "toolCalls": result.tool_calls,
        });

        if result.success {
            Ok(ToolResult {
                success: true,
                result: result_json,
                error: None,
                metadata: Some(json!({
                    "subAgentId": result.agent_id,
                    "iterations": result.iterations,
                    "toolCalls": result.tool_calls,
                })),
            })
        } else {
            Ok(ToolResult {
                success: false,
                result: result_json,
                error: result.error.clone(),
                metadata: Some(json!({
                    "subAgentId": result.agent_id,
                    "iterations": result.iterations,
                    "toolCalls": result.tool_calls,
                })),
            })
        }
    }
}

use tokio::sync::RwLock;
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 集成测试:调用 task 工具执行简单子任务

---

### T4.04:子 Agent 事件隔离与传播

**文件**:
- 修改:`src-tauri/src/services/agent/sub_executor.rs`
- 修改:`src-tauri/src/events/types.rs`

**实施内容**:

**新增事件常量**:
```rust
// 在 events/types.rs 中添加
pub const AGENT_SUB_AGENT_STATUS: &str = "agent:sub_agent_status";
pub const AGENT_SUB_AGENT_TOOL_CALL: &str = "agent:sub_agent_tool_call";

/// 子 Agent 状态变更事件
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentStatusPayload {
    /// 父 Agent 会话 ID
    pub parent_session_id: String,
    /// 子 Agent ID
    pub agent_id: String,
    /// 状态: "running" | "completed" | "failed" | "cancelled"
    pub status: String,
    /// 附加消息(如错误信息或结果摘要)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// 当前迭代次数
    pub iteration: u32,
}

/// 子 Agent 工具调用事件
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentToolCallPayload {
    pub parent_session_id: String,
    pub agent_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub iteration: u32,
}
```

**在 SubAgentExecutor 中发射事件**:
```rust
// 在 execute_inner 方法中,每次工具调用时发射事件
async fn execute_inner(&self, config: SubAgentConfig) -> Result<ExecResult, crate::errors::CommandError> {
    // ... 现有逻辑

    while iterations < config.max_iterations {
        iterations += 1;

        // 调用 LLM
        let response = provider.chat(&messages, Some(&tools), None).await?;

        if let Some(tool_call) = &response.tool_calls.first() {
            tool_calls += 1;

            // 发射子 Agent 工具调用事件
            self.emit_event(
                crate::events::types::AGENT_SUB_AGENT_TOOL_CALL,
                &SubAgentToolCallPayload {
                    parent_session_id: config.parent_session_id.clone(),
                    agent_id: config.agent_id.clone(),
                    tool_name: tool_call.function.name.clone(),
                    arguments: tool_call.function.arguments.clone(),
                    iteration: iterations,
                },
            );

            // ... 执行工具
        }
    }
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 前端能收到子 Agent 事件

---

### T4.05:并行子 Agent 执行

**文件**:
- 修改:`src-tauri/src/services/tool/builtin/task.rs`

**实施内容**:

扩展 TaskTool 支持批量委托多个子任务并行执行:

```rust
// 扩展 parameters() 支持批量模式
fn parameters(&self) -> Value {
    json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["single", "batch"],
                "default": "single",
                "description": "操作类型:single=单个子任务;batch=批量并行子任务"
            },
            "description": {
                "type": "string",
                "description": "子任务描述(action=single 时必填)"
            },
            "tasks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "description": {"type": "string"},
                        "maxIterations": {"type": "integer", "default": 10},
                        "timeoutSeconds": {"type": "integer", "default": 300}
                    },
                    "required": ["description"]
                },
                "description": "子任务列表(action=batch 时必填)"
            },
            "maxIterations": {
                "type": "integer",
                "default": 10,
                "description": "子 Agent 最大迭代次数(1-50)"
            },
            "timeoutSeconds": {
                "type": "integer",
                "default": 300,
                "description": "子 Agent 超时时间(秒,最大 600)"
            }
        },
        "required": ["action"]
    })
}

// 在 execute 方法中处理批量模式
async fn execute(
    &self,
    params: Value,
    workspace_root: &str,
) -> Result<ToolResult, crate::errors::CommandError> {
    let action = params.get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("single");

    match action {
        "single" => {
            // 单个子任务(见 T4.03)
            self.execute_single(params, workspace_root).await
        }
        "batch" => {
            // 批量并行子任务
            self.execute_batch(params, workspace_root).await
        }
        _ => Err(crate::errors::CommandError::tool(
            crate::errors::TOOL_INVALID_PARAMS,
            format!("未知 action: {}", action),
        )),
    }
}

/// 批量并行执行子任务
async fn execute_batch(
    &self,
    params: Value,
    workspace_root: &str,
) -> Result<ToolResult, crate::errors::CommandError> {
    let tasks = params.get("tasks")
        .and_then(|v| v.as_array())
        .ok_or_else(|| crate::errors::CommandError::tool(
            crate::errors::TOOL_INVALID_PARAMS,
            "action=batch 时必须提供 tasks 参数",
        ))?;

    let session_id = params.get("_session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("default");

    let parent_prompt = self.parent_system_prompt.read().await.clone();
    let agent_mode = self.agent_mode.read().await.clone();

    // 构建所有子任务配置
    let configs: Vec<SubAgentConfig> = tasks.iter().map(|task| {
        let description = task.get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let max_iter = task.get("maxIterations")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .min(50) as u32;
        let timeout = task.get("timeoutSeconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(300)
            .min(600);

        SubAgentConfig {
            agent_id: Uuid::new_v4().to_string(),
            parent_session_id: session_id.to_string(),
            task_description: description.to_string(),
            system_prompt: parent_prompt.clone(),
            workspace_root: workspace_root.to_string(),
            max_iterations: max_iter,
            timeout_seconds: timeout,
            allow_nested_task: false,
            allowed_tools: Vec::new(),
            agent_mode: agent_mode.clone(),
        }
    }).collect();

    log::info!("批量执行 {} 个子任务", configs.len());

    // 并行执行所有子任务
    let mut futures = Vec::new();
    for config in configs {
        let executor = self.sub_executor.clone();
        futures.push(tokio::spawn(async move {
            executor.execute(config).await
        }));
    }

    // 等待所有子任务完成
    let mut results = Vec::new();
    for future in futures {
        let result = future.await
            .map_err(|e| crate::errors::CommandError::agent(
                crate::errors::AGENT_EXECUTION_ERROR,
                format!("子任务 join 失败: {}", e),
            ))?;
        results.push(result);
    }

    // 构建结果
    let success_count = results.iter().filter(|r| r.success).count();
    let total_count = results.len();

    Ok(ToolResult {
        success: success_count > 0,
        result: json!({
            "action": "batch",
            "total": total_count,
            "successCount": success_count,
            "failedCount": total_count - success_count,
            "results": results.iter().map(|r| {
                json!({
                    "agentId": r.agent_id,
                    "success": r.success,
                    "result": r.result,
                    "error": r.error,
                    "iterations": r.iterations,
                    "durationMs": r.duration_ms,
                    "toolCalls": r.tool_calls,
                })
            }).collect::<Vec<_>>(),
        }),
        error: if success_count == total_count {
            None
        } else {
            Some(format!("{} 个子任务失败", total_count - success_count))
        },
        metadata: Some(json!({
            "totalSubAgents": total_count,
            "successCount": success_count,
        })),
    })
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 集成测试:批量执行 3 个子任务,验证并行完成

---

### T4.06:新增 WebFetch 依赖

**文件**:
- 修改:`src-tauri/Cargo.toml`

**实施内容**:
```toml
[dependencies]
# 现有依赖...

# WebFetch:HTML 转 Markdown
html2md = "0.2"
# WebFetch:URL 解析
url = "2.5"
```

**验证**:
- `cargo build -p workmolde_lib` 成功

---

### T4.07:实现 URL 验证与安全检查

**文件**:
- 创建:`src-tauri/src/services/web/url_validator.rs`
- 创建:`src-tauri/src/services/web/mod.rs`

**实施内容**:

**mod.rs**:
```rust
//! Web 服务模块入口
pub mod url_validator;
pub mod fetcher;
pub mod searcher;

pub use url_validator::UrlValidator;
pub use fetcher::WebFetcher;
pub use searcher::WebSearcher;
```

**url_validator.rs**:
```rust
//! URL 验证器:确保 URL 安全合法
//! 防止访问内网地址、恶意 URL

use url::Url;

/// URL 验证结果
pub enum ValidationResult {
    /// 验证通过
    Valid,
    /// 验证失败,包含原因
    Invalid(String),
}

/// URL 验证器
pub struct UrlValidator {
    /// 允许的协议(默认仅 https)
    allowed_schemes: Vec<String>,
    /// 禁止的主机(内网地址等)
    blocked_hosts: Vec<String>,
    /// 最大 URL 长度
    max_url_length: usize,
}

impl Default for UrlValidator {
    fn default() -> Self {
        Self {
            allowed_schemes: vec!["https".to_string(), "http".to_string()],
            blocked_hosts: vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "0.0.0.0".to_string(),
                "::1".to_string(),
                "169.254.0.0".to_string(), // 链路本地
                "10.0.0.0".to_string(),
                "172.16.0.0".to_string(),
                "192.168.0.0".to_string(),
            ],
            max_url_length: 2048,
        }
    }
}

impl UrlValidator {
    pub fn new() -> Self {
        Self::default()
    }

    /// 验证 URL
    pub fn validate(&self, url_str: &str) -> ValidationResult {
        // 检查 URL 长度
        if url_str.len() > self.max_url_length {
            return ValidationResult::Invalid(format!(
                "URL 长度超过限制({} 字符)",
                self.max_url_length
            ));
        }

        // 解析 URL
        let url = match Url::parse(url_str) {
            Ok(u) => u,
            Err(e) => return ValidationResult::Invalid(format!("URL 解析失败: {}", e)),
        };

        // 检查协议
        if !self.allowed_schemes.contains(url.scheme()) {
            return ValidationResult::Invalid(format!(
                "不允许的协议: {}(仅允许 {:?})",
                url.scheme(),
                self.allowed_schemes
            ));
        }

        // 检查主机
        let host = url.host_str().unwrap_or("");
        for blocked in &self.blocked_hosts {
            if host == blocked || host.starts_with(blocked) {
                return ValidationResult::Invalid(format!(
                    "禁止访问的主机: {}(内网/本地地址)",
                    host
                ));
            }
        }

        // 检查端口(禁止常见内部服务端口)
        if let Some(port) = url.port() {
            if matches!(port, 22 | 23 | 25 | 110 | 143 | 3306 | 5432 | 6379 | 8080 | 9200) {
                return ValidationResult::Invalid(format!(
                    "禁止访问的端口: {}(内部服务端口)",
                    port
                ));
            }
        }

        ValidationResult::Valid
    }
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 单元测试:验证合法/非法 URL

---

### T4.08:实现 HTML 转 Markdown

**文件**:
- 创建:`src-tauri/src/services/web/fetcher.rs`

**实施内容**:
```rust
//! Web 内容获取器:获取 URL 内容并转换为 Markdown

use crate::services::web::url_validator::{UrlValidator, ValidationResult};
use reqwest::Client;
use std::time::Duration;

/// Web 内容获取结果
pub struct FetchResult {
    /// 原始 URL
    pub url: String,
    /// 最终 URL(可能经过重定向)
    pub final_url: String,
    /// 内容类型(text/html, application/json 等)
    pub content_type: String,
    /// 转换后的 Markdown 内容
    pub markdown: String,
    /// 内容长度(字符数)
    pub content_length: usize,
    /// 获取耗时(毫秒)
    pub fetch_duration_ms: u64,
}

/// Web 内容获取器
pub struct WebFetcher {
    /// HTTP 客户端
    client: Client,
    /// URL 验证器
    validator: UrlValidator,
    /// 请求超时时间
    timeout: Duration,
    /// 最大内容长度(字符数)
    max_content_length: usize,
}

impl Default for WebFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetcher {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .user_agent("Mozilla/5.0 (compatible; WorkMolde AI/1.0)")
            .build()
            .expect("创建 HTTP 客户端失败");

        Self {
            client,
            validator: UrlValidator::new(),
            timeout: Duration::from_secs(30),
            max_content_length: 100_000, // 10 万字符
        }
    }

    /// 获取 URL 内容并转换为 Markdown
    pub async fn fetch(&self, url: &str) -> Result<FetchResult, String> {
        // 验证 URL
        match self.validator.validate(url) {
            ValidationResult::Valid => {}
            ValidationResult::Invalid(reason) => return Err(reason),
        }

        let start = std::time::Instant::now();

        // 发送请求
        let response = self.client
            .get(url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| format!("请求失败: {}", e))?;

        let final_url = response.url().to_string();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/plain")
            .to_string();

        // 检查状态码
        if !response.status().is_success() {
            return Err(format!(
                "HTTP 状态码错误: {}",
                response.status()
            ));
        }

        // 获取响应体
        let body = response
            .text()
            .await
            .map_err(|e| format!("读取响应失败: {}", e))?;

        let fetch_duration_ms = start.elapsed().as_millis() as u64;

        // 根据内容类型转换
        let markdown = if content_type.contains("text/html") {
            // HTML 转 Markdown
            let mut md = html2md::parse_html(&body, false);
            // 截断过长的内容
            if md.len() > self.max_content_length {
                md.truncate(self.max_content_length);
                md.push_str("\n\n[内容已截断,原始长度 {} 字符]", body.len());
            }
            md
        } else if content_type.contains("application/json") {
            // JSON 内容,格式化输出
            let parsed: serde_json::Value = serde_json::from_str(&body)
                .unwrap_or(serde_json::Value::String(body.clone()));
            serde_json::to_string_pretty(&parsed).unwrap_or(body)
        } else if content_type.contains("text/plain") || content_type.contains("text/markdown") {
            // 纯文本或 Markdown,直接使用
            body
        } else {
            // 其他类型,提示不支持
            format!("[不支持的内容类型: {}]", content_type)
        };

        Ok(FetchResult {
            url: url.to_string(),
            final_url,
            content_type,
            markdown,
            content_length: markdown.len(),
            fetch_duration_ms,
        })
    }

    /// 设置最大内容长度
    pub fn with_max_content_length(mut self, max: usize) -> Self {
        self.max_content_length = max;
        self
    }
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 单元测试:获取示例网页,验证转换结果

---

### T4.09:实现 WebFetch 工具

> **命名规则说明**：`webfetch`/`websearch` 工具沿用 OpenCode 原名，不适用"复合词保留下划线"规则。OpenCode 生态中这些工具名已约定俗成。

**文件**:
- 创建:`src-tauri/src/services/tool/builtin/webfetch.rs`
- 修改:`src-tauri/src/services/tool/builtin.rs`(注册 WebFetchTool)

**实施内容**:
```rust
//! WebFetch 工具:获取 URL 内容并转换为 Markdown
//! Agent 通过此工具读取在线文档、API 参考等资源

use async_trait::async_trait;
use crate::services::tool::trait_def::Tool;
use crate::services::web::fetcher::WebFetcher;
use crate::models::tool::ToolResult;
use serde_json::{json, Value};
use std::sync::Mutex;

/// WebFetch 工具:获取 URL 内容并转换为 Markdown
///
/// 设计说明:`new()` 不接收参数,因为 WebFetch 工具的配置(超时、UA 等)使用默认值。
/// 与 WebSearchTool 的 `new(config)` 不同,WebFetch 不需要搜索引擎配置。
/// 这是合理的设计差异,因为两个工具的职责不同:
/// - WebFetch:仅获取指定 URL 内容,无需外部配置
/// - WebSearch:需要配置搜索引擎后端(MCP/Tavily/SerpAPI)和 API Key
pub struct WebFetchTool {
    /// Web 内容获取器(Mutex 保护,因为内部有可变状态)
    fetcher: Mutex<WebFetcher>,
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self {
            fetcher: Mutex::new(WebFetcher::new()),
        }
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn tool_name(&self) -> &str {
        "webfetch"
    }

    fn description(&self) -> &str {
        "获取指定 URL 的网页内容,自动转换为 Markdown 格式。支持 HTML、JSON、纯文本内容。自动截断过长内容(默认 10 万字符)。禁止访问内网地址和本地端口。"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "要获取的 URL(必须以 http:// 或 https:// 开头)"
                },
                "maxLength": {
                    "type": "integer",
                    "default": 100000,
                    "description": "最大返回内容长度(字符数)"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(
        &self,
        params: Value,
        workspace_root: &str,
    ) -> Result<ToolResult, crate::errors::CommandError> {
        let url = params.get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                "缺少 url 参数",
            ))?;

        let max_length = params.get("maxLength")
            .and_then(|v| v.as_u64())
            .unwrap_or(100_000) as usize;

        // 获取内容
        let mut fetcher = self.fetcher.lock().unwrap();
        let fetcher = fetcher.with_max_content_length(max_length);

        match fetcher.fetch(url).await {
            Ok(result) => {
                Ok(ToolResult {
                    success: true,
                    result: json!({
                        "url": result.url,
                        "finalUrl": result.final_url,
                        "contentType": result.content_type,
                        "content": result.markdown,
                        "contentLength": result.content_length,
                        "fetchDurationMs": result.fetch_duration_ms,
                    }),
                    error: None,
                    metadata: Some(json!({
                        "url": result.url,
                        "contentType": result.content_type,
                        "contentLength": result.content_length,
                    })),
                })
            }
            Err(e) => {
                Ok(ToolResult {
                    success: false,
                    result: json!({
                        "url": url,
                        "error": e,
                    }),
                    error: Some(e),
                    metadata: None,
                })
            }
        }
    }
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 集成测试:获取示例 URL,验证返回 Markdown

---

### T4.10:WebFetch 权限集成

**文件**:
- 修改:`src-tauri/src/services/agent/executor.rs`

**实施内容**:

在 AgentExecutor 中,对 WebFetch 工具调用进行权限检查:

```rust
// 在 execute_tool 方法中,针对 WebFetchTool 添加权限检查
async fn execute_tool(&self, tool_name: &str, params: Value, ...) -> ... {
    // 通用权限检查
    let permission_type = match tool_name {
        "webfetch" => PermissionType::WebFetch,
        "websearch" => PermissionType::WebSearch,
        "task" => PermissionType::Task,
        _ => // ... 其他工具的权限类型
    };

    let permission_result = self.check_permission(
        permission_type,
        tool_name,
        &params,
        session_id,
    ).await?;

    // ... 执行工具
}
```

**权限规则示例**:
```json
{
  "webfetch": {
    "*": "allow",
    "*.internal.*": "deny",
    "*.local": "deny"
  }
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 权限检查:用户配置 `webfetch: "ask"` 时,首次访问 URL 弹窗确认

---

### T4.11:定义 WebSearch 配置

**文件**:
- 修改:`src-tauri/src/config/app_settings.rs`

**实施内容**:
```rust
/// WebSearch 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchConfig {
    pub enabled: bool,
    /// 搜索后端类型: "mcp" (MCP 协议,参照 OpenCode Exa AI) | "tavily" (Tavily API) | "serpapi" (SerpAPI)
    pub backend: String,
    /// MCP 服务端点(当 backend="mcp" 时使用,默认 Exa AI 托管服务)
    pub mcp_endpoint: String,
    /// API Key(当 backend="tavily" 或 "serpapi" 时使用)
    #[serde(skip_serializing)]
    pub api_key: String,
    pub max_results: usize,
    pub timeout_seconds: u64,
}

impl Default for WebSearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: "mcp".to_string(),
            mcp_endpoint: "https://mcp.exa.ai".to_string(), // Exa AI 托管 MCP 服务
            api_key: String::new(),
            max_results: 5,
            timeout_seconds: 30,
        }
    }
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功

---

### T4.12:实现搜索引擎适配

**文件**:
- 创建:`src-tauri/src/services/web/searcher.rs`

**实施内容**:
```rust
//! Web 搜索器:支持多种搜索后端
//! 默认使用 MCP 协议(Exa AI 托管服务),也支持 Tavily/SerpAPI(需 API Key)

use crate::config::app_settings::WebSearchConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 搜索结果项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultItem {
    /// 标题
    pub title: String,
    /// URL
    pub url: String,
    /// 摘要
    pub snippet: String,
    /// 显示 URL(部分搜索引擎提供)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_url: Option<String>,
}

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    /// 搜索查询
    pub query: String,
    /// 搜索引擎
    pub engine: String,
    /// 结果列表
    pub results: Vec<SearchResultItem>,
    /// 总结果数(估计)
    pub total_results: Option<u64>,
    /// 搜索耗时(毫秒)
    pub search_duration_ms: u64,
}

/// Web 搜索器
pub struct WebSearcher {
    /// 配置
    config: WebSearchConfig,
    /// HTTP 客户端
    client: Client,
}

impl WebSearcher {
    pub fn new(config: WebSearchConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .user_agent("Mozilla/5.0 (compatible; WorkMolde AI/1.0)")
            .build()
            .expect("创建 HTTP 客户端失败");

        Self { config, client }
    }

    /// 执行搜索
    pub async fn search(&self, query: &str) -> Result<SearchResponse, String> {
        if !self.config.enabled {
            return Err("网络搜索已被禁用".to_string());
        }

        let start = std::time::Instant::now();

        let results = match self.config.backend.as_str() {
            "mcp" => self.search_via_mcp(query).await,
            "tavily" => self.search_tavily(query).await,
            "serpapi" => self.search_serpapi(query).await,
            _ => self.search_via_mcp(query).await, // 默认 MCP
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(SearchResponse {
            query: query.to_string(),
            engine: self.config.search_engine.clone(),
            results,
            total_results: None,
            search_duration_ms: duration_ms,
        })
    }

    /// 通过 MCP 协议搜索(参照 OpenCode Exa AI 实现)
    async fn search_via_mcp(&self, query: &str) -> Result<Vec<SearchResultItem>, CommandError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.config.timeout_seconds))
            .build()?;

        // MCP 协议: JSON-RPC 2.0 格式
        let request = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "web_search",
                "arguments": {
                    "query": query,
                    "max_results": self.config.max_results
                }
            },
            "id": 1
        });

        let response = client
            .post(&self.config.mcp_endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|e| CommandError::network_error(format!("MCP 搜索请求失败: {}", e)))?;

        let result: serde_json::Value = response.json().await
            .map_err(|e| CommandError::parse_error(format!("解析 MCP 响应失败: {}", e)))?;

        // 解析 MCP 响应格式
        let results = result["result"]["content"]
            .as_array()
            .ok_or_else(|| CommandError::parse_error("MCP 响应格式错误"))?
            .iter()
            .map(|item| SearchResultItem {
                title: item["title"].as_str().unwrap_or("").to_string(),
                url: item["url"].as_str().unwrap_or("").to_string(),
                snippet: item["snippet"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        Ok(results)
    }

    /// Tavily 搜索(需要 API Key)
    async fn search_tavily(&self, query: &str) -> Result<Vec<SearchResultItem>, String> {
        if self.config.api_key.is_empty() {
            return Err("Tavily 搜索需要 API Key".to_string());
        }

        let url = "https://api.tavily.com/search";
        let request_body = serde_json::json!({
            "api_key": self.config.api_key,
            "query": query,
            "max_results": self.config.max_results,
            "include_answer": false,
        });

        let response = self.client
            .post(url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Tavily 请求失败: {}", e))?;

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("解析 Tavily 响应失败: {}", e))?;

        let results = body.get("results")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter().take(self.config.max_results).map(|item| {
                    SearchResultItem {
                        title: item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        url: item.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        snippet: item.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        display_url: None,
                    }
                }).collect()
            })
            .unwrap_or_default();

        Ok(results)
    }

    /// SerpAPI 搜索(需要 API Key)
    async fn search_serpapi(&self, query: &str) -> Result<Vec<SearchResultItem>, String> {
        if self.config.api_key.is_empty() {
            return Err("SerpAPI 搜索需要 API Key".to_string());
        }

        let url = format!(
            "https://serpapi.com/search?q={}&api_key={}&num={}",
            urlencoding::encode(query),
            self.config.api_key,
            self.config.max_results
        );

        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("SerpAPI 请求失败: {}", e))?;

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("解析 SerpAPI 响应失败: {}", e))?;

        let results = body.get("organic_results")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter().take(self.config.max_results).map(|item| {
                    SearchResultItem {
                        title: item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        url: item.get("link").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        snippet: item.get("snippet").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        display_url: None,
                    }
                }).collect()
            })
            .unwrap_or_default();

        Ok(results)
    }
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 单元测试:模拟搜索请求(使用 mock)

---

### T4.13:实现 WebSearch 工具

**文件**:
- 创建:`src-tauri/src/services/tool/builtin/websearch.rs`
- 修改:`src-tauri/src/services/tool/builtin.rs`(注册 WebSearchTool)

**实施内容**:
```rust
//! WebSearch 工具:执行网络搜索
//! Agent 通过此工具搜索网络信息,返回结果列表

use async_trait::async_trait;
use crate::services::tool::trait_def::Tool;
use crate::services::web::searcher::{WebSearcher, SearchResponse};
use crate::config::app_settings::WebSearchConfig;
use crate::models::tool::ToolResult;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct WebSearchTool {
    /// 搜索器
    searcher: Arc<WebSearcher>,
}

impl WebSearchTool {
    pub fn new(config: WebSearchConfig) -> Self {
        Self {
            searcher: Arc::new(WebSearcher::new(config)),
        }
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn tool_name(&self) -> &str {
        "websearch"
    }

    fn description(&self) -> &str {
        "执行网络搜索,返回相关结果列表(标题、URL、摘要)。支持 MCP 协议(默认,Exa AI 托管服务)、Tavily、SerpAPI。用于主动发现信息,而非被动等待用户提供。"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "搜索查询词"
                },
                "maxResults": {
                    "type": "integer",
                    "default": 5,
                    "description": "最大返回结果数(1-20)"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(
        &self,
        params: Value,
        workspace_root: &str,
    ) -> Result<ToolResult, crate::errors::CommandError> {
        let query = params.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                "缺少 query 参数",
            ))?;

        // 执行搜索
        match self.searcher.search(query).await {
            Ok(response) => {
                Ok(ToolResult {
                    success: true,
                    result: json!({
                        "query": response.query,
                        "engine": response.engine,
                        "results": response.results,
                        "totalResults": response.total_results,
                        "searchDurationMs": response.search_duration_ms,
                    }),
                    error: None,
                    metadata: Some(json!({
                        "query": response.query,
                        "resultCount": response.results.len(),
                        "engine": response.engine,
                    })),
                })
            }
            Err(e) => {
                Ok(ToolResult {
                    success: false,
                    result: json!({
                        "query": query,
                        "error": e,
                    }),
                    error: Some(e),
                    metadata: None,
                })
            }
        }
    }
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 集成测试:执行搜索,验证返回结果

---

### T4.14:WebSearch 权限集成

**文件**:
- 修改:`src-tauri/src/services/agent/executor.rs`

**实施内容**:

在 AgentExecutor 中,对 WebSearch 工具调用进行权限检查:

```rust
// 在 execute_tool 方法中,针对 WebSearchTool 添加权限检查
// 权限类型: PermissionType::WebSearch
// 默认规则: allow(允许搜索)
// 用户可配置: ask(每次搜索需确认)

// 详见 T4.10 的权限检查逻辑
```

**权限规则示例**:
```json
{
  "websearch": "allow"
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 权限检查:用户配置 `websearch: "ask"` 时,首次搜索弹窗确认

---

### T4.15:调整 Tauri CSP 配置

**文件**:
- 修改:`src-tauri/tauri.conf.json`

**实施内容**:

> **注意**:当前 CSP 实际已允许 `https:`(用于 LLM API 调用),文档中"仅允许 localhost"的描述不准确。本任务主要是为 WebFetch/WebSearch 添加 `http://*` 支持(如需访问非 HTTPS URL),并保留现有的 `https:` 支持。

当前 CSP 为 `connect-src 'self' https: http://localhost:* http://127.0.0.1:*`,需要调整为允许 WebFetch/WebSearch 访问外部 URL:

```json
{
  "app": {
    "security": {
      "csp": "default-src 'self'; img-src 'self' data: https:; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self' http://localhost:* http://127.0.0.1:* https://* http://*; font-src 'self' data:"
    }
  }
}
```

**说明**:
- `connect-src` 添加 `https://*` 和 `http://*`,允许访问外部 URL
- 保留 `http://localhost:*` 和 `http://127.0.0.1:*` 用于 LLM API 调用
- 其他指令保持不变

**注意**:CSP 仅影响前端 fetch/XHR 请求。Rust 后端的 reqwest 不受 CSP 限制,因此 WebFetch/WebSearch 在 Rust 端执行时无需调整 CSP。但如果前端需要直接展示外部图片等内容,需要调整 `img-src`。

**验证**:
- `cargo build -p workmolde_lib` 成功
- Tauri 应用启动正常,无 CSP 错误

---

### T4.16:前端子 Agent 展示

**文件**:
- 修改:`src/components/workflow/WorkflowTimeline.tsx`
- 修改:`src/components/workflow/WorkflowNode.tsx`
- 修改:`src/services/event.ts`(添加子 Agent 事件监听)

**实施内容**:

**在 event.ts 中添加子 Agent 事件监听**:
```typescript
// 添加子 Agent 事件常量
export const AGENT_SUB_AGENT_STATUS = 'agent:sub_agent_status';
export const AGENT_SUB_AGENT_TOOL_CALL = 'agent:sub_agent_tool_call';

// 添加事件监听函数
export function onSubAgentStatus(
  callback: (payload: SubAgentStatusPayload) => void
): UnlistenFn {
  return listen(AGENT_SUB_AGENT_STATUS, (event) => {
    callback(event.payload as SubAgentStatusPayload);
  });
}

export function onSubAgentToolCall(
  callback: (payload: SubAgentToolCallPayload) => void
): UnlistenFn {
  return listen(AGENT_SUB_AGENT_TOOL_CALL, (event) => {
    callback(event.payload as SubAgentToolCallPayload);
  });
}

// 类型定义
interface SubAgentStatusPayload {
  parentSessionId: string;
  agentId: string;
  status: 'running' | 'completed' | 'failed' | 'cancelled';
  message?: string;
  iteration: number;
}

interface SubAgentToolCallPayload {
  parentSessionId: string;
  agentId: string;
  toolName: string;
  arguments: any;
  iteration: number;
}
```

**在 WorkflowNode 中添加子 Agent 节点类型**:
```tsx
// 在 WorkflowNode 组件中添加 sub_agent 类型
case 'sub_agent':
  return (
    <div className="workflow-node sub-agent-node">
      <div className="sub-agent-header">
        <Icon name="robot" />
        <span>子 Agent</span>
        <span className={`sub-agent-status ${data.status}`}>
          {data.status === 'running' && '执行中...'}
          {data.status === 'completed' && '已完成'}
          {data.status === 'failed' && '失败'}
        </span>
      </div>
      <div className="sub-agent-task">{data.task}</div>
      {data.iterations > 0 && (
        <div className="sub-agent-meta">
          迭代: {data.iterations} | 工具调用: {data.toolCalls}
        </div>
      )}
      {data.result && (
        <div className="sub-agent-result">
          <pre>{data.result}</pre>
        </div>
      )}
    </div>
  );

// 在 WorkflowTimeline 中集成子 Agent 事件
// 收到 AGENT_SUB_AGENT_STATUS 事件时,更新对应的子 Agent 节点状态
// 收到 AGENT_SUB_AGENT_TOOL_CALL 事件时,在子 Agent 节点下添加工具调用子节点
```

**验证**:
- `npx tsc -b` 成功
- 前端正确展示子 Agent 节点和状态

---

### T4.17:编写集成测试

**文件**:
- 创建:`src-tauri/tests/sub_agent_tools_integration_test.rs`

**实施内容**:
```rust
//! 阶段 4 集成测试:子 Agent、WebFetch、WebSearch

use workmolde_lib::services::agent::sub_executor::SubAgentExecutor;
use workmolde_lib::services::tool::builtin::{WebFetchTool, WebSearchTool};
use workmolde_lib::services::tool::trait_def::Tool;
use workmolde_lib::services::web::{UrlValidator, ValidationResult};
use workmolde_lib::models::sub_agent::{SubAgentConfig, SubAgentStatus};
use serde_json::json;

/// 测试:URL 验证器拒绝内网地址
#[tokio::test]
async fn test_url_validator_rejects_internal_addresses() {
    let validator = UrlValidator::new();
    
    // 内网地址应被拒绝
    match validator.validate("http://127.0.0.1/admin") {
        ValidationResult::Invalid(_) => {}
        ValidationResult::Valid => panic!("应拒绝 127.0.0.1"),
    }
    
    match validator.validate("http://192.168.1.1/api") {
        ValidationResult::Invalid(_) => {}
        ValidationResult::Valid => panic!("应拒绝 192.168.1.1"),
    }
    
    match validator.validate("http://localhost:3000") {
        ValidationResult::Invalid(_) => {}
        ValidationResult::Valid => panic!("应拒绝 localhost"),
    }
}

/// 测试:URL 验证器接受合法外网地址
#[tokio::test]
async fn test_url_validator_accepts_external_urls() {
    let validator = UrlValidator::new();
    
    match validator.validate("https://example.com/page") {
        ValidationResult::Valid => {}
        ValidationResult::Invalid(reason) => panic!("应接受 example.com: {}", reason),
    }
    
    match validator.validate("https://docs.rs/reqwest") {
        ValidationResult::Valid => {}
        ValidationResult::Invalid(reason) => panic!("应接受 docs.rs: {}", reason),
    }
}

/// 测试:URL 验证器拒绝非 http/https 协议
#[tokio::test]
async fn test_url_validator_rejects_non_http_protocols() {
    let validator = UrlValidator::new();
    
    match validator.validate("file:///etc/passwd") {
        ValidationResult::Invalid(_) => {}
        ValidationResult::Valid => panic!("应拒绝 file:// 协议"),
    }
    
    match validator.validate("ftp://example.com/file") {
        ValidationResult::Invalid(_) => {}
        ValidationResult::Valid => panic!("应拒绝 ftp:// 协议"),
    }
}

/// 测试:WebFetch 工具参数定义
#[tokio::test]
async fn test_webfetch_tool_parameters() {
    let tool = WebFetchTool::new();
    let params = tool.parameters();
    
    assert_eq!(params["type"], "object");
    assert!(params["properties"]["url"]["type"].is_string());
    assert!(params["required"].as_array().unwrap().contains(&json!("url")));
}

/// 测试:WebSearch 工具参数定义
#[tokio::test]
async fn test_websearch_tool_parameters() {
    let config = workmolde_lib::config::app_settings::WebSearchConfig::default();
    let tool = WebSearchTool::new(config);
    let params = tool.parameters();
    
    assert_eq!(params["type"], "object");
    assert!(params["properties"]["query"]["type"].is_string());
    assert!(params["required"].as_array().unwrap().contains(&json!("query")));
}

/// 测试:子 Agent 配置默认值
#[tokio::test]
async fn test_sub_agent_config_defaults() {
    let config = SubAgentConfig::default();
    
    assert_eq!(config.max_iterations, 10);
    assert_eq!(config.timeout_seconds, 300);
    assert!(!config.allow_nested_task);
    assert!(config.allowed_tools.is_empty());
    // v1.1: 默认模式为 "build"(也支持 "plan" / "document")
    assert_eq!(config.agent_mode, "build");
}

/// v1.1 新增:测试子 Agent 工具列表按模式过滤
/// 验证非 Document 模式下,文档 Handler 不对子 Agent 可见
#[tokio::test]
async fn test_sub_agent_tool_filtering_by_mode() {
    // 构建包含文档 Handler 的工具注册表(模拟)
    let registry = build_test_tool_registry_with_handlers();
    let executor = SubAgentExecutor::new(
        /* ... */
    );

    // Build 模式:文档 Handler 不应在工具列表中
    let build_config = SubAgentConfig {
        agent_mode: "build".to_string(),
        allowed_tools: vec![],
        ..Default::default()
    };
    let build_tools = executor.list_tools_for_config(&build_config);
    let build_names: Vec<&str> = build_tools.iter().map(|t| t.name.as_str()).collect();
    assert!(!build_names.contains(&"docx"));
    assert!(!build_names.contains(&"xlsx"));
    assert!(!build_names.contains(&"pptx"));
    assert!(!build_names.contains(&"pdf"));

    // Document 模式:文档 Handler 应在工具列表中
    let document_config = SubAgentConfig {
        agent_mode: "document".to_string(),
        allowed_tools: vec![],
        ..Default::default()
    };
    let document_tools = executor.list_tools_for_config(&document_config);
    let document_names: Vec<&str> = document_tools.iter().map(|t| t.name.as_str()).collect();
    assert!(document_names.contains(&"docx"));
    assert!(document_names.contains(&"xlsx"));
    assert!(document_names.contains(&"pptx"));
    assert!(document_names.contains(&"pdf"));
}
```

**验证**:
- `cargo test` 全部通过

---

### T4.18:更新文档与工具注册

**文件**:
- 修改:`src-tauri/src/services/tool/builtin.rs`(注册新工具)
- 修改:`src-tauri/src/lib.rs`(初始化 SubAgentExecutor)
- 修改:`CLAUDE.md`(更新工具列表)

**实施内容**:

**在 register_builtin_tools 中注册新工具**:

> **接口对齐说明**:本方法签名必须与 overview 4.4.1 节统一接口定义一致。
> 本阶段增加 `sub_executor` 和 `web_search_config` 参数(从 Option 改为实际值)。

```rust
// 在 builtin.rs 的 register_builtin_tools 函数中添加
// Phase 4 阶段签名(渐进式扩展:在 Phase 3 基础上增加 sub_executor 和 web_search_config 参数)
pub fn register_builtin_tools(
    registry: &mut ToolRegistry,
    git_bash_path: String,
    agent_mode_manager: Arc<AgentModeManager>,       // Phase 2 引入
    db: Arc<Database>,                                // Phase 3 引入
    sub_executor: Arc<SubAgentExecutor>,              // [本阶段新增]
    web_search_config: WebSearchConfig,                // [本阶段新增]
) -> SharedScratchpadStates {
    // ... 现有工具注册

    // 阶段 4 新增工具
    registry.register(Box::new(TaskTool::new(sub_executor)));
    registry.register(Box::new(WebFetchTool::new()));
    registry.register(Box::new(WebSearchTool::new(web_search_config)));

    // ... 其他工具
}
```

**在 lib.rs 中初始化 SubAgentExecutor**:
```rust
// 在 setup 函数中初始化 SubAgentExecutor
let sub_executor = Arc::new(SubAgentExecutor::new(
    llm_router.clone(),
    tool_registry.clone(),
    permission_registry.clone(),
    session_whitelist.clone(),
));

// 注册工具时传入 sub_executor(对齐 overview 4.4.1 统一签名,Phase 4 阶段)
let scratchpad_states = register_builtin_tools(
    &mut tool_registry,
    git_bash_path,
    agent_mode_manager.clone(),
    db.clone(),
    sub_executor,                 // [本阶段新增]
    web_search_config,            // [本阶段新增]
);
```

**更新 CLAUDE.md**:
```markdown
## 内置工具

- `task`: 委托子任务给子 Agent(支持批量并行)
- `webfetch`: 获取 URL 内容并转换为 Markdown
- `websearch`: 执行网络搜索(MCP/Tavily/SerpAPI)
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 应用启动后,工具列表包含 task/webfetch/websearch

---

### T4.19: 新增 question 工具(向用户提问)

**参照 OpenCode**: OpenCode 内置 question 工具,允许 Agent 在执行中向用户提问。

**工具定义**:
- 工具名: `question`
- 功能: 向用户提问,获取澄清信息或决策输入
- 权限: 默认 allow(无需确认)

**参数 Schema**:
```json
{
  "type": "object",
  "properties": {
    "questions": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "header": {"type": "string", "description": "短标签(最多12字符)"},
          "question": {"type": "string", "description": "完整问题文本"},
          "options": {
            "type": "array",
            "items": {"type": "object", "properties": {"label": {"type": "string"}, "description": {"type": "string"}}},
            "description": "选项列表(2-4个)"
          },
          "multiSelect": {"type": "boolean", "description": "是否允许多选"}
        },
        "required": ["header", "question", "options"]
      },
      "description": "问题列表(1-4个问题)"
    }
  },
  "required": ["questions"]
}
```

**实现要点**:
1. Agent 调用 question 工具时,通过 Tauri 事件发射 `agent:question` 事件到前端
2. 前端显示问题对话框,用户可在多个问题间导航
3. 用户提交后,通过 oneshot channel 返回答案
4. 超时: 5 分钟自动返回空结果

**与 confirm 的区别**:
- confirm: 二元确认(是/否),用于高风险操作授权
- question: 开放式提问,用于获取澄清信息或决策输入

---

## 四、数据库迁移

本阶段无新增数据库表。子 Agent 的执行状态通过内存和事件系统管理,不持久化。

---

## 五、配置变更

### 5.1 新增配置项

| 配置项 | 位置 | 默认值 | 说明 |
|--------|------|--------|------|
| `webSearch.enabled` | `WebSearchConfig` | `true` | 是否启用网络搜索 |
| `webSearch.backend` | `WebSearchConfig` | `"mcp"` | 搜索后端类型(mcp/tavily/serpapi) |
| `webSearch.mcpEndpoint` | `WebSearchConfig` | `"https://mcp.exa.ai"` | MCP 服务端点 |
| `webSearch.apiKey` | `WebSearchConfig` | `""` | API Key(tavily/serpapi 使用) |
| `webSearch.maxResults` | `WebSearchConfig` | `5` | 每次搜索结果数 |
| `webSearch.timeoutSeconds` | `WebSearchConfig` | `30` | 搜索超时时间 |

### 5.2 Tauri CSP 变更

| 配置项 | 原值 | 新值 | 说明 |
|--------|------|------|------|
| `connect-src` | `'self' http://localhost:* http://127.0.0.1:*` | `'self' http://localhost:* http://127.0.0.1:* https://* http://*` | 允许访问外部 URL |

---

## 六、事件清单

### 6.1 新增事件

| 事件名 | Payload | 说明 |
|--------|---------|------|
| `agent:sub_agent_status` | `SubAgentStatusPayload` | 子 Agent 状态变更 |
| `agent:sub_agent_tool_call` | `SubAgentToolCallPayload` | 子 Agent 工具调用 |

### 6.2 事件时序

```
[主 Agent 执行 task 工具]
   │
   ├── emit(agent:tool_call, tool_name="task")
   │
   ├── 创建子 Agent
   │
   ├── emit(agent:sub_agent_status, status="running")
   │
   ├── 子 Agent 迭代执行
   │   ├── emit(agent:sub_agent_tool_call, tool_name="...")
   │   └── ... (多次工具调用)
   │
   ├── emit(agent:sub_agent_status, status="completed")
   │
   └── emit(agent:tool_result, tool_name="task")
```

---

## 七、参考资源

### 7.1 OpenCode 相关源码

- **Task 工具**: `packages/opencode/src/tool/task.ts`
- **WebFetch 工具**: `packages/opencode/src/tool/webfetch.ts`
- **WebSearch 工具**: `packages/opencode/src/tool/websearch.ts`

### 7.2 技术文档

- **html2md crate**:https://docs.rs/html2md
- **reqwest crate**:https://docs.rs/reqwest
- **Tauri CSP 配置**:https://tauri.app/v1/guides/security/csp/
- **Exa AI MCP 服务**:https://mcp.exa.ai

### 7.3 相关文档

- [阶段 1:核心架构与工具链](./2026-07-08-coding-agent-refactor-phase1-core.md)
- [阶段 2:权限系统与 Agent 模式](./2026-07-08-coding-agent-refactor-phase2-permission.md)
- [阶段 3:Skill 系统与上下文管理](./2026-07-08-coding-agent-refactor-phase3-skill-context.md)
- [总体改造计划](./2026-07-08-coding-agent-refactor-overview.md)

---

## 八、任务完成状态追踪

| 任务 ID | 任务名称 | 状态 | 完成时间 | 备注 |
|---------|---------|------|---------|------|
| T4.01 | 定义子 Agent 配置与类型 | 待实施 | - | |
| T4.02 | 实现 SubAgentExecutor | 待实施 | - | |
| T4.03 | 实现 Task 工具 | 待实施 | - | |
| T4.04 | 子 Agent 事件隔离与传播 | 待实施 | - | |
| T4.05 | 并行子 Agent 执行 | 待实施 | - | |
| T4.06 | 新增 WebFetch 依赖 | 待实施 | - | |
| T4.07 | 实现 URL 验证与安全检查 | 待实施 | - | |
| T4.08 | 实现 HTML 转 Markdown | 待实施 | - | |
| T4.09 | 实现 WebFetch 工具 | 待实施 | - | |
| T4.10 | WebFetch 权限集成 | 待实施 | - | |
| T4.11 | 定义 WebSearch 配置 | 待实施 | - | |
| T4.12 | 实现搜索引擎适配 | 待实施 | - | |
| T4.13 | 实现 WebSearch 工具 | 待实施 | - | |
| T4.14 | WebSearch 权限集成 | 待实施 | - | |
| T4.15 | 调整 Tauri CSP 配置 | 待实施 | - | |
| T4.16 | 前端子 Agent 展示 | 待实施 | - | |
| T4.17 | 编写集成测试 | 待实施 | - | |
| T4.18 | 更新文档与工具注册 | 待实施 | - | |
| T4.19 | 新增 question 工具(向用户提问) | 待实施 | - | |

---

## 九、风险与回滚策略

### 9.1 主要风险点

1. **子 Agent 递归调用**:子 Agent 调用 task 工具导致无限递归
   - 缓解:`allow_nested_task` 默认为 false,禁止子 Agent 调用 task 工具
   - 回滚:在 TaskTool 中硬编码禁止嵌套调用

2. **子 Agent 资源耗尽**:并行子 Agent 消耗大量 LLM token
   - 缓解:限制最大并行数(默认 5);设置超时时间
   - 回滚:禁用批量模式,仅支持单个子任务

3. **WebFetch 访问恶意 URL**:Agent 被诱导访问恶意网站
   - 缓解:URL 验证器拒绝内网地址;权限系统可配置 `ask`
   - 回滚:设置 `webfetch: "deny"` 禁用工具

4. **CSP 放宽安全风险**:允许 `https://*` 可能引入 XSS 风险
   - 缓解:仅 Rust 端执行网络请求,前端不直接 fetch;CSP 主要影响前端
   - 回滚:恢复原始 CSP,WebFetch 仅支持白名单域名

5. **搜索引擎 API 限额**:Tavily/SerpAPI 有调用次数限制
   - 缓解:默认使用 MCP(Exa AI 托管服务);配置缓存
   - 回滚:切换到 MCP 后端

6. **v1.1 新增 - 子 Agent 模式过滤失效**:子 Agent 未按父 Agent 的 AgentMode 过滤工具列表,导致 Build 模式下子 Agent 仍能看到文档 Handler
   - 缓解:SubAgentExecutor 的工具构建逻辑先按 mode 过滤,再按 allowed_tools 过滤;集成测试覆盖 Build/Document 两种模式的工具可见性
   - 回滚:若过滤逻辑有缺陷,临时禁用子 Agent 的文档 Handler 调用(在 execute_tool 中拦截)

### 9.2 验收标准

- 所有 18 个任务(T4.01-T4.18)实施完成
- `cargo test` 全部通过(包括 7 个新增集成测试,含 v1.1 的子 Agent 模式过滤测试)
- `cargo clippy` 无警告
- `npx tsc -b` 无类型错误
- 手动测试:调用 `task` 工具委托子任务,子 Agent 独立完成并返回结果
- 手动测试:调用 `webfetch` 工具获取 URL 内容,返回 Markdown
- 手动测试:调用 `websearch` 工具搜索关键词,返回结果列表
- 手动测试:前端正确展示子 Agent 执行过程和状态
- v1.1 手动测试:Build 模式下子 Agent 工具列表不含文档 Handler;Document 模式下子 Agent 工具列表含文档 Handler

---

## 十、后续阶段衔接说明

本阶段完成后,后续阶段将基于子 Agent 和高级工具进行扩展:

- **阶段 5(LSP 集成)**:LSP 工具可与 SourceCode 工具互补,子 Agent 可使用 LSP 工具进行代码分析
- **未来扩展**:子 Agent 可支持不同模型(如用更强模型执行复杂子任务,用轻量模型执行简单子任务)

子 Agent 和高级工具是 Agent 能力扩展的关键,必须确保本阶段完全实施并通过验收后再进入下一阶段。
