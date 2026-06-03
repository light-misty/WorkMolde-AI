use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::errors::{CommandError, CONFIG_PROVIDER_NOT_FOUND, CONFIG_DEFAULT_PROVIDER_REQUIRED};

/// 内置 Provider 的固定 ID
pub const BUILTIN_PROVIDER_ID: &str = "builtin_deepseek";

/// 内置 Provider 配置文件名
const BUILTIN_PROVIDER_FILENAME: &str = "builtin_provider.json";

/// 内置 Provider 配置（从 JSON 文件加载）
#[derive(Deserialize)]
struct BuiltinProviderConfig {
    name: String,
    provider_type: String,
    api_base_url: String,
    api_key: String,
    model: String,
    #[serde(default)]
    context_window: Option<usize>,
    #[serde(default = "default_supports_vision_builtin")]
    supports_vision: bool,
}

/// 内置配置的 supports_vision 默认值
fn default_supports_vision_builtin() -> bool {
    false
}

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
    /// LLM 最大输出 token 数（如 4096），不是上下文窗口大小
    pub max_tokens: u32,
    pub timeout_seconds: u32,
    pub max_retries: u32,
    #[serde(default)]
    pub extra_headers: HashMap<String, String>,
    /// 是否将 reasoning_content 折叠到 content 字段中发送
    /// true: 不支持 reasoning_content 输入的 Provider（OpenAI/Ollama），将思考内容用 <agent-reasoning> 标签包裹后合并到 content
    /// false: 支持 reasoning_content 输入的 Provider（DeepSeek），保持原样发送
    #[serde(default = "default_reasoning_in_content")]
    pub reasoning_in_content: bool,
    /// 上下文窗口大小 (tokens)，None 表示使用自动推断
    /// 与 max_tokens 不同：max_tokens 是 LLM 最大输出 token 数，context_window 是模型上下文窗口总大小
    #[serde(default)]
    pub context_window: Option<usize>,
}

/// reasoning_in_content 默认值：true（安全默认，将思考内容折叠到 content）
fn default_reasoning_in_content() -> bool {
    true
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
            reasoning_in_content: true,
            context_window: None,
        }
    }
}

/// supports_vision 默认值：true（默认支持视觉）
fn default_supports_vision() -> bool {
    true
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
    /// 是否支持视觉/图片多模态
    #[serde(default = "default_supports_vision")]
    pub supports_vision: bool,
}

impl LlmProvider {
    /// 解析上下文窗口大小
    /// 优先使用手动配置的 context_window，否则从预设表推断
    pub fn resolve_context_window(&self) -> usize {
        if let Some(cw) = self.advanced.context_window {
            return cw;
        }
        let provider_type_str = match &self.provider_type {
            ProviderType::OpenAI => "openai",
            ProviderType::Anthropic => "anthropic",
            ProviderType::Ollama => "ollama",
            ProviderType::Gemini => "gemini",
            ProviderType::Custom => "custom",
        };
        crate::services::llm::context_presets::resolve_context_window(
            &self.model,
            Some(provider_type_str),
        )
    }
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

/// 从配置文件加载内置 Provider 并注入到 LLM 配置中
/// 仅当内置 Provider 不存在时才注入，已存在时不覆盖（保留用户修改）
/// project_root: 项目根目录，builtin_provider.json 所在位置
pub fn inject_builtin_provider(config: &mut LlmConfig, project_root: &Path) {
    let builtin_path = project_root.join(BUILTIN_PROVIDER_FILENAME);

    // 配置文件不存在时静默跳过（生产环境可能没有此文件）
    if !builtin_path.exists() {
        log::info!("内置 Provider 配置文件不存在: {}", builtin_path.display());
        return;
    }

    // 如果已存在内置 Provider，跳过注入（保留用户可能的修改）
    if config.providers.iter().any(|p| p.id == BUILTIN_PROVIDER_ID) {
        log::info!("内置 Provider 已存在，跳过注入");
        return;
    }

    // 读取并解析配置文件
    let content = match std::fs::read_to_string(&builtin_path) {
        Ok(c) => c,
        Err(e) => {
            log::error!("读取内置 Provider 配置文件失败: {}", e);
            return;
        }
    };

    let builtin: BuiltinProviderConfig = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            log::error!("解析内置 Provider 配置文件失败: {}", e);
            return;
        }
    };

    // 解析 Provider 类型
    let provider_type = match builtin.provider_type.as_str() {
        "openai" => ProviderType::OpenAI,
        "anthropic" => ProviderType::Anthropic,
        "ollama" => ProviderType::Ollama,
        "gemini" => ProviderType::Gemini,
        _ => ProviderType::Custom,
    };

    // 如果是第一个 Provider，自动设为默认
    let is_default = config.providers.is_empty();

    let provider = LlmProvider {
        id: BUILTIN_PROVIDER_ID.to_string(),
        provider_type,
        name: builtin.name,
        api_base_url: builtin.api_base_url,
        api_key_encrypted: builtin.api_key,
        model: builtin.model,
        is_default,
        advanced: AdvancedConfig {
            context_window: builtin.context_window,
            ..Default::default()
        },
        supports_vision: builtin.supports_vision,
    };

    log::info!(
        "注入内置 Provider: id={}, name={}, model={}, is_default={}",
        provider.id, provider.name, provider.model, provider.is_default
    );
    config.providers.push(provider);
}
