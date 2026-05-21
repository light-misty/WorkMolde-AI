use std::path::PathBuf;

use serde_json::json;
use tauri::State;

use crate::db::snapshot_repo;
use crate::errors::{CommandError, DOC_FILE_NOT_FOUND, DOC_FORMAT_UNSUPPORTED, DOC_VERSION_NOT_FOUND, FS_PATH_NOT_FOUND, FS_ALREADY_EXISTS};
use crate::models::document::{PreviewContent, VersionInfo};
use crate::AppState;

/// 预览文档内容
/// 对于文本类文件直接读取，对于二进制格式文件通过 Sidecar 解析
#[tauri::command]
pub async fn preview_document(
    workspace_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<PreviewContent, CommandError> {
    log::info!("preview_document: 预览文档, workspace_id={}, path={}", workspace_id, path);
    let config = state.config.lock().await;
    let ws_config = config.load_workspaces()?;

    let workspace = ws_config
        .workspaces
        .iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| {
            log::error!("preview_document: 工作区 '{}' 不存在", workspace_id);
            CommandError::fs(
                crate::errors::FS_PATH_NOT_FOUND,
                format!("工作区 '{}' 不存在", workspace_id),
            )
        })?;

    let file_path = PathBuf::from(&workspace.path).join(&path);
    if !file_path.exists() {
        log::error!("preview_document: 文件不存在: {}", path);
        return Err(CommandError::doc(
            DOC_FILE_NOT_FOUND,
            format!("文件不存在: {}", path),
        ));
    }

    let extension = file_path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let file_type = match extension.as_str() {
        "docx" | "doc" => "docx",
        "xlsx" | "xls" => "xlsx",
        "pptx" | "ppt" => "pptx",
        "pdf" => "pdf",
        "md" | "markdown" => "md",
        "txt" => "txt",
        _ => {
            log::warn!("preview_document: 不支持的文件格式: .{}", extension);
            return Err(CommandError::doc(
                DOC_FORMAT_UNSUPPORTED,
                format!("不支持的文件格式: .{}", extension),
            ))
        }
    };

    log::debug!("preview_document: 文件类型={}", file_type);

    // 释放配置锁后再调用 Sidecar（避免长时间持锁）
    drop(config);

    let content = match file_type {
        "md" | "txt" => std::fs::read_to_string(&file_path)?,
        _ => {
            let sidecar_params = json!({
                "input_path": file_path.to_string_lossy().to_string(),
                "options": {
                    "include_formatting": false,
                },
            });
            match state.doc_service.process("read", file_type, sidecar_params).await {
                Ok(data) => serde_json::to_string_pretty(&data).unwrap_or_else(|_| "[预览] 文档内容解析失败".to_string()),
                Err(e) => {
                    log::warn!("preview_document: Sidecar 解析失败, 降级为占位提示: {}", e.message);
                    format!("[预览] {} 格式文件解析失败: {}", extension.to_uppercase(), e.message)
                }
            }
        }
    };

    log::info!("preview_document: 预览完成, file_type={}", file_type);
    Ok(PreviewContent {
        path: path.clone(),
        file_type: file_type.to_string(),
        content,
        page_count: None,
        sheet_names: None,
        metadata: None,
    })
}

/// 获取文档版本历史
#[tauri::command]
pub async fn get_document_versions(
    workspace_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<VersionInfo>, CommandError> {
    log::info!("get_document_versions: 查询版本历史, workspace_id={}, path={}", workspace_id, path);
    let conn = state.db.conn()?;

    let versions = snapshot_repo::list_snapshots(&conn, Some(&workspace_id), Some(&path));
    log::info!("get_document_versions: 查询完成, 版本数={}", versions.len());
    Ok(versions)
}

/// 回滚到指定版本
#[tauri::command]
pub async fn rollback_version(
    workspace_id: String,
    path: String,
    version_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("rollback_version: 回滚版本, workspace_id={}, path={}, version_id={}", workspace_id, path, version_id);
    let config = state.config.lock().await;
    let ws_config = config.load_workspaces()?;

    let workspace = ws_config
        .workspaces
        .iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| {
            log::error!("rollback_version: 工作区 '{}' 不存在", workspace_id);
            CommandError::fs(
                crate::errors::FS_PATH_NOT_FOUND,
                format!("工作区 '{}' 不存在", workspace_id),
            )
        })?;

    let conn = state.db.conn()?;

    // 查找快照记录
    let snapshots = snapshot_repo::list_snapshots(&conn, Some(&workspace_id), Some(&path));
    let snapshot = snapshots
        .iter()
        .find(|s| s.version_id == version_id)
        .ok_or_else(|| {
            log::error!("rollback_version: 版本 '{}' 不存在", version_id);
            CommandError::doc(
                DOC_VERSION_NOT_FOUND,
                format!("版本 '{}' 不存在", version_id),
            )
        })?;

    let snapshot_path = PathBuf::from(&snapshot.path);
    if !snapshot_path.exists() {
        log::error!("rollback_version: 快照文件不存在: {}", snapshot.path);
        return Err(CommandError::doc(
            DOC_FILE_NOT_FOUND,
            format!("快照文件不存在: {}", snapshot.path),
        ));
    }

    // 将快照文件复制回原路径
    let target_path = PathBuf::from(&workspace.path).join(&path);
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&snapshot_path, &target_path)?;

    // 记录回滚操作
    let rollback_id = uuid::Uuid::new_v4().to_string();
    snapshot_repo::create_snapshot(
        &conn,
        &rollback_id,
        &workspace_id,
        "",
        &path,
        &snapshot.path,
        "rollback",
    )?;

    log::info!("rollback_version: 回滚成功, version_id={}, rollback_id={}", version_id, rollback_id);
    Ok(())
}

/// 解析工作区路径，返回工作区信息和绝对路径
async fn resolve_workspace_path(
    workspace_id: &str,
    relative_path: &str,
    state: &State<'_, AppState>,
) -> Result<(String, PathBuf), CommandError> {
    let config = state.config.lock().await;
    let ws_config = config.load_workspaces()?;

    let workspace = ws_config
        .workspaces
        .iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| {
            log::error!("工作区 '{}' 不存在", workspace_id);
            CommandError::fs(
                FS_PATH_NOT_FOUND,
                format!("工作区 '{}' 不存在", workspace_id),
            )
        })?;

    let abs_path = PathBuf::from(&workspace.path).join(relative_path);
    Ok((workspace.path.clone(), abs_path))
}

/// 创建空文件
#[tauri::command]
pub async fn create_file(
    workspace_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("create_file: 创建文件, workspace_id={}, path={}", workspace_id, path);
    let (_, abs_path) = resolve_workspace_path(&workspace_id, &path, &state).await?;

    if abs_path.exists() {
        log::error!("create_file: 文件已存在: {}", path);
        return Err(CommandError::fs(
            FS_ALREADY_EXISTS,
            format!("文件已存在: {}", path),
        ));
    }

    // 确保父目录存在
    if let Some(parent) = abs_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::File::create(&abs_path)?;
    log::info!("create_file: 文件创建成功, path={}", path);
    Ok(())
}

/// 创建目录
#[tauri::command]
pub async fn create_directory(
    workspace_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("create_directory: 创建目录, workspace_id={}, path={}", workspace_id, path);
    let (_, abs_path) = resolve_workspace_path(&workspace_id, &path, &state).await?;

    if abs_path.exists() {
        log::error!("create_directory: 目录已存在: {}", path);
        return Err(CommandError::fs(
            FS_ALREADY_EXISTS,
            format!("目录已存在: {}", path),
        ));
    }

    std::fs::create_dir_all(&abs_path)?;
    log::info!("create_directory: 目录创建成功, path={}", path);
    Ok(())
}

/// 重命名文件或目录
#[tauri::command]
pub async fn rename_file(
    workspace_id: String,
    old_path: String,
    new_path: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("rename_file: 重命名, workspace_id={}, old_path={}, new_path={}", workspace_id, old_path, new_path);
    let (_, abs_old) = resolve_workspace_path(&workspace_id, &old_path, &state).await?;
    let (_, abs_new) = resolve_workspace_path(&workspace_id, &new_path, &state).await?;

    if !abs_old.exists() {
        log::error!("rename_file: 源路径不存在: {}", old_path);
        return Err(CommandError::fs(
            FS_PATH_NOT_FOUND,
            format!("源路径不存在: {}", old_path),
        ));
    }

    if abs_new.exists() {
        log::error!("rename_file: 目标路径已存在: {}", new_path);
        return Err(CommandError::fs(
            FS_ALREADY_EXISTS,
            format!("目标路径已存在: {}", new_path),
        ));
    }

    // 确保目标父目录存在
    if let Some(parent) = abs_new.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::rename(&abs_old, &abs_new)?;
    log::info!("rename_file: 重命名成功, old_path={} -> new_path={}", old_path, new_path);
    Ok(())
}

/// 删除文件或目录（永久删除）
#[tauri::command]
pub async fn delete_file(
    workspace_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("delete_file: 删除, workspace_id={}, path={}", workspace_id, path);
    let (_, abs_path) = resolve_workspace_path(&workspace_id, &path, &state).await?;

    if !abs_path.exists() {
        log::error!("delete_file: 路径不存在: {}", path);
        return Err(CommandError::fs(
            FS_PATH_NOT_FOUND,
            format!("路径不存在: {}", path),
        ));
    }

    if abs_path.is_dir() {
        std::fs::remove_dir_all(&abs_path)?;
    } else {
        std::fs::remove_file(&abs_path)?;
    }

    log::info!("delete_file: 删除成功, path={}", path);
    Ok(())
}

/// 在系统文件管理器中显示文件或目录
#[tauri::command]
pub async fn show_in_file_manager(
    workspace_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("show_in_file_manager: 在文件管理器中显示, workspace_id={}, path={}", workspace_id, path);
    let (_, abs_path) = resolve_workspace_path(&workspace_id, &path, &state).await?;

    if !abs_path.exists() {
        log::error!("show_in_file_manager: 路径不存在: {}", path);
        return Err(CommandError::fs(
            FS_PATH_NOT_FOUND,
            format!("路径不存在: {}", path),
        ));
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: 使用 explorer /select,"path" 选中并定位到文件
        std::process::Command::new("explorer")
            .arg(format!("/select,\"{}\"", abs_path.to_string_lossy()))
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: 使用 open -R 在 Finder 中显示
        std::process::Command::new("open")
            .arg("-R")
            .arg(&abs_path)
            .spawn()?;
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: 打开文件所在目录
        let dir = if abs_path.is_dir() {
            abs_path.clone()
        } else {
            abs_path.parent().unwrap_or(&abs_path).to_path_buf()
        };
        std::process::Command::new("xdg-open")
            .arg(&dir)
            .spawn()?;
    }

    log::info!("show_in_file_manager: 已打开文件管理器, path={}", path);
    Ok(())
}
