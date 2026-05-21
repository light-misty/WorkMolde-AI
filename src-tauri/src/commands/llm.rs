use std::collections::HashMap;
use std::sync::Arc;

use tauri::State;

use crate::errors::{CommandError, LLM_CONNECTION_FAILED};
use crate::models::llm::{ConnectionResult, ProviderConfig, ProviderInfo};
use crate::AppState;

/// 测试 LLM Provider 连接
#[tauri::command]
pub async fn test_connection(
    provider_id: String,
    state: State<'_, AppState>,
) -> Result<ConnectionResult, CommandError> {
    log::info!("测试 LLM Provider 连接: provider_id={}", provider_id);
    let router = state.llm_router.read().await;
    router.test_connection(&provider_id).await
}

/// 列出所有 LLM Provider
#[tauri::command]
pub async fn list_providers(state: State<'_, AppState>) -> Result<Vec<ProviderInfo>, CommandError> {
    log::info!("列出所有 LLM Provider");
    let router = state.llm_router.read().await;
    let providers = router.list_providers();
    log::info!("列出 Provider 完成: count={}", providers.len());
    Ok(providers)
}

/// 添加 LLM Provider 并重建路由器
#[tauri::command]
pub async fn add_provider(
    config: ProviderConfig,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "添加 LLM Provider: name={}, provider_type={}, model={}",
        config.name,
        config.provider_type,
        config.model
    );
    let cfg_manager = state.config.lock().await;
    let mut llm_config = cfg_manager.load_llm_config().map_err(|e| {
        log::error!("加载 LLM 配置失败: {}", e);
        e
    })?;

    let provider_type = match config.provider_type.as_str() {
        "openai" => crate::config::llm_config::ProviderType::OpenAI,
        "anthropic" => crate::config::llm_config::ProviderType::Anthropic,
        "ollama" => crate::config::llm_config::ProviderType::Ollama,
        _ => crate::config::llm_config::ProviderType::Custom,
    };

    let provider = crate::config::llm_config::LlmProvider {
        id: uuid::Uuid::new_v4().to_string(),
        provider_type,
        name: config.name,
        api_base_url: config.api_base,
        api_key_encrypted: config.api_key,
        model: config.model,
        is_default: llm_config.providers.is_empty(),
        advanced: crate::config::llm_config::AdvancedConfig::default(),
    };

    crate::config::llm_config::add_provider(&mut llm_config, provider).map_err(|e| {
        log::error!("添加 Provider 失败: {}", e);
        e
    })?;
    cfg_manager.save_llm_config(&llm_config).map_err(|e| {
        log::error!("保存 LLM 配置失败: {}", e);
        e
    })?;

    // 重建 LlmRouter
    rebuild_router(&state, &llm_config).await;

    log::info!("Provider 添加成功");
    Ok(())
}

/// 更新 LLM Provider 并重建路由器
#[tauri::command]
pub async fn update_provider(
    provider_id: String,
    config: ProviderConfig,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "更新 LLM Provider: provider_id={}, name={}, provider_type={}, model={}",
        provider_id,
        config.name,
        config.provider_type,
        config.model
    );
    let cfg_manager = state.config.lock().await;
    let mut llm_config = cfg_manager.load_llm_config().map_err(|e| {
        log::error!("加载 LLM 配置失败: {}", e);
        e
    })?;

    let provider_type = match config.provider_type.as_str() {
        "openai" => crate::config::llm_config::ProviderType::OpenAI,
        "anthropic" => crate::config::llm_config::ProviderType::Anthropic,
        "ollama" => crate::config::llm_config::ProviderType::Ollama,
        _ => crate::config::llm_config::ProviderType::Custom,
    };

    let existing = llm_config
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| {
            log::error!("Provider 不存在: provider_id={}", provider_id);
            CommandError::llm(
                LLM_CONNECTION_FAILED,
                format!("Provider '{}' 不存在", provider_id),
            )
        })?;

    let provider = crate::config::llm_config::LlmProvider {
        id: provider_id.clone(),
        provider_type,
        name: config.name,
        api_base_url: config.api_base,
        api_key_encrypted: config.api_key,
        model: config.model,
        is_default: existing.is_default,
        advanced: existing.advanced.clone(),
    };

    crate::config::llm_config::update_provider(&mut llm_config, &provider_id, provider).map_err(|e| {
        log::error!("更新 Provider 失败: provider_id={}, error={}", provider_id, e);
        e
    })?;
    cfg_manager.save_llm_config(&llm_config).map_err(|e| {
        log::error!("保存 LLM 配置失败: {}", e);
        e
    })?;

    // 重建 LlmRouter
    rebuild_router(&state, &llm_config).await;

    log::info!("Provider 更新成功: provider_id={}", provider_id);
    Ok(())
}

/// 删除 LLM Provider 并重建路由器
#[tauri::command]
pub async fn delete_provider(
    provider_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("删除 LLM Provider: provider_id={}", provider_id);
    let cfg_manager = state.config.lock().await;
    let mut llm_config = cfg_manager.load_llm_config().map_err(|e| {
        log::error!("加载 LLM 配置失败: {}", e);
        e
    })?;
    crate::config::llm_config::delete_provider(&mut llm_config, &provider_id).map_err(|e| {
        log::error!("删除 Provider 失败: provider_id={}, error={}", provider_id, e);
        e
    })?;
    cfg_manager.save_llm_config(&llm_config).map_err(|e| {
        log::error!("保存 LLM 配置失败: {}", e);
        e
    })?;

    // 重建 LlmRouter
    rebuild_router(&state, &llm_config).await;

    log::info!("Provider 删除成功: provider_id={}", provider_id);
    Ok(())
}

/// 设置默认 LLM Provider 并重建路由器
#[tauri::command]
pub async fn set_default_provider(
    provider_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("设置默认 LLM Provider: provider_id={}", provider_id);
    let cfg_manager = state.config.lock().await;
    let mut llm_config = cfg_manager.load_llm_config().map_err(|e| {
        log::error!("加载 LLM 配置失败: {}", e);
        e
    })?;
    crate::config::llm_config::set_default_provider(&mut llm_config, &provider_id).map_err(|e| {
        log::error!("设置默认 Provider 失败: provider_id={}, error={}", provider_id, e);
        e
    })?;
    cfg_manager.save_llm_config(&llm_config).map_err(|e| {
        log::error!("保存 LLM 配置失败: {}", e);
        e
    })?;

    // 重建 LlmRouter
    rebuild_router(&state, &llm_config).await;

    log::info!("默认 Provider 设置成功: provider_id={}", provider_id);
    Ok(())
}

/// 根据 LLM 配置重建 LlmRouter
async fn rebuild_router(state: &State<'_, AppState>, llm_config: &crate::config::llm_config::LlmConfig) {
    // 保留旧路由器的 AppHandle，避免重建后丢失事件通知能力
    let app_handle = {
        let guard = state.llm_router.read().await;
        guard.app_handle()
    };
    let new_router = crate::services::llm::router::LlmRouter::from_config(llm_config)
        .with_app_handle(app_handle);
    let mut guard = state.llm_router.write().await;
    *guard = Arc::new(new_router);
    log::info!("LlmRouter 已重建");
}

/// 对所有 LLM Provider 执行健康检查
#[tauri::command]
pub async fn health_check_providers(
    state: State<'_, AppState>,
) -> Result<HashMap<String, ConnectionResult>, CommandError> {
    log::info!("手动触发 Provider 健康检查");
    let router = state.llm_router.read().await;
    let results = router.health_check_all().await;
    log::info!("手动健康检查完成, 检查了 {} 个 Provider", results.len());
    Ok(results)
}
