//! 子 Agent 执行器
//! 独立执行子任务，继承父 Agent 的上下文配置
//! 与主 AgentExecutor 共享 LLM Router、Tool Registry 等基础设施

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Wry};
use tokio::sync::RwLock;

use super::{is_document_handler, AgentMode};
use crate::db::sub_agent_message_repo;
use crate::db::Database;
use crate::errors::{CommandError, TOOL_INVALID_PARAMS, TOOL_NOT_FOUND};
use crate::events::types::{
    SubAgentContentPayload, SubAgentStatusPayload, SubAgentThinkingPayload,
    SubAgentToolCallPayload, SubAgentToolResultPayload, AGENT_SUB_AGENT_CONTENT,
    AGENT_SUB_AGENT_STATUS, AGENT_SUB_AGENT_THINKING, AGENT_SUB_AGENT_TOOL_CALL,
    AGENT_SUB_AGENT_TOOL_RESULT,
};
use crate::models::llm::{ChatMessage, LlmToolCall, ToolDefinition};
use crate::models::sub_agent::{SubAgentConfig, SubAgentResult, ToolCallRecord};
use crate::models::tool::ToolResult;
use crate::services::llm::router::LlmRouter;
use crate::services::permission::evaluator::{PermissionEvaluator, PermissionRequest};
use crate::services::permission::registry::PermissionRegistry;
use crate::services::permission::types::{PermissionAction, PermissionType};
use crate::services::tool::registry::ToolRegistry;

/// 将 SubAgentConfig.agent_mode (String) 转为 AgentMode 枚举
/// 无效字符串回退为 Build 模式
fn parse_agent_mode(mode: &str) -> AgentMode {
    match mode {
        "plan" => AgentMode::Plan,
        "document" => AgentMode::Document,
        _ => AgentMode::Build,
    }
}

/// 按子 Agent 配置过滤工具定义列表
/// - 非 Document 模式下过滤掉 docx/xlsx/pptx/pdf
/// - allowed_tools 非空时仅保留白名单中的工具
///
/// 此函数为纯函数，便于单元测试，由 SubAgentExecutor::list_tools_for_config 调用
pub fn filter_tools_for_sub_agent(
    tool_defs: Vec<Value>,
    agent_mode: &str,
    allowed_tools: &[String],
) -> Vec<Value> {
    let mode = parse_agent_mode(agent_mode);
    let mut defs = tool_defs;

    // 非 Document 模式下过滤掉文档 Handler
    if !mode.includes_document_handlers() {
        defs.retain(|d| {
            d.get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .map(|n| !is_document_handler(n))
                .unwrap_or(true)
        });
    }

    // 按 allowed_tools 白名单过滤
    if !allowed_tools.is_empty() {
        defs.retain(|d| {
            d.get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .map(|n| allowed_tools.iter().any(|allowed| allowed == n))
                .unwrap_or(false)
        });
    }

    defs
}

/// 子 Agent 执行器 trait（类型擦除，避免 SubAgentExecutor 的 Drop glue 在 cdylib 模式下的符号导出问题）
/// TaskTool 通过此 trait 存储执行器引用，而非直接引用 SubAgentExecutor 具体类型
#[async_trait]
pub trait SubAgentExecTrait: Send + Sync {
    /// 执行子 Agent
    async fn exec_sub_agent(&self, config: SubAgentConfig) -> SubAgentResult;
}

/// 子 Agent 执行内部结果
struct ExecResult {
    /// 最终消息（LLM 最后一次回复内容）
    final_message: String,
    /// 执行迭代次数
    iterations: u32,
    /// 工具调用次数
    tool_calls: u32,
    /// 工具调用记录列表（完整工具调用历史）
    tool_call_records: Vec<ToolCallRecord>,
}

/// 子 Agent 执行器
/// 独立执行子任务，继承父 Agent 的上下文配置
/// 与主 AgentExecutor 共享 LLM Router、Tool Registry 等基础设施
pub struct SubAgentExecutor {
    /// LLM 路由器（与主 Agent 共享，AppState 中为 Arc<RwLock<Arc<LlmRouter>>>）
    llm_router: Arc<RwLock<Arc<LlmRouter>>>,
    /// 工具注册表（与主 Agent 共享）
    tool_registry: Arc<ToolRegistry>,
    /// 权限注册表（T4.03/T4.04 用于工具执行权限校验）
    permission_registry: Arc<PermissionRegistry>,
    /// Tauri AppHandle，用于发射事件（T4.04 完善事件发射）
    app_handle: Option<AppHandle<Wry>>,
    /// 数据库连接（用于持久化子 Agent 消息）
    db: Arc<Database>,
}

impl SubAgentExecutor {
    /// 创建子 Agent 执行器，接收主 Agent 共享的基础设施
    pub fn new(
        llm_router: Arc<RwLock<Arc<LlmRouter>>>,
        tool_registry: Arc<ToolRegistry>,
        permission_registry: Arc<PermissionRegistry>,
        app_handle: Option<AppHandle<Wry>>,
        db: Arc<Database>,
    ) -> Self {
        Self {
            llm_router,
            tool_registry,
            permission_registry,
            app_handle,
            db,
        }
    }

    /// 统一事件发射方法
    /// app_handle 为 None 时仅记录日志（不报错），保证子 Agent 在无 Tauri 上下文时仍可运行
    fn emit_event(&self, event_name: &str, payload: impl Serialize + Clone) {
        if let Some(handle) = &self.app_handle {
            if let Err(e) = handle.emit(event_name, payload) {
                log::debug!("事件 {} 发射失败（非关键）: {}", event_name, e);
            }
        } else {
            log::debug!("事件发射跳过(app_handle 未设置): {}", event_name);
        }
    }

    /// 发射子 Agent 状态变更事件
    fn emit_status(
        &self,
        parent_session_id: &str,
        agent_id: &str,
        status: &str,
        message: Option<String>,
        iteration: u32,
        task_description: &str,
    ) {
        let payload = SubAgentStatusPayload {
            parent_session_id: parent_session_id.to_string(),
            agent_id: agent_id.to_string(),
            status: status.to_string(),
            message,
            task_description: task_description.to_string(),
            iteration,
        };
        self.emit_event(AGENT_SUB_AGENT_STATUS, payload);
    }

    /// 列出子 Agent 可用的工具定义（OpenAI function calling 格式 Vec<Value>）
    /// 按 AgentMode 和 allowed_tools 过滤：
    /// - 非 Document 模式下过滤掉 docx/xlsx/pptx/pdf
    /// - allowed_tools 非空时仅保留白名单中的工具
    pub fn list_tools_for_config(&self, config: &SubAgentConfig) -> Vec<Value> {
        let defs = self.tool_registry.tool_definitions();
        filter_tools_for_sub_agent(defs, &config.agent_mode, &config.allowed_tools)
    }

    /// 执行子 Agent
    /// 使用 tokio::time::timeout 控制超时，处理成功/失败/超时三种结果
    pub async fn execute(&self, config: SubAgentConfig) -> SubAgentResult {
        let start_time = std::time::Instant::now();
        let agent_id = config.agent_id.clone();
        let parent_session_id = config.parent_session_id.clone();

        log::info!(
            "子 Agent 开始执行: agent_id={}, parent_session={}, nesting_depth={}",
            agent_id,
            config.parent_session_id,
            config.nesting_depth
        );

        // 发射状态变更事件：开始执行
        self.emit_status(
            &parent_session_id,
            &agent_id,
            "running",
            None,
            0,
            &config.task_description,
        );

        // 使用超时控制执行
        let timeout = Duration::from_secs(config.timeout_seconds);
        let result = tokio::time::timeout(timeout, self.execute_inner(config.clone())).await;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        match result {
            // 执行成功
            Ok(Ok(exec_result)) => {
                log::info!(
                    "子 Agent 执行成功: agent_id={}, 迭代={}, 工具调用={}, 耗时={}ms",
                    agent_id,
                    exec_result.iterations,
                    exec_result.tool_calls,
                    duration_ms
                );
                // 发射状态变更事件：执行成功（附带结果摘要）
                self.emit_status(
                    &parent_session_id,
                    &agent_id,
                    "completed",
                    Some(exec_result.final_message.clone()),
                    exec_result.iterations,
                    &config.task_description,
                );
                SubAgentResult {
                    agent_id,
                    success: true,
                    result: exec_result.final_message,
                    error: None,
                    iterations: exec_result.iterations,
                    duration_ms,
                    tool_calls: exec_result.tool_calls,
                    task_description: config.task_description.clone(),
                    tool_call_records: exec_result.tool_call_records,
                }
            }
            // 执行失败（返回错误）
            Ok(Err(e)) => {
                log::warn!("子 Agent 执行失败: agent_id={}, 错误: {}", agent_id, e);
                // 发射状态变更事件：执行失败（附带错误信息）
                self.emit_status(
                    &parent_session_id,
                    &agent_id,
                    "failed",
                    Some(e.to_string()),
                    0,
                    &config.task_description,
                );
                SubAgentResult {
                    agent_id,
                    success: false,
                    result: String::new(),
                    error: Some(e.to_string()),
                    iterations: 0,
                    duration_ms,
                    tool_calls: 0,
                    task_description: config.task_description.clone(),
                    tool_call_records: Vec::new(),
                }
            }
            // 执行超时
            Err(_) => {
                log::warn!(
                    "子 Agent 执行超时: agent_id={}, 超时={}秒",
                    agent_id,
                    config.timeout_seconds
                );
                // 发射状态变更事件：执行失败（超时视为失败）
                self.emit_status(
                    &parent_session_id,
                    &agent_id,
                    "failed",
                    Some(format!("执行超时（{}秒）", config.timeout_seconds)),
                    0,
                    &config.task_description,
                );
                SubAgentResult {
                    agent_id,
                    success: false,
                    result: String::new(),
                    error: Some(format!(
                        "Execution timeout ({} seconds)",
                        config.timeout_seconds
                    )),
                    iterations: 0,
                    duration_ms,
                    tool_calls: 0,
                    task_description: config.task_description.clone(),
                    tool_call_records: Vec::new(),
                }
            }
        }
    }

    /// 子 Agent 执行内部逻辑
    /// 构建 system + user 消息，迭代调用 LLM 并执行工具，直到无 tool_calls 或达到最大迭代次数
    async fn execute_inner(&self, config: SubAgentConfig) -> Result<ExecResult, CommandError> {
        // 构建初始消息列表
        let mut messages: Vec<ChatMessage> = Vec::new();
        // 子 Agent 消息持久化序号（按添加顺序递增）
        let mut seq: u32 = 0;

        // 工具调用记录列表（用于持久化和前端恢复）
        let mut tool_call_records: Vec<ToolCallRecord> = Vec::new();

        // 添加 system 消息（继承自父 Agent 的系统提示词）
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: config.system_prompt.clone(),
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            attachments: None,
            metadata: None,
        });
        // 持久化 system 消息（失败时仅记录警告，不中断执行）
        self.persist_sub_agent_message(&config, seq, messages.last().unwrap());
        seq += 1;

        // 添加 user 消息（子任务描述）
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: format!(
                "Please execute the following subtask:\n\n{}\n\nProvide a summary of the final result upon completion.",
                config.task_description
            ),
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            attachments: None,
            metadata: None,
        });
        // 持久化 user 消息（失败时仅记录警告，不中断执行）
        self.persist_sub_agent_message(&config, seq, messages.last().unwrap());
        seq += 1;

        // 构建工具定义列表：从 Vec<Value> 转换为 Vec<ToolDefinition>
        let tool_defs_json = self.list_tools_for_config(&config);
        let tools: Vec<ToolDefinition> = tool_defs_json
            .iter()
            .filter_map(|v| {
                let func = v.get("function")?;
                Some(ToolDefinition {
                    name: func["name"].as_str()?.to_string(),
                    description: func["description"].as_str()?.to_string(),
                    parameters: func["parameters"].clone(),
                })
            })
            .collect();

        let mut iterations: u32 = 0;
        let mut tool_calls_count: u32 = 0;

        // 迭代循环：调用 LLM → 执行工具 → 返回结果给 LLM
        while iterations < config.max_iterations {
            iterations += 1;

            // 获取 LlmRouter 的 Arc 引用（读锁获取后立即 clone 释放）
            let router = self.llm_router.read().await.clone();

            // 调用 LLM（流式）
            let mut stream_rx = router.chat_stream(&messages, &tools, None).await?;

            // 流式累加器
            let mut assistant_content = String::new();
            let mut reasoning_content = String::new();
            let mut collected_tool_calls: HashMap<u32, LlmToolCall> = HashMap::new();

            // 流式接收 chunk
            while let Some(chunk_result) = stream_rx.recv().await {
                match chunk_result {
                    Ok(chunk) => {
                        for choice in chunk.choices {
                            // 处理 reasoning_content 增量
                            if let Some(rc) = &choice.delta.reasoning_content {
                                reasoning_content.push_str(rc);
                                // 发射思考增量事件（实时）
                                self.emit_event(
                                    AGENT_SUB_AGENT_THINKING,
                                    SubAgentThinkingPayload {
                                        parent_session_id: config.parent_session_id.clone(),
                                        agent_id: config.agent_id.clone(),
                                        content: rc.clone(),
                                        is_streaming: true,
                                        iteration: iterations,
                                    },
                                );
                            }

                            // 处理 content 增量
                            if let Some(content) = &choice.delta.content {
                                assistant_content.push_str(content);
                                // 发射内容增量事件（实时）
                                self.emit_event(
                                    AGENT_SUB_AGENT_CONTENT,
                                    SubAgentContentPayload {
                                        parent_session_id: config.parent_session_id.clone(),
                                        agent_id: config.agent_id.clone(),
                                        content: content.clone(),
                                        is_streaming: true,
                                        iteration: iterations,
                                    },
                                );
                            }

                            // 处理 tool_calls 增量，按 index 合并
                            if let Some(delta_tool_calls) = choice.delta.tool_calls {
                                for tc in delta_tool_calls {
                                    let tc_index = tc.index;
                                    match collected_tool_calls.get_mut(&tc_index) {
                                        Some(existing) => {
                                            if !tc.id.is_empty() {
                                                existing.id = tc.id;
                                            }
                                            existing.name.push_str(&tc.name);
                                            existing.arguments.push_str(&tc.arguments);
                                        }
                                        None => {
                                            collected_tool_calls.insert(tc_index, tc);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("子 Agent 流式响应错误: {}", e.message);
                        // 流式错误时中断
                        return Err(e);
                    }
                }
            }

            // 流式结束：关闭思考节点（若有 reasoning_content）
            if !reasoning_content.is_empty() {
                self.emit_event(
                    AGENT_SUB_AGENT_THINKING,
                    SubAgentThinkingPayload {
                        parent_session_id: config.parent_session_id.clone(),
                        agent_id: config.agent_id.clone(),
                        content: String::new(),
                        is_streaming: false,
                        iteration: iterations,
                    },
                );
            }

            // 将 HashMap 转为按 index 排序的 Vec
            let mut tool_calls_vec: Vec<LlmToolCall> =
                collected_tool_calls.into_values().collect::<Vec<_>>();
            tool_calls_vec.sort_by_key(|tc| tc.index);

            // 构建 assistant 消息
            let assistant_message = ChatMessage {
                role: "assistant".to_string(),
                content: assistant_content.clone(),
                content_parts: None,
                tool_calls: if tool_calls_vec.is_empty() {
                    None
                } else {
                    Some(tool_calls_vec.clone())
                },
                tool_call_id: None,
                reasoning_content: if reasoning_content.is_empty() {
                    None
                } else {
                    Some(reasoning_content.clone())
                },
                attachments: None,
                metadata: None,
            };

            // 推入 messages 并持久化
            messages.push(assistant_message);
            self.persist_sub_agent_message(&config, seq, messages.last().unwrap());
            seq += 1;

            // 关闭内容节点（若有 content）
            // 公共逻辑：无论是否有 tool_calls，流式结束后都需关闭内容节点
            if !assistant_content.is_empty() {
                self.emit_event(
                    AGENT_SUB_AGENT_CONTENT,
                    SubAgentContentPayload {
                        parent_session_id: config.parent_session_id.clone(),
                        agent_id: config.agent_id.clone(),
                        content: String::new(),
                        is_streaming: false,
                        iteration: iterations,
                    },
                );
            }

            // 检查是否有 tool_calls
            if !tool_calls_vec.is_empty() {
                // 遍历执行所有 tool_calls
                for tool_call in tool_calls_vec.iter() {
                    tool_calls_count += 1;

                    // 解析 arguments 字符串为 Value（解析失败时使用空对象）
                    let tool_args: Value = if tool_call.arguments.is_empty() {
                        serde_json::json!({})
                    } else {
                        serde_json::from_str(&tool_call.arguments).unwrap_or(serde_json::json!({}))
                    };

                    // 发射子 Agent 工具调用事件（含 tool_call_id）
                    self.emit_event(
                        AGENT_SUB_AGENT_TOOL_CALL,
                        SubAgentToolCallPayload {
                            parent_session_id: config.parent_session_id.clone(),
                            agent_id: config.agent_id.clone(),
                            tool_call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                            arguments: tool_args.clone(),
                            iteration: iterations,
                        },
                    );

                    // 记录工具调用（用于持久化和前端恢复）
                    tool_call_records.push(ToolCallRecord {
                        tool_name: tool_call.name.clone(),
                        arguments: tool_args,
                    });

                    // 执行工具调用（出错时不中断，将错误信息作为 tool 结果返回给 LLM）
                    let (tool_result, tool_success) =
                        match self.execute_tool(tool_call, &config).await {
                            Ok(result) => {
                                // 判断是否为错误结果（execute_tool 返回的 JSON 中可能包含 error 字段）
                                let is_error = result.contains("\"error\"");
                                (result, !is_error)
                            }
                            Err(e) => {
                                let error_msg = serde_json::json!({
                                    "error": "tool_execution_failed",
                                    "tool": tool_call.name,
                                    "message": e.to_string()
                                })
                                .to_string();
                                (error_msg, false)
                            }
                        };

                    // 发射子 Agent 工具结果事件
                    self.emit_event(
                        AGENT_SUB_AGENT_TOOL_RESULT,
                        SubAgentToolResultPayload {
                            parent_session_id: config.parent_session_id.clone(),
                            agent_id: config.agent_id.clone(),
                            tool_call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                            result: if tool_success {
                                Some(tool_result.clone())
                            } else {
                                None
                            },
                            error: if !tool_success {
                                Some(tool_result.clone())
                            } else {
                                None
                            },
                            success: tool_success,
                            iteration: iterations,
                        },
                    );

                    // 添加 tool 消息（携带 tool_call_id 关联工具调用）
                    messages.push(ChatMessage {
                        role: "tool".to_string(),
                        content: tool_result,
                        content_parts: None,
                        tool_calls: None,
                        tool_call_id: Some(tool_call.id.clone()),
                        reasoning_content: None,
                        attachments: None,
                        metadata: None,
                    });
                    // 持久化 tool 消息
                    self.persist_sub_agent_message(&config, seq, messages.last().unwrap());
                    seq += 1;
                }

                continue;
            }

            // 无 tool_calls，任务完成
            log::info!(
                "子 Agent 任务完成: iterations={}, tool_calls={}",
                iterations,
                tool_calls_count
            );
            return Ok(ExecResult {
                final_message: assistant_content,
                iterations,
                tool_calls: tool_calls_count,
                tool_call_records,
            });
        }

        // 达到最大迭代次数
        log::warn!(
            "子 Agent 达到最大迭代次数 {}: agent_id={}",
            config.max_iterations,
            config.agent_id
        );
        Ok(ExecResult {
            final_message: format!(
                "子 Agent 达到最大迭代次数 {}，未完成任务",
                config.max_iterations
            ),
            iterations,
            tool_calls: tool_calls_count,
            tool_call_records,
        })
    }

    /// 持久化子 Agent 消息到数据库
    /// 失败时仅记录警告日志，不中断子 Agent 执行
    fn persist_sub_agent_message(&self, config: &SubAgentConfig, seq: u32, msg: &ChatMessage) {
        match self.db.conn() {
            Ok(conn) => {
                if let Err(e) = sub_agent_message_repo::create_sub_agent_message(
                    &conn,
                    &config.parent_session_id,
                    &config.agent_id,
                    seq,
                    msg,
                ) {
                    log::warn!("子 Agent 消息持久化失败(seq={}): {}", seq, e);
                }
            }
            Err(e) => {
                log::warn!(
                    "获取数据库连接失败，跳过子 Agent 消息持久化(seq={}): {}",
                    seq,
                    e.message
                );
            }
        }
    }

    /// 执行工具调用
    /// 从 ToolRegistry 获取工具，注入系统参数后执行
    /// T4.10/T4.14: 接入权限检查（复用 permission_registry）
    async fn execute_tool(
        &self,
        tool_call: &LlmToolCall,
        config: &SubAgentConfig,
    ) -> Result<String, CommandError> {
        // 获取工具的 Arc 引用
        let tool = self.tool_registry.get_arc(&tool_call.name).ok_or_else(|| {
            CommandError::tool(
                TOOL_NOT_FOUND,
                format!("Tool {} does not exist", tool_call.name),
            )
        })?;

        // 解析 tool_call.arguments（String）为 serde_json::Value
        let mut params: Value = if tool_call.arguments.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&tool_call.arguments).map_err(|e| {
                CommandError::tool(
                    TOOL_INVALID_PARAMS,
                    format!("Tool {} parameter parsing failed: {}", tool_call.name, e),
                )
            })?
        };

        // 注入系统参数到 params（以下划线开头的键表示系统注入，不暴露给 LLM）
        if let Some(obj) = params.as_object_mut() {
            // 注入 workspace_root（带下划线前缀，子 Agent 框架内部使用）
            obj.insert(
                "_workspace_root".to_string(),
                Value::String(config.workspace_root.clone()),
            );
            // 同时注入无下划线版本，供工具内部读取（与主 Agent executor 一致）
            obj.insert(
                "workspace_root".to_string(),
                Value::String(config.workspace_root.clone()),
            );
            // 注入 session_id（供 scratchpad/todowrite 等工具按会话隔离状态）
            obj.insert(
                "_session_id".to_string(),
                Value::String(config.parent_session_id.clone()),
            );
            // 注入 nesting_depth（子 Agent 嵌套深度，用于限制递归）
            obj.insert(
                "_nesting_depth".to_string(),
                Value::Number(serde_json::Number::from(config.nesting_depth)),
            );
        }

        // T4.10/T4.14: 权限检查（子 Agent 复用父会话的权限规则）
        if !self
            .check_permission(config, &tool_call.name, &params)
            .await?
        {
            // 权限拒绝，返回错误信息给 LLM
            return Ok(serde_json::json!({
                "error": "Permission denied",
                "tool": tool_call.name,
                "message": "Sub-agent tool call denied by permission system"
            })
            .to_string());
        }

        // 执行工具（execute 返回 ToolResult，不是 Result）
        let result: ToolResult = tool.execute(params).await;

        // 序列化工具结果为字符串
        // 优先序列化 output 字段（工具的实际输出），为 None 时序列化整个 ToolResult
        let result_str = if let Some(output) = &result.output {
            serde_json::to_string(output).unwrap_or_else(|_| output.to_string())
        } else {
            serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
        };

        Ok(result_str)
    }

    /// T4.10/T4.14: 子 Agent 权限检查
    /// 复用父会话的权限规则和白名单，Ask 规则在子 Agent 上下文中默认 Allow
    /// （子 Agent 无法直接与用户交互，Ask 视为 Allow 以避免阻塞）
    async fn check_permission(
        &self,
        config: &SubAgentConfig,
        tool_name: &str,
        params: &Value,
    ) -> Result<bool, CommandError> {
        let mode = parse_agent_mode(&config.agent_mode);

        // 1. Plan 模式拒绝修改类操作
        if mode.is_plan() && PermissionType::from_tool_name(tool_name).is_modification() {
            log::warn!(
                "子 Agent 权限拒绝(Plan 模式): agent_id={}, tool={}",
                config.agent_id,
                tool_name
            );
            return Ok(false);
        }

        // 2. 构造权限评估请求
        let request = PermissionRequest::from_tool_call(tool_name, params);

        // 3. 规则评估（子 Agent 无 workspace_id，使用 None）
        let rules = self
            .permission_registry
            .load_effective_rules(None, Some(&config.parent_session_id));
        let decision = PermissionEvaluator::evaluate(&request, &rules);

        match decision.action {
            PermissionAction::Allow => {
                log::debug!(
                    "子 Agent 权限允许(规则): agent_id={}, tool={}",
                    config.agent_id,
                    tool_name
                );
                Ok(true)
            }
            PermissionAction::Deny => {
                log::warn!(
                    "子 Agent 权限拒绝(规则): agent_id={}, tool={}",
                    config.agent_id,
                    tool_name
                );
                Ok(false)
            }
            // 子 Agent 无法与用户交互，Ask 视为 Allow
            PermissionAction::Ask => {
                log::debug!(
                    "子 Agent 权限允许(Ask→Allow): agent_id={}, tool={}",
                    config.agent_id,
                    tool_name
                );
                Ok(true)
            }
        }
    }
}

// 为 SubAgentExecutor 实现 SubAgentExecTrait（委托到现有 execute 方法）
// 通过 trait 对象实现类型擦除，避免 TaskTool 直接持有 SubAgentExecutor 具体类型
#[async_trait]
impl SubAgentExecTrait for SubAgentExecutor {
    async fn exec_sub_agent(&self, config: SubAgentConfig) -> SubAgentResult {
        self.execute(config).await
    }
}
