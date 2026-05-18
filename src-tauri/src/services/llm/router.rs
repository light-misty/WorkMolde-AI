use std::collections::HashMap;

use crate::config::llm_config::{LlmConfig, ProviderType};
use crate::errors::CommandError;
use crate::models::llm::*;
use super::provider::LlmProvider;
use super::openai_adapter::OpenAiAdapter;

/// Provider 元数据，用于 list_providers 返回完整信息
struct ProviderMeta {
    name: String,
    provider_type: String,
    api_base: String,
    model: String,
    created_at: String,
}

/// LLM Provider 路由器
/// 管理多个 LLM Provider，支持默认选择和 Fallback 切换
pub struct LlmRouter {
    providers: HashMap<String, Box<dyn LlmProvider>>,
    meta: HashMap<String, ProviderMeta>,
    default_id: Option<String>,
    fallback_order: Vec<String>,
}

impl LlmRouter {
    /// 从配置创建路由器
    pub fn from_config(config: &LlmConfig) -> Self {
        let mut providers: HashMap<String, Box<dyn LlmProvider>> = HashMap::new();
        let mut meta: HashMap<String, ProviderMeta> = HashMap::new();
        let mut default_id = None;

        for provider in &config.providers {
            let advanced = provider.advanced.clone();
            let provider_type_str = match provider.provider_type {
                ProviderType::OpenAI => "openai",
                ProviderType::Anthropic => "anthropic",
                ProviderType::Ollama => "ollama",
                ProviderType::Custom => "custom",
            };
            let adapter: Box<dyn LlmProvider> = match provider.provider_type {
                ProviderType::OpenAI | ProviderType::Custom => {
                    Box::new(OpenAiAdapter::new(
                        provider.api_base_url.clone(),
                        provider.api_key_encrypted.clone(),
                        provider.model.clone(),
                        advanced,
                    ))
                }
                ProviderType::Anthropic => {
                    // Anthropic 暂时使用 OpenAI 兼容模式
                    Box::new(OpenAiAdapter::new(
                        provider.api_base_url.clone(),
                        provider.api_key_encrypted.clone(),
                        provider.model.clone(),
                        advanced,
                    ))
                }
                ProviderType::Ollama => {
                    // Ollama 兼容 OpenAI API 格式
                    Box::new(OpenAiAdapter::new(
                        provider.api_base_url.clone(),
                        provider.api_key_encrypted.clone(),
                        provider.model.clone(),
                        advanced,
                    ))
                }
            };

            meta.insert(provider.id.clone(), ProviderMeta {
                name: provider.name.clone(),
                provider_type: provider_type_str.to_string(),
                api_base: provider.api_base_url.clone(),
                model: provider.model.clone(),
                created_at: String::new(),
            });

            if provider.is_default {
                default_id = Some(provider.id.clone());
            }
            providers.insert(provider.id.clone(), adapter);
        }

        log::info!("LLM 路由器初始化完成, 加载 {} 个 Provider, 默认 Provider: {:?}, Fallback 顺序: {:?}", providers.len(), default_id, config.fallback_order);

        Self {
            providers,
            meta,
            default_id,
            fallback_order: config.fallback_order.clone(),
        }
    }

    /// 创建空路由器
    pub fn empty() -> Self {
        Self {
            providers: HashMap::new(),
            meta: HashMap::new(),
            default_id: None,
            fallback_order: Vec::new(),
        }
    }

    /// 检查路由器是否为空（无可用 Provider）
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// 非流式对话，自动选择 Provider
    pub async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ChatResponse, CommandError> {
        let provider = self.get_default_provider()
            .ok_or_else(|| CommandError::llm(1002, "未配置 LLM Provider".to_string()))?;

        log::info!("非流式对话, 使用默认 Provider: {}", self.default_id.as_deref().unwrap_or("首个可用"));

        match provider.chat(messages, tools).await {
            Ok(response) => {
                log::info!("非流式对话完成, Provider: {}", self.default_id.as_deref().unwrap_or("首个可用"));
                Ok(response)
            }
            Err(e) => {
                log::warn!("默认 Provider 请求失败, 尝试 Fallback, 错误: {}", e.message);
                for fallback_id in &self.fallback_order {
                    if let Some(fb_provider) = self.providers.get(fallback_id) {
                        log::info!("尝试 Fallback Provider: {}", fallback_id);
                        if let Ok(response) = fb_provider.chat(messages, tools).await {
                            log::info!("Fallback 成功, Provider: {}", fallback_id);
                            return Ok(response);
                        }
                    }
                }
                log::error!("所有 Provider 均失败, 无可用 Fallback");
                Err(e)
            }
        }
    }

    /// 流式对话，自动选择 Provider
    pub async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamChunk, CommandError>>, CommandError> {
        let provider = self.get_default_provider()
            .ok_or_else(|| CommandError::llm(1002, "未配置 LLM Provider".to_string()))?;
        log::info!("流式对话, 使用默认 Provider: {}", self.default_id.as_deref().unwrap_or("首个可用"));
        provider.chat_stream(messages, tools).await
    }

    /// 测试指定 Provider 的连接
    pub async fn test_connection(&self, provider_id: &str) -> Result<ConnectionResult, CommandError> {
        log::info!("测试 Provider 连接, provider_id={}", provider_id);
        let provider = self.providers.get(provider_id)
            .ok_or_else(|| CommandError::llm(1002, format!("Provider 不存在: {}", provider_id)))?;
        let mut result = provider.test_connection().await?;
        log::info!("Provider 连接测试完成, provider_id={}, 成功={}", provider_id, result.success);
        result.provider_id = Some(provider_id.to_string());
        Ok(result)
    }

    /// 获取默认 Provider
    fn get_default_provider(&self) -> Option<&dyn LlmProvider> {
        if let Some(id) = &self.default_id {
            self.providers.get(id).map(|p| p.as_ref())
        } else {
            self.providers.values().next().map(|p| p.as_ref())
        }
    }

    /// 列出所有 Provider 信息，包含完整的元数据
    pub fn list_providers(&self) -> Vec<ProviderInfo> {
        self.providers.keys().map(|id| {
            let m = self.meta.get(id);
            ProviderInfo {
                id: id.clone(),
                name: m.map(|m| m.name.clone()).unwrap_or_default(),
                provider_type: m.map(|m| m.provider_type.clone()).unwrap_or_default(),
                api_base: m.map(|m| m.api_base.clone()).unwrap_or_default(),
                model: m.map(|m| m.model.clone()).unwrap_or_default(),
                is_default: self.default_id.as_ref() == Some(id),
                is_available: true,
                created_at: m.map(|m| m.created_at.clone()).unwrap_or_default(),
                is_connected: None,
            }
        }).collect()
    }
}
