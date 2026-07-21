use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use serde_json::json;
use tauri::{Emitter, Runtime};

use super::compaction::ContextCompactor;
use super::context::AgentContext;
use super::is_document_handler;
use crate::config::app_settings::{CompactionConfig, ConfirmationLevel};
use crate::errors::CommandError;
use crate::events::emitter::AgentEmitter;
use crate::events::types::*;
use crate::models::llm::{ChatMessage, ChatUsage, ContentPart, LlmToolCall};
use crate::services::handler::registry::HandlerRegistry;
use crate::services::llm::router::LlmRouter;
use crate::services::permission::{
    doom_loop::DoomLoopDetector, evaluator::PermissionEvaluator, evaluator::PermissionRequest,
    registry::PermissionRegistry, types::PermissionAction, types::PermissionResponse,
    types::PermissionType,
};
use crate::services::tool::registry::ToolRegistry;
use crate::ConfirmDecision;

const MAX_LLM_RETRIES: u32 = 2;
const RETRY_DELAY_SECONDS: u64 = 2;
/// 确认操作超时时间（秒）
const CONFIRM_TIMEOUT_SECS: u64 = 300;
/// 始终需要确认的高风险 Handler 列表
const HIGH_RISK_HANDLERS: &[&str] = &["remove"];

// DOCUMENT_HANDLER_NAMES 和 is_document_handler 已提取到 mod.rs 共享，通过 super:: 引用

/// 判断 Shell 命令是否为高风险命令（需要用户确认）
/// 检测破坏性命令模式：rm/del/rmdir/mkfs/format/shutdown 等
fn is_high_risk_command(command: &str) -> bool {
    let lower = command.to_lowercase();
    // 危险命令关键字（前后需为单词边界，避免误判如 "format" 出现在 "formatter" 中）
    // 部分模式末尾带空格以避免误匹配（如 "del " 不应匹配 "delete"）
    let dangerous_patterns = [
        // 文件删除类
        "rm -rf",
        "rm -r",
        "rm -f",
        "rm ", // 匹配不带 flag 的 rm 命令(如 rm test.txt)
        "rmdir",
        "del /f",
        "del /q",
        "rd /s",
        "del ",
        // 磁盘/系统破坏类
        "mkfs",
        "format ",
        "shutdown",
        "reboot",
        "halt",
        "poweroff",
        // 提权执行类
        "sudo ",
        "su ",
        // 注册表操作类
        "reg delete",
        "reg add",
        // 进程终止类
        "killall",
        "taskkill /f",
        "taskkill /im",
        "kill -9",
        // 网络下载类
        "curl ",
        "wget ",
        // 管道执行类
        "| bash",
        "| sh",
        "| python",
        // 后台执行类
        "nohup",
        // Git 危险操作类
        "git push --force",
        "git push -f",
        "git reset --hard",
        "git clean -f",
        "git checkout .",
        "git restore .",
        // 设备写入类
        "dd if=",
        "> /dev/sd",
        "mv / ",
        "chmod -R 777",
    ];
    for pattern in &dangerous_patterns {
        if lower.contains(pattern) {
            return true;
        }
    }
    false
}
/// 截断重试最大次数（每次翻倍 max_tokens）
const MAX_TRUNCATION_RETRIES: u32 = 2;
/// 截断重试时 max_tokens 的最大上限
const MAX_TOKENS_CEILING: u32 = 131072;
/// 缓存友好：工具结果最大字符数，超过此长度的结果会被截断
/// 大工具结果（如 read_file 的文件内容）会占据大量对话历史 token，
/// 且每次读取内容不同导致缓存无法命中，截断后缓存命中率显著提升
const MAX_TOOL_RESULT_CHARS: usize = 6000;

/// 权限检查结果
#[derive(Debug, Clone)]
pub enum PermissionResult {
    /// 允许执行
    Allow,
    /// 允许执行（经过用户规则 Ask 弹窗批准后的 Allow）
    AllowWithPermissionAsked,
    /// 拒绝执行，附带拒绝原因
    Deny { reason: String },
}

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

/// 根据错误码生成面向用户友好的错误消息
fn user_facing_error_message(code: u32) -> String {
    match code {
        crate::errors::LLM_INVALID_REQUEST => "对话历史格式错误，请尝试新建会话".to_string(),
        crate::errors::LLM_AUTH_FAILED => "API 认证失败，请检查 API Key 配置".to_string(),
        crate::errors::LLM_RATE_LIMITED => "API 请求频率过高，请稍后重试".to_string(),
        crate::errors::LLM_QUOTA_EXCEEDED => "API 配额已用尽，请检查账户余额".to_string(),
        crate::errors::LLM_MODEL_NOT_FOUND => "模型不存在或已停用，请检查模型配置".to_string(),
        crate::errors::LLM_TIMEOUT => "请求超时，请检查网络连接后重试".to_string(),
        _ => {
            // 网络类错误统一提示
            "网络连接已断开，请检查网络后重试".to_string()
        }
    }
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
pub type ContextUsagePersistFn =
    Arc<dyn Fn(&str, &crate::models::llm::ContextUsageInfo) + Send + Sync>;

/// 版本快照回调类型
/// 接收 (workspace_id, session_id, file_path, operation)，在文件修改/删除前创建快照
type SnapshotFn = Arc<dyn Fn(&str, &str, &str, &str) -> Result<(), CommandError> + Send + Sync>;

pub struct AgentExecutor<R: Runtime> {
    router: Arc<LlmRouter>,
    tool_registry: Arc<ToolRegistry>,
    registry: Arc<tokio::sync::Mutex<HandlerRegistry>>,
    emitter: AgentEmitter<R>,
    confirm_channels:
        Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<ConfirmDecision>>>>,
    /// 权限审批通道（双态 once/reject）
    permission_channels: Arc<
        tokio::sync::Mutex<
            HashMap<String, tokio::sync::oneshot::Sender<crate::PermissionDecision>>,
        >,
    >,
    /// 权限注册表（默认规则 + 用户规则合并）
    permission_registry: Arc<PermissionRegistry>,
    /// Doom loop 检测器
    doom_loop_detector: Arc<DoomLoopDetector>,
    /// Agent 模式管理器（Plan/Build/Document）
    agent_mode_manager: Arc<super::AgentModeManager>,
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
    /// 上下文压缩器（可选，从配置初始化；为 None 时不执行压缩）
    compactor: Option<ContextCompactor>,
}

impl<R: Runtime> AgentExecutor<R> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        router: Arc<LlmRouter>,
        tool_registry: Arc<ToolRegistry>,
        registry: Arc<tokio::sync::Mutex<HandlerRegistry>>,
        emitter: AgentEmitter<R>,
        confirm_channels: Arc<
            tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<ConfirmDecision>>>,
        >,
        permission_channels: Arc<
            tokio::sync::Mutex<
                HashMap<String, tokio::sync::oneshot::Sender<crate::PermissionDecision>>,
            >,
        >,
        permission_registry: Arc<PermissionRegistry>,
        doom_loop_detector: Arc<DoomLoopDetector>,
        agent_mode_manager: Arc<super::AgentModeManager>,
    ) -> Self {
        Self {
            router,
            tool_registry,
            registry,
            emitter,
            confirm_channels,
            permission_channels,
            permission_registry,
            doom_loop_detector,
            agent_mode_manager,
            max_iterations: 100,
            should_stop: Arc::new(|_| false),
            persist_fn: None,
            context_usage_persist_fn: None,
            snapshot_fn: None,
            confirmation_level: ConfirmationLevel::default(),
            compactor: None,
        }
    }

    /// 设置停止检查回调
    pub fn with_stop_check(mut self, check: Arc<dyn Fn(&str) -> bool + Send + Sync>) -> Self {
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

    /// 设置上下文压缩器
    /// 当 config.enabled 为 true 时创建压缩器，否则保持 None
    pub fn with_compactor(mut self, config: CompactionConfig) -> Self {
        if config.enabled {
            self.compactor = Some(ContextCompactor::new(config));
        }
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

    /// 估算消息列表的 token 数（简化：字符数 / 3）
    /// 包含 content、content_parts 文本、tool_calls 参数和 reasoning_content
    fn estimate_tokens(&self, messages: &[ChatMessage]) -> u64 {
        let total_chars: usize = messages
            .iter()
            .map(|m| {
                let mut len = m.content.len();
                // 多模态消息的文本部分
                if let Some(parts) = &m.content_parts {
                    for part in parts {
                        if let ContentPart::Text { text } = part {
                            len += text.len();
                        }
                    }
                }
                // 工具调用的名称和参数
                if let Some(calls) = &m.tool_calls {
                    for c in calls {
                        len += c.name.len() + c.arguments.len();
                    }
                }
                // 推理内容（DeepSeek R1 等模型）
                if let Some(rc) = &m.reasoning_content {
                    len += rc.len();
                }
                len
            })
            .sum();
        (total_chars / 3) as u64
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
            // 先清理不完整的 tool_calls 消息链，避免将损坏的对话历史持久化
            ctx.cleanup_incomplete_tool_calls();
            self.persist_new_messages(ctx);
            ctx.mark_persisted();
            self.emitter
                .emit_stopped(StoppedPayload {
                    session_id: ctx.session_id.clone(),
                    reason: "用户手动停止".to_string(),
                    completed_steps: total_steps,
                })
                .ok();
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
    /// - DeleteOnly: 仅删除操作需要确认
    /// - Always: 所有 Handler/Tool 调用都需要确认
    fn needs_confirmation(&self, name: &str, params: &serde_json::Value) -> bool {
        match self.confirmation_level {
            ConfirmationLevel::Never => false,
            ConfirmationLevel::DeleteOnly => {
                // 仅删除操作需要确认
                if HIGH_RISK_HANDLERS.contains(&name) {
                    return true;
                }
                // bash 中的高风险命令需确认
                // 含 rm/del/rmdir/rm -rf/mkfs/format 等破坏性命令
                if name == "bash" {
                    if let Some(cmd) = params.get("command").and_then(|v| v.as_str()) {
                        if is_high_risk_command(cmd) {
                            return true;
                        }
                    }
                }
                false
            }
            ConfirmationLevel::Always => true,
        }
    }

    /// 从 Handler 参数中提取需要创建快照的文件路径列表
    /// remove: 单文件路径
    /// write（覆盖模式）: 单文件路径
    /// 文档 Handler（docx/xlsx/pptx/pdf）: 精简后不再有 modify 操作，无需快照
    fn extract_snapshot_paths(
        &self,
        handler_name: &str,
        params: &serde_json::Value,
    ) -> Vec<String> {
        match handler_name {
            "remove" => {
                vec![params["path"].as_str().unwrap_or("").to_string()]
            }
            "write" => {
                let append = params
                    .get("append")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !append {
                    vec![params["path"].as_str().unwrap_or("").to_string()]
                } else {
                    Vec::new()
                }
            }
            "docx" | "xlsx" | "pptx" | "pdf" => {
                // 文档 Handler 精简后不再有 modify 操作，无需创建快照
                Vec::new()
            }
            "edit" => {
                // edit 工具修改文件，需要创建快照
                vec![params["path"].as_str().unwrap_or("").to_string()]
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
                    log::warn!(
                        "增量持久化失败: session_id={}, 错误: {}",
                        ctx.session_id,
                        e.message
                    );
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
                if tool_name == "remove" {
                    "critical"
                } else if tool_name == "bash" {
                    "high"
                } else {
                    "normal"
                }
            }
            _ => {
                if tool_name == "remove" {
                    "critical"
                } else {
                    "high"
                }
            }
        };

        let description = match tool_name {
            "remove" => format!("删除文件: {}", arguments["path"].as_str().unwrap_or("未知")),
            "bash" => format!(
                "执行命令: {}",
                arguments["command"].as_str().unwrap_or("未知")
            ),
            "docx" | "xlsx" | "pptx" | "pdf" => {
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

        if self
            .emitter
            .emit_confirm(ConfirmPayload {
                session_id: session_id.to_string(),
                operation_id: operation_id.clone(),
                operation_type: tool_name.to_string(),
                description,
                details: arguments.clone(),
                risk_level: risk_level.to_string(),
            })
            .is_err()
        {
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
                    log::info!(
                        "用户确认操作: operation_id={}, tool={}",
                        operation_id,
                        tool_name
                    );
                    Ok(true)
                } else {
                    log::info!(
                        "用户拒绝操作: operation_id={}, tool={}, feedback={:?}",
                        operation_id,
                        tool_name,
                        decision.feedback
                    );
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
                self.emitter
                    .emit_error(ErrorPayload {
                        session_id: session_id.to_string(),
                        code: crate::errors::AGENT_CONFIRMATION_TIMEOUT,
                        message: format!("操作确认超时 ({}秒)", CONFIRM_TIMEOUT_SECS),
                        recoverable: true,
                    })
                    .ok();
                Ok(false)
            }
        }
    }

    /// 权限系统检查（替代原有 ConfirmationLevel 机制）
    /// 按以下顺序检查：Plan 模式 → Doom loop → 白名单 → 外部目录 → 规则评估
    /// 返回 Ok(PermissionResult::Allow) 表示允许执行，Ok(PermissionResult::Deny) 表示拒绝（附带拒绝原因）
    async fn check_permission(
        &self,
        ctx: &AgentContext,
        tool_name: &str,
        params: &serde_json::Value,
    ) -> Result<PermissionResult, CommandError> {
        // 1. 获取当前 AgentMode
        let mode = self.agent_mode_manager.get_mode(&ctx.session_id).await;

        // 2. Plan 模式拒绝修改类操作
        let perm_type = PermissionType::from_tool_name(tool_name);
        if mode.is_plan() && perm_type.is_modification() {
            let category = perm_type.category_name();
            log::warn!(
                "权限拒绝(Plan 模式): session_id={}, tool={}, category={}",
                ctx.session_id,
                tool_name,
                category
            );
            return Ok(PermissionResult::Deny {
                reason: format!(
                    "Plan mode prohibits using {} tools: {}",
                    category, tool_name
                ),
            });
        }

        // 3. Doom loop 检测：连续多次相同调用
        if self
            .doom_loop_detector
            .record_and_check(&ctx.session_id, tool_name, params)
            .await
        {
            log::warn!(
                "权限拒绝(Doom loop): session_id={}, tool={}",
                ctx.session_id,
                tool_name
            );
            return Ok(PermissionResult::Deny {
                reason: format!(
                    "Doom loop detected: tool {} called too many times consecutively",
                    tool_name
                ),
            });
        }

        // 4. 构造权限评估请求
        let request = PermissionRequest::from_tool_call(tool_name, params);

        // 5. 外部目录检查：文件操作且路径在工作区外时强制 Ask
        let is_external = if !ctx.workspace_path.is_empty() {
            // 从参数中提取路径，判断是否为外部目录
            let path_str = params
                .get("path")
                .or_else(|| params.get("file_path"))
                .or_else(|| params.get("input_path"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !path_str.is_empty() && path_str != "*" {
                PermissionEvaluator::is_external_directory(path_str, &ctx.workspace_path)
            } else {
                false
            }
        } else {
            false
        };

        // 7. 规则评估（load_effective_rules 是同步方法）
        let rules = self.permission_registry.load_effective_rules(
            if ctx.workspace_id.is_empty() {
                None
            } else {
                Some(&ctx.workspace_id)
            },
            Some(&ctx.session_id),
        );
        let decision = PermissionEvaluator::evaluate(&request, &rules);

        // 8. 根据评估结果处理（外部目录访问强制升级为 Ask）
        let final_action = if is_external {
            PermissionAction::Ask
        } else {
            decision.action
        };

        // 9. 根据 confirmation_level 调整 Ask 行为
        // Never/Always 级别:将 Ask 转为 Allow,避免权限系统弹窗(Never 由用户选择不确认,Always 由 needs_confirmation 统一处理)
        // 保留 Deny 结果(.env 等安全保护仍生效)
        // 但用户自定义规则的 Ask 不应被 confirmation_level 覆盖,以尊重用户显式配置的权限规则
        // 判断匹配的规则是否为用户自定义规则(非默认规则)
        let is_user_rule = decision
            .matched_rule_id
            .as_ref()
            .map(|id| {
                !self
                    .permission_registry
                    .default_rules()
                    .iter()
                    .any(|r| &r.id == id)
            })
            .unwrap_or(false);
        let final_action = match (&self.confirmation_level, &final_action) {
            (ConfirmationLevel::Never, PermissionAction::Ask) if !is_user_rule => {
                log::debug!(
                    "权限允许(confirmation_level=Never,跳过 Ask): session_id={}, tool={}",
                    ctx.session_id,
                    tool_name
                );
                PermissionAction::Allow
            }
            (ConfirmationLevel::Always, PermissionAction::Ask) if !is_user_rule => {
                log::debug!(
                    "权限允许(confirmation_level=Always,由 needs_confirmation 处理): session_id={}, tool={}",
                    ctx.session_id,
                    tool_name
                );
                PermissionAction::Allow
            }
            _ => final_action,
        };

        match final_action {
            PermissionAction::Allow => {
                log::debug!(
                    "权限允许(规则): session_id={}, tool={}",
                    ctx.session_id,
                    tool_name
                );
                Ok(PermissionResult::Allow)
            }
            PermissionAction::Deny => {
                log::warn!(
                    "权限拒绝(规则): session_id={}, tool={}, rule={:?}, desc={}",
                    ctx.session_id,
                    tool_name,
                    decision.matched_rule_id,
                    decision.matched_description
                );
                Ok(PermissionResult::Deny {
                    reason: format!(
                        "Operation denied by permission rules: {} ({})",
                        tool_name, decision.matched_description
                    ),
                })
            }
            PermissionAction::Ask => {
                // 询问用户，等待双态回复
                let user_decision = self
                    .request_permission_with_response(&ctx.session_id, tool_name, params)
                    .await?;

                match user_decision.response {
                    PermissionResponse::Reject => {
                        log::info!(
                            "权限拒绝(用户): session_id={}, tool={}",
                            ctx.session_id,
                            tool_name
                        );
                        Ok(PermissionResult::Deny {
                            reason: format!("User rejected the operation: {}", tool_name),
                        })
                    }
                    PermissionResponse::Once => {
                        log::info!(
                            "权限允许(用户 Once): session_id={}, tool={}",
                            ctx.session_id,
                            tool_name
                        );
                        Ok(PermissionResult::AllowWithPermissionAsked)
                    }
                }
            }
        }
    }

    /// 请求用户权限审批（双态：once/reject）
    /// 使用 permission_channels 等待用户回复，5 分钟超时
    async fn request_permission_with_response(
        &self,
        session_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<crate::PermissionDecision, CommandError> {
        let operation_id = format!("perm_{}", uuid::Uuid::new_v4());

        let risk_level = self.assess_risk_level(tool_name, arguments);
        let description = self.format_permission_description(tool_name, arguments);

        // 先创建 channel 并插入 map，再发射事件，避免竞态条件
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut channels = self.permission_channels.lock().await;
            channels.insert(operation_id.clone(), tx);
        }

        if self
            .emitter
            .emit_confirm(ConfirmPayload {
                session_id: session_id.to_string(),
                operation_id: operation_id.clone(),
                operation_type: tool_name.to_string(),
                description,
                details: arguments.clone(),
                risk_level: risk_level.to_string(),
            })
            .is_err()
        {
            // 发射事件失败，清理通道，避免泄漏
            let mut channels = self.permission_channels.lock().await;
            channels.remove(&operation_id);
            return Err(CommandError::new(
                crate::errors::RUNTIME_EVENT_EMIT_ERROR,
                "发射权限确认事件失败",
            ));
        }

        match tokio::time::timeout(Duration::from_secs(CONFIRM_TIMEOUT_SECS), rx).await {
            Ok(Ok(decision)) => {
                let mut channels = self.permission_channels.lock().await;
                channels.remove(&operation_id);
                log::info!(
                    "用户权限审批回复: operation_id={}, tool={}, response={:?}",
                    operation_id,
                    tool_name,
                    decision.response
                );
                Ok(decision)
            }
            Ok(Err(_)) => {
                let mut channels = self.permission_channels.lock().await;
                channels.remove(&operation_id);
                log::warn!("权限通道关闭: operation_id={}", operation_id);
                Ok(crate::PermissionDecision {
                    response: PermissionResponse::Reject,
                    feedback: Some("权限通道已关闭".to_string()),
                })
            }
            Err(_) => {
                let mut channels = self.permission_channels.lock().await;
                channels.remove(&operation_id);
                log::warn!("权限审批超时: operation_id={}", operation_id);
                self.emitter
                    .emit_error(ErrorPayload {
                        session_id: session_id.to_string(),
                        code: crate::errors::AGENT_CONFIRMATION_TIMEOUT,
                        message: format!("权限审批超时 ({}秒)", CONFIRM_TIMEOUT_SECS),
                        recoverable: true,
                    })
                    .ok();
                Ok(crate::PermissionDecision {
                    response: PermissionResponse::Reject,
                    feedback: Some("权限审批超时".to_string()),
                })
            }
        }
    }

    /// 评估操作风险等级
    /// 用于权限确认弹窗的风险等级展示
    fn assess_risk_level(&self, tool_name: &str, params: &serde_json::Value) -> &'static str {
        match tool_name {
            "remove" | "remove_dir" => "critical",
            "bash" => {
                if let Some(cmd) = params.get("command").and_then(|v| v.as_str()) {
                    if is_high_risk_command(cmd) {
                        return "critical";
                    }
                }
                "high"
            }
            "edit" | "write" => "high",
            "docx" | "xlsx" | "pptx" | "pdf" => "medium",
            _ => "normal",
        }
    }

    /// 格式化权限确认弹窗的操作描述
    fn format_permission_description(&self, tool_name: &str, params: &serde_json::Value) -> String {
        match tool_name {
            "remove" => format!("删除文件: {}", params["path"].as_str().unwrap_or("未知")),
            "remove_dir" => format!("删除目录: {}", params["path"].as_str().unwrap_or("未知")),
            "bash" => format!("执行命令: {}", params["command"].as_str().unwrap_or("未知")),
            "edit" => format!("编辑文件: {}", params["path"].as_str().unwrap_or("未知")),
            "write" => format!("写入文件: {}", params["path"].as_str().unwrap_or("未知")),
            "docx" | "xlsx" | "pptx" | "pdf" => {
                let action = params["action"].as_str().unwrap_or("操作");
                let path = params
                    .get("input_path")
                    .or_else(|| params.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("未知文件");
                format!("{} - {}: {}", tool_name, action, path)
            }
            _ => format!("执行操作: {}", tool_name),
        }
    }

    /// 发射上下文窗口使用情况事件
    async fn emit_context_usage(
        &self,
        ctx: &mut AgentContext,
        response_tokens: usize,
        usage: Option<&ChatUsage>,
    ) {
        let model_name = self.router.current_model_name();
        let cache_type = self.router.current_cache_type().to_string();

        // 缓存诊断：记录本轮和累计的缓存命中统计
        if let Some(u) = usage {
            let total = u.prompt_cache_hit_tokens + u.prompt_cache_miss_tokens;
            let hit_rate = if total > 0 {
                u.prompt_cache_hit_tokens as f64 / total as f64 * 100.0
            } else {
                0.0
            };
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

        let usage_info =
            ctx.calculate_context_usage(response_tokens, model_name, cache_type, usage);

        if let Some(ref persist_fn) = self.context_usage_persist_fn {
            persist_fn(&ctx.session_id, &usage_info);
        }

        self.emitter
            .emit_context_usage(crate::events::types::ContextUsagePayload {
                session_id: ctx.session_id.clone(),
                context_usage: usage_info,
            })
            .ok();
    }

    /// 按当前 AgentMode 动态构建工具定义列表
    /// Build/Plan 模式：仅 Tool 定义，过滤掉文档 Handler
    /// Document 模式：Tool 定义 + 文档 Handler 定义
    /// 所有定义按 function.name 字母序稳定排序
    async fn build_tool_definitions(&self, session_id: &str) -> Vec<serde_json::Value> {
        let mode = self.agent_mode_manager.get_mode(session_id).await;
        let mut defs = self.tool_registry.tool_definitions();
        if mode.includes_document_handlers() {
            // Document 模式：加入文档 Handler 定义
            let reg = self.registry.lock().await;
            let mut handler_defs = reg.tool_definitions();
            // 过滤：仅保留文档 Handler（防御性，确保非文档 Handler 不被暴露）
            handler_defs.retain(|d| {
                d.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .map(is_document_handler)
                    .unwrap_or(false)
            });
            defs.extend(handler_defs);
        }
        // 按 function.name 字母序稳定排序
        defs.sort_by(|a, b| {
            let name_a = a["function"]["name"].as_str().unwrap_or("");
            let name_b = b["function"]["name"].as_str().unwrap_or("");
            name_a.cmp(name_b)
        });
        defs
    }

    pub async fn execute(&self, ctx: &mut AgentContext) -> Result<ExecutionResult, CommandError> {
        let start_time = std::time::Instant::now();
        let mut total_steps = 0u32;

        log::info!("Agent 开始执行, session_id={}", ctx.session_id);

        // 如果注入了 skill_registry，按当前 AgentMode 追加 Skill 清单到系统提示词
        // 此处不修改 build_system_prompt_with_task 的签名，而是在 executor 层追加
        if let Some(skill_registry) = &ctx.skill_registry {
            let mode = self.agent_mode_manager.get_mode(&ctx.session_id).await;
            let mode_str = match mode {
                super::AgentMode::Plan => "plan",
                super::AgentMode::Build => "build",
                super::AgentMode::Document => "document",
            };
            let skill_summary = skill_registry.build_summary_for_prompt(mode_str);
            if !skill_summary.is_empty() {
                ctx.system_prompt.push_str(&skill_summary);
                log::info!(
                    "已注入 Skill 清单到系统提示词, session_id={}, mode={}",
                    ctx.session_id,
                    mode_str
                );
            }
        }

        // 保存基础系统提示词（含 Skill 清单，不含动态 TodoList 摘要）
        // 每轮迭代从此基础重建 system_prompt，追加最新的 TodoList 摘要
        // 避免 TodoList 摘要在多轮迭代中重复追加
        let base_system_prompt = ctx.system_prompt.clone();

        // 按 AgentMode 动态过滤工具列表（Document 模式加入文档 Handler）
        let tool_defs_json = self.build_tool_definitions(&ctx.session_id).await;
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
        ctx.function_definitions_tokens =
            crate::services::agent::prompts::token_budget::TokenBudgetManager::estimate_tokens(
                &func_defs_str,
            );

        // 缓存诊断：记录工具定义序列化稳定性（若工具定义 JSON 在各会话间不一致，将导致缓存持续未命中）
        // 使用 char_indices 安全地截取前 80 个字符，避免切割多字节 UTF-8 字符导致 panic
        let safe_prefix = func_defs_str.chars().take(80).collect::<String>();
        log::debug!(
            "缓存诊断: 工具定义序列化={{长度={}, 工具数量={}, 前80字符={}}}",
            func_defs_str.len(),
            tools.len(),
            safe_prefix,
        );

        for iteration in 0..self.max_iterations {
            // 检查是否被用户停止
            if let Some(result) = self.handle_stop_if_needed(ctx, total_steps, start_time) {
                return Ok(result);
            }

            log::debug!(
                "Agent 迭代 #{}, session_id={}",
                iteration + 1,
                ctx.session_id
            );

            let current_iteration = iteration + 1;

            // 每轮迭代开始时刷新 Scratchpad 笔记摘要
            // 这会从共享状态中读取当前会话的笔记，格式化为摘要字符串
            // get_messages_for_iteration 会将摘要作为独立 user 消息追加到末尾
            // 设计依据：Anthropic Effective Context Engineering 的 Structured Note-taking 模式
            ctx.refresh_scratchpad_summary();

            // T3.13: 每轮迭代刷新 system_prompt 中的 TodoList 摘要
            // 从数据库读取当前会话的 TodoList，若有任务则追加摘要到 system_prompt
            // 每轮迭代从 base_system_prompt 重建，避免 TodoList 摘要重复追加
            // TodoList 状态可能在上一轮被 TodoWrite 工具更新，需每轮读取最新状态
            let mut prompt = base_system_prompt.clone();
            if let Some(db) = &ctx.db {
                if let Ok(conn) = db.conn() {
                    if let Ok(todo_list) =
                        crate::db::todo_repo::get_todo_list(&conn, &ctx.session_id)
                    {
                        if let Some(summary) = todo_list.build_summary() {
                            prompt.push_str(&summary);
                        }
                    }
                }
            }
            ctx.system_prompt = prompt;

            // 上下文压缩检查：当 token 数接近上下文窗口阈值时触发
            // 压缩将旧消息汇总为摘要 system 消息，保留最近 N 条消息
            if let Some(compactor) = &self.compactor {
                let all_messages = ctx.get_messages();
                let current_tokens = self.estimate_tokens(&all_messages);
                let context_window = ctx.context_window() as u64;

                if compactor.should_compact(current_tokens, context_window) {
                    log::info!(
                        "上下文压缩触发: {} tokens >= {}, session_id={}",
                        current_tokens,
                        context_window,
                        ctx.session_id
                    );

                    // 发射压缩开始事件
                    let _ = self.emitter.app_handle_ref().emit(
                        AGENT_COMPACTION_START,
                        CompactionStartPayload {
                            session_id: ctx.session_id.clone(),
                            tokens_before: current_tokens,
                        },
                    );

                    let preferred = if ctx.preferred_provider_id.is_empty() {
                        None
                    } else {
                        Some(ctx.preferred_provider_id.as_str())
                    };
                    match self
                        .router
                        .compact_messages(&all_messages, compactor, preferred)
                        .await
                    {
                        Ok(result) => {
                            if result.compacted {
                                let tokens_after = self.estimate_tokens(&result.messages);
                                log::info!(
                                    "上下文压缩完成, session_id={}, 消息数 {} -> {}, 估算 token {} -> {}",
                                    ctx.session_id,
                                    all_messages.len(),
                                    result.messages.len(),
                                    current_tokens,
                                    tokens_after
                                );

                                // 更新上下文消息为压缩后的消息
                                // result.messages = [摘要 system 消息, ...最近消息]
                                // 下次 get_messages_for_iteration 会产生 [原始 system_prompt, 摘要, ...最近消息]
                                ctx.messages = result.messages;
                                // 标记为已持久化：压缩是运行时优化，数据库保留完整历史
                                ctx.mark_persisted();

                                // 发射压缩完成事件
                                let _ = self.emitter.app_handle_ref().emit(
                                    AGENT_COMPACTION_DONE,
                                    CompactionDonePayload {
                                        session_id: ctx.session_id.clone(),
                                        tokens_before: current_tokens,
                                        tokens_after,
                                        compacted: true,
                                        error: None,
                                    },
                                );
                            }
                        }
                        Err(e) => {
                            log::warn!(
                                "上下文压缩失败,继续使用原始消息, session_id={}, 错误: {}",
                                ctx.session_id,
                                e.message
                            );
                            // 压缩失败不中断流程，继续使用原消息
                            let _ = self.emitter.app_handle_ref().emit(
                                AGENT_COMPACTION_DONE,
                                CompactionDonePayload {
                                    session_id: ctx.session_id.clone(),
                                    tokens_before: current_tokens,
                                    tokens_after: current_tokens,
                                    compacted: false,
                                    error: Some(e.message.clone()),
                                },
                            );
                        }
                    }
                }
            }

            let messages = ctx.get_messages_for_iteration(current_iteration);

            let mut llm_retry_count = 0;
            let mut _last_error: Option<CommandError> = None;
            let mut stream_rx = loop {
                if self.check_stopped(&ctx.session_id) {
                    return Ok(self
                        .handle_stop_if_needed(ctx, total_steps, start_time)
                        .expect("check_stopped 返回 true 但 handle_stop_if_needed 返回 None"));
                }

                // 缓存诊断：记录每次 LLM 调用的消息特征，便于分析跨迭代/跨会话缓存命中率
                let msg_summary: String = messages
                    .iter()
                    .map(|m| m.role.chars().next().unwrap_or('?').to_string())
                    .collect::<Vec<_>>()
                    .join("");
                let estimated_prompt_tokens = crate::services::agent::prompts::token_budget::TokenBudgetManager::estimate_tokens(
                    &messages.iter().map(|m| format!("{}:{}", m.role, m.content)).collect::<String>()
                );
                log::debug!(
                    "缓存诊断: session_id={}, 迭代#{}, 消息模式={}, 消息数={}, 估算输入token={}",
                    ctx.session_id,
                    current_iteration,
                    msg_summary,
                    messages.len(),
                    estimated_prompt_tokens,
                );
                log::debug!(
                    "调用 LLM 流式接口, session_id={}, 消息数={}, 重试次数={}",
                    ctx.session_id,
                    messages.len(),
                    llm_retry_count
                );
                match self
                    .router
                    .chat_stream(
                        &messages,
                        &tools,
                        if ctx.preferred_provider_id.is_empty() {
                            None
                        } else {
                            Some(ctx.preferred_provider_id.as_str())
                        },
                    )
                    .await
                {
                    Ok(rx) => break rx,
                    Err(e) => {
                        _last_error = Some(e.clone());
                        log::error!(
                            "LLM 流式调用失败, session_id={}, 错误: {}",
                            ctx.session_id,
                            e.message
                        );

                        if !is_retryable_error(e.code) || llm_retry_count >= MAX_LLM_RETRIES {
                            let error_msg = user_facing_error_message(e.code);
                            self.emitter
                                .emit_error(ErrorPayload {
                                    session_id: ctx.session_id.clone(),
                                    code: e.code,
                                    message: error_msg.clone(),
                                    recoverable: is_retryable_error(e.code),
                                })
                                .ok();
                            // 持久化 error 节点信息（LLM 调用失败且不可重试，Agent 终止）
                            ctx.add_assistant_message_with_metadata(
                                &error_msg,
                                Some(serde_json::json!({
                                    "nodeType": "error",
                                    "code": e.code,
                                    "message": error_msg,
                                    "recoverable": is_retryable_error(e.code),
                                })),
                            );
                            self.persist_new_messages(ctx);
                            ctx.mark_persisted();
                            return Err(e);
                        }

                        llm_retry_count += 1;
                        let wait_secs = RETRY_DELAY_SECONDS * (1 << (llm_retry_count - 1));
                        log::info!(
                            "LLM 调用失败，等待 {} 秒后重试 (第 {}/{} 次), session_id={}",
                            wait_secs,
                            llm_retry_count,
                            MAX_LLM_RETRIES,
                            ctx.session_id
                        );

                        // 重试前：如果 Provider 被标记不可用，先尝试恢复
                        // 避免重试时因 Provider 不可用而直接失败
                        if e.code == crate::errors::LLM_PROVIDER_UNAVAILABLE {
                            log::info!(
                                "Provider 不可用，尝试恢复后重试, session_id={}",
                                ctx.session_id
                            );
                            self.router.force_recover_all().await;
                            self.router.rebuild_all_clients().await;
                        }

                        self.emitter
                            .emit_network_retry(NetworkRetryPayload {
                                session_id: ctx.session_id.clone(),
                                attempt: llm_retry_count,
                                max_attempts: MAX_LLM_RETRIES,
                                reason: e.message.clone(),
                            })
                            .ok();

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

            while let Some(chunk_result) = stream_rx.recv().await {
                match chunk_result {
                    Ok(chunk) => {
                        message_id = chunk.id.clone();
                        for choice in chunk.choices {
                            if let Some(rc) = &choice.delta.reasoning_content {
                                reasoning_content.push_str(rc);
                                self.emitter
                                    .emit_deep_thinking(DeepThinkingPayload {
                                        session_id: ctx.session_id.clone(),
                                        step: total_steps,
                                        thought: rc.clone(),
                                        is_streaming: true,
                                        iteration: Some(current_iteration),
                                    })
                                    .ok();
                            }

                            if let Some(content) = &choice.delta.content {
                                assistant_content.push_str(content);
                                // 当已检测到 tool_call 时，不再发射 content 事件
                                // 避免前端在 tool_call 关闭 streaming 节点后，
                                // 又收到残余 content 创建新的重复节点
                                if early_announced_tool_indices.is_empty() {
                                    self.emitter
                                        .emit_content(ContentPayload {
                                            session_id: ctx.session_id.clone(),
                                            message_id: message_id.clone(),
                                            content: content.clone(),
                                            is_streaming: true,
                                            iteration: Some(current_iteration),
                                        })
                                        .ok();
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
                                        if let Some(collected) = collected_tool_calls.get(&tc_index)
                                        {
                                            if !collected.name.is_empty() {
                                                early_announced_tool_indices.insert(tc_index);
                                                let early_params =
                                                    serde_json::from_str(&collected.arguments)
                                                        .unwrap_or(json!({}));
                                                log::debug!(
                                                    "流式阶段提前发射 tool_call 事件, session_id={}, tool={}, call_id={}",
                                                    ctx.session_id, collected.name, collected.id
                                                );
                                                self.emitter
                                                    .emit_tool_call(ToolCallPayload {
                                                        session_id: ctx.session_id.clone(),
                                                        call_id: if collected.id.is_empty() {
                                                            format!("streaming_{}", tc_index)
                                                        } else {
                                                            collected.id.clone()
                                                        },
                                                        tool_name: collected.name.clone(),
                                                        arguments: early_params,
                                                        iteration: Some(current_iteration),
                                                    })
                                                    .ok();
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
                            self.emitter
                                .emit_error(ErrorPayload {
                                    session_id: ctx.session_id.clone(),
                                    code: e.code,
                                    message: format!("LLM 流式响应错误: {}", e.message),
                                    recoverable: false,
                                })
                                .ok();
                            break;
                        }

                        if !assistant_content.is_empty() || !collected_tool_calls.is_empty() {
                            log::info!("流式响应中断但已有部分内容，尝试恢复, session_id={}, 内容长度={}, tool_calls数={}", ctx.session_id, assistant_content.len(), collected_tool_calls.len());

                            self.emitter
                                .emit_network_retry(NetworkRetryPayload {
                                    session_id: ctx.session_id.clone(),
                                    attempt: 1,
                                    max_attempts: 1,
                                    reason: "流式响应中断，尝试续写".to_string(),
                                })
                                .ok();

                            let recovered_messages = Self::build_recovery_messages(
                                &messages,
                                &assistant_content,
                                &reasoning_content,
                                &collected_tool_calls,
                            );

                            match self
                                .router
                                .chat_stream(
                                    &recovered_messages,
                                    &tools,
                                    if ctx.preferred_provider_id.is_empty() {
                                        None
                                    } else {
                                        Some(ctx.preferred_provider_id.as_str())
                                    },
                                )
                                .await
                            {
                                Ok(new_rx) => {
                                    log::info!(
                                        "流式恢复成功，继续接收, session_id={}",
                                        ctx.session_id
                                    );
                                    stream_rx = new_rx;
                                    continue;
                                }
                                Err(recover_err) => {
                                    log::error!("流式恢复失败: {}", recover_err.message);
                                    self.emitter
                                        .emit_error(ErrorPayload {
                                            session_id: ctx.session_id.clone(),
                                            code: recover_err.code,
                                            message: user_facing_error_message(recover_err.code),
                                            recoverable: is_retryable_error(recover_err.code),
                                        })
                                        .ok();
                                    break;
                                }
                            }
                        } else {
                            // 可重试错误且无部分内容：尝试重新获取流式响应
                            // 不直接发射 agent:error，而是先尝试重连，避免向用户展示不必要的红色错误
                            log::info!(
                                "流式响应中断且无部分内容，尝试重新请求, session_id={}",
                                ctx.session_id
                            );
                            self.emitter
                                .emit_network_retry(NetworkRetryPayload {
                                    session_id: ctx.session_id.clone(),
                                    attempt: 1,
                                    max_attempts: 1,
                                    reason: "流式响应中断，尝试重新请求".to_string(),
                                })
                                .ok();

                            match self
                                .router
                                .chat_stream(
                                    &messages,
                                    &tools,
                                    if ctx.preferred_provider_id.is_empty() {
                                        None
                                    } else {
                                        Some(ctx.preferred_provider_id.as_str())
                                    },
                                )
                                .await
                            {
                                Ok(new_rx) => {
                                    log::info!(
                                        "流式重新请求成功，继续接收, session_id={}",
                                        ctx.session_id
                                    );
                                    stream_rx = new_rx;
                                    continue;
                                }
                                Err(recover_err) => {
                                    log::error!("流式重新请求失败: {}", recover_err.message);
                                    self.emitter
                                        .emit_error(ErrorPayload {
                                            session_id: ctx.session_id.clone(),
                                            code: recover_err.code,
                                            message: user_facing_error_message(recover_err.code),
                                            recoverable: is_retryable_error(recover_err.code),
                                        })
                                        .ok();
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            // 将 HashMap 转为按 index 排序的 Vec
            let mut collected_tool_calls: Vec<LlmToolCall> =
                collected_tool_calls.into_values().collect::<Vec<_>>();
            collected_tool_calls.sort_by_key(|tc| tc.index);

            // 后处理：检测并清理 LLM content 中的 XML 标签和特殊 token
            // DeepSeek R1 等推理模型可能将 <agent-reasoning> 和 <tool-call>
            // 标签作为 content 输出（而非通过标准 tool_calls 字段），需要：
            // 1. 过滤 <agent-reasoning> 等内部推理标签（不应显示给用户）
            // 2. 从 <tool-call> 标签中提取工具调用信息（补充到 tool_calls）
            // 3. 清理特殊 token（如 <｜tool▁call▁end｜><｜tool▁calls▁end｜>）
            let (cleaned_content, extracted_tool_calls) =
                Self::sanitize_llm_content(&assistant_content);

            if cleaned_content != assistant_content {
                log::info!(
                    "已清理 LLM content 中的 XML 标签/特殊 token, session_id={}, 原始长度={}, 清理后长度={}",
                    ctx.session_id, assistant_content.len(), cleaned_content.len()
                );
                assistant_content = cleaned_content;
            }

            // 将从 content 中提取的 tool_calls 合并到已有列表
            for tc in extracted_tool_calls {
                let next_index = collected_tool_calls
                    .iter()
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
                self.emitter
                    .emit_deep_thinking(DeepThinkingPayload {
                        session_id: ctx.session_id.clone(),
                        step: total_steps,
                        thought: String::new(),
                        is_streaming: false,
                        iteration: Some(current_iteration),
                    })
                    .ok();
            }

            // 发送流式结束事件，携带清理后的完整内容
            // 无论是否存在 tool_calls，只要有内容就发射此事件：
            // 1. 流式阶段可能因 early_announced_tool_indices 而未发射部分 content delta
            //    （LLM 可能在 tool_use 块之后继续输出文本内容块），导致前端显示截断
            // 2. 前端通过 is_streaming=false + iteration 定位已有 content 节点进行更新，
            //    不会创建重复节点
            // 3. 无 tool_calls 时仍需发射，以便前端清除之前流式显示的 XML 标签片段
            if !assistant_content.is_empty() {
                self.emitter
                    .emit_content(ContentPayload {
                        session_id: ctx.session_id.clone(),
                        message_id: message_id.clone(),
                        content: assistant_content.clone(),
                        is_streaming: false,
                        iteration: Some(current_iteration),
                    })
                    .ok();
            }
            // 检测响应是否因 max_tokens 不足被截断（DeepSeek R1 等推理模型的
            // reasoning_content 可能消耗大量 token 导致实际响应被截断）
            let mut is_truncated = finish_reason.as_deref() == Some("length");
            log::debug!(
                "LLM 响应解析完成, session_id={}, tool_calls数={}, 内容长度={}, finish_reason={:?}",
                ctx.session_id,
                collected_tool_calls.len(),
                assistant_content.len(),
                finish_reason
            );

            if has_tool_calls {
                // 将助手消息（含 tool_calls）添加到上下文
                ctx.add_assistant_message(
                    &assistant_content,
                    Some(collected_tool_calls.clone()),
                    if reasoning_content.is_empty() {
                        None
                    } else {
                        Some(reasoning_content.clone())
                    },
                );

                // 如果响应被截断，tool_call 的 JSON 参数可能不完整
                if is_truncated {
                    log::warn!(
                        "LLM 响应被截断且包含 tool_calls, session_id={}, 检查参数完整性",
                        ctx.session_id
                    );
                }

                // 截断重试：当响应被截断且 tool_call 参数解析失败时，
                // 回滚上下文，用翻倍的 max_tokens 重新调用 LLM
                if is_truncated {
                    let mut all_params_valid = true;

                    for tool_call in collected_tool_calls.iter() {
                        let params_result =
                            serde_json::from_str::<serde_json::Value>(&tool_call.arguments);
                        if params_result.is_err() {
                            all_params_valid = false;
                            log::warn!(
                                "截断响应的 tool_call 参数解析失败, session_id={}, tool={}, arguments长度={}",
                                ctx.session_id, tool_call.name, tool_call.arguments.len()
                            );
                            break;
                        }
                    }

                    if !all_params_valid {
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
                                self.emitter
                                    .emit_tool_result(ToolResultPayload {
                                        session_id: ctx.session_id.clone(),
                                        call_id: if tool_call.id.is_empty() {
                                            format!("streaming_{}", tool_call.index)
                                        } else {
                                            tool_call.id.clone()
                                        },
                                        success: false,
                                        result: json!(null),
                                        error: Some(
                                            "响应被截断，正在增加输出限制重试...".to_string(),
                                        ),
                                        duration_ms: 0,
                                    })
                                    .ok();
                            }
                        }

                        // 发射思考事件，让用户看到重试提示
                        self.emitter
                            .emit_thinking(ThinkingPayload {
                                session_id: ctx.session_id.clone(),
                                step: total_steps,
                                thought: "输出被截断，正在增加输出限制重试...".to_string(),
                            })
                            .ok();

                        // 用翻倍的 max_tokens 重试 LLM 调用
                        let mut truncation_retry_count = 0;
                        let mut current_max_tokens = self.get_current_max_tokens().await;

                        while truncation_retry_count < MAX_TRUNCATION_RETRIES {
                            truncation_retry_count += 1;
                            let new_max_tokens = std::cmp::min(
                                current_max_tokens.saturating_mul(2),
                                MAX_TOKENS_CEILING,
                            );
                            log::info!(
                                "截断重试 #{}, max_tokens: {} -> {}, session_id={}",
                                truncation_retry_count,
                                current_max_tokens,
                                new_max_tokens,
                                ctx.session_id
                            );

                            if self.check_stopped(&ctx.session_id) {
                                return Ok(self.handle_stop_if_needed(ctx, total_steps, start_time).expect("check_stopped 返回 true 但 handle_stop_if_needed 返回 None"));
                            }

                            // 重试前刷新 Scratchpad 摘要（笔记可能在重试间隔被更新）
                            ctx.refresh_scratchpad_summary();
                            let retry_messages = ctx.get_messages_for_iteration(current_iteration);
                            match self
                                .router
                                .chat_stream_with_max_tokens(
                                    &retry_messages,
                                    &tools,
                                    new_max_tokens,
                                    if ctx.preferred_provider_id.is_empty() {
                                        None
                                    } else {
                                        Some(ctx.preferred_provider_id.as_str())
                                    },
                                )
                                .await
                            {
                                Ok(mut retry_rx) => {
                                    // 收集重试响应
                                    let mut retry_content = String::new();
                                    let mut retry_reasoning = String::new();
                                    let mut retry_tool_calls: HashMap<u32, LlmToolCall> =
                                        HashMap::new();
                                    let mut retry_finish_reason: Option<String> = None;
                                    let mut retry_message_id = String::new();

                                    while let Some(chunk_result) = retry_rx.recv().await {
                                        match chunk_result {
                                            Ok(chunk) => {
                                                retry_message_id = chunk.id.clone();
                                                for choice in chunk.choices {
                                                    if let Some(rc) =
                                                        &choice.delta.reasoning_content
                                                    {
                                                        retry_reasoning.push_str(rc);
                                                    }
                                                    if let Some(content) = &choice.delta.content {
                                                        retry_content.push_str(content);
                                                        // 截断重试时不发射流式 content 事件
                                                        // 避免与原始截断响应的 content 拼接导致重复
                                                        // 重试完成后统一发射 is_streaming=false 事件替换
                                                    }
                                                    if let Some(tool_calls) =
                                                        &choice.delta.tool_calls
                                                    {
                                                        for tc in tool_calls {
                                                            let entry = retry_tool_calls
                                                                .entry(tc.index)
                                                                .or_insert_with(|| LlmToolCall {
                                                                    index: tc.index,
                                                                    id: tc.id.clone(),
                                                                    name: tc.name.clone(),
                                                                    arguments: String::new(),
                                                                });
                                                            if !tc.id.is_empty() {
                                                                entry.id = tc.id.clone();
                                                            }
                                                            if !tc.name.is_empty() {
                                                                entry.name = tc.name.clone();
                                                            }
                                                            entry.arguments.push_str(&tc.arguments);
                                                        }
                                                    }
                                                    if choice.finish_reason.is_some() {
                                                        retry_finish_reason =
                                                            choice.finish_reason.clone();
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                log::error!(
                                                    "截断重试流式响应错误, session_id={}, 错误: {}",
                                                    ctx.session_id,
                                                    e.message
                                                );
                                                break;
                                            }
                                        }
                                    }

                                    let retry_is_truncated =
                                        retry_finish_reason.as_deref() == Some("length");
                                    let retry_has_tool_calls = !retry_tool_calls.is_empty();

                                    // 重试完成后发射 is_streaming=false 的 content 事件
                                    // 替换前端已有的原始截断响应内容，避免显示过时内容
                                    self.emitter
                                        .emit_content(ContentPayload {
                                            session_id: ctx.session_id.clone(),
                                            message_id: retry_message_id.clone(),
                                            content: retry_content.clone(),
                                            is_streaming: false,
                                            iteration: Some(current_iteration),
                                        })
                                        .ok();

                                    if !retry_reasoning.is_empty() {
                                        self.emitter
                                            .emit_deep_thinking(DeepThinkingPayload {
                                                session_id: ctx.session_id.clone(),
                                                step: total_steps,
                                                thought: String::new(),
                                                is_streaming: false,
                                                iteration: Some(current_iteration),
                                            })
                                            .ok();
                                    }

                                    log::debug!(
                                        "截断重试响应解析完成, session_id={}, tool_calls数={}, 内容长度={}, finish_reason={:?}",
                                        ctx.session_id, retry_tool_calls.len(), retry_content.len(), retry_finish_reason
                                    );

                                    if retry_has_tool_calls {
                                        // 将 HashMap 转为 Vec（按 index 排序）
                                        let mut retry_tc_vec: Vec<LlmToolCall> =
                                            retry_tool_calls.values().cloned().collect();
                                        retry_tc_vec.sort_by_key(|tc| tc.index);

                                        // 添加重试的 assistant message
                                        ctx.add_assistant_message(
                                            &retry_content,
                                            Some(retry_tc_vec.clone()),
                                            if retry_reasoning.is_empty() {
                                                None
                                            } else {
                                                Some(retry_reasoning.clone())
                                            },
                                        );

                                        // 检查重试响应的参数完整性
                                        let mut retry_params_valid = true;
                                        for tc in &retry_tc_vec {
                                            let pr = serde_json::from_str::<serde_json::Value>(
                                                &tc.arguments,
                                            );
                                            if pr.is_err() {
                                                retry_params_valid = false;
                                                break;
                                            }
                                        }

                                        if !retry_is_truncated && retry_params_valid {
                                            // 重试成功，用重试的 tool_calls 替换原来的
                                            collected_tool_calls = retry_tc_vec;
                                            assistant_content = retry_content;
                                            reasoning_content = retry_reasoning;
                                            is_truncated = false;
                                            log::info!(
                                                "截断重试成功, session_id={}, 新max_tokens={}",
                                                ctx.session_id,
                                                new_max_tokens
                                            );
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
                                                    if retry_reasoning.is_empty() {
                                                        None
                                                    } else {
                                                        Some(retry_reasoning)
                                                    },
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
                                    log::error!(
                                        "截断重试 LLM 调用失败, session_id={}, 错误: {}",
                                        ctx.session_id,
                                        e.message
                                    );
                                    // 重试失败，降级为旧的截断处理逻辑
                                    break;
                                }
                            }
                        }
                    }
                }

                for tool_call in collected_tool_calls.iter() {
                    if let Some(result) = self.handle_stop_if_needed(ctx, total_steps, start_time) {
                        return Ok(result);
                    }

                    log::info!(
                        "执行 Tool, session_id={}, tool={}, call_id={}",
                        ctx.session_id,
                        tool_call.name,
                        tool_call.id
                    );

                    // 尝试解析 tool_call 参数，如果响应被截断则参数可能不完整
                    let params_result =
                        serde_json::from_str::<serde_json::Value>(&tool_call.arguments);

                    // 截断重试耗尽后仍解析失败的降级处理：
                    // 跳过执行，反馈给 LLM 重新生成
                    if is_truncated && params_result.is_err() {
                        log::warn!(
                            "截断响应的 tool_call 参数解析失败（重试耗尽）, 跳过执行, session_id={}, tool={}, arguments长度={}",
                            ctx.session_id, tool_call.name, tool_call.arguments.len()
                        );
                        let retry_msg = format!(
                            "The parameters of the previous {} call were incomplete due to response truncation. Please regenerate complete code. Control the code length to ensure parameter completeness.",
                            tool_call.name
                        );
                        // 发射思考事件，让用户看到重试提示
                        self.emitter
                            .emit_thinking(ThinkingPayload {
                                session_id: ctx.session_id.clone(),
                                step: total_steps,
                                thought: "输出限制不足导致响应被截断，正在重新生成...".to_string(),
                            })
                            .ok();
                        // 必须发射 tool_result 事件，否则前端对应节点永远显示加载动画
                        self.emitter
                            .emit_tool_result(ToolResultPayload {
                                session_id: ctx.session_id.clone(),
                                call_id: tool_call.id.clone(),
                                success: false,
                                result: json!(null),
                                error: Some("响应被截断，正在重新生成代码...".to_string()),
                                duration_ms: 0,
                            })
                            .ok();
                        // 将截断信息作为 tool_result 添加到对话上下文
                        ctx.add_tool_result(&tool_call.id, &retry_msg);
                        continue;
                    }

                    let params = params_result.unwrap_or(json!({}));

                    // 更新任务类型（基于已调用的工具）
                    ctx.update_task_type_from_tool(&tool_call.name, Some(&params));

                    // 权限系统检查（替代原有 ConfirmationLevel 机制）
                    // 按顺序检查：Plan 模式 → Doom loop → 白名单 → 外部目录 → 规则评估
                    // confirm_metadata 用于持久化 confirm 节点信息（权限审批/用户确认）
                    let mut confirm_metadata: Option<serde_json::Value> = None;
                    let permitted = self.check_permission(ctx, &tool_call.name, &params).await?;
                    if let PermissionResult::Deny { reason } = permitted {
                        // 权限被拒绝（Plan 模式/Doom loop/规则拒绝/用户拒绝）
                        // 直接使用 check_permission 返回的拒绝原因
                        // 发射带正确 call_id 的 tool_result，确保前端能关闭对应工具节点
                        let reject_msg = reason;
                        log::info!(
                            "操作被权限系统拒绝: session_id={}, tool={}",
                            ctx.session_id,
                            tool_call.name
                        );
                        self.emitter
                            .emit_tool_result(ToolResultPayload {
                                session_id: ctx.session_id.clone(),
                                call_id: tool_call.id.clone(),
                                success: false,
                                result: json!(null),
                                error: Some(reject_msg.clone()),
                                duration_ms: 0,
                            })
                            .ok();
                        // 持久化权限拒绝的 confirm 节点信息
                        let deny_metadata = serde_json::json!({
                            "nodeType": "confirm",
                            "operationType": tool_call.name.clone(),
                            "approved": false,
                        });
                        // 添加失败结果到上下文，让 LLM 感知被拒绝
                        ctx.add_tool_result_with_metadata(
                            &tool_call.id,
                            &reject_msg,
                            Some(deny_metadata),
                        );
                        continue;
                    }

                    // 判断是否已通过权限弹窗批准(避免双重弹窗)
                    let already_confirmed =
                        matches!(permitted, PermissionResult::AllowWithPermissionAsked);
                    if already_confirmed {
                        // 已通过权限弹窗批准，记录 confirm 节点信息
                        confirm_metadata = Some(serde_json::json!({
                            "nodeType": "confirm",
                            "operationType": tool_call.name.clone(),
                            "approved": true,
                        }));
                    }

                    if !already_confirmed && self.needs_confirmation(&tool_call.name, &params) {
                        // 高风险技能：始终发射 tool_call 事件
                        // 若流式阶段已提前发射，此处携带完整参数重新发射，前端通过 callId 去重更新
                        self.emitter
                            .emit_tool_call(ToolCallPayload {
                                session_id: ctx.session_id.clone(),
                                call_id: tool_call.id.clone(),
                                tool_name: format!("{} (awaiting confirmation)", tool_call.name),
                                arguments: params.clone(),
                                iteration: Some(current_iteration),
                            })
                            .ok();

                        let approved = self
                            .request_confirmation(&ctx.session_id, &tool_call.name, &params)
                            .await?;

                        if !approved {
                            let skip_msg = format!("User denied the operation: {}", tool_call.name);
                            log::info!(
                                "操作被拒绝: session_id={}, tool={}",
                                ctx.session_id,
                                tool_call.name
                            );

                            self.emitter
                                .emit_tool_result(ToolResultPayload {
                                    session_id: ctx.session_id.clone(),
                                    call_id: tool_call.id.clone(),
                                    success: false,
                                    result: json!(null),
                                    error: Some(skip_msg.clone()),
                                    duration_ms: 0,
                                })
                                .ok();

                            // 持久化用户拒绝确认的 confirm 节点信息
                            let user_deny_metadata = serde_json::json!({
                                "nodeType": "confirm",
                                "operationType": tool_call.name.clone(),
                                "approved": false,
                            });
                            ctx.add_tool_result_with_metadata(
                                &tool_call.id,
                                &skip_msg,
                                Some(user_deny_metadata),
                            );
                            continue;
                        }

                        // 用户确认批准，记录 confirm 节点信息
                        confirm_metadata = Some(serde_json::json!({
                            "nodeType": "confirm",
                            "operationType": tool_call.name.clone(),
                            "approved": true,
                        }));
                    } else {
                        // 普通工具：始终发射 tool_call 事件
                        // 若流式阶段已提前发射，此处携带完整参数重新发射，前端通过 callId 去重更新
                        self.emitter
                            .emit_tool_call(ToolCallPayload {
                                session_id: ctx.session_id.clone(),
                                call_id: tool_call.id.clone(),
                                tool_name: tool_call.name.clone(),
                                arguments: params.clone(),
                                iteration: Some(current_iteration),
                            })
                            .ok();
                    }

                    // 防御性校验 - 非 Document 模式下拒绝文档 Handler 调用
                    let current_mode = self.agent_mode_manager.get_mode(&ctx.session_id).await;
                    if is_document_handler(&tool_call.name)
                        && !current_mode.includes_document_handlers()
                    {
                        let reject_msg = format!(
                            "Document handlers are not allowed in the current mode ({:?}): {}. Please switch to Document mode",
                            current_mode, tool_call.name
                        );
                        log::warn!(
                            "文档 Handler 被模式过滤拒绝: session_id={}, tool={}, mode={:?}",
                            ctx.session_id,
                            tool_call.name,
                            current_mode
                        );
                        self.emitter
                            .emit_tool_result(ToolResultPayload {
                                session_id: ctx.session_id.clone(),
                                call_id: tool_call.id.clone(),
                                success: false,
                                result: json!(null),
                                error: Some(reject_msg.clone()),
                                duration_ms: 0,
                            })
                            .ok();
                        ctx.add_tool_result_with_metadata(
                            &tool_call.id,
                            &reject_msg,
                            confirm_metadata.take(),
                        );
                        continue;
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
                        "list"
                            | "search"
                            | "read"
                            | "file_info"
                            | "exists"
                            | "remove"
                            | "mkdir"
                            | "write"
                            | "rename"
                            | "copy"
                            | "remove_dir"
                            | "hash"
                            | "edit"
                            | "glob"
                            | "grep"
                            | "docx"
                            | "xlsx"
                            | "pptx"
                            | "pdf"
                            | "validator"
                            | "write_script"
                            | "bash"
                            | "source_code"
                    );
                    if needs_workspace_root && !ctx.workspace_path.is_empty() {
                        safe_params["workspace_root"] = json!(ctx.workspace_path);
                    }

                    // 对 scratchpad 工具，注入 _session_id 和 _iteration
                    // 这些系统参数以下划线开头，不暴露给 LLM（工具 schema 中未声明）
                    // _session_id 用于按会话隔离笔记状态，_iteration 用于调试和排序
                    if tool_call.name == "scratchpad" {
                        safe_params["_session_id"] = json!(ctx.session_id);
                        safe_params["_iteration"] = json!(current_iteration);
                    }

                    // T3.13: 为 todowrite 工具注入 _session_id
                    // _session_id 用于按会话隔离任务列表，不暴露给 LLM
                    if tool_call.name == "todowrite" {
                        safe_params["_session_id"] = json!(ctx.session_id);
                    }

                    // 为 question 工具注入 _session_id
                    // _session_id 用于在 AGENT_QUESTION 事件中携带正确会话 ID，前端据此路由到当前会话
                    if tool_call.name == "question" {
                        safe_params["_session_id"] = json!(ctx.session_id);
                    }

                    // 为 task 工具注入父 Agent 上下文（session_id/workspace_root/nesting_depth/system_prompt/agent_mode）
                    // 这些参数以下划线开头，不暴露给 LLM，用于子 Agent 继承父 Agent 配置
                    if tool_call.name == "task" {
                        safe_params["_session_id"] = json!(ctx.session_id);
                        if !ctx.workspace_path.is_empty() {
                            safe_params["_workspace_root"] = json!(ctx.workspace_path);
                        }
                        safe_params["_nesting_depth"] = json!(0u32); // 主 Agent 的嵌套深度为 0
                        safe_params["_system_prompt"] = json!(ctx.system_prompt);
                        // current_mode 在上方已获取，复用以避免重复查询
                        let mode_str = match current_mode {
                            super::AgentMode::Plan => "plan",
                            super::AgentMode::Build => "build",
                            super::AgentMode::Document => "document",
                        };
                        safe_params["_agent_mode"] = json!(mode_str);
                    }

                    // 在文件修改/删除操作前自动创建版本快照
                    if let Some(ref snapshot_fn) = self.snapshot_fn {
                        let files_to_snapshot =
                            self.extract_snapshot_paths(&tool_call.name, &safe_params);
                        for file_path in &files_to_snapshot {
                            if !file_path.is_empty() {
                                let operation = match tool_call.name.as_str() {
                                    "remove" => "delete",
                                    "edit" => "edit",
                                    "docx" | "xlsx" | "pptx" | "pdf" => "read",
                                    _ => "unknown",
                                };
                                match snapshot_fn(
                                    &ctx.workspace_id,
                                    &ctx.session_id,
                                    file_path,
                                    operation,
                                ) {
                                    Ok(_) => {
                                        log::info!(
                                            "版本快照已创建: file={}, operation={}",
                                            file_path,
                                            operation
                                        );
                                    }
                                    Err(e) => {
                                        log::warn!(
                                            "版本快照创建失败: file={}, 错误: {}",
                                            file_path,
                                            e.message
                                        );
                                    }
                                }
                            }
                        }
                    }

                    // 提前捕获 question 工具的 questions 参数（safe_params 在 execute 时会被 move）
                    let question_questions = if tool_call.name == "question" {
                        safe_params.get("questions").cloned()
                    } else {
                        None
                    };

                    // 提前捕获 task 工具的 description 参数（safe_params 在 execute 时会被 move）
                    // 用于在 tool_result metadata 中持久化子 Agent 任务描述
                    let task_description = if tool_call.name == "task" {
                        safe_params
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    };

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
                                error_code: r.error_code,
                            },
                            Err(_) => {
                                log::error!("Tool 执行发生 panic: tool={}", tool_call.name);
                                crate::models::handler::HandlerResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!(
                                        "Internal error occurred during tool execution: {}",
                                        tool_call.name
                                    )),
                                    duration_ms: 0,
                                    error_code: None,
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
                                    error: Some(format!(
                                        "Internal error occurred during handler execution: {}",
                                        tool_call.name
                                    )),
                                    duration_ms: 0,
                                    error_code: None,
                                }
                            }
                        }
                    } else {
                        crate::models::handler::HandlerResult {
                            success: false,
                            output: None,
                            error: Some(format!(
                                "Tool or handler does not exist: {}",
                                tool_call.name
                            )),
                            duration_ms: 0,
                            error_code: Some(crate::errors::AGENT_HANDLER_NOT_FOUND),
                        }
                    };

                    let duration_ms = tool_start.elapsed().as_millis() as u64;
                    log::debug!(
                        "Tool 执行完成, session_id={}, tool={}, 成功={}, 耗时={}ms",
                        ctx.session_id,
                        tool_call.name,
                        result.success,
                        duration_ms
                    );

                    let clean_output = result.output.clone();

                    self.emitter
                        .emit_tool_result(ToolResultPayload {
                            session_id: ctx.session_id.clone(),
                            call_id: tool_call.id.clone(),
                            success: result.success,
                            result: clean_output.clone().unwrap_or(json!(null)),
                            error: result.error.clone(),
                            duration_ms,
                        })
                        .ok();

                    // 将工具结果添加到上下文
                    // 缓存优化：对大结果进行截断，避免巨量动态内容冲淡缓存命中率
                    let result_content = if result.success {
                        let output_val = clean_output.as_ref().map(|v| {
                            // 如果是 JSON 对象且包含 content 字段，截断该字段
                            if let Some(obj) = v.as_object() {
                                if let Some(content_val) = obj.get("content") {
                                    if let Some(content_str) = content_val.as_str() {
                                        // 使用 chars().count() 获取字符数（而非 len() 字节数）
                                        // 避免多字节 UTF-8 字符（如中文）在字节切片时 panic
                                        let total_chars = content_str.chars().count();
                                        if total_chars > MAX_TOOL_RESULT_CHARS {
                                            let mut truncated = v.clone();
                                            // 智能截断：保留头部 70% + 尾部 30%
                                            // 避免丢失文档末尾结论、表格尾部或代码 traceback
                                            let head_chars = MAX_TOOL_RESULT_CHARS * 7 / 10;
                                            let tail_chars = MAX_TOOL_RESULT_CHARS - head_chars;
                                            let head: String = content_str.chars().take(head_chars).collect();
                                            // 反向取尾部再反序，避免收集整个字符串
                                            let tail: String = content_str
                                                .chars()
                                                .rev()
                                                .take(tail_chars)
                                                .collect::<String>()
                                                .chars()
                                                .rev()
                                                .collect();
                                            let truncated_content = format!(
                                                "{}\n\n...[truncated: original {} chars, kept head {} + tail {}, omitted middle {} chars]...\n\n{}",
                                                head,
                                                total_chars,
                                                head_chars,
                                                tail_chars,
                                                total_chars - MAX_TOOL_RESULT_CHARS,
                                                tail,
                                            );
                                            if let Some(obj) = truncated.as_object_mut() {
                                                obj.insert(
                                                    "content".to_string(),
                                                    json!(truncated_content),
                                                );
                                            }
                                            // 截断必须有可观测日志，否则工具结果中间内容丢失时无法诊断
                                            // （智能体读大文档时只看到头尾，中间章节缺失会被误判为"文档截断"）
                                            log::info!(
                                                "工具结果内容字段已截断, tool={}, 原始 {} 字符 -> 保留头部 {} + 尾部 {}, 省略中间 {} 字符",
                                                tool_call.name,
                                                total_chars,
                                                head_chars,
                                                tail_chars,
                                                total_chars - MAX_TOOL_RESULT_CHARS
                                            );
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
                            log::warn!(
                                "工具结果字符串级安全截断, tool={}, 原始 {} 字节 -> 仅保留前 {} 字符（极端情况，可能丢失关键信息）",
                                tool_call.name,
                                serialized.len(),
                                MAX_TOOL_RESULT_CHARS * 2
                            );
                            let safe_truncated: String =
                                serialized.chars().take(MAX_TOOL_RESULT_CHARS * 2).collect();
                            format!(
                                "{}...\n[truncated: tool result too large, kept first {} chars only]",
                                safe_truncated,
                                MAX_TOOL_RESULT_CHARS * 2
                            )
                        } else {
                            serialized
                        }
                    } else {
                        format!("Error: {}", result.error.clone().unwrap_or_default())
                    };
                    // 构建 tool_result 的 metadata：
                    // - question 工具：持久化 questions/answers 作为 question 节点
                    // - task 工具：持久化子 Agent 执行结果摘要作为 sub_agent 节点
                    // - 其他工具：若经历了 confirm/permission 流程，持久化 confirm 节点信息
                    let tool_metadata = if tool_call.name == "question" {
                        Some(serde_json::json!({
                            "nodeType": "question",
                            "questions": question_questions.unwrap_or(serde_json::Value::Array(vec![])),
                            "answers": result.output.as_ref().and_then(|o| o.get("answers")).cloned().unwrap_or(serde_json::Value::Array(vec![])),
                        }))
                    } else if tool_call.name == "task" {
                        // 从 ToolResult.output 解析子 Agent 执行结果（SubAgentResult 序列化为 JSON）
                        let output = clean_output.as_ref().unwrap_or(&serde_json::Value::Null);
                        Some(serde_json::json!({
                            "nodeType": "sub_agent",
                            "agentId": output["agentId"],
                            "taskDescription": task_description,
                            "success": output.get("success").and_then(|v| v.as_bool()).unwrap_or(false),
                            "iterations": output["iterations"],
                            "toolCalls": output["toolCallRecords"],
                            "error": output["error"],
                        }))
                    } else {
                        confirm_metadata.take()
                    };
                    ctx.add_tool_result_with_metadata(
                        &tool_call.id,
                        &result_content,
                        tool_metadata,
                    );
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
                self.emit_context_usage(ctx, response_tokens, final_usage.as_ref())
                    .await;

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
                    if reasoning_content.is_empty() {
                        None
                    } else {
                        Some(reasoning_content.clone())
                    },
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
                    ctx.add_assistant_message("", None, Some(reasoning_content.clone()));
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
            ctx.add_assistant_message(
                &assistant_content,
                None,
                if reasoning_content.is_empty() {
                    None
                } else {
                    Some(reasoning_content.clone())
                },
            );

            // 最终回复后增量持久化
            self.persist_new_messages(ctx);
            ctx.mark_persisted();

            // 正常完成前发射上下文使用情况
            let response_tokens = if let Some(ref usage) = final_usage {
                usage.completion_tokens as usize
            } else {
                crate::services::agent::prompts::token_budget::TokenBudgetManager::estimate_tokens(
                    &assistant_content,
                )
            };
            self.emit_context_usage(ctx, response_tokens, final_usage.as_ref())
                .await;

            let total_duration_ms = start_time.elapsed().as_millis() as u64;
            log::info!(
                "Agent 执行完成, session_id={}, 总步骤={}, 总耗时={}ms",
                ctx.session_id,
                total_steps,
                total_duration_ms
            );
            self.emitter
                .emit_done(DonePayload {
                    session_id: ctx.session_id.clone(),
                    summary: assistant_content.clone(),
                    total_steps,
                    duration_ms: total_duration_ms,
                })
                .ok();

            return Ok(ExecutionResult {
                summary: assistant_content,
                total_steps,
                duration_ms: total_duration_ms,
            });
        }

        // 超过最大迭代次数
        let error = CommandError::agent(
            crate::errors::AGENT_MAX_ITERATIONS,
            format!("Agent 执行超过最大迭代次数 ({})", self.max_iterations),
        );
        log::error!(
            "Agent 执行超过最大迭代次数, session_id={}, max_iterations={}",
            ctx.session_id,
            self.max_iterations
        );
        self.emitter
            .emit_error(ErrorPayload {
                session_id: ctx.session_id.clone(),
                code: error.code,
                message: error.message.clone(),
                recoverable: false,
            })
            .ok();
        // 持久化 error 节点信息（超过最大迭代次数，Agent 终止）
        ctx.add_assistant_message_with_metadata(
            &error.message,
            Some(serde_json::json!({
                "nodeType": "error",
                "code": error.code,
                "message": error.message,
                "recoverable": false,
            })),
        );
        self.persist_new_messages(ctx);
        ctx.mark_persisted();

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
            reasoning_content: if reasoning.is_empty() {
                None
            } else {
                Some(reasoning.to_string())
            },
            tool_calls: if tool_calls_vec.is_empty() {
                None
            } else {
                Some(tool_calls_vec)
            },
            tool_call_id: None,
            attachments: None,
            metadata: None,
        });

        messages.push(ChatMessage {
            role: "user".to_string(),
            content: "Please continue completing the previous response. Do not repeat already output content.".to_string(),
            content_parts: None,
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
            attachments: None,
            metadata: None,
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
    fn extract_and_remove_tool_call_tags(
        content: &str,
        extracted: &mut Vec<ExtractedToolCall>,
    ) -> String {
        let open_tag = "<tool-call>";
        let close_tag = "</tool-call>";
        let mut result = content.to_string();
        let mut search_from = 0;

        while let Some(pos) = result[search_from..].find(open_tag) {
            let start = search_from + pos;

            // 尝试查找闭合标签
            let (block_end, content_end) =
                if let Some(pos) = result[start + open_tag.len()..].find(close_tag) {
                    // 正常闭合：block_end 是闭合标签结束位置，content_end 是内容结束位置
                    (
                        start + open_tag.len() + pos + close_tag.len(),
                        start + open_tag.len() + pos,
                    )
                } else {
                    // 未闭合：尝试在特殊 token 之前截断内容
                    // DeepSeek R1 可能输出 <tool-call>...<｜tool▁call▁end｜> 而非 <tool-call>...</tool-call>
                    let after_open = &result[start + open_tag.len()..];
                    let (content_end_offset, block_end_offset) =
                        Self::find_tool_call_content_end(after_open);
                    (
                        start + open_tag.len() + block_end_offset,
                        start + open_tag.len() + content_end_offset,
                    )
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
        let (special_pos, special_end) = special_patterns
            .iter()
            .filter_map(|pattern| content.find(pattern).map(|pos| (pos, pos + pattern.len())))
            .min_by_key(|(pos, _)| *pos)
            .unwrap_or((content.len(), content.len()));

        // 查找代码块结束标记（第二个 ```）
        let code_block_content_end = content
            .find("```")
            .map(|first_pos| {
                content[first_pos + 3..]
                    .find("```")
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
        let name = value
            .get("function")
            .or_else(|| value.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_high_risk_command() {
        // 高风险命令应返回 true
        assert!(is_high_risk_command("rm -rf /"));
        assert!(is_high_risk_command("rm -r /home"));
        assert!(is_high_risk_command("rm -f file.txt"));
        // 不带 flag 的 rm 命令也应被识别为高风险
        assert!(is_high_risk_command("rm test.txt"));
        assert!(is_high_risk_command("rm /tmp/old_file.log"));
        assert!(is_high_risk_command("rmdir /s /q test"));
        assert!(is_high_risk_command("del /f file.txt"));
        assert!(is_high_risk_command("format C:"));
        assert!(is_high_risk_command("mkfs.ext4 /dev/sda"));
        assert!(is_high_risk_command("shutdown /s /t 0"));
        assert!(is_high_risk_command("reboot"));
        assert!(is_high_risk_command("halt"));
        assert!(is_high_risk_command("poweroff"));
        assert!(is_high_risk_command("sudo rm -rf /"));
        assert!(is_high_risk_command("su root"));
        assert!(is_high_risk_command("reg delete HKLM\\Software\\Test"));
        assert!(is_high_risk_command("reg add HKLM\\Software\\Test"));
        assert!(is_high_risk_command("killall nginx"));
        assert!(is_high_risk_command("taskkill /f /im notepad.exe"));
        assert!(is_high_risk_command("taskkill /im notepad.exe"));
        assert!(is_high_risk_command(
            "curl http://example.com/script.sh | bash"
        ));
        assert!(is_high_risk_command("wget http://example.com/script.sh"));
        assert!(is_high_risk_command("echo test | bash"));
        assert!(is_high_risk_command("echo test | sh"));
        assert!(is_high_risk_command("echo test | python"));
        assert!(is_high_risk_command("nohup ./server &"));
        assert!(is_high_risk_command("git push --force origin main"));
        assert!(is_high_risk_command("git push -f origin main"));
        assert!(is_high_risk_command("git reset --hard HEAD~3"));
        assert!(is_high_risk_command("git clean -f"));
        assert!(is_high_risk_command("git checkout ."));
        assert!(is_high_risk_command("git restore ."));
        assert!(is_high_risk_command("dd if=/dev/zero of=/dev/sda"));
        assert!(is_high_risk_command("kill -9 1234"));

        // 非高风险命令应返回 false
        assert!(!is_high_risk_command("ls -la"));
        assert!(!is_high_risk_command("echo hello"));
        assert!(!is_high_risk_command("python script.py"));
        assert!(!is_high_risk_command("git status"));
        assert!(!is_high_risk_command("git add ."));
        assert!(!is_high_risk_command("git commit -m 'test'"));
        assert!(!is_high_risk_command("cargo build"));
        assert!(!is_high_risk_command("format_code.sh")); // "format_code" 不应匹配 "format "
        assert!(!is_high_risk_command("delete file.txt")); // "delete" 不应匹配 "del "
        assert!(!is_high_risk_command("formatter")); // "formatter" 不应匹配 "rm " 或 "format "
        assert!(!is_high_risk_command("rmfile")); // "rmfile" 不应匹配 "rm "(无空格)
    }
}
