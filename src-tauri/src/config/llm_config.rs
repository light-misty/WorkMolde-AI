use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::errors::{CommandError, CONFIG_PROVIDER_NOT_FOUND, CONFIG_DEFAULT_PROVIDER_REQUIRED};

/// LLM Provider 类型
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ProviderType {
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "custom")]
    Custom,
    #[serde(rename = "gemini")]
    Gemini,
}

/// Provider 高级配置
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedConfig {
    pub temperature: f64,
    pub top_p: f64,
    pub max_tokens: u32,
    pub timeout_seconds: u32,
    pub max_retries: u32,
    #[serde(default)]
    pub extra_headers: HashMap<String, String>,
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 1.0,
            max_tokens: 4096,
            timeout_seconds: 60,
            max_retries: 3,
            extra_headers: HashMap::new(),
        }
    }
}

/// LLM Provider 配置项
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LlmProvider {
    /// 唯一标识
    pub id: String,
    /// Provider 类型
    pub provider_type: ProviderType,
    /// 显示名称
    pub name: String,
    /// API 基础地址
    pub api_base_url: String,
    /// 加密后的 API Key
    pub api_key_encrypted: String,
    /// 模型名称
    pub model: String,
    /// 是否为默认 Provider
    pub is_default: bool,
    /// 高级配置
    #[serde(default)]
    pub advanced: AdvancedConfig,
}

/// LLM 配置，包含所有 Provider 及回退顺序
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    /// Provider 列表
    #[serde(default)]
    pub providers: Vec<LlmProvider>,
    /// 回退顺序，存储 Provider ID
    #[serde(default)]
    pub fallback_order: Vec<String>,
}

/// 获取 LLM 配置文件路径
fn config_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("config").join("llm_config.json")
}

/// 从磁盘加载 LLM 配置，文件不存在时返回默认值
pub fn load_llm_config(data_dir: &Path) -> Result<LlmConfig, CommandError> {
    let path = config_path(data_dir);
    if !path.exists() {
        log::info!("LLM 配置文件不存在，返回默认值: {}", path.display());
        return Ok(LlmConfig::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let config: LlmConfig = serde_json::from_str(&content)?;
    log::info!("已加载 LLM 配置 (providers数量: {})", config.providers.len());
    Ok(config)
}

/// 将 LLM 配置保存到磁盘
pub fn save_llm_config(data_dir: &Path, config: &LlmConfig) -> Result<(), CommandError> {
    let path = config_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, content)?;
    log::info!("已保存 LLM 配置 (providers数量: {})", config.providers.len());
    Ok(())
}

/// 获取默认 Provider
pub fn get_default_provider(config: &LlmConfig) -> Option<&LlmProvider> {
    config.providers.iter().find(|p| p.is_default)
}

/// 添加新 Provider
pub fn add_provider(config: &mut LlmConfig, provider: LlmProvider) -> Result<(), CommandError> {
    // 检查 ID 是否重复
    if config.providers.iter().any(|p| p.id == provider.id) {
        log::warn!("添加 Provider 失败，ID 已存在: {}", provider.id);
        return Err(CommandError::config(
            CONFIG_PROVIDER_NOT_FOUND,
            format!("Provider ID '{}' 已存在", provider.id),
        ));
    }

    // 如果新 Provider 设为默认，取消其他 Provider 的默认标记
    if provider.is_default {
        for p in &mut config.providers {
            p.is_default = false;
        }
    }

    // 如果是第一个 Provider，自动设为默认
    let mut provider = provider;
    if config.providers.is_empty() {
        provider.is_default = true;
        log::debug!("首个 Provider，自动设为默认: {}", provider.id);
    }

    config.providers.push(provider);
    log::info!("已添加 Provider，当前总数: {}", config.providers.len());
    Ok(())
}

/// 更新指定 ID 的 Provider
pub fn update_provider(
    config: &mut LlmConfig,
    id: &str,
    provider: LlmProvider,
) -> Result<(), CommandError> {
    let index = config
        .providers
        .iter()
        .position(|p| p.id == id)
        .ok_or_else(|| {
            log::warn!("更新 Provider 失败，不存在: {}", id);
            CommandError::config(
                CONFIG_PROVIDER_NOT_FOUND,
                format!("Provider '{}' 不存在", id),
            )
        })?;

    // 如果更新后设为默认，取消其他 Provider 的默认标记
    if provider.is_default {
        for p in &mut config.providers {
            p.is_default = false;
        }
    }

    config.providers[index] = provider;
    log::info!("已更新 Provider: {}", id);
    Ok(())
}

/// 删除指定 ID 的 Provider
pub fn delete_provider(config: &mut LlmConfig, id: &str) -> Result<(), CommandError> {
    let index = config
        .providers
        .iter()
        .position(|p| p.id == id)
        .ok_or_else(|| {
            log::warn!("删除 Provider 失败，不存在: {}", id);
            CommandError::config(
                CONFIG_PROVIDER_NOT_FOUND,
                format!("Provider '{}' 不存在", id),
            )
        })?;

    let was_default = config.providers[index].is_default;
    config.providers.remove(index);

    // 如果删除的是默认 Provider，将第一个设为默认
    if was_default {
        if let Some(first) = config.providers.first_mut() {
            first.is_default = true;
            log::debug!("已删除默认 Provider，新默认: {}", first.id);
        }
    }

    // 从回退顺序中移除
    config.fallback_order.retain(|fid| fid != id);

    log::info!("已删除 Provider: {}，剩余数量: {}", id, config.providers.len());
    Ok(())
}

/// 设置默认 Provider
pub fn set_default_provider(config: &mut LlmConfig, id: &str) -> Result<(), CommandError> {
    let exists = config.providers.iter().any(|p| p.id == id);
    if !exists {
        log::warn!("设置默认 Provider 失败，不存在: {}", id);
        return Err(CommandError::config(
            CONFIG_DEFAULT_PROVIDER_REQUIRED,
            format!("Provider '{}' 不存在", id),
        ));
    }

    for p in &mut config.providers {
        p.is_default = p.id == id;
    }
    log::info!("已设置默认 Provider: {}", id);
    Ok(())
}
