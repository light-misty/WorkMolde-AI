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
use crate::models::llm::{ChatMessage, ChatUsage, LlmToolCall};
use crate::services::llm::router::LlmRouter;
use crate::services::handler::registry::HandlerRegistry;
use crate::services::tool::registry::ToolRegistry;
use crate::ConfirmDecision;
use super::context::AgentContext;

const MAX_LLM_RETRIES: u32 = 2;
const RETRY_DELAY_SECONDS: u64 = 2;
/// 确认操作超时时间（秒）
const CONFIRM_TIMEOUT_SECS: u64 = 300;
/// 始终需要确认的高风险 Handler 列表
const HIGH_RISK_HANDLERS: &[&str] = &["delete_file"];
/// 截断重试最大次数（每次翻倍 max_tokens）
const MAX_TRUNCATION_RETRIES: u32 = 2;
/// 截断重试时 max_tokens 的最大上限
const MAX_TOKENS_CEILING: u32 = 131072;
/// 缓存友好：工具结果最大字符数，超过此长度的结果会被截断
/// 大工具结果（如 read_file 的文件内容）会占据大量对话历史 token，
/// 且每次读取内容不同导致缓存无法命中，截断后缓存命中率显著提升
const MAX_TOOL_RESULT_CHARS: usize = 6000;

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
    registry: Arc<tokio::sync::Mutex<HandlerRegistry>>,
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
        registry: Arc<tokio::sync::Mutex<HandlerRegistry>>,
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

    /// 获取当前 Provider 的 max_tokens 配置
    async fn get_current_max_tokens(&self) -> u32 {
        self.router.get_default_max_tokens().await
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

    /// 在流式阶段的不完整 JSON 字符串中查找 "code" 字段值的起始位置
    /// LLM 输出格式通常为 {"code":"xxx","description":"xxx"}
    /// 此方法查找 "code" 键后紧跟的 ":" 和引号，返回值内容的起始字节偏移
    /// 返回 None 表示尚未找到 "code" 键的值起始位置
    fn find_code_value_start(json_str: &str) -> Option<usize> {
        // 查找 "code" 键
        let key_pattern = "\"code\"";
        let key_pos = json_str.find(key_pattern)?;
        let after_key = &json_str[key_pos + key_pattern.len()..];
        // 使用 char_indices 遍历，跳过空白找到冒号和引号
        let mut chars = after_key.char_indices().peekable();
        // 跳过空白
        while let Some(&(_, c)) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }
        // 期望冒号
        if chars.peek().map(|&(_, c)| c) != Some(':') {
            return None;
        }
        chars.next(); // 消费冒号
        // 跳过冒号后的空白
        while let Some(&(_, c)) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }
        // 期望双引号（JSON 标准使用双引号）
        if chars.peek().map(|&(_, c)| c) != Some('"') {
            return None;
        }
        chars.next(); // 消费开头引号
        // 计算值内容在原始字符串中的起始字节偏移
        let value_start_in_after_key = chars.peek().map(|&(i, _)| i).unwrap_or(after_key.len());
        Some(key_pos + key_pattern.len() + value_start_in_after_key)
    }

    /// 反转义 JSON 字符串中的转义序列
    /// 将 \n → 换行, \t → 制表符, \" → 双引号, \\ → 反斜杠 等
    /// 遇到未闭合的转义序列（如末尾单独的 \）时安全截断
    fn unescape_json_string(raw: &str) -> String {
        let mut result = String::with_capacity(raw.len());
        let mut chars = raw.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => result.push('\n'),
                    Some('t') => result.push('\t'),
                    Some('r') => result.push('\r'),
                    Some('"') => result.push('"'),
                    Some('\\') => result.push('\\'),
                    Some('/') => result.push('/'),
                    Some('b') => result.push('\x08'),
                    Some('f') => result.push('\x0C'),
                    Some('u') => {
                        // Unicode 转义: \uXXXX
                        let hex: String = chars.by_ref().take(4).collect();
                        if let Ok(code) = u32::from_str_radix(&hex, 16) {
                            if let Some(ch) = char::from_u32(code) {
                                result.push(ch);
                            }
                        }
                    }
                    Some(other) => result.push(other), // 未知转义，保留原字符
                    None => result.push('\\'),         // 末尾单独的 \，保留
                }
            } else if c == '"' {
                // 遇到闭合引号，说明 code 字符串值结束
                break;
            } else {
                result.push(c);
            }
        }
        result
    }

    /// 检查是否为高风险操作（需要用户确认）
    /// 根据确认级别决定哪些操作需要用户确认：
    /// - Never: 任何操作都不需要确认
    /// - EditOnly: 仅高风险操作需要确认
    /// - Always: 所有 Handler/Tool 调用都需要确认
    fn needs_confirmation(&self, name: &str, params: &serde_json::Value) -> bool {
        // code_interpreter_handler 始终不需要确认（代码自动执行）
        if name == "code_interpreter_handler" {
            return false;
        }
        match self.confirmation_level {
            ConfirmationLevel::Never => false,
            ConfirmationLevel::EditOnly => {
                // 仅编辑/删除操作需要确认
                if HIGH_RISK_HANDLERS.contains(&name) {
                    return true;
                }
                // write_text_file 在覆盖模式（非追加）下属于修改操作
                if name == "write_text_file" && !params.get("append").and_then(|v| v.as_bool()).unwrap_or(false) {
                    return true;
                }
                false
            }
            ConfirmationLevel::Always => true,
        }
    }

    /// 从 Handler 参数中提取需要创建快照的文件路径列表
    /// delete_file: 单文件路径
    /// write_text_file（覆盖模式）: 单文件路径
    /// 文档 Handler（docx_handler/xlsx_handler/pptx_handler/pdf_handler）: 精简后不再有 modify 操作，无需快照
    fn extract_snapshot_paths(&self, handler_name: &str, params: &serde_json::Value) -> Vec<String> {
        match handler_name {
            "delete_file" => {
                vec![params["path"].as_str().unwrap_or("").to_string()]
            }
            "write_text_file" => {
                let append = params.get("append").and_then(|v| v.as_bool()).unwrap_or(false);
                if !append {
                    vec![params["path"].as_str().unwrap_or("").to_string()]
                } else {
                    Vec::new()
                }
            }
            "docx_handler" | "xlsx_handler" | "pptx_handler" | "pdf_handler" => {
                // 文档 Handler 精简后不再有 modify 操作，无需创建快照
                Vec::new()
            }
            "code_interpreter_handler" => {
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
            "docx_handler" | "xlsx_handler" | "pptx_handler" | "pdf_handler" => {
                let action = arguments["action"].as_str().unwrap_or("操作");
                let path = arguments["path"].as_str().unwrap_or("未知文件");
                format!("{} - {}: {}", tool_name, action, path)
            }
            _ => format!("执行操作: {}", tool_name),
        };

        // 先创建 channel 并插入 map，再发射事件，避免竞态条件
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut channels = self.confirm_channels.lock().await;
            channels.insert(operation_id.clone(), tx);
        }

        if self.emitter.emit_confirm(ConfirmPayload {
            session_id: session_id.to_string(),
            operation_id: operation_id.clone(),
            operation_type: tool_name.to_string(),
            description,
            details: arguments.clone(),
            risk_level: risk_level.to_string(),
        }).is_err() {
            // 发射事件失败，清理通道，避免泄漏
            let mut channels = self.confirm_channels.lock().await;
            channels.remove(&operation_id);
            return Err(CommandError::new(
                crate::errors::RUNTIME_EVENT_EMIT_ERROR,
                "发射确认事件失败",
            ));
        }

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
    async fn emit_context_usage(&self, ctx: &mut AgentContext, response_tokens: usize, usage: Option<&ChatUsage>) {
        let model_name = self.router.current_model_name();
        let cache_type = self.router.current_cache_type().to_string();

        // 缓存诊断：记录本轮和累计的缓存命中统计
        if let Some(u) = usage {
            let total = u.prompt_cache_hit_tokens + u.prompt_cache_miss_tokens;
            let hit_rate = if total > 0 { u.prompt_cache_hit_tokens as f64 / total as f64 * 100.0 } else { 0.0 };
            log::info!(
                "缓存诊断: session_id={}, provider_cache_type={}, 本轮缓存命中={}/{}({:.1}%), 累计命中率={:.1}%",
                ctx.session_id, cache_type,
                u.prompt_cache_hit_tokens, total, hit_rate,
                {
                    let lt = ctx.lifetime_cache_hit_tokens + u.prompt_cache_hit_tokens;
                    let lm = ctx.lifetime_cache_miss_tokens + u.prompt_cache_miss_tokens;
                    let lt_total = lt + lm;
                    if lt_total > 0 { lt as f64 / lt_total as f64 * 100.0 } else { 0.0 }
                },
            );
        }

        let usage_info = ctx.calculate_context_usage(response_tokens, model_name, cache_type, usage);

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

        // 合并 Tool + Handler 的工具定义
        let tool_defs_json = {
            let tool_defs = self.tool_registry.tool_definitions();
            let handler_defs = {
                let reg = self.registry.lock().await;
                reg.tool_definitions()
            };
            let mut all = [tool_defs, handler_defs].concat();
            // 按 function.name 字母序稳定排序，确保相同工具集产生相同 JSON 序列化
            all.sort_by(|a, b| {
                let name_a = a["function"]["name"].as_str().unwrap_or("");
                let name_b = b["function"]["name"].as_str().unwrap_or("");
                name_a.cmp(name_b)
            });
            all
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

        // 缓存诊断：记录工具定义序列化稳定性（若工具定义 JSON 在各会话间不一致，将导致缓存持续未命中）
        // 使用 char_indices 安全地截取前 80 个字符，避免切割多字节 UTF-8 字符导致 panic
        let safe_prefix = func_defs_str.chars().take(80).collect::<String>();
        log::debug!(
            "缓存诊断: 工具定义序列化={{长度={}, 工具数量={}, 前80字符={}}}",
            func_defs_str.len(),
            tools.len(),
            safe_prefix,
        );

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
                    return Ok(self.handle_stop_if_needed(ctx, total_steps, start_time).expect("check_stopped 返回 true 但 handle_stop_if_needed 返回 None"));
                }
                
                // 缓存诊断：记录每次 LLM 调用的消息特征，便于分析跨迭代/跨会话缓存命中率
                let msg_summary: String = messages.iter()
                    .map(|m| m.role.chars().next().unwrap_or('?').to_string())
                    .collect::<Vec<_>>()
                    .join("");
                let estimated_prompt_tokens = crate::services::agent::prompts::token_budget::TokenBudgetManager::estimate_tokens(
                    &messages.iter().map(|m| format!("{}:{}", m.role, m.content)).collect::<String>()
                );
                log::debug!(
                    "缓存诊断: session_id={}, 迭代#{}, 消息模式={}, 消息数={}, 估算输入token={}",
                    ctx.session_id, current_iteration, msg_summary, messages.len(), estimated_prompt_tokens,
                );
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
            // 从流中捕获最后一个携带 usage 的 chunk（含缓存字段）
            let mut final_usage: Option<ChatUsage> = None;
            // 跟踪流式响应的 finish_reason，用于检测响应截断（DeepSeek R1 等推理模型
            // 的 reasoning_content 可能耗尽 max_tokens 导致实际响应被截断）
            let mut finish_reason: Option<String> = None;
            // 追踪已在流式阶段提前发射 agent:tool_call 事件的工具索引
            // 避免 LLM 流式输出工具参数期间前端无反馈，也避免流结束后重复发射
            let mut early_announced_tool_indices: HashSet<u32> = HashSet::new();
            // 代码流式状态：记录每个 code_interpreter_handler tool_call 已发射的 code 长度
            let mut code_streaming_state: HashMap<u32, usize> = HashMap::new();

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

                                    // 代码流式增量：当检测到 code_interpreter_handler 时，发射 code_streaming 事件
                                    // 注意：流式阶段 arguments 是不完整的 JSON，不能用 serde_json::from_str 解析
                                    // 改用字符串搜索定位 "code" 字段的值，提取后反转义 JSON 转义序列
                                    // 由于转义序列可能被流式分块切割，无法正确计算增量（delta），
                                    // 因此每次发射完整的反转义代码内容，前端直接替换而非追加
                                    if let Some(collected) = collected_tool_calls.get(&tc_index) {
                                        if collected.name == "code_interpreter_handler" {
                                            let args = &collected.arguments;
                                            if let Some(code_start) = Self::find_code_value_start(args) {
                                                let raw_code_so_far = &args[code_start..];
                                                // 反转义 JSON 转义序列（\n → 换行, \t → 制表符 等）
                                                let unescaped_code = Self::unescape_json_string(raw_code_so_far);
                                                // 用 raw 字节偏移判断是否有新内容（raw 偏移单调递增，不受转义影响）
                                                let prev_raw_len = code_streaming_state
                                                    .get(&tc_index)
                                                    .copied()
                                                    .unwrap_or(0);
                                                if raw_code_so_far.len() > prev_raw_len {
                                                    self.emitter.emit_code_streaming(CodeStreamingPayload {
                                                        session_id: ctx.session_id.clone(),
                                                        call_id: if collected.id.is_empty() {
                                                            format!("streaming_{}", tc_index)
                                                        } else {
                                                            collected.id.clone()
                                                        },
                                                        // 发射完整的反转义代码（非增量），前端直接替换
                                                        code_delta: unescaped_code,
                                                        is_final: false,
                                                    }).ok();
                                                    code_streaming_state.insert(tc_index, raw_code_so_far.len());
                                                }
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

                        // 保存最后一个包含 usage 的 chunk（含缓存字段）
                        if chunk.usage.is_some() {
                            final_usage = chunk.usage.clone();
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

            // 为所有 code_interpreter_handler 的 tool_call 发射 is_final 事件
            for (tc_index, collected) in collected_tool_calls.iter() {
                if collected.name == "code_interpreter_handler" {
                    self.emitter.emit_code_streaming(CodeStreamingPayload {
                        session_id: ctx.session_id.clone(),
                        call_id: if collected.id.is_empty() {
                            format!("streaming_{}", tc_index)
                        } else {
                            collected.id.clone()
                        },
                        code_delta: String::new(),
                        is_final: true,
                    }).ok();
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
            // 无论是否存在 tool_calls，只要有内容就发射此事件：
            // 1. 流式阶段可能因 early_announced_tool_indices 而未发射部分 content delta
            //    （LLM 可能在 tool_use 块之后继续输出文本内容块），导致前端显示截断
            // 2. 前端通过 is_streaming=false + iteration 定位已有 content 节点进行更新，
            //    不会创建重复节点
            // 3. 无 tool_calls 时仍需发射，以便前端清除之前流式显示的 XML 标签片段
            if !assistant_content.is_empty() {
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
            let mut is_truncated = finish_reason.as_deref() == Some("length");
            log::debug!("LLM 响应解析完成, session_id={}, tool_calls数={}, 内容长度={}, finish_reason={:?}", ctx.session_id, collected_tool_calls.len(), assistant_content.len(), finish_reason);

            if has_tool_calls {
                // 将助手消息（含 tool_calls）添加到上下文
                ctx.add_assistant_message(&assistant_content, Some(collected_tool_calls.clone()), if reasoning_content.is_empty() { None } else { Some(reasoning_content.clone()) });

                // 如果响应被截断，tool_call 的 JSON 参数可能不完整
                if is_truncated {
                    log::warn!("LLM 响应被截断且包含 tool_calls, session_id={}, 检查参数完整性", ctx.session_id);
                }

                // 截断重试：当响应被截断且 tool_call 参数解析失败时，
                // 回滚上下文，用翻倍的 max_tokens 重新调用 LLM
                if is_truncated {
                    let mut all_params_valid = true;
                    let mut has_empty_code = false;

                    for tool_call in collected_tool_calls.iter() {
                        let params_result = serde_json::from_str::<serde_json::Value>(&tool_call.arguments);
                        if params_result.is_err() {
                            all_params_valid = false;
                            log::warn!(
                                "截断响应的 tool_call 参数解析失败, session_id={}, tool={}, arguments长度={}",
                                ctx.session_id, tool_call.name, tool_call.arguments.len()
                            );
                            break;
                        }
                        // 检查 code_interpreter_handler 的 code 字段是否为空
                        let params = params_result.unwrap_or(json!({}));
                        if tool_call.name == "code_interpreter_handler"
                            && params["code"].as_str().unwrap_or("").is_empty() {
                                has_empty_code = true;
                                log::warn!(
                                    "截断响应的 code_interpreter_handler 参数中 code 为空, session_id={}",
                                    ctx.session_id
                                );
                                break;
                        }
                    }

                    if !all_params_valid || has_empty_code {
                        // 回滚刚添加的不完整 assistant message
                        ctx.pop_last_assistant_message();

                        // 为截断响应中已提前发射 tool_call 事件的节点发射关闭事件
                        // 避免前端节点永远处于加载状态
                        for tool_call in collected_tool_calls.iter() {
                            if early_announced_tool_indices.contains(&tool_call.index) {
                                log::debug!(
                                    "为截断的 tool_call 发射关闭事件, session_id={}, tool={}, call_id={}",
                                    ctx.session_id, tool_call.name, tool_call.id
                                );
                                self.emitter.emit_tool_result(ToolResultPayload {
                                    session_id: ctx.session_id.clone(),
                                    call_id: if tool_call.id.is_empty() { format!("streaming_{}", tool_call.index) } else { tool_call.id.clone() },
                                    success: false,
                                    result: json!(null),
                                    error: Some("响应被截断，正在增加输出限制重试...".to_string()),
                                    duration_ms: 0,
                                }).ok();
                            }
                        }

                        // 发射思考事件，让用户看到重试提示
                        self.emitter.emit_thinking(ThinkingPayload {
                            session_id: ctx.session_id.clone(),
                            step: total_steps,
                            thought: "输出被截断，正在增加输出限制重试...".to_string(),
                        }).ok();

                        // 用翻倍的 max_tokens 重试 LLM 调用
                        let mut truncation_retry_count = 0;
                        let mut current_max_tokens = self.get_current_max_tokens().await;

                        while truncation_retry_count < MAX_TRUNCATION_RETRIES {
                            truncation_retry_count += 1;
                            let new_max_tokens = std::cmp::min(current_max_tokens.saturating_mul(2), MAX_TOKENS_CEILING);
                            log::info!(
                                "截断重试 #{}, max_tokens: {} -> {}, session_id={}",
                                truncation_retry_count, current_max_tokens, new_max_tokens, ctx.session_id
                            );

                            if self.check_stopped(&ctx.session_id) {
                                return Ok(self.handle_stop_if_needed(ctx, total_steps, start_time).expect("check_stopped 返回 true 但 handle_stop_if_needed 返回 None"));
                            }

                            let retry_messages = ctx.get_messages_for_iteration(current_iteration);
                            match self.router.chat_stream_with_max_tokens(&retry_messages, &tools, new_max_tokens).await {
                                Ok(mut retry_rx) => {
                                    // 收集重试响应
                                    let mut retry_content = String::new();
                                    let mut retry_reasoning = String::new();
                                    let mut retry_tool_calls: HashMap<u32, LlmToolCall> = HashMap::new();
                                    let mut retry_finish_reason: Option<String> = None;
                                    let mut retry_message_id = String::new();

                                    while let Some(chunk_result) = retry_rx.recv().await {
                                        match chunk_result {
                                            Ok(chunk) => {
                                                retry_message_id = chunk.id.clone();
                                                for choice in chunk.choices {
                                                    if let Some(rc) = &choice.delta.reasoning_content {
                                                        retry_reasoning.push_str(rc);
                                                    }
                                                    if let Some(content) = &choice.delta.content {
                                                        retry_content.push_str(content);
                                                        // 截断重试时不发射流式 content 事件
                                                        // 避免与原始截断响应的 content 拼接导致重复
                                                        // 重试完成后统一发射 is_streaming=false 事件替换
                                                    }
                                                    if let Some(tool_calls) = &choice.delta.tool_calls {
                                                        for tc in tool_calls {
                                                            let entry = retry_tool_calls.entry(tc.index).or_insert_with(|| LlmToolCall {
                                                                index: tc.index,
                                                                id: tc.id.clone(),
                                                                name: tc.name.clone(),
                                                                arguments: String::new(),
                                                            });
                                                            if !tc.id.is_empty() { entry.id = tc.id.clone(); }
                                                            if !tc.name.is_empty() { entry.name = tc.name.clone(); }
                                                            entry.arguments.push_str(&tc.arguments);
                                                        }
                                                    }
                                                    if choice.finish_reason.is_some() {
                                                        retry_finish_reason = choice.finish_reason.clone();
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                log::error!("截断重试流式响应错误, session_id={}, 错误: {}", ctx.session_id, e.message);
                                                break;
                                            }
                                        }
                                    }

                                    let retry_is_truncated = retry_finish_reason.as_deref() == Some("length");
                                    let retry_has_tool_calls = !retry_tool_calls.is_empty();

                                    // 重试完成后发射 is_streaming=false 的 content 事件
                                    // 替换前端已有的原始截断响应内容，避免显示过时内容
                                    self.emitter.emit_content(ContentPayload {
                                        session_id: ctx.session_id.clone(),
                                        message_id: retry_message_id.clone(),
                                        content: retry_content.clone(),
                                        is_streaming: false,
                                        iteration: Some(current_iteration),
                                    }).ok();

                                    if !retry_reasoning.is_empty() {
                                        self.emitter.emit_deep_thinking(DeepThinkingPayload {
                                            session_id: ctx.session_id.clone(),
                                            step: total_steps,
                                            thought: String::new(),
                                            is_streaming: false,
                                            iteration: Some(current_iteration),
                                        }).ok();
                                    }

                                    log::debug!(
                                        "截断重试响应解析完成, session_id={}, tool_calls数={}, 内容长度={}, finish_reason={:?}",
                                        ctx.session_id, retry_tool_calls.len(), retry_content.len(), retry_finish_reason
                                    );

                                    if retry_has_tool_calls {
                                        // 将 HashMap 转为 Vec（按 index 排序）
                                        let mut retry_tc_vec: Vec<LlmToolCall> = retry_tool_calls.values().cloned().collect();
                                        retry_tc_vec.sort_by_key(|tc| tc.index);

                                        // 添加重试的 assistant message
                                        ctx.add_assistant_message(
                                            &retry_content,
                                            Some(retry_tc_vec.clone()),
                                            if retry_reasoning.is_empty() { None } else { Some(retry_reasoning.clone()) },
                                        );

                                        // 检查重试响应的参数完整性
                                        let mut retry_params_valid = true;
                                        let mut retry_empty_code = false;
                                        for tc in &retry_tc_vec {
                                            let pr = serde_json::from_str::<serde_json::Value>(&tc.arguments);
                                            if pr.is_err() {
                                                retry_params_valid = false;
                                                break;
                                            }
                                            let p = pr.unwrap_or(json!({}));
                                            if tc.name == "code_interpreter_handler"
                                                && p["code"].as_str().unwrap_or("").is_empty() {
                                                    retry_empty_code = true;
                                                    break;
                                            }
                                        }

                                        if !retry_is_truncated && retry_params_valid && !retry_empty_code {
                                            // 重试成功，用重试的 tool_calls 替换原来的
                                            collected_tool_calls = retry_tc_vec;
                                            assistant_content = retry_content;
                                            reasoning_content = retry_reasoning;
                                            is_truncated = false;
                                            log::info!("截断重试成功, session_id={}, 新max_tokens={}", ctx.session_id, new_max_tokens);
                                            break;
                                        } else {
                                            // 重试后仍然截断，回滚并继续重试
                                            ctx.pop_last_assistant_message();
                                            current_max_tokens = new_max_tokens;
                                            if truncation_retry_count >= MAX_TRUNCATION_RETRIES {
                                                log::warn!("截断重试次数耗尽, 降级为提示LLM重写, session_id={}", ctx.session_id);
                                                // 降级：将最后一次截断的响应作为上下文，让 LLM 重写
                                                ctx.add_assistant_message(
                                                    &retry_content,
                                                    Some(retry_tc_vec.clone()),
                                                    if retry_reasoning.is_empty() { None } else { Some(retry_reasoning) },
                                                );
                                                // 使用重试的 tool_calls 继续处理（会在下面的 for 循环中走旧的截断处理逻辑）
                                                collected_tool_calls = retry_tc_vec;
                                                assistant_content = retry_content;
                                                is_truncated = true;
                                                break;
                                            }
                                        }
                                    } else {
                                        // 重试后没有 tool_calls（LLM 直接回复了文本），直接使用
                                        // 这种情况不太常见，但作为安全处理
                                        collected_tool_calls.clear();
                                        assistant_content = retry_content;
                                        reasoning_content = retry_reasoning;
                                        is_truncated = false;
                                        break;
                                    }
                                }
                                Err(e) => {
                                    log::error!("截断重试 LLM 调用失败, session_id={}, 错误: {}", ctx.session_id, e.message);
                                    // 重试失败，降级为旧的截断处理逻辑
                                    break;
                                }
                            }
                        }
                    }
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

                    // 截断重试耗尽后仍解析失败的降级处理：
                    // 跳过执行，反馈给 LLM 重新生成
                    if is_truncated && params_result.is_err() {
                        log::warn!(
                            "截断响应的 tool_call 参数解析失败（重试耗尽）, 跳过执行, session_id={}, tool={}, arguments长度={}",
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
                            thought: "输出限制不足导致响应被截断，正在重新生成...".to_string(),
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

                    // 截断重试耗尽后 code_interpreter_handler 的 code 字段仍为空的降级处理
                    if is_truncated && tool_call.name == "code_interpreter_handler" {
                        let code_content = params["code"].as_str().unwrap_or("");
                        if code_content.is_empty() {
                            log::warn!(
                                "截断响应的 code_interpreter_handler 参数中 code 为空（重试耗尽）, 跳过执行, session_id={}",
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

                    // 先查 ToolRegistry（基础操作优先），再查 HandlerRegistry（文档处理器）
                    let tool_arc = self.tool_registry.get_arc(&tool_call.name);
                    let handler_arc = if tool_arc.is_none() {
                        let reg = self.registry.lock().await;
                        reg.get_arc(&tool_call.name)
                    } else {
                        None
                    };

                    // 对需要路径安全校验的 Tool/Handler，注入工作区根目录
                    let mut safe_params = params;
                    let needs_workspace_root = matches!(
                        tool_call.name.as_str(),
                        "list_directory" | "search_files" | "read_file" | "file_info"
                        | "file_exists" | "delete_file" | "create_directory" | "write_text_file"
                        | "docx_handler" | "xlsx_handler" | "pptx_handler" | "pdf_handler"
                        | "code_interpreter_handler"  // 需要workspace_root作为working_dir
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
                                    "docx_handler" | "xlsx_handler" | "pptx_handler" | "pdf_handler" => "read",
                                    "code_interpreter_handler" => "code_execute",
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

                    // 执行 Tool 或 Handler
                    let result = if let Some(tool) = tool_arc {
                        // 执行 Tool
                        let fut = std::panic::AssertUnwindSafe(tool.execute(safe_params));
                        match fut.catch_unwind().await {
                            Ok(r) => crate::models::handler::HandlerResult {
                                success: r.success,
                                output: r.output,
                                error: r.error,
                                duration_ms: r.duration_ms,
                            },
                            Err(_) => {
                                log::error!("Tool 执行发生 panic: tool={}", tool_call.name);
                                crate::models::handler::HandlerResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!("工具执行发生内部错误: {}", tool_call.name)),
                                    duration_ms: 0,
                                }
                            }
                        }
                    } else if let Some(handler) = handler_arc {
                        // 执行 Handler
                        let fut = std::panic::AssertUnwindSafe(handler.execute(safe_params));
                        match fut.catch_unwind().await {
                            Ok(r) => r,
                            Err(_) => {
                                log::error!("Handler 执行发生 panic: tool={}", tool_call.name);
                                crate::models::handler::HandlerResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!("处理器执行发生内部错误: {}", tool_call.name)),
                                    duration_ms: 0,
                                }
                            }
                        }
                    } else {
                        crate::models::handler::HandlerResult {
                            success: false,
                            output: None,
                            error: Some(format!("工具或处理器不存在: {}", tool_call.name)),
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
                    // 缓存优化：对大结果进行截断，避免巨量动态内容冲淡缓存命中率
                    let result_content = if result.success {
                        let output_val = result.output.as_ref().map(|v| {
                            // 如果是 JSON 对象且包含 content 字段，截断该字段
                            if let Some(obj) = v.as_object() {
                                if let Some(content_val) = obj.get("content") {
                                    if let Some(content_str) = content_val.as_str() {
                                        if content_str.len() > MAX_TOOL_RESULT_CHARS {
                                            let mut truncated = v.clone();
                                            let truncated_content = format!(
                                                "{}...\n[已截断: 原始内容 {} 字符，仅保留前 {} 字符]",
                                                &content_str[..MAX_TOOL_RESULT_CHARS],
                                                content_str.len(),
                                                MAX_TOOL_RESULT_CHARS,
                                            );
                                            if let Some(obj) = truncated.as_object_mut() {
                                                obj.insert(
                                                    "content".to_string(),
                                                    json!(truncated_content),
                                                );
                                            }
                                            return truncated;
                                        }
                                    }
                                }
                            }
                            v.clone()
                        });
                        let serialized = serde_json::to_string(&output_val).unwrap_or_default();
                        // 最终的字符串级安全截断（防止递归嵌套等极端情况）
                        // 使用 chars().take() 避免切割多字节 UTF-8 字符导致 panic
                        if serialized.len() > MAX_TOOL_RESULT_CHARS * 2 {
                            let safe_truncated: String = serialized.chars().take(MAX_TOOL_RESULT_CHARS * 2).collect();
                            format!("{}...\n[已截断: 工具结果过大，仅保留前 {} 字符]",
                                safe_truncated,
                                MAX_TOOL_RESULT_CHARS * 2)
                        } else {
                            serialized
                        }
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
                let response_tokens = if let Some(ref usage) = final_usage {
                    usage.completion_tokens as usize
                } else {
                    crate::services::agent::prompts::token_budget::TokenBudgetManager::estimate_tokens(&assistant_content)
                };
                self.emit_context_usage(ctx, response_tokens, final_usage.as_ref()).await;

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
            let response_tokens = if let Some(ref usage) = final_usage {
                usage.completion_tokens as usize
            } else {
                crate::services::agent::prompts::token_budget::TokenBudgetManager::estimate_tokens(&assistant_content)
            };
            self.emit_context_usage(ctx, response_tokens, final_usage.as_ref()).await;

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
