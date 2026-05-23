use crate::errors::{CommandError, FS_IO_ERROR, FS_PATH_NOT_FOUND};

/// 获取错误日志文件内容
/// 读取项目根目录下 log/docagent.log 文件并返回其内容
#[tauri::command]
pub async fn get_error_log() -> Result<String, CommandError> {
    log::info!("获取错误日志");

    // 日志文件路径：项目根目录/log/docagent.log
    let log_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.join("log").join("docagent.log"))
        .unwrap_or_else(|| std::path::PathBuf::from("log/docagent.log"));

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
