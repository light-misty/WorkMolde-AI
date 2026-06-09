use crate::errors::{CommandError, FS_IO_ERROR, FS_PATH_NOT_FOUND};
use serde::Serialize;
use tauri::Manager;

/// 日志路径信息
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogPathInfo {
    /// 日志源文件路径
    log_source: String,
    /// 浏览器下载目录路径
    download_dir: String,
}

/// 获取日志路径信息
/// 返回日志源文件路径和浏览器下载目录路径，供前端展示
#[tauri::command]
pub async fn get_log_path(app_handle: tauri::AppHandle) -> Result<LogPathInfo, CommandError> {
    // 日志文件路径：与 lib.rs 中日志初始化使用相同的目录计算逻辑
    let log_dir = crate::utils::logger::resolve_log_dir(
        app_handle.path().app_log_dir().ok(),
        app_handle.path().app_data_dir().ok(),
    );
    let log_path = log_dir.join("docagent.log");

    // 浏览器下载目录：使用系统默认下载目录
    let download_dir = app_handle
        .path()
        .download_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("Downloads"));

    Ok(LogPathInfo {
        log_source: log_path.to_string_lossy().to_string(),
        download_dir: download_dir.to_string_lossy().to_string(),
    })
}

/// 获取错误日志文件内容
/// 读取日志目录下的 docagent.log 文件并返回其内容
#[tauri::command]
pub async fn get_error_log(app_handle: tauri::AppHandle) -> Result<String, CommandError> {
    log::info!("获取错误日志");

    // 日志文件路径：与 lib.rs 中日志初始化使用相同的目录计算逻辑
    let log_dir = crate::utils::logger::resolve_log_dir(
        app_handle.path().app_log_dir().ok(),
        app_handle.path().app_data_dir().ok(),
    );
    let log_path = log_dir.join("docagent.log");

    if !log_path.exists() {
        log::warn!("日志文件不存在: {:?}", log_path);
        return Err(CommandError::fs(
            FS_PATH_NOT_FOUND,
            format!("日志文件不存在: {}", log_path.display()),
        ));
    }

    let content = std::fs::read_to_string(&log_path).map_err(|e| {
        log::error!("读取日志文件失败: {}", e);
        CommandError::fs(FS_IO_ERROR, format!("读取日志文件失败: {}", e))
    })?;

    log::info!("获取错误日志成功，长度: {} 字节", content.len());
    Ok(content)
}
