use std::collections::HashMap;
use std::sync::RwLock as StdRwLock;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

use super::anthropic_adapter::AnthropicAdapter;
use super::gemini_adapter::GeminiAdapter;
use super::openai_adapter::OpenAiAdapter;
use super::provider::LlmProvider;
use crate::config::llm_config::{LlmConfig, ProviderType};
use crate::errors::CommandError;
use crate::events::types::{ProviderSwitchPayload, LLM_PROVIDER_SWITCH};
use crate::models::llm::*;

/// 连续失败次数阈值，超过此值标记为不可用
const MAX_CONSECUTIVE_FAILURES: u32 = 3;
/// 不可用 Provider 的自动恢复时间（5 分钟）
const RECOVERY_DURATION: Duration = Duration::from_secs(300);
/// 健康检查超时时间（10 秒）
const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(10);
/// 延迟指数移动平均的平滑因子（历史权重）
const LATENCY_EMA_ALPHA: f64 = 0.7;

/// Provider 元数据，用于 list_providers 返回完整信息
struct ProviderMeta {
    name: String,
    provider_type: String,
    api_base: String,
    model: String,
    created_at: String,
    /// 上下文窗口大小 (tokens)，运行时计算后的最终值
    context_window: usize,
    /// 是否支持视觉/图片多模态
    supports_vision: bool,
}

/// Provider 健康状态
struct ProviderHealth {
    /// 是否可用
    is_available: bool,
    /// 连续失败次数
    consecutive_failures: u32,
    /// 最近一次错误信息
    last_error: Option<String>,
    /// 最近一次成功时间
    last_success_at: Option<std::time::Instant>,
    /// 最近一次失败时间
    last_failure_at: Option<std::time::Instant>,
    /// 平均响应延迟（毫秒），使用指数移动平均
    avg_latency_ms: u64,
}

impl Default for ProviderHealth {
    fn default() -> Self {
        Self {
            is_available: true,
            consecutive_failures: 0,
            last_error: None,
            last_success_at: None,
            last_failure_at: None,
            avg_latency_ms: 0,
        }
    }
}

/// LLM Provider 路由器
/// 管理多个 LLM Provider，支持默认选择、Fallback 切换和健康检查
pub struct LlmRouter {
    /// Provider 注册表，使用 tokio RwLock 支持跨 await 持锁
    providers: RwLock<HashMap<String, Box<dyn LlmProvider>>>,
    meta: HashMap<String, ProviderMeta>,
    default_id: Option<String>,
    fallback_order: Vec<String>,
    /// Provider 健康状态追踪（使用 std RwLock，不跨 await）
    health: StdRwLock<HashMap<String, ProviderHealth>>,
    /// Tauri AppHandle，用于发送 Fallback 切换通知事件
    app_handle: Option<AppHandle<tauri::Wry>>,
}

impl LlmRouter {
    /// 从配置创建路由器
    pub fn from_config(config: &LlmConfig) -> Self {
        let mut providers: HashMap<String, Box<dyn LlmProvider>> = HashMap::new();
        let mut meta: HashMap<String, ProviderMeta> = HashMap::new();
        // 默认 Provider 取列表第一个（用于路由回退，不再有"设为默认"操作）
        let default_id = config.providers.first().map(|p| p.id.clone());

        for provider in &config.providers {
            let mut advanced = provider.advanced.clone();
            let provider_type_str = match provider.provider_type {
                ProviderType::OpenAI => "openai",
                ProviderType::Anthropic => "anthropic",
                ProviderType::Ollama => "ollama",
                ProviderType::Custom => "custom",
                ProviderType::Gemini => "gemini",
            };

            // 根据 Provider 类型和 API base URL 自动检测 reasoning_in_content 配置
            let is_deepseek = provider.api_base_url.to_lowercase().contains("deepseek");
            if is_deepseek {
                advanced.reasoning_in_content = false;
                log::info!(
                    "检测到 DeepSeek Provider, 设置 reasoning_in_content=false, id={}",
                    provider.id
                );
            }

            let adapter: Box<dyn LlmProvider> = match provider.provider_type {
                ProviderType::OpenAI | ProviderType::Custom => Box::new(OpenAiAdapter::new(
                    provider.api_base_url.clone(),
                    provider.api_key_encrypted.clone(),
                    provider.model.clone(),
                    advanced,
                )),
                ProviderType::Anthropic => Box::new(AnthropicAdapter::new(
                    provider.api_base_url.clone(),
                    provider.api_key_encrypted.clone(),
                    provider.model.clone(),
                    advanced,
                )),
                ProviderType::Ollama => Box::new(OpenAiAdapter::new(
                    provider.api_base_url.clone(),
                    provider.api_key_encrypted.clone(),
                    provider.model.clone(),
                    advanced,
                )),
                ProviderType::Gemini => Box::new(GeminiAdapter::new(
                    provider.api_base_url.clone(),
                    provider.api_key_encrypted.clone(),
                    provider.model.clone(),
                    advanced,
                )),
            };

            meta.insert(
                provider.id.clone(),
                ProviderMeta {
                    name: provider.name.clone(),
                    provider_type: provider_type_str.to_string(),
                    api_base: provider.api_base_url.clone(),
                    model: provider.model.clone(),
                    created_at: String::new(),
                    context_window: provider.resolve_context_window(),
                    supports_vision: provider.supports_vision,
                },
            );

            providers.insert(provider.id.clone(), adapter);
        }

        log::info!(
            "LLM 路由器初始化完成, 加载 {} 个 Provider, 主 Provider: {:?}, Fallback 顺序: {:?}",
            providers.len(),
            default_id,
            config.fallback_order
        );

        Self {
            providers: RwLock::new(providers),
            meta,
            default_id,
            fallback_order: config.fallback_order.clone(),
            health: StdRwLock::new(HashMap::new()),
            app_handle: None,
        }
    }

    /// 创建空路由器
    pub fn empty() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            meta: HashMap::new(),
            default_id: None,
            fallback_order: Vec::new(),
            health: StdRwLock::new(HashMap::new()),
            app_handle: None,
        }
    }

    /// 设置 Tauri AppHandle，用于发送事件通知
    pub fn with_app_handle(mut self, handle: Option<AppHandle<tauri::Wry>>) -> Self {
        self.app_handle = handle;
        self
    }

    /// 获取 AppHandle 的克隆（用于重建路由器时保留引用）
    pub fn app_handle(&self) -> Option<AppHandle<tauri::Wry>> {
        self.app_handle.clone()
    }

    /// 检查路由器是否为空（无可用 Provider）
    pub async fn is_empty(&self) -> bool {
        self.providers.read().await.is_empty()
    }

    // ================================================================
    // 健康状态管理
    // ================================================================

    /// 标记 Provider 成功
    fn mark_success(&self, provider_id: &str, latency_ms: u64) {
        let mut health_map = self.health.write().unwrap();
        let health = health_map.entry(provider_id.to_string()).or_default();
        health.is_available = true;
        health.consecutive_failures = 0;
        health.last_error = None;
        health.last_success_at = Some(std::time::Instant::now());
        if health.avg_latency_ms == 0 {
            health.avg_latency_ms = latency_ms;
        } else {
            health.avg_latency_ms = (health.avg_latency_ms as f64 * LATENCY_EMA_ALPHA
                + latency_ms as f64 * (1.0 - LATENCY_EMA_ALPHA))
                as u64;
        }
        log::debug!(
            "Provider {} 标记成功, 延迟={}ms, 平均延迟={}ms",
            provider_id,
            latency_ms,
            health.avg_latency_ms
        );
    }

    /// 标记 Provider 失败
    fn mark_failure(&self, provider_id: &str, error: &str) {
        let mut health_map = self.health.write().unwrap();
        let health = health_map.entry(provider_id.to_string()).or_default();
        health.consecutive_failures += 1;
        health.last_error = Some(error.to_string());
        health.last_failure_at = Some(std::time::Instant::now());

        if health.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
            health.is_available = false;
            log::warn!(
                "Provider {} 连续失败 {} 次，标记为不可用",
                provider_id,
                health.consecutive_failures
            );
        }

        log::debug!(
            "Provider {} 标记失败, 连续失败次数={}, 错误: {}",
            provider_id,
            health.consecutive_failures,
            error
        );
    }

    /// 检查 Provider 是否可用（根据健康状态判断，含自动恢复逻辑）
    fn is_provider_available(&self, provider_id: &str) -> bool {
        let mut health_map = self.health.write().unwrap();
        match health_map.get_mut(provider_id) {
            Some(h) => {
                if !h.is_available {
                    if let Some(last_failure) = h.last_failure_at {
                        if last_failure.elapsed() >= RECOVERY_DURATION {
                            log::info!(
                                "Provider {} 已过恢复期（{}秒），自动标记为可用",
                                provider_id,
                                RECOVERY_DURATION.as_secs()
                            );
                            h.is_available = true;
                            h.consecutive_failures = 0;
                        }
                    }
                }
                h.is_available
            }
            None => true,
        }
    }

    /// 对所有 Provider 执行健康检查
    /// 优先使用轻量级健康检查（HEAD 请求），减少 Token 消耗
    pub async fn health_check_all(&self) -> HashMap<String, ConnectionResult> {
        let mut results = HashMap::new();

        let provider_ids: Vec<String> = self.providers.read().await.keys().cloned().collect();

        for id in provider_ids {
            let result = {
                let providers = self.providers.read().await;
                let provider = match providers.get(&id) {
                    Some(p) => p,
                    None => continue,
                };
                // 优先使用轻量级健康检查，减少 Token 消耗
                tokio::time::timeout(HEALTH_CHECK_TIMEOUT, provider.lightweight_health_check())
                    .await
            };

            let conn_result = match result {
                Ok(Ok(mut r)) => {
                    r.provider_id = Some(id.clone());
                    if r.success {
                        self.mark_success(&id, r.latency_ms);
                    } else {
                        let error_msg = r
                            .error
                            .as_deref()
                            .or(r.error_message.as_deref())
                            .unwrap_or("未知错误");
                        self.mark_failure(&id, error_msg);
                    }
                    r
                }
                Ok(Err(e)) => {
                    self.mark_failure(&id, &e.message);
                    ConnectionResult {
                        success: false,
                        provider_id: Some(id.clone()),
                        latency_ms: 0,
                        model_info: None,
                        model: None,
                        error_message: Some(e.message.clone()),
                        error: Some(e.message),
                    }
                }
                Err(_) => {
                    self.mark_failure(&id, "连接超时");
                    ConnectionResult {
                        success: false,
                        provider_id: Some(id.clone()),
                        latency_ms: 0,
                        model_info: None,
                        model: None,
                        error_message: Some("连接超时".to_string()),
                        error: Some("连接超时".to_string()),
                    }
                }
            };

            log::info!(
                "健康检查: Provider {}, 成功={}, 延迟={}ms",
                id,
                conn_result.success,
                conn_result.latency_ms
            );
            results.insert(id.clone(), conn_result);
        }

        log::info!("健康检查完成, 检查了 {} 个 Provider", results.len());
        results
    }

    /// 强制恢复所有 Provider 为可用状态（用于网络恢复时）
    pub async fn force_recover_all(&self) {
        let provider_ids: Vec<String> = self.providers.read().await.keys().cloned().collect();
        let mut health_map = self.health.write().unwrap();
        for id in provider_ids {
            if let Some(health) = health_map.get_mut(&id) {
                if !health.is_available {
                    health.is_available = true;
                    health.consecutive_failures = 0;
                    log::info!("强制恢复 Provider {} 为可用状态", id);
                }
            }
        }
    }

    /// 重建所有 Provider 的 HTTP 客户端（用于网络切换后清理连接池）
    pub async fn rebuild_all_clients(&self) {
        let mut providers = self.providers.write().await;
        for (id, provider) in providers.iter_mut() {
            provider.rebuild_client();
            log::info!("重建 Provider {} 的 HTTP 客户端", id);
        }
    }

    // ================================================================
    // 事件通知
    // ================================================================

    /// 发送 Provider 切换通知事件
    fn emit_provider_switch(&self, from_id: &str, to_id: &str, reason: &str, is_automatic: bool) {
        if let Some(ref app_handle) = self.app_handle {
            let payload = ProviderSwitchPayload {
                from_provider_id: from_id.to_string(),
                to_provider_id: to_id.to_string(),
                reason: reason.to_string(),
                is_automatic,
            };
            if let Err(e) = app_handle.emit(LLM_PROVIDER_SWITCH, payload) {
                log::warn!("发送 Provider 切换通知失败: {}", e);
            }
        }
    }

    // ================================================================
    // 对话方法（含健康追踪和 Fallback 通知）
    // ================================================================

    /// 非流式对话，自动选择 Provider，支持健康检查和 Fallback 通知
    pub async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ChatResponse, CommandError> {
        let default_id = self
            .default_id
            .clone()
            .ok_or_else(|| CommandError::llm(1002, "未配置 LLM Provider".to_string()))?;

        log::info!("非流式对话, 使用默认 Provider: {}", default_id);

        if self.is_provider_available(&default_id) {
            let providers = self.providers.read().await;
            if let Some(provider) = providers.get(&default_id) {
                let start = std::time::Instant::now();
                match provider.chat(messages, tools).await {
                    Ok(response) => {
                        let latency = start.elapsed().as_millis() as u64;
                        self.mark_success(&default_id, latency);
                        log::info!("非流式对话完成, Provider: {}", default_id);
                        return Ok(response);
                    }
                    Err(e) => {
                        self.mark_failure(&default_id, &e.message);
                        log::warn!("默认 Provider 请求失败, 尝试 Fallback, 错误: {}", e.message);
                        drop(providers);
                        return self.fallback_chat(messages, tools, &default_id, e).await;
                    }
                }
            }
        } else {
            log::warn!(
                "默认 Provider {} 不可用（健康检查未通过），跳过",
                default_id
            );
        }

        let error = CommandError::llm(
            crate::errors::LLM_PROVIDER_UNAVAILABLE,
            format!("默认 Provider {} 不可用", default_id),
        );
        self.fallback_chat(messages, tools, &default_id, error)
            .await
    }

    /// 非流式 Fallback 逻辑
    async fn fallback_chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        failed_provider_id: &str,
        original_error: CommandError,
    ) -> Result<ChatResponse, CommandError> {
        for fallback_id in &self.fallback_order {
            if fallback_id == failed_provider_id {
                continue;
            }

            if !self.is_provider_available(fallback_id) {
                log::info!("Fallback Provider {} 不可用，跳过", fallback_id);
                continue;
            }

            let providers = self.providers.read().await;
            if let Some(fb_provider) = providers.get(fallback_id) {
                log::info!("尝试 Fallback Provider: {}", fallback_id);
                let start = std::time::Instant::now();
                match fb_provider.chat(messages, tools).await {
                    Ok(response) => {
                        let latency = start.elapsed().as_millis() as u64;
                        self.mark_success(fallback_id, latency);
                        log::info!("Fallback 成功, Provider: {}", fallback_id);
                        self.emit_provider_switch(
                            failed_provider_id,
                            fallback_id,
                            "默认 Provider 请求失败",
                            true,
                        );
                        return Ok(response);
                    }
                    Err(e) => {
                        self.mark_failure(fallback_id, &e.message);
                        log::warn!("Fallback Provider {} 也失败: {}", fallback_id, e.message);
                    }
                }
            }
        }
        log::error!("所有 Provider 均失败, 无可用 Fallback");
        Err(original_error)
    }

    /// 流式对话，自动选择 Provider，支持健康检查和 Fallback 通知
    /// preferred_provider_id 为 Some 时优先使用指定 Provider（不存在时回退到默认）
    pub async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        preferred_provider_id: Option<&str>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamChunk, CommandError>>, CommandError> {
        let provider_id = self
            .resolve_stream_provider_id(preferred_provider_id)
            .await?;

        log::info!("流式对话, 使用 Provider: {}", provider_id);

        if self.is_provider_available(&provider_id) {
            let providers = self.providers.read().await;
            if let Some(provider) = providers.get(&provider_id) {
                let start = std::time::Instant::now();
                match provider.chat_stream(messages, tools).await {
                    Ok(rx) => {
                        let latency = start.elapsed().as_millis() as u64;
                        self.mark_success(&provider_id, latency);
                        return Ok(rx);
                    }
                    Err(e) => {
                        self.mark_failure(&provider_id, &e.message);
                        log::warn!(
                            "Provider {} 流式请求失败, 尝试 Fallback, 错误: {}",
                            provider_id,
                            e.message
                        );
                        drop(providers);
                        return self
                            .fallback_chat_stream(messages, tools, &provider_id, e)
                            .await;
                    }
                }
            }
        } else {
            log::warn!("Provider {} 不可用（健康检查未通过），跳过", provider_id);
        }

        let error = CommandError::llm(
            crate::errors::LLM_PROVIDER_UNAVAILABLE,
            format!("Provider {} 不可用", provider_id),
        );
        self.fallback_chat_stream(messages, tools, &provider_id, error)
            .await
    }

    /// 流式对话，支持覆盖 max_tokens 参数
    /// 用于响应截断时以更大的 max_tokens 重试，避免因输出限制导致 tool_call 参数不完整
    /// preferred_provider_id 为 Some 时优先使用指定 Provider（不存在时回退到默认）
    pub async fn chat_stream_with_max_tokens(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        max_tokens_override: u32,
        preferred_provider_id: Option<&str>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamChunk, CommandError>>, CommandError> {
        let provider_id = self
            .resolve_stream_provider_id(preferred_provider_id)
            .await?;

        log::info!(
            "流式对话 (max_tokens={}), 使用 Provider: {}",
            max_tokens_override,
            provider_id
        );

        if self.is_provider_available(&provider_id) {
            let providers = self.providers.read().await;
            if let Some(provider) = providers.get(&provider_id) {
                let start = std::time::Instant::now();
                match provider
                    .chat_stream_with_max_tokens(messages, tools, max_tokens_override)
                    .await
                {
                    Ok(rx) => {
                        let latency = start.elapsed().as_millis() as u64;
                        self.mark_success(&provider_id, latency);
                        return Ok(rx);
                    }
                    Err(e) => {
                        self.mark_failure(&provider_id, &e.message);
                        log::warn!(
                            "Provider {} 流式请求 (max_tokens={}) 失败, 尝试 Fallback, 错误: {}",
                            provider_id,
                            max_tokens_override,
                            e.message
                        );
                        drop(providers);
                        return self
                            .fallback_chat_stream(messages, tools, &provider_id, e)
                            .await;
                    }
                }
            }
        } else {
            log::warn!("Provider {} 不可用（健康检查未通过），跳过", provider_id);
        }

        let error = CommandError::llm(
            crate::errors::LLM_PROVIDER_UNAVAILABLE,
            format!("Provider {} 不可用", provider_id),
        );
        self.fallback_chat_stream(messages, tools, &provider_id, error)
            .await
    }

    /// 解析流式对话应使用的 Provider ID：优先使用用户指定的 Provider，不存在时回退默认 Provider
    async fn resolve_stream_provider_id(
        &self,
        preferred_provider_id: Option<&str>,
    ) -> Result<String, CommandError> {
        let providers = self.providers.read().await;
        if let Some(id) = preferred_provider_id {
            if providers.contains_key(id) {
                return Ok(id.to_string());
            }
            log::warn!("指定的首选 Provider {} 不存在，回退到默认 Provider", id);
        }
        self.default_id
            .clone()
            .ok_or_else(|| CommandError::llm(1002, "未配置 LLM Provider".to_string()))
    }

    /// 流式 Fallback 逻辑
    async fn fallback_chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        failed_provider_id: &str,
        original_error: CommandError,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamChunk, CommandError>>, CommandError> {
        for fallback_id in &self.fallback_order {
            if fallback_id == failed_provider_id {
                continue;
            }

            if !self.is_provider_available(fallback_id) {
                log::info!("Fallback Provider {} 不可用，跳过", fallback_id);
                continue;
            }

            let providers = self.providers.read().await;
            if let Some(fb_provider) = providers.get(fallback_id) {
                log::info!("尝试 Fallback Provider (流式): {}", fallback_id);
                let start = std::time::Instant::now();
                match fb_provider.chat_stream(messages, tools).await {
                    Ok(rx) => {
                        let latency = start.elapsed().as_millis() as u64;
                        self.mark_success(fallback_id, latency);
                        log::info!("Fallback (流式) 成功, Provider: {}", fallback_id);
                        self.emit_provider_switch(
                            failed_provider_id,
                            fallback_id,
                            "默认 Provider 流式请求失败",
                            true,
                        );
                        return Ok(rx);
                    }
                    Err(e) => {
                        self.mark_failure(fallback_id, &e.message);
                        log::warn!(
                            "Fallback Provider {} (流式) 也失败: {}",
                            fallback_id,
                            e.message
                        );
                    }
                }
            }
        }
        log::error!("所有 Provider 流式请求均失败, 无可用 Fallback");
        Err(original_error)
    }

    // ================================================================
    // 其他方法
    // ================================================================

    /// 获取指定 Provider 的 max_tokens 配置
    /// 用于截断重试时计算翻倍后的 max_tokens
    /// 若 provider_id 为 None 或空，回退到默认 Provider
    pub async fn get_max_tokens_for(&self, provider_id: Option<&str>) -> u32 {
        let pid = provider_id
            .filter(|s| !s.is_empty())
            .or(self.default_id.as_deref())
            .map(|s| s.to_string());
        let pid = match pid {
            Some(id) => id,
            None => return crate::config::llm_config::AdvancedConfig::default().max_tokens,
        };
        let providers = self.providers.read().await;
        if let Some(provider) = providers.get(&pid) {
            provider.get_max_tokens()
        } else {
            crate::config::llm_config::AdvancedConfig::default().max_tokens
        }
    }

    /// 执行上下文压缩
    /// 优先使用 preferred_provider_id 指定的 Provider，为 None 时回退到默认 Provider
    /// 持有 providers 读锁直到压缩完成（与 chat() 方法的锁持有模式一致）
    pub async fn compact_messages(
        &self,
        messages: &[ChatMessage],
        compactor: &crate::services::agent::compaction::ContextCompactor,
        preferred_provider_id: Option<&str>,
    ) -> Result<crate::services::agent::compaction::CompactionResult, CommandError> {
        // 解析 Provider ID：优先使用 preferred_provider_id，为 None 或不存在时回退到 default_id
        let provider_id = {
            let providers = self.providers.read().await;
            if let Some(id) = preferred_provider_id {
                if providers.contains_key(id) {
                    id.to_string()
                } else {
                    log::warn!("指定的 Provider {} 不存在，回退到默认 Provider", id);
                    self.default_id
                        .clone()
                        .ok_or_else(|| CommandError::llm(1002, "未配置 LLM Provider".to_string()))?
                }
            } else {
                self.default_id
                    .clone()
                    .ok_or_else(|| CommandError::llm(1002, "未配置 LLM Provider".to_string()))?
            }
        };

        // 检查 Provider 可用性
        if !self.is_provider_available(&provider_id) {
            return Err(CommandError::llm(
                crate::errors::LLM_PROVIDER_UNAVAILABLE,
                format!("Provider {} 不可用", provider_id),
            ));
        }

        let providers = self.providers.read().await;
        let provider = providers
            .get(&provider_id)
            .ok_or_else(|| CommandError::llm(1002, format!("Provider 不存在: {}", provider_id)))?;

        compactor.compact(messages, provider.as_ref()).await
    }

    /// 测试指定 Provider 的连接
    pub async fn test_connection(
        &self,
        provider_id: &str,
    ) -> Result<ConnectionResult, CommandError> {
        log::info!("测试 Provider 连接, provider_id={}", provider_id);
        let providers = self.providers.read().await;
        let provider = providers
            .get(provider_id)
            .ok_or_else(|| CommandError::llm(1002, format!("Provider 不存在: {}", provider_id)))?;
        let mut result = provider.test_connection().await?;
        log::info!(
            "Provider 连接测试完成, provider_id={}, 成功={}",
            provider_id,
            result.success
        );
        result.provider_id = Some(provider_id.to_string());
        Ok(result)
    }

    /// 列出所有 Provider 信息，包含完整的元数据和健康状态
    pub fn list_providers(&self) -> Vec<ProviderInfo> {
        // 同步获取 providers 的读锁用于遍历 key
        // 注意：tokio RwLock 的 read() 是 async，但 list_providers 是同步方法
        // 这里使用 try_read() 避免在同步上下文中 await
        let providers = self.providers.try_read();
        match providers {
            Ok(p) => p
                .keys()
                .map(|id| {
                    let m = self.meta.get(id);
                    ProviderInfo {
                        id: id.clone(),
                        name: m.map(|m| m.name.clone()).unwrap_or_default(),
                        provider_type: m.map(|m| m.provider_type.clone()).unwrap_or_default(),
                        api_base: m.map(|m| m.api_base.clone()).unwrap_or_default(),
                        model: m.map(|m| m.model.clone()).unwrap_or_default(),
                        is_available: self.is_provider_available(id),
                        created_at: m.map(|m| m.created_at.clone()).unwrap_or_default(),
                        is_connected: None,
                        context_window: m.map(|m| m.context_window).unwrap_or(200_000),
                        supports_vision: m.map(|m| m.supports_vision).unwrap_or(false),
                    }
                })
                .collect(),
            Err(_) => {
                // 无法获取锁时，仅基于 meta 返回基本信息
                self.meta
                    .keys()
                    .map(|id| {
                        let m = self.meta.get(id);
                        ProviderInfo {
                            id: id.clone(),
                            name: m.map(|m| m.name.clone()).unwrap_or_default(),
                            provider_type: m.map(|m| m.provider_type.clone()).unwrap_or_default(),
                            api_base: m.map(|m| m.api_base.clone()).unwrap_or_default(),
                            model: m.map(|m| m.model.clone()).unwrap_or_default(),
                            is_available: true,
                            created_at: m.map(|m| m.created_at.clone()).unwrap_or_default(),
                            is_connected: None,
                            context_window: m.map(|m| m.context_window).unwrap_or(200_000),
                            supports_vision: m.map(|m| m.supports_vision).unwrap_or(false),
                        }
                    })
                    .collect()
            }
        }
    }

    /// 按 provider_id 查询上下文窗口大小
    /// 若 provider_id 为 None 或空，回退到默认 Provider
    pub fn context_window_for(&self, provider_id: Option<&str>) -> usize {
        let pid = provider_id.filter(|s| !s.is_empty()).or(self.default_id.as_deref());
        pid
            .and_then(|id| self.meta.get(id))
            .map(|m| m.context_window)
            .unwrap_or(200_000)
    }

    /// 按 provider_id 查询模型名称
    /// 若 provider_id 为 None 或空，回退到默认 Provider
    pub fn model_name_for(&self, provider_id: Option<&str>) -> String {
        let pid = provider_id.filter(|s| !s.is_empty()).or(self.default_id.as_deref());
        pid
            .and_then(|id| self.meta.get(id))
            .map(|m| m.model.clone())
            .unwrap_or_default()
    }

    /// 按 provider_id 查询缓存类型
    /// 若 provider_id 为 None 或空，回退到默认 Provider
    /// 返回 "deepseek" | "anthropic" | "gemini" | "none"
    pub fn cache_type_for(&self, provider_id: Option<&str>) -> &str {
        let pid = provider_id.filter(|s| !s.is_empty()).or(self.default_id.as_deref());
        pid
            .and_then(|id| self.meta.get(id))
            .map(|m| match m.provider_type.as_str() {
                "openai" | "custom" | "anthropic" => {
                    // DeepSeek 可通过 OpenAI ChatCompletions 或 Anthropic Messages 两种接口访问，
                    // 其磁盘缓存机制相同，均以 prompt_cache_hit_tokens / prompt_cache_miss_tokens 标识
                    if m.model.to_lowercase().contains("deepseek") {
                        "deepseek"
                    } else if m.provider_type == "anthropic" {
                        "anthropic"
                    } else {
                        "none"
                    }
                }
                "gemini" => "gemini",
                "ollama" => "none",
                _ => "none",
            })
            .unwrap_or("none")
    }

    /// 按 provider_id 查询是否支持视觉/图片多模态
    /// 若 provider_id 为 None 或空，回退到默认 Provider
    pub fn supports_vision_for(&self, provider_id: Option<&str>) -> bool {
        let pid = provider_id.filter(|s| !s.is_empty()).or(self.default_id.as_deref());
        pid
            .and_then(|id| self.meta.get(id))
            .map(|m| m.supports_vision)
            .unwrap_or(false)
    }
}
