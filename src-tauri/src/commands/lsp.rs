//! LSP Tauri 命令:提供前端调用 LSP 功能的接口

use crate::AppState;
use tauri::State;

/// 获取所有 LSP 服务器状态
#[tauri::command]
pub async fn lsp_get_status(
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::lsp::LspServerInfo>, crate::errors::CommandError> {
    let statuses = state.lsp_manager.get_all_status().await;
    Ok(statuses)
}

/// 重启指定语言的 LSP 服务器
#[tauri::command]
pub async fn lsp_restart_server(
    state: State<'_, AppState>,
    language: String,
) -> Result<(), crate::errors::CommandError> {
    // 先停止现有服务器(stop 失败降级为日志,不中断重启流程)
    if let Err(e) = state.lsp_manager.stop(&language).await {
        log::warn!(
            "LSP 停止失败(language={},将继续尝试启动): {}",
            language,
            e.message
        );
    }
    // 重新启动(会自动使用已注册的配置)
    state.lsp_manager.get_or_start(&language).await.map_err(|e| {
        // 保留原错误码,但 message 改为更友好的描述
        let friendly_msg = match e.code {
            crate::errors::FS_PATH_NOT_FOUND => format!(
                "未找到 LSP 服务器程序,请确保已安装该语言服务器并添加到 PATH(原始错误: {})",
                e.message
            ),
            crate::errors::RUNTIME_EVENT_EMIT_ERROR => {
                if e.message.contains("超时") {
                    format!("LSP 服务器响应超时,可能正在索引或工作区配置异常(原始错误: {})", e.message)
                } else {
                    format!("LSP 服务器启动后立即退出,请检查工作区是否包含项目标识文件(原始错误: {})", e.message)
                }
            }
            _ => e.message.clone(),
        };
        crate::errors::CommandError::new(e.code, friendly_msg)
    })?;
    log::info!("LSP 服务器已重启: language={}", language);
    Ok(())
}

/// 停止所有 LSP 服务器
#[tauri::command]
pub async fn lsp_stop_all(state: State<'_, AppState>) -> Result<(), crate::errors::CommandError> {
    state.lsp_manager.stop_all().await?;
    log::info!("所有 LSP 服务器已停止");
    Ok(())
}

/// 初始化 LSP：从配置注册并启动所有启用的语言服务器
/// 由前端在用户确认开启总开关后调用
#[tauri::command]
pub async fn lsp_initialize(
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::lsp::LspServerInfo>, crate::errors::CommandError> {
    let settings = {
        let config_manager = state.config.lock().await;
        config_manager.load_app_settings().map_err(|e| {
            crate::errors::CommandError::config(
                crate::errors::CONFIG_INVALID_FORMAT,
                format!("加载 LSP 配置失败: {}", e),
            )
        })?
    };

    let lsp_config = settings.lsp;

    // 注册所有启用的服务器配置
    for server_config in &lsp_config.servers {
        if server_config.enabled {
            let config = crate::models::lsp::LspServerConfig {
                language: server_config.language.clone(),
                command: server_config.command.clone(),
                root_patterns: server_config.root_patterns.clone(),
                initialization_options: server_config.initialization_options.clone(),
            };
            state.lsp_manager.register_config(config).await;
            log::info!("LSP 初始化: 已注册配置 language={}", server_config.language);
        }
    }

    // 启动所有已注册的服务器
    for server_config in &lsp_config.servers {
        if server_config.enabled {
            if let Err(e) = state
                .lsp_manager
                .get_or_start(&server_config.language)
                .await
            {
                log::warn!("LSP 启动失败 ({}): {}", server_config.language, e.message);
            }
        }
    }

    let statuses = state.lsp_manager.get_all_status().await;
    Ok(statuses)
}
