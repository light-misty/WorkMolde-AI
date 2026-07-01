pub mod llm_config;
pub mod app_settings;
pub mod workspace_config;

use std::path::{Path, PathBuf};

use crate::errors::CommandError;

/// 配置管理器，统一管理所有配置文件的读写
pub struct ConfigManager {
    /// 应用数据目录，所有配置文件存储在此目录的 config/ 子目录下
    data_dir: PathBuf,
}

impl ConfigManager {
    /// 创建配置管理器实例
    pub fn new(app_data_dir: PathBuf) -> Self {
        log::info!("创建配置管理器，数据目录: {}", app_data_dir.display());

        Self {
            data_dir: app_data_dir,
        }
    }

    /// 获取数据目录路径
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    // ================================================================
    // LLM 配置
    // ================================================================

    /// 加载 LLM 配置
    pub fn load_llm_config(&self) -> Result<llm_config::LlmConfig, CommandError> {
        log::info!("加载 LLM 配置");
        let result = llm_config::load_llm_config(&self.data_dir);
        if let Err(ref e) = result {
            log::error!("加载 LLM 配置失败: {}", e);
        }
        result
    }

    /// 保存 LLM 配置
    pub fn save_llm_config(&self, config: &llm_config::LlmConfig) -> Result<(), CommandError> {
        log::info!("保存 LLM 配置 (providers数量: {})", config.providers.len());
        let result = llm_config::save_llm_config(&self.data_dir, config);
        if let Err(ref e) = result {
            log::error!("保存 LLM 配置失败: {}", e);
        }
        result
    }

    /// 添加 Provider
    pub fn add_provider(
        &self,
        config: &mut llm_config::LlmConfig,
        provider: llm_config::LlmProvider,
    ) -> Result<(), CommandError> {
        log::info!("添加 Provider: id={}, name={}", provider.id, provider.name);
        let result = llm_config::add_provider(config, provider);
        if let Err(ref e) = result {
            log::error!("添加 Provider 失败: {}", e);
        }
        result
    }

    /// 更新 Provider
    pub fn update_provider(
        &self,
        config: &mut llm_config::LlmConfig,
        id: &str,
        provider: llm_config::LlmProvider,
    ) -> Result<(), CommandError> {
        log::info!("更新 Provider: id={}", id);
        let result = llm_config::update_provider(config, id, provider);
        if let Err(ref e) = result {
            log::error!("更新 Provider 失败: {}", e);
        }
        result
    }

    /// 删除 Provider
    pub fn delete_provider(
        &self,
        config: &mut llm_config::LlmConfig,
        id: &str,
    ) -> Result<(), CommandError> {
        log::info!("删除 Provider: id={}", id);
        let result = llm_config::delete_provider(config, id);
        if let Err(ref e) = result {
            log::error!("删除 Provider 失败: {}", e);
        }
        result
    }

    // ================================================================
    // 应用设置
    // ================================================================

    /// 加载应用设置
    pub fn load_app_settings(&self) -> Result<app_settings::AppSettings, CommandError> {
        log::info!("加载应用设置");
        let result = app_settings::load_app_settings(&self.data_dir);
        if let Err(ref e) = result {
            log::error!("加载应用设置失败: {}", e);
        }
        result
    }

    /// 保存应用设置
    pub fn save_app_settings(
        &self,
        settings: &app_settings::AppSettings,
    ) -> Result<(), CommandError> {
        log::info!("保存应用设置");
        let result = app_settings::save_app_settings(&self.data_dir, settings);
        if let Err(ref e) = result {
            log::error!("保存应用设置失败: {}", e);
        }
        result
    }

    // ================================================================
    // 工作区配置
    // ================================================================

    /// 加载工作区配置
    pub fn load_workspaces(&self) -> Result<workspace_config::WorkspacesConfig, CommandError> {
        log::info!("加载工作区配置");
        let result = workspace_config::load_workspaces(&self.data_dir);
        if let Err(ref e) = result {
            log::error!("加载工作区配置失败: {}", e);
        }
        result
    }

    /// 保存工作区配置
    pub fn save_workspaces(
        &self,
        config: &workspace_config::WorkspacesConfig,
    ) -> Result<(), CommandError> {
        log::info!("保存工作区配置 (工作区数量: {})", config.workspaces.len());
        let result = workspace_config::save_workspaces(&self.data_dir, config);
        if let Err(ref e) = result {
            log::error!("保存工作区配置失败: {}", e);
        }
        result
    }

    /// 添加工作区
    pub fn add_workspace(
        &self,
        config: &mut workspace_config::WorkspacesConfig,
        path: &str,
        name: &str,
    ) -> Result<workspace_config::WorkspaceEntry, CommandError> {
        log::info!("添加工作区: name={}, path={}", name, path);
        let result = workspace_config::add_workspace(config, path, name);
        if let Err(ref e) = result {
            log::error!("添加工作区失败: {}", e);
        }
        result
    }

    /// 移除工作区
    pub fn remove_workspace(
        &self,
        config: &mut workspace_config::WorkspacesConfig,
        id: &str,
    ) -> Result<(), CommandError> {
        log::info!("移除工作区: id={}", id);
        let result = workspace_config::remove_workspace(config, id);
        if let Err(ref e) = result {
            log::error!("移除工作区失败: {}", e);
        }
        result
    }
}
