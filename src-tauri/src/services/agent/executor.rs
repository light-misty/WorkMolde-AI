use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use serde_json::json;
use tauri::Runtime;

use crate::config::app_settings::ConfirmationLevel;
use crate::errors::CommandError;
use crate::events::emitter::AgentEmitter;
use crate::events::types::*;
use crate::models::llm::{ChatMessage, LlmToolCall};
use crate::services::llm::router::LlmRouter;
use crate::services::skill::registry::SkillRegistry;
use crate::services::tool::registry::ToolRegistry;
use crate::ConfirmDecision;
use super::context::AgentContext;

const MAX_LLM_RETRIES: u32 = 2;
const RETRY_DELAY_SECONDS: u64 = 2;
/// 确认操作超时时间（秒）
const CONFIRM_TIMEOUT_SECS: u64 = 300;
/// 始终需要确认的高风险 Skill 列表
const HIGH_RISK_SKILLS: &[&str] = &["delete_file", "code_interpreter_skill"];

/// 检查错误码是否可重试
fn is_retryable_error(code: u32) -> bool {
    matches!(
        code,
        crate::errors::LLM_CONNECTION_FAILED
            | crate::errors::LLM_RATE_LIMITED
            | crate::errors::LLM_TIMEOUT
            | crate::errors::LLM_STREAM_ERROR
            | crate::errors::LLM_PROVIDER_UNAVAILABLE
            | crate::errors::LLM_DNS_RESOLVE_FAILED
            | crate::errors::LLM_CONNECTION_REFUSED
            | crate::errors::LLM_SSL_ERROR
            | crate::errors::LLM_NETWORK_UNREACHABLE
    )
}

pub struct ExecutionResult {
    pub summary: String,
    pub total_steps: u32,
    pub duration_ms: u64,
}

/// 增量持久化回调类型
/// 接收 session_id 和新增消息列表，返回持久化结果
type PersistFn = Arc<dyn Fn(&str, &[ChatMessage]) -> Result<(), CommandError> + Send + Sync>;

/// 上下文窗口使用信息持久化回调
type ContextUsagePersistFn = Arc<dyn Fn(&str, &crate::models::llm::ContextUsageInfo) + Send + Sync>;

/// 版本快照回调类型
/// 接收 (workspace_id, session_id, file_path, operation)，在文件修改/删除前创建快照
type SnapshotFn = Arc<dyn Fn(&str, &str, &str, &str) -> Result<(), CommandError> + Send + Sync>;

pub struct AgentExecutor<R: Runtime> {
    router: Arc<LlmRouter>,
    tool_registry: Arc<ToolRegistry>,
    registry: Arc<tokio::sync::Mutex<SkillRegistry>>,
    emitter: AgentEmitter<R>,
    confirm_channels: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<ConfirmDecision>>>>,
    max_iterations: u32,
    should_stop: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    /// 增量持久化回调，每轮迭代后调用，防止崩溃丢失消息
    persist_fn: Option<PersistFn>,
    /// 上下文窗口使用信息持久化回调，每次发射事件时调用，确保切换会话后数据一致
    context_usage_persist_fn: Option<ContextUsagePersistFn>,
    /// 版本快照回调，在文件修改/删除前调用，自动创建快照
    snapshot_fn: Option<SnapshotFn>,
    /// 操作确认级别，决定哪些操作需要用户手动确认
    confirmation_level: ConfirmationLevel,
}

impl<R: Runtime> AgentExecutor<R> {
    pub fn new(
        router: Arc<LlmRouter>,
        tool_registry: Arc<ToolRegistry>,
        registry: Arc<tokio::sync::Mutex<SkillRegistry>>,
        emitter: AgentEmitter<R>,
        confirm_channels: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<ConfirmDecision>>>>,
    ) -> Self {
        Self {
            router,
            tool_registry,
            registry,
            emitter,
            confirm_channels,
            max_iterations: 20,
            should_stop: Arc::new(|_| false),
            persist_fn: None,
            context_usage_persist_fn: None,
            snapshot_fn: None,
            confirmation_level: ConfirmationLevel::default(),
        }
    }

    /// 设置停止检查回调
    pub fn with_stop_check(
        mut self,
        check: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    ) -> Self {
        self.should_stop = check;
        self
    }

    /// 设置最大迭代次数
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    /// 设置增量持久化回调
    pub fn with_persist_fn(mut self, f: PersistFn) -> Self {
        self.persist_fn = Some(f);
        self
    }

    /// 设置上下文窗口使用信息持久化回调
    pub fn with_context_usage_persist_fn(mut self, f: ContextUsagePersistFn) -> Self {
        self.context_usage_persist_fn = Some(f);
        self
    }

    /// 设置版本快照回调，在文件修改/删除前自动创建快照
    pub fn with_snapshot_fn(mut self, f: SnapshotFn) -> Self {
        self.snapshot_fn = Some(f);
        self
    }

    /// 设置操作确认级别
    pub fn with_confirmation_level(mut self, level: ConfirmationLevel) -> Self {
        self.confirmation_level = level;
        self
    }

    /// 检查是否应该停止
    fn check_stopped(&self, session_id: &str) -> bool {
        (self.should_stop)(session_id)
    }

    /// 检查并处理停止逻辑，如果需要停止则返回 Some(ExecutionResult)
    fn handle_stop_if_needed(
        &self,
        ctx: &mut AgentContext,
        total_steps: u32,
        start_time: std::time::Instant,
    ) -> Option<ExecutionResult> {
        if self.check_stopped(&ctx.session_id) {
            log::info!("Agent 被用户停止, session_id={}", ctx.session_id);
            self.persist_new_messages(ctx);
            ctx.mark_persisted();
            self.emitter.emit_stopped(StoppedPayload {
                session_id: ctx.session_id.clone(),
                reason: "用户手动停止".to_string(),
                completed_steps: total_steps,
            }).ok();
            Some(ExecutionResult {
                summary: "Agent 已被用户停止".to_string(),
                total_steps,
                duration_ms: start_time.elapsed().as_millis() as u64,
            })
        } else {
            None
        }
    }

    /// 检查是否为高风险操作（需要用户确认）
    /// 根据确认级别决定哪些操作需要用户确认：
    /// - Never: 任何操作都不需要确认
    /// - EditOnly: 仅高风险操作需要确认
    /// - Always: 所有 Skill/Tool 调用都需要确认
    fn needs_confirmation(&self, name: &str, _params: &serde_json::Value) -> bool {
        match self.confirmation_level {
            ConfirmationLevel::Never => false,
            ConfirmationLevel::EditOnly => {
                // 仅编辑/删除操作需要确认
                // 1. delete_file 始终为高风险
                if HIGH_RISK_SKILLS.contains(&name) {
                    return true;
                }
                false
            }
            ConfirmationLevel::Always => true,
        }
    }

    /// 从 Skill 参数中提取需要创建快照的文件路径列表
    /// delete_file: 单文件路径
    /// 文档 Skill（docx_skill/xlsx_skill/pptx_skill/pdf_skill）: 精简后不再有 modify 操作，无需快照
    fn extract_snapshot_paths(&self, skill_name: &str, params: &serde_json::Value) -> Vec<String> {
        match skill_name {
            "delete_file" => {
                vec![params["path"].as_str().unwrap_or("").to_string()]
            }
            "docx_skill" | "xlsx_skill" | "pptx_skill" | "pdf_skill" => {
                // 文档 Skill 精简后不再有 modify 操作，无需创建快照
                Vec::new()
            }
            "code_interpreter_skill" => {
                // Code Interpreter 可能修改多个文件，提取预期文件列表
                // 1. 优先从 expected_files 参数提取（LLM 声明的预期输出文件）
                // 2. 如果没有 expected_files，则不创建快照（因为无法预知文件路径）
                if let Some(files) = params["expected_files"].as_array() {
                    files.iter()
                        .filter_map(|f| f.as_str().map(|s| s.to_string()))
                        .collect()
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    /// 调用增量持久化回调，将新增消息写入数据库
    fn persist_new_messages(&self, ctx: &AgentContext) {
        if let Some(ref persist_fn) = self.persist_fn {
            let unpersisted = ctx.get_unpersisted_messages();
            if !unpersisted.is_empty() {
                if let Err(e) = persist_fn(&ctx.session_id, unpersisted) {
                    log::warn!("增量持久化失败: session_id={}, 错误: {}", ctx.session_id, e.message);
                }
            }
        }
    }

    async fn request_confirmation(
        &self,
        session_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<bool, CommandError> {
        let operation_id = format!("confirm_{}", uuid::Uuid::new_v4());

        let risk_level = match self.confirmation_level {
            ConfirmationLevel::Always => {
                // 全部需确认模式下，根据操作类型区分风险等级
                if tool_name == "delete_file" {
                    "critical"
                } else if tool_name == "code_interpreter_skill" {
                    "high"  // 代码执行始终为高风险
                } else {
                    "normal"
                }
            }
            _ => {
                if tool_name == "delete_file" {
                    "critical"
                } else {
                    "high"
                }
            }
        };

        let description = match tool_name {
            "delete_file" => format!("删除文件: {}", arguments["path"].as_str().unwrap_or("未知")),
            "docx_skill" | "xlsx_skill" | "pptx_skill" | "pdf_skill" => {
                let action = arguments["action"].as_str().unwrap_or("操作");
                let path = arguments["path"].as_str().unwrap_or("未知文件");
                format!("{} - {}: {}", tool_name, action, path)
            }
            "code_interpreter_skill" => {
                // 展示代码描述和代码摘要
                let desc = arguments["description"].as_str().unwrap_or("执行代码");
                // 安全截取：按字符边界切片，避免在多字节UTF-8字符中间切割导致panic
                let code_preview: String = arguments["code"].as_str()
                    .map(|c| {
                        if c.chars().count() > 200 {
                            format!("{}...", c.chars().take(200).collect::<String>())
                        } else {
                            c.to_string()
                        }
                    })
                    .unwrap_or_default();
                format!("执行代码: {}\n{}", desc, code_preview)
            }
            _ => format!("执行操作: {}", tool_name),
        };

        // 先创建 channel 并插入 map，再发射事件，避免竞态条件
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut channels = self.confirm_channels.lock().await;
            channels.insert(operation_id.clone(), tx);
        }

        self.emitter.emit_confirm(ConfirmPayload {
            session_id: session_id.to_string(),
            operation_id: operation_id.clone(),
            operation_type: tool_name.to_string(),
            description,
            details: arguments.clone(),
            risk_level: risk_level.to_string(),
        }).ok();

        match tokio::time::timeout(Duration::from_secs(CONFIRM_TIMEOUT_SECS), rx).await {
            Ok(Ok(decision)) => {
                let mut channels = self.confirm_channels.lock().await;
                channels.remove(&operation_id);
                if decision.approved {
                    log::info!("用户确认操作: operation_id={}, tool={}", operation_id, tool_name);
                    Ok(true)
                } else {
                    log::info!("用户拒绝操作: operation_id={}, tool={}, feedback={:?}", operation_id, tool_name, decision.feedback);
                    Ok(false)
                }
            }
            Ok(Err(_)) => {
                let mut channels = self.confirm_channels.lock().await;
                channels.remove(&operation_id);
                log::warn!("确认通道关闭: operation_id={}", operation_id);
                Ok(false)
            }
            Err(_) => {
                let mut channels = self.confirm_channels.lock().await;
                channels.remove(&operation_id);
                log::warn!("确认超时: operation_id={}", operation_id);
                self.emitter.emit_error(ErrorPayload {
                    session_id: session_id.to_string(),
                    code: crate::errors::AGENT_CONFIRMATION_TIMEOUT,
                    message: format!("操作确认超时 ({}秒)", CONFIRM_TIMEOUT_SECS),
                    recoverable: true,
                }).ok();
                Ok(false)
            }
        }
    }

    fn emit_todo_progress(
        &self,
        session_id: &str,
        current_step: u32,
        total_possible: u32,
        tool_name: &str,
    ) {
        let mut todos = Vec::new();

        if current_step > 1 {
            todos.push(TodoItem {
                id: format!("step_{}", current_step - 1),
                content: format!("步骤 {} 已完成", current_step - 1),
                status: "completed".to_string(),
            });
        }

        todos.push(TodoItem {
            id: format!("step_{}", current_step),
            content: format!("正在执行: {}", tool_name),
            status: "in_progress".to_string(),
        });

        if current_step < total_possible {
            todos.push(TodoItem {
                id: format!("step_{}", current_step + 1),
                content: format!("步骤 {} 待执行", current_step + 1),
                status: "pending".to_string(),
            });
        }

        self.emitter.emit_todo_update(TodoUpdatePayload {
            session_id: session_id.to_string(),
            todos,
        }).ok();
    }

    /// 发射上下文窗口使用情况事件
    async fn emit_context_usage(&self, ctx: &AgentContext, response_tokens: usize) {
        // 获取当前模型名称
        let model_name = self.router.current_model_name();

        let usage_info = ctx.calculate_context_usage(response_tokens, model_name);

        // 持久化上下文窗口使用信息到数据库，确保切换会话后数据一致
        if let Some(ref persist_fn) = self.context_usage_persist_fn {
            persist_fn(&ctx.session_id, &usage_info);
        }

        self.emitter.emit_context_usage(crate::events::types::ContextUsagePayload {
            session_id: ctx.session_id.clone(),
            context_usage: usage_info,
        }).ok();
    }

    pub async fn execute(&self, ctx: &mut AgentContext) -> Result<ExecutionResult, CommandError> {
        let start_time = std::time::Instant::now();
        let mut total_steps = 0u32;

        log::info!("Agent 开始执行, session_id={}", ctx.session_id);

        // 合并 Tool + Skill 的工具定义
        let tool_defs_json = {
            let tool_defs = self.tool_registry.tool_definitions();
            let skill_defs = {
                let reg = self.registry.lock().await;
                reg.tool_definitions()
            };
            [tool_defs, skill_defs].concat()
        };
        let tools: Vec<crate::models::llm::ToolDefinition> = tool_defs_json
            .iter()
            .filter_map(|v| {
                let func = v.get("function")?;
                Some(crate::models::llm::ToolDefinition {
                    name: func["name"].as_str()?.to_string(),
                    description: func["description"].as_str()?.to_string(),
                    parameters: func["parameters"].clone(),
                })
            })
            .collect();

        // 估算工具定义的 Token 数并设置到上下文中
        let func_defs_str = tool_defs_json
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        ctx.function_definitions_tokens = crate::services::agent::prompts::token_budget::TokenBudgetManager::estimate_tokens(&func_defs_str);

        self.emitter.emit_todo_update(TodoUpdatePayload {
            session_id: ctx.session_id.clone(),
            todos: vec![TodoItem {
                id: "step_0".to_string(),
                content: "正在分析用户请求...".to_string(),
                status: "in_progress".to_string(),
            }],
        }).ok();

        for iteration in 0..self.max_iterations {
            // 检查是否被用户停止
            if let Some(result) = self.handle_stop_if_needed(
                ctx,
                total_steps,
                start_time,
            ) {
                return Ok(result);
            }

            log::debug!("Agent 迭代 #{}, session_id={}", iteration + 1, ctx.session_id);

            let current_iteration = iteration + 1;
            let messages = ctx.get_messages_for_iteration(current_iteration);
            
            let mut llm_retry_count = 0;
            let mut _last_error: Option<CommandError> = None;
            let mut stream_rx = loop {
                if self.check_stopped(&ctx.session_id) {
                    return Ok(self.handle_stop_if_needed(ctx, total_steps, start_time).unwrap());
                }
                
                log::debug!("调用 LLM 流式接口, session_id={}, 消息数={}, 重试次数={}", ctx.session_id, messages.len(), llm_retry_count);
                match self.router.chat_stream(&messages, &tools).await {
                    Ok(rx) => break rx,
                    Err(e) => {
                        _last_error = Some(e.clone());
                        log::error!("LLM 流式调用失败, session_id={}, 错误: {}", ctx.session_id, e.message);
                        
                        if !is_retryable_error(e.code) || llm_retry_count >= MAX_LLM_RETRIES {
                            self.emitter.emit_error(ErrorPayload {
                                session_id: ctx.session_id.clone(),
                                code: e.code,
                                message: "网络连接已断开，请检查网络后重试".to_string(),
                                recoverable: is_retryable_error(e.code),
                            }).ok();
                            return Err(e);
                        }
                        
                        llm_retry_count += 1;
                        let wait_secs = RETRY_DELAY_SECONDS * (1 << (llm_retry_count - 1));
                        log::info!("LLM 调用失败，等待 {} 秒后重试 (第 {}/{} 次), session_id={}", wait_secs, llm_retry_count, MAX_LLM_RETRIES, ctx.session_id);
                        
                        // 重试前：如果 Provider 被标记不可用，先尝试恢复
                        // 避免重试时因 Provider 不可用而直接失败
                        if e.code == crate::errors::LLM_PROVIDER_UNAVAILABLE {
                            log::info!("Provider 不可用，尝试恢复后重试, session_id={}", ctx.session_id);
                            self.router.force_recover_all().await;
                            self.router.rebuild_all_clients().await;
                        }
                        
                        self.emitter.emit_network_retry(NetworkRetryPayload {
                            session_id: ctx.session_id.clone(),
                            attempt: llm_retry_count,
                            max_attempts: MAX_LLM_RETRIES,
                            reason: e.message.clone(),
                        }).ok();
                        
                        tokio::time::sleep(Duration::from_secs(wait_secs)).await;
                    }
                }
            };

            // LLM 调用成功后才递增步骤计数
            total_steps += 1;

            // 收集流式响应
            let mut assistant_content = String::new();
            let mut reasoning_content = String::new();
            let mut collected_tool_calls: HashMap<u32, LlmToolCall> = HashMap::new();
            let mut message_id = String::new();
            // 跟踪流式响应的 finish_reason，用于检测响应截断（DeepSeek R1 等推理模型
            // 的 reasoning_content 可能耗尽 max_tokens 导致实际响应被截断）
            let mut finish_reason: Option<String> = None;
            // 追踪已在流式阶段提前发射 agent:tool_call 事件的工具索引
            // 避免 LLM 流式输出工具参数期间前端无反馈，也避免流结束后重复发射
            let mut early_announced_tool_indices: HashSet<u32> = HashSet::new();

            while let Some(chunk_result) = stream_rx.recv().await {
                match chunk_result {
                    Ok(chunk) => {
                        message_id = chunk.id.clone();
                        for choice in chunk.choices {
                            if let Some(rc) = &choice.delta.reasoning_content {
                                reasoning_content.push_str(rc);
                                self.emitter.emit_deep_thinking(DeepThinkingPayload {
                                    session_id: ctx.session_id.clone(),
                                    step: total_steps,
                                    thought: rc.clone(),
                                    is_streaming: true,
                                    iteration: Some(current_iteration),
                                }).ok();
                            }

                            if let Some(content) = &choice.delta.content {
                                assistant_content.push_str(content);
                                // 当已检测到 tool_call 时，不再发射 content 事件
                                // 避免前端在 tool_call 关闭 streaming 节点后，
                                // 又收到残余 content 创建新的重复节点
                                if early_announced_tool_indices.is_empty() {
                                    self.emitter.emit_content(ContentPayload {
                                        session_id: ctx.session_id.clone(),
                                        message_id: message_id.clone(),
                                        content: content.clone(),
                                        is_streaming: true,
                                        iteration: Some(current_iteration),
                                    }).ok();
                                }
                            }

                            // 收集 tool_calls 增量，按 index 合并
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

                                    // 尽早发射 tool_call 事件：当检测到工具名称时立即通知前端
                                    // 避免 LLM 流式输出工具参数（可能很长）期间前端无反馈
                                    if !early_announced_tool_indices.contains(&tc_index) {
                                        if let Some(collected) = collected_tool_calls.get(&tc_index) {
                                            if !collected.name.is_empty() {
                                                early_announced_tool_indices.insert(tc_index);
                                                let early_params = serde_json::from_str(&collected.arguments).unwrap_or(json!({}));
                                                log::debug!(
                                                    "流式阶段提前发射 tool_call 事件, session_id={}, tool={}, call_id={}",
                                                    ctx.session_id, collected.name, collected.id
                                                );
                                                self.emitter.emit_tool_call(ToolCallPayload {
                                                    session_id: ctx.session_id.clone(),
                                                    call_id: if collected.id.is_empty() { format!("streaming_{}", tc_index) } else { collected.id.clone() },
                                                    tool_name: collected.name.clone(),
                                                    arguments: early_params,
                                                    iteration: Some(current_iteration),
                                                }).ok();
                                            }
                                        }
                                    }
                                }
                            }

                            // 跟踪 finish_reason，用于检测响应截断
                            if choice.finish_reason.is_some() {
                                finish_reason = choice.finish_reason.clone();
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("流式响应错误: {}", e.message);
                        
                        if !is_retryable_error(e.code) {
                            self.emitter.emit_error(ErrorPayload {
                                session_id: ctx.session_id.clone(),
                                code: e.code,
                                message: format!("LLM 流式响应错误: {}", e.message),
                                recoverable: false,
                            }).ok();
                            break;
                        }
                        
                        if !assistant_content.is_empty() || !collected_tool_calls.is_empty() {
                            log::info!("流式响应中断但已有部分内容，尝试恢复, session_id={}, 内容长度={}, tool_calls数={}", ctx.session_id, assistant_content.len(), collected_tool_calls.len());
                            
                            self.emitter.emit_network_retry(NetworkRetryPayload {
                                session_id: ctx.session_id.clone(),
                                attempt: 1,
                                max_attempts: 1,
                                reason: "流式响应中断，尝试续写".to_string(),
                            }).ok();
                            
                            let recovered_messages = Self::build_recovery_messages(&messages, &assistant_content, &reasoning_content, &collected_tool_calls);
                            
                            match self.router.chat_stream(&recovered_messages, &tools).await {
                                Ok(new_rx) => {
                                    log::info!("流式恢复成功，继续接收, session_id={}", ctx.session_id);
                                    stream_rx = new_rx;
                                    continue;
                                }
                                Err(recover_err) => {
                                    log::error!("流式恢复失败: {}", recover_err.message);
                                    self.emitter.emit_error(ErrorPayload {
                                        session_id: ctx.session_id.clone(),
                                        code: recover_err.code,
                                        message: "网络连接已断开，请检查网络后重试".to_string(),
                                        recoverable: is_retryable_error(recover_err.code),
                                    }).ok();
                                    break;
                                }
                            }
                        } else {
                            // 可重试错误且无部分内容：尝试重新获取流式响应
                            // 不直接发射 agent:error，而是先尝试重连，避免向用户展示不必要的红色错误
                            log::info!("流式响应中断且无部分内容，尝试重新请求, session_id={}", ctx.session_id);
                            self.emitter.emit_network_retry(NetworkRetryPayload {
                                session_id: ctx.session_id.clone(),
                                attempt: 1,
                                max_attempts: 1,
                                reason: "流式响应中断，尝试重新请求".to_string(),
                            }).ok();

                            match self.router.chat_stream(&messages, &tools).await {
                                Ok(new_rx) => {
                                    log::info!("流式重新请求成功，继续接收, session_id={}", ctx.session_id);
                                    stream_rx = new_rx;
                                    continue;
                                }
                                Err(recover_err) => {
                                    log::error!("流式重新请求失败: {}", recover_err.message);
                                    self.emitter.emit_error(ErrorPayload {
                                        session_id: ctx.session_id.clone(),
                                        code: recover_err.code,
                                        message: "网络连接已断开，请检查网络后重试".to_string(),
                                        recoverable: is_retryable_error(recover_err.code),
                                    }).ok();
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            // 将 HashMap 转为按 index 排序的 Vec
            let mut collected_tool_calls: Vec<LlmToolCall> = collected_tool_calls.into_values()
                .collect::<Vec<_>>();
            collected_tool_calls.sort_by_key(|tc| tc.index);

            // 后处理：检测并清理 LLM content 中的 XML 标签和特殊 token
            // DeepSeek R1 等推理模型可能将 <agent-reasoning> 和 <tool-call>
            // 标签作为 content 输出（而非通过标准 tool_calls 字段），需要：
            // 1. 过滤 <agent-reasoning> 等内部推理标签（不应显示给用户）
            // 2. 从 <tool-call> 标签中提取工具调用信息（补充到 tool_calls）
            // 3. 清理特殊 token（如 <｜tool▁call▁end｜><｜tool▁calls▁end｜>）
            let (cleaned_content, extracted_tool_calls) = Self::sanitize_llm_content(&assistant_content);

            if cleaned_content != assistant_content {
                log::info!(
                    "已清理 LLM content 中的 XML 标签/特殊 token, session_id={}, 原始长度={}, 清理后长度={}",
                    ctx.session_id, assistant_content.len(), cleaned_content.len()
                );
                assistant_content = cleaned_content;
            }

            // 将从 content 中提取的 tool_calls 合并到已有列表
            for tc in extracted_tool_calls {
                let next_index = collected_tool_calls.iter()
                    .map(|t| t.index)
                    .max()
                    .map_or(0, |max_idx| max_idx + 1);
                collected_tool_calls.push(LlmToolCall {
                    index: next_index,
                    id: format!("extracted_{}", uuid::Uuid::new_v4()),
                    name: tc.name,
                    arguments: tc.arguments,
                });
            }

            // 检查是否有 tool_calls（需在发射最终 content 事件前判断）
            let has_tool_calls = !collected_tool_calls.is_empty();

            if !reasoning_content.is_empty() {
                self.emitter.emit_deep_thinking(DeepThinkingPayload {
                    session_id: ctx.session_id.clone(),
                    step: total_steps,
                    thought: String::new(),
                    is_streaming: false,
                    iteration: Some(current_iteration),
                }).ok();
            }

            // 发送流式结束事件，携带清理后的完整内容
            // 当存在 tool_calls 时不发射此事件，因为：
            // 1. 流式阶段已通过增量 content 事件将文本内容展示给用户
            // 2. 此时再发射 is_streaming=false 的完整内容会触发前端创建新的内容节点，导致重复显示
            // 3. 无 tool_calls 时仍需发射，以便前端清除之前流式显示的 XML 标签片段
            if !has_tool_calls {
                self.emitter.emit_content(ContentPayload {
                    session_id: ctx.session_id.clone(),
                    message_id: message_id.clone(),
                    content: assistant_content.clone(),
                    is_streaming: false,
                    iteration: Some(current_iteration),
                }).ok();
            }
            // 检测响应是否因 max_tokens 不足被截断（DeepSeek R1 等推理模型的
            // reasoning_content 可能消耗大量 token 导致实际响应被截断）
            let is_truncated = finish_reason.as_deref() == Some("length");
            log::debug!("LLM 响应解析完成, session_id={}, tool_calls数={}, 内容长度={}, finish_reason={:?}", ctx.session_id, collected_tool_calls.len(), assistant_content.len(), finish_reason);

            if has_tool_calls {
                // 将助手消息（含 tool_calls）添加到上下文
                ctx.add_assistant_message(&assistant_content, Some(collected_tool_calls.clone()), if reasoning_content.is_empty() { None } else { Some(reasoning_content.clone()) });

                // 如果响应被截断，tool_call 的 JSON 参数可能不完整
                if is_truncated {
                    log::warn!("LLM 响应被截断且包含 tool_calls, session_id={}, 检查参数完整性", ctx.session_id);
                }

                for tool_call in collected_tool_calls.iter() {
                    if let Some(result) = self.handle_stop_if_needed(
                        ctx,
                        total_steps,
                        start_time,
                    ) {
                        return Ok(result);
                    }

                    log::info!("执行 Tool, session_id={}, tool={}, call_id={}", ctx.session_id, tool_call.name, tool_call.id);

                    self.emit_todo_progress(
                        &ctx.session_id,
                        total_steps,
                        self.max_iterations,
                        &tool_call.name,
                    );

                    // 尝试解析 tool_call 参数，如果响应被截断则参数可能不完整
                    let params_result = serde_json::from_str::<serde_json::Value>(&tool_call.arguments);

                    // 截断响应时参数解析失败：跳过执行，反馈给LLM重新生成
                    // 必须发射 tool_result 事件关闭前端加载状态，同时发射 thinking 事件提示重试
                    if is_truncated && params_result.is_err() {
                        log::warn!(
                            "截断响应的 tool_call 参数解析失败, 跳过执行, session_id={}, tool={}, arguments长度={}",
                            ctx.session_id, tool_call.name, tool_call.arguments.len()
                        );
                        let retry_msg = format!(
                            "上一次 {} 调用的参数因响应被截断而不完整，请重新生成完整的代码。注意控制代码长度，确保参数完整。",
                            tool_call.name
                        );
                        // 发射思考事件，让用户看到重试提示
                        self.emitter.emit_thinking(ThinkingPayload {
                            session_id: ctx.session_id.clone(),
                            step: total_steps,
                            thought: "代码较长导致响应被截断，正在重新生成...".to_string(),
                        }).ok();
                        // 必须发射 tool_result 事件，否则前端对应节点永远显示加载动画
                        self.emitter.emit_tool_result(ToolResultPayload {
                            session_id: ctx.session_id.clone(),
                            call_id: tool_call.id.clone(),
                            success: false,
                            result: json!(null),
                            error: Some("响应被截断，正在重新生成代码...".to_string()),
                            duration_ms: 0,
                        }).ok();
                        // 将截断信息作为 tool_result 添加到对话上下文
                        ctx.add_tool_result(&tool_call.id, &retry_msg);
                        continue;
                    }

                    let params = params_result.unwrap_or(json!({}));

                    // 截断响应时，code_interpreter_skill 的 code 字段可能为空（参数部分截断）
                    // 此时也应跳过执行，避免无意义的确认弹窗
                    if is_truncated && tool_call.name == "code_interpreter_skill" {
                        let code_content = params["code"].as_str().unwrap_or("");
                        if code_content.is_empty() {
                            log::warn!(
                                "截断响应的 code_interpreter_skill 参数中 code 为空, 跳过执行, session_id={}",
                                ctx.session_id
                            );
                            // 发射思考事件，让用户看到重试提示
                            self.emitter.emit_thinking(ThinkingPayload {
                                session_id: ctx.session_id.clone(),
                                step: total_steps,
                                thought: "代码内容因响应被截断而缺失，正在重新生成...".to_string(),
                            }).ok();
                            // 必须发射 tool_result 事件，否则前端对应节点永远显示加载动画
                            self.emitter.emit_tool_result(ToolResultPayload {
                                session_id: ctx.session_id.clone(),
                                call_id: tool_call.id.clone(),
                                success: false,
                                result: json!(null),
                                error: Some("代码内容因响应被截断而缺失，正在重新生成...".to_string()),
                                duration_ms: 0,
                            }).ok();
                            let retry_msg = "代码内容因响应被截断而缺失，请重新生成完整的代码。注意控制代码长度，确保参数完整。".to_string();
                            ctx.add_tool_result(&tool_call.id, &retry_msg);
                            continue;
                        }
                    }

                    // 更新任务类型（基于已调用的工具）
                    ctx.update_task_type_from_tool(&tool_call.name, Some(&params));

                    // 记录当前执行的步骤
                    ctx.set_current_step(format!("执行 {}", tool_call.name));

                    if self.needs_confirmation(&tool_call.name, &params) {
                        // 高风险技能：始终发射 tool_call 事件
                        // 若流式阶段已提前发射，此处携带完整参数重新发射，前端通过 callId 去重更新
                        self.emitter.emit_tool_call(ToolCallPayload {
                            session_id: ctx.session_id.clone(),
                            call_id: tool_call.id.clone(),
                            tool_name: format!("{} (等待确认)", tool_call.name),
                            arguments: params.clone(),
                            iteration: Some(current_iteration),
                        }).ok();

                        let approved = self.request_confirmation(
                            &ctx.session_id,
                            &tool_call.name,
                            &params,
                        ).await?;

                        if !approved {
                            let skip_msg = format!("用户拒绝了操作: {}", tool_call.name);
                            log::info!("操作被拒绝: session_id={}, tool={}", ctx.session_id, tool_call.name);

                            self.emitter.emit_tool_result(ToolResultPayload {
                                session_id: ctx.session_id.clone(),
                                call_id: tool_call.id.clone(),
                                success: false,
                                result: json!(null),
                                error: Some(skip_msg.clone()),
                                duration_ms: 0,
                            }).ok();

                            ctx.add_tool_result(&tool_call.id, &skip_msg);
                            continue;
                        }
                    } else {
                        // 普通工具：始终发射 tool_call 事件
                        // 若流式阶段已提前发射，此处携带完整参数重新发射，前端通过 callId 去重更新
                        self.emitter.emit_tool_call(ToolCallPayload {
                            session_id: ctx.session_id.clone(),
                            call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                            arguments: params.clone(),
                            iteration: Some(current_iteration),
                        }).ok();
                    }

                    let tool_start = std::time::Instant::now();

                    // 先查 ToolRegistry（基础操作优先），再查 SkillRegistry（高级技能）
                    let tool_arc = self.tool_registry.get_arc(&tool_call.name);
                    let skill_arc = if tool_arc.is_none() {
                        let reg = self.registry.lock().await;
                        reg.get_arc(&tool_call.name)
                    } else {
                        None
                    };

                    // 对需要路径安全校验的 Tool/Skill，注入工作区根目录
                    let mut safe_params = params;
                    let needs_workspace_root = matches!(
                        tool_call.name.as_str(),
                        "list_directory" | "search_files" | "read_file" | "file_info"
                        | "file_exists" | "delete_file" | "create_directory" | "write_text_file"
                        | "docx_skill" | "xlsx_skill" | "pptx_skill" | "pdf_skill"
                        | "code_interpreter_skill"  // 需要workspace_root作为working_dir
                    );
                    if needs_workspace_root && !ctx.workspace_path.is_empty() {
                        safe_params["workspace_root"] = json!(ctx.workspace_path);
                    }

                    // 在文件修改/删除操作前自动创建版本快照
                    if let Some(ref snapshot_fn) = self.snapshot_fn {
                        let files_to_snapshot = self.extract_snapshot_paths(&tool_call.name, &safe_params);
                        for file_path in &files_to_snapshot {
                            if !file_path.is_empty() {
                                let operation = match tool_call.name.as_str() {
                                    "delete_file" => "delete",
                                    "docx_skill" | "xlsx_skill" | "pptx_skill" | "pdf_skill" => "read",
                                    "code_interpreter_skill" => "code_execute",
                                    _ => "unknown",
                                };
                                match snapshot_fn(&ctx.workspace_id, &ctx.session_id, file_path, operation) {
                                    Ok(_) => {
                                        log::info!("版本快照已创建: file={}, operation={}", file_path, operation);
                                    }
                                    Err(e) => {
                                        log::warn!("版本快照创建失败: file={}, 错误: {}", file_path, e.message);
                                    }
                                }
                            }
                        }
                    }

                    // 执行 Tool 或 Skill
                    let result = if let Some(tool) = tool_arc {
                        // 执行 Tool
                        let fut = std::panic::AssertUnwindSafe(tool.execute(safe_params));
                        match fut.catch_unwind().await {
                            Ok(r) => crate::models::skill::SkillResult {
                                success: r.success,
                                output: r.output,
                                error: r.error,
                                duration_ms: r.duration_ms,
                            },
                            Err(_) => {
                                log::error!("Tool 执行发生 panic: tool={}", tool_call.name);
                                crate::models::skill::SkillResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!("工具执行发生内部错误: {}", tool_call.name)),
                                    duration_ms: 0,
                                }
                            }
                        }
                    } else if let Some(skill) = skill_arc {
                        // 执行 Skill
                        let fut = std::panic::AssertUnwindSafe(skill.execute(safe_params));
                        match fut.catch_unwind().await {
                            Ok(r) => r,
                            Err(_) => {
                                log::error!("Skill 执行发生 panic: tool={}", tool_call.name);
                                crate::models::skill::SkillResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!("技能执行发生内部错误: {}", tool_call.name)),
                                    duration_ms: 0,
                                }
                            }
                        }
                    } else {
                        crate::models::skill::SkillResult {
                            success: false,
                            output: None,
                            error: Some(format!("工具或技能不存在: {}", tool_call.name)),
                            duration_ms: 0,
                        }
                    };

                    let duration_ms = tool_start.elapsed().as_millis() as u64;
                    log::debug!("Tool 执行完成, session_id={}, tool={}, 成功={}, 耗时={}ms", ctx.session_id, tool_call.name, result.success, duration_ms);

                    self.emitter.emit_tool_result(ToolResultPayload {
                        session_id: ctx.session_id.clone(),
                        call_id: tool_call.id.clone(),
                        success: result.success,
                        result: result.output.clone().unwrap_or(json!(null)),
                        error: result.error.clone(),
                        duration_ms,
                    }).ok();

                    // 将工具结果添加到上下文
                    let result_content = if result.success {
                        serde_json::to_string(&result.output).unwrap_or_default()
                    } else {
                        format!("错误: {}", result.error.clone().unwrap_or_default())
                    };
                    ctx.add_tool_result(&tool_call.id, &result_content);

                    // 记录已完成的步骤
                    let step_desc = if result.success {
                        format!("{} - 成功", tool_call.name)
                    } else {
                        format!("{} - 失败: {}", tool_call.name, result.error.as_deref().unwrap_or("未知错误"))
                    };
                    ctx.record_completed_step(step_desc);
                }

                // 每轮迭代后增量持久化，防止崩溃丢失消息
                self.persist_new_messages(ctx);
                ctx.mark_persisted();

                // 有 tool_calls 的迭代完成后发射上下文使用情况
                let response_tokens = crate::services::agent::prompts::token_budget::TokenBudgetManager::estimate_tokens(&assistant_content);
                self.emit_context_usage(ctx, response_tokens).await;

                // 继续循环，让 LLM 处理工具结果
                continue;
            }

            // 无 tool_calls：判断是否应该结束还是继续循环

            // 情况1: 响应被截断（finish_reason == "length"，max_tokens 不足）
            // DeepSeek R1 等推理模型的 reasoning_content 可能耗尽 token 配额，
            // 导致实际回复内容或 tool_calls 被截断。需要自动继续循环让 LLM 补充输出。
            if is_truncated {
                log::warn!(
                    "LLM 响应被截断 (finish_reason=length), 自动继续, session_id={}, 已收集内容长度={}",
                    ctx.session_id, assistant_content.len()
                );
                ctx.add_assistant_message(
                    &assistant_content,
                    None,
                    if reasoning_content.is_empty() { None } else { Some(reasoning_content.clone()) }
                );
                self.persist_new_messages(ctx);
                ctx.mark_persisted();
                continue;
            }

            // 情况2: 仅有 reasoning_content，无 content 和 tool_calls
            // LLM 只输出了思考链但没有产生实际回复或工具调用，需要继续循环
            if assistant_content.is_empty() {
                if !reasoning_content.is_empty() {
                    log::warn!(
                        "LLM 仅返回推理内容无最终输出, 自动继续, session_id={}",
                        ctx.session_id
                    );
                    ctx.add_assistant_message(
                        "",
                        None,
                        Some(reasoning_content.clone())
                    );
                } else {
                    log::warn!(
                        "LLM 返回完全空响应, 自动继续, session_id={}",
                        ctx.session_id
                    );
                }
                self.persist_new_messages(ctx);
                ctx.mark_persisted();
                continue;
            }

            // 情况3: 有实际内容，正常完成
            ctx.add_assistant_message(&assistant_content, None, if reasoning_content.is_empty() { None } else { Some(reasoning_content.clone()) });

            // 最终回复后增量持久化
            self.persist_new_messages(ctx);
            ctx.mark_persisted();

            // 正常完成前发射上下文使用情况
            let response_tokens = crate::services::agent::prompts::token_budget::TokenBudgetManager::estimate_tokens(&assistant_content);
            self.emit_context_usage(ctx, response_tokens).await;

            self.emitter.emit_todo_update(TodoUpdatePayload {
                session_id: ctx.session_id.clone(),
                todos: vec![TodoItem {
                    id: "done".to_string(),
                    content: "任务完成".to_string(),
                    status: "completed".to_string(),
                }],
            }).ok();

            let total_duration_ms = start_time.elapsed().as_millis() as u64;
            log::info!("Agent 执行完成, session_id={}, 总步骤={}, 总耗时={}ms", ctx.session_id, total_steps, total_duration_ms);
            self.emitter.emit_done(DonePayload {
                session_id: ctx.session_id.clone(),
                summary: assistant_content.clone(),
                total_steps,
                duration_ms: total_duration_ms,
            }).ok();

            return Ok(ExecutionResult {
                summary: assistant_content,
                total_steps,
                duration_ms: total_duration_ms,
            });
        }

        // 超过最大迭代次数
        let error = CommandError::agent(crate::errors::AGENT_MAX_ITERATIONS, format!("Agent 执行超过最大迭代次数 ({})", self.max_iterations));
        log::error!("Agent 执行超过最大迭代次数, session_id={}, max_iterations={}", ctx.session_id, self.max_iterations);
        self.emitter.emit_error(ErrorPayload {
            session_id: ctx.session_id.clone(),
            code: error.code,
            message: error.message.clone(),
            recoverable: false,
        }).ok();

        Err(error)
    }
}

/// 从 LLM content 中提取的工具调用信息
struct ExtractedToolCall {
    name: String,
    arguments: String,
}

impl<R: Runtime> AgentExecutor<R> {
    /// 构建流式恢复的消息列表
    /// 当流式响应因网络错误中断但已有部分内容时，构造续写请求让 LLM 继续生成
    fn build_recovery_messages(
        original_messages: &[ChatMessage],
        content: &str,
        reasoning: &str,
        tool_calls: &HashMap<u32, LlmToolCall>,
    ) -> Vec<ChatMessage> {
        let mut messages = original_messages.to_vec();

        let tool_calls_vec: Vec<LlmToolCall> = tool_calls.values().cloned().collect();

        messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: content.to_string(),
            content_parts: None,
            reasoning_content: if reasoning.is_empty() { None } else { Some(reasoning.to_string()) },
            tool_calls: if tool_calls_vec.is_empty() { None } else { Some(tool_calls_vec) },
            tool_call_id: None,
            attachments: None,
        });

        messages.push(ChatMessage {
            role: "user".to_string(),
            content: "请继续完成之前的回复，不要重复已输出的内容。".to_string(),
            content_parts: None,
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
            attachments: None,
        });

        messages
    }

    /// 清理 LLM content 中的 XML 标签和特殊 token，并尝试提取嵌入的 tool_call
    ///
    /// DeepSeek R1 等推理模型有时会将 <agent-reasoning> 和 <tool-call> 标签
    /// 作为 content 字段输出（而非通过标准 tool_calls 字段），此方法负责：
    /// 1. 过滤 <agent-reasoning> 等内部推理标签（不应显示给用户）
    /// 2. 从 <tool-call> 标签中提取工具调用信息
    /// 3. 清理特殊 token（如 <｜tool▁call▁end｜><｜tool▁calls▁end｜>）
    ///
    /// 返回 (清理后的 content, 提取的 tool_calls 列表)
    fn sanitize_llm_content(content: &str) -> (String, Vec<ExtractedToolCall>) {
        let mut result = content.to_string();
        let mut extracted_calls = Vec::new();

        // 步骤1：提取并移除 <tool-call> 标签中的工具调用
        // 匹配格式：<tool-call>\n```json\n{...}\n```\n</tool-call>
        // 或 <tool-call>\n```json\n{"function": "xxx", "arguments": {...}}\n```\n</tool-call>
        result = Self::extract_and_remove_tool_call_tags(&result, &mut extracted_calls);

        // 步骤2：移除 <agent-reasoning> 标签及其内容
        result = Self::remove_xml_tag_with_content(&result, "agent-reasoning");

        // 步骤3：移除其他已知的 LLM 内部标签
        for tag in &["think", "reflection", "scratchpad"] {
            result = Self::remove_xml_tag_with_content(&result, tag);
        }

        // 步骤4：清理特殊 token
        // DeepSeek R1 模型可能输出 <｜tool▁call▁end｜> 和 <｜tool▁calls▁end｜> 等特殊 token
        result = Self::remove_special_tokens(&result);

        // 步骤5：清理残留空行（连续多个空行压缩为最多两个换行）
        while result.contains("\n\n\n") {
            result = result.replace("\n\n\n", "\n\n");
        }

        // 步骤6：去除首尾空白
        result = result.trim().to_string();

        (result, extracted_calls)
    }

    /// 提取并移除 <tool-call>...</tool-call> 块，从中解析出工具调用信息
    /// 也处理未闭合的 <tool-call> 标签（DeepSeek R1 等模型可能用特殊 token 替代闭合标签）
    fn extract_and_remove_tool_call_tags(content: &str, extracted: &mut Vec<ExtractedToolCall>) -> String {
        let open_tag = "<tool-call>";
        let close_tag = "</tool-call>";
        let mut result = content.to_string();
        let mut search_from = 0;

        while let Some(pos) = result[search_from..].find(open_tag) {
            let start = search_from + pos;

            // 尝试查找闭合标签
            let (block_end, content_end) = if let Some(pos) = result[start + open_tag.len()..].find(close_tag) {
                // 正常闭合：block_end 是闭合标签结束位置，content_end 是内容结束位置
                (start + open_tag.len() + pos + close_tag.len(), start + open_tag.len() + pos)
            } else {
                // 未闭合：尝试在特殊 token 之前截断内容
                // DeepSeek R1 可能输出 <tool-call>...<｜tool▁call▁end｜> 而非 <tool-call>...</tool-call>
                let after_open = &result[start + open_tag.len()..];
                let (content_end_offset, block_end_offset) = Self::find_tool_call_content_end(after_open);
                (start + open_tag.len() + block_end_offset, start + open_tag.len() + content_end_offset)
            };

            // 提取块内容
            let block_content = result[start + open_tag.len()..content_end].to_string();
            // 从代码块中提取 JSON 内容
            if let Some(json_str) = Self::extract_json_from_code_block(&block_content) {
                if let Some(tc) = Self::parse_tool_call_json(&json_str) {
                    extracted.push(tc);
                }
            }

            result = format!("{}{}", &result[..start], &result[block_end..]);
            search_from = start.min(result.len());
        }

        result
    }

    /// 在未闭合的 <tool-call> 内容中查找有效内容的结束位置
    /// 返回 (content_end, block_end)，其中：
    /// - content_end: 有效内容（JSON）的结束位置
    /// - block_end: 整个块的结束位置（包含特殊 token），用于从原文中移除
    fn find_tool_call_content_end(content: &str) -> (usize, usize) {
        // 已知的特殊 token 模式（用于定位内容边界）
        let special_patterns: &[&str] = &[
            "<｜tool▁call▁end｜>",
            "<｜tool▁calls▁end｜>",
            "<|tool_call_end|>",
            "<|tool_calls_end|>",
        ];

        // 查找最早出现的特殊 token
        let (special_pos, special_end) = special_patterns.iter()
            .filter_map(|pattern| {
                content.find(pattern).map(|pos| (pos, pos + pattern.len()))
            })
            .min_by_key(|(pos, _)| *pos)
            .unwrap_or((content.len(), content.len()));

        // 查找代码块结束标记（第二个 ```）
        let code_block_content_end = content.find("```")
            .map(|first_pos| {
                content[first_pos + 3..].find("```")
                    .map(|p| first_pos + 3 + p)
                    .unwrap_or(first_pos + 3)
            })
            .unwrap_or(content.len());

        // content_end 取特殊token位置和代码块内容结束位置的较小值
        let content_end = special_pos.min(code_block_content_end);
        // block_end 取特殊token结束位置和代码块内容结束位置的较大值
        let block_end = special_end.max(code_block_content_end);

        (content_end, block_end)
    }

    /// 从代码块内容中提取 JSON 字符串（去除 ```json 和 ``` 包裹）
    fn extract_json_from_code_block(block_content: &str) -> Option<String> {
        let trimmed = block_content.trim();
        if !trimmed.starts_with("```") {
            return Some(trimmed.to_string());
        }
        let after_open = trimmed[3..].trim();
        let inner = if let Some(stripped) = after_open.strip_prefix("json") {
            stripped.trim()
        } else {
            after_open
        };
        if let Some(close_pos) = inner.rfind("```") {
            Some(inner[..close_pos].to_string())
        } else {
            Some(inner.to_string())
        }
    }

    /// 移除指定名称的 XML 标签及其内容
    fn remove_xml_tag_with_content(content: &str, tag_name: &str) -> String {
        let open_tag = format!("<{}>", tag_name);
        let close_tag = format!("</{}>", tag_name);
        let mut result = content.to_string();

        while let Some(start) = result.find(&open_tag) {
            let end = match result[start + open_tag.len()..].find(&close_tag) {
                Some(pos) => start + open_tag.len() + pos + close_tag.len(),
                None => start + open_tag.len(),
            };
            result = format!("{}{}", &result[..start], &result[end..]);
        }

        result
    }

    /// 清理特殊 token（全角和半角版本）
    /// 仅移除已知的 LLM 特殊 token 模式，避免误匹配正常文本
    fn remove_special_tokens(content: &str) -> String {
        let mut result = content.to_string();
        // 已知的 DeepSeek R1 特殊 token 模式（全角版本）
        let fullwidth_patterns = &[
            "<｜tool▁calls▁begin｜>",
            "<｜tool▁call▁begin｜>",
            "<｜tool▁call▁end｜>",
            "<｜tool▁calls▁end｜>",
        ];
        for pattern in fullwidth_patterns {
            result = result.replace(*pattern, "");
        }
        // 已知的半角版本特殊 token
        let halfwidth_patterns = &[
            "<|tool_calls_begin|>",
            "<|tool_call_begin|>",
            "<|tool_call_end|>",
            "<|tool_calls_end|>",
        ];
        for pattern in halfwidth_patterns {
            result = result.replace(*pattern, "");
        }
        result
    }

    /// 从 JSON 字符串中解析工具调用信息
    /// 支持两种格式：
    /// 1. {"function": "tool_name", "arguments": {...}}
    /// 2. {"name": "tool_name", "arguments": {...}}
    fn parse_tool_call_json(json_str: &str) -> Option<ExtractedToolCall> {
        let value: serde_json::Value = serde_json::from_str(json_str).ok()?;

        // 尝试从 "function" 或 "name" 字段获取工具名称
        let name = value.get("function")
            .or_else(|| value.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("").to_string();

        if name.is_empty() {
            return None;
        }

        // 获取 arguments，可能是对象或字符串
        let arguments = if let Some(args) = value.get("arguments") {
            if args.is_object() {
                serde_json::to_string(args).unwrap_or_default()
            } else if args.is_string() {
                args.as_str().unwrap_or("").to_string()
            } else {
                serde_json::to_string(args).unwrap_or_default()
            }
        } else {
            "{}".to_string()
        };

        Some(ExtractedToolCall { name, arguments })
    }
}
