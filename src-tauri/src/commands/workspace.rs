use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::errors::{CommandError, FS_PATH_NOT_FOUND, FS_NOT_A_DIRECTORY};
use crate::events::AgentEmitter;
use crate::events::types;
use crate::models::workspace::{FileNode, SearchOptions, SearchResult, WorkspaceInfo};
use crate::AppState;

/// 列出所有工作区
#[tauri::command]
pub async fn list_workspaces(state: State<'_, AppState>) -> Result<Vec<WorkspaceInfo>, CommandError> {
    log::info!("list_workspaces: 查询所有工作区");
    let config = state.config.lock().await;
    let ws_config = config.load_workspaces()?;

    // 从应用设置中读取默认工作区 ID，用于判断 is_active
    let default_workspace_id = config.load_app_settings()
        .map(|s| s.workspace.default_workspace_id)
        .unwrap_or_default();

    let result: Vec<WorkspaceInfo> = ws_config
        .workspaces
        .iter()
        .map(|w| {
            let path = PathBuf::from(&w.path);
            let file_count = count_files_in_dir(&path).unwrap_or(0);
            WorkspaceInfo {
                id: w.id.clone(),
                name: w.name.clone(),
                path: w.path.clone(),
                is_active: w.id == default_workspace_id,
                file_count,
                created_at: w.created_at.clone(),
                last_accessed: w.created_at.clone(),
            }
        })
        .collect();

    log::info!("list_workspaces: 查询完成, 共 {} 个工作区", result.len());
    Ok(result)
}

/// 添加工作区
#[tauri::command]
pub async fn add_workspace(
    path: String,
    name: Option<String>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<WorkspaceInfo, CommandError> {
    log::info!("add_workspace: 添加工作区, path={}", path);
    let dir_path = PathBuf::from(&path);
    if !dir_path.exists() {
        log::error!("add_workspace: 路径不存在: {}", path);
        return Err(CommandError::fs(
            FS_PATH_NOT_FOUND,
            format!("路径不存在: {}", path),
        ));
    }
    if !dir_path.is_dir() {
        log::error!("add_workspace: 路径不是目录: {}", path);
        return Err(CommandError::fs(
            FS_NOT_A_DIRECTORY,
            format!("路径不是目录: {}", path),
        ));
    }

    let cfg_manager = state.config.lock().await;
    let mut ws_config = cfg_manager.load_workspaces()?;

    let display_name = name.unwrap_or_else(|| {
        dir_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "未命名工作区".to_string())
    });

    let entry = cfg_manager.add_workspace(&mut ws_config, &path, &display_name)?;
    cfg_manager.save_workspaces(&ws_config)?;

    let file_count = count_files_in_dir(&dir_path).unwrap_or(0);
    log::info!("add_workspace: 工作区添加成功, name={}, id={}", display_name, entry.id);

    // 发射工作区变更事件
    let emitter = AgentEmitter::new(app_handle);
    let _ = emitter.emit_workspace_change(types::WorkspaceChangePayload {
        workspace_id: entry.id.clone(),
        workspace_name: entry.name.clone(),
        workspace_path: entry.path.clone(),
    });

    Ok(WorkspaceInfo {
        id: entry.id,
        name: entry.name,
        path: entry.path,
        is_active: false,
        file_count,
        created_at: entry.created_at.clone(),
        last_accessed: entry.created_at,
    })
}

/// 移除工作区
#[tauri::command]
pub async fn remove_workspace(
    workspace_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("remove_workspace: 移除工作区, id={}", workspace_id);
    let cfg_manager = state.config.lock().await;
    let mut ws_config = cfg_manager.load_workspaces()?;

    // 在移除前获取工作区信息，用于发射事件
    let removed_ws = ws_config.workspaces.iter().find(|w| w.id == workspace_id).cloned();

    cfg_manager.remove_workspace(&mut ws_config, &workspace_id)?;
    cfg_manager.save_workspaces(&ws_config)?;
    log::info!("remove_workspace: 工作区移除成功, id={}", workspace_id);

    // 发射工作区变更事件
    if let Some(ws) = removed_ws {
        let emitter = AgentEmitter::new(app_handle);
        let _ = emitter.emit_workspace_change(types::WorkspaceChangePayload {
            workspace_id: ws.id.clone(),
            workspace_name: ws.name.clone(),
            workspace_path: ws.path.clone(),
        });
    }

    Ok(())
}

/// 设置活动工作区
#[tauri::command]
pub async fn set_active_workspace(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("set_active_workspace: 设置活动工作区, id={}", workspace_id);
    let cfg_manager = state.config.lock().await;
    let ws_config = cfg_manager.load_workspaces()?;

    let workspace = ws_config.workspaces.iter().find(|w| w.id == workspace_id);
    if workspace.is_none() {
        log::error!("set_active_workspace: 工作区 '{}' 不存在", workspace_id);
        return Err(CommandError::fs(
            FS_PATH_NOT_FOUND,
            format!("工作区 '{}' 不存在", workspace_id),
        ));
    }

    let ws = workspace.unwrap();

    // 更新应用设置中的默认工作区
    let mut settings = cfg_manager.load_app_settings()?;
    settings.workspace.default_workspace_id = workspace_id.clone();
    cfg_manager.save_app_settings(&settings)?;

    // 启动文件监听
    drop(cfg_manager);
    state.fs_watcher.watch(workspace_id, ws.path.clone()).await;

    log::info!("set_active_workspace: 活动工作区设置成功, id={}", settings.workspace.default_workspace_id);
    Ok(())
}

/// 获取文件树，实际遍历文件系统目录
#[tauri::command]
pub async fn get_file_tree(
    workspace_id: String,
    path: Option<String>,
    depth: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<FileNode>, CommandError> {
    log::info!("get_file_tree: 获取文件树, workspace_id={}, path={:?}, depth={:?}", workspace_id, path, depth);
    let config = state.config.lock().await;
    let ws_config = config.load_workspaces()?;

    let workspace = ws_config
        .workspaces
        .iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| {
            log::error!("get_file_tree: 工作区 '{}' 不存在", workspace_id);
            CommandError::fs(
                FS_PATH_NOT_FOUND,
                format!("工作区 '{}' 不存在", workspace_id),
            )
        })?;

    let root = PathBuf::from(&workspace.path);
    let base = match &path {
        Some(p) => root.join(p),
        None => root.clone(),
    };

    let max_depth = depth.unwrap_or(3);
    let result = build_file_tree(&base, &root, max_depth, 0);
    log::info!("get_file_tree: 文件树构建完成, 节点数={}", result.len());
    Ok(result)
}

/// 搜索文件，目前只做文件名搜索
#[tauri::command]
pub async fn search_files(
    workspace_id: String,
    query: String,
    options: Option<SearchOptions>,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, CommandError> {
    log::info!("search_files: 搜索文件, workspace_id={}, query={}", workspace_id, query);
    let config = state.config.lock().await;
    let ws_config = config.load_workspaces()?;

    let workspace = ws_config
        .workspaces
        .iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| {
            log::error!("search_files: 工作区 '{}' 不存在", workspace_id);
            CommandError::fs(
                FS_PATH_NOT_FOUND,
                format!("工作区 '{}' 不存在", workspace_id),
            )
        })?;

    let max_results = options
        .as_ref()
        .and_then(|o| o.max_results)
        .unwrap_or(50) as usize;

    let extensions: Vec<String> = options
        .as_ref()
        .and_then(|o| o.extensions.clone())
        .unwrap_or_default();

    if !extensions.is_empty() {
        log::debug!("search_files: 扩展名过滤={:?}", extensions);
    }

    let root = PathBuf::from(&workspace.path);
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    search_files_recursive(&root, &root, &query_lower, &extensions, max_results, &mut results);

    log::info!("search_files: 搜索完成, 结果数={}", results.len());
    Ok(results)
}

/// 递归构建文件树
fn build_file_tree(
    dir: &PathBuf,
    root: &PathBuf,
    max_depth: u32,
    current_depth: u32,
) -> Vec<FileNode> {
    let mut nodes = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return nodes,
    };

    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by(|a, b| {
        let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_is_dir.cmp(&a_is_dir).then(
            a.file_name()
                .to_string_lossy()
                .to_lowercase()
                .cmp(&b.file_name().to_string_lossy().to_lowercase()),
        )
    });

    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();

        // 跳过隐藏文件和目录
        if name.starts_with('.') {
            continue;
        }

        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let is_dir = metadata.is_dir();
        let size = if is_dir { None } else { Some(metadata.len()) };
        let modified = metadata
            .modified()
            .ok()
            .map(|t| {
                let dt: chrono::DateTime<chrono::Utc> = t.into();
                dt.to_rfc3339()
            });
        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_string());

        let children = if is_dir && current_depth < max_depth - 1 {
            Some(build_file_tree(&path, root, max_depth, current_depth + 1))
        } else {
            None
        };

        nodes.push(FileNode {
            name,
            path: relative,
            is_dir,
            size,
            modified,
            extension,
            children,
        });
    }

    nodes
}

/// 递归搜索文件名
fn search_files_recursive(
    dir: &PathBuf,
    root: &PathBuf,
    query: &str,
    extensions: &[String],
    max_results: usize,
    results: &mut Vec<SearchResult>,
) {
    if results.len() >= max_results {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        if results.len() >= max_results {
            return;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }

        let path = entry.path();

        if path.is_dir() {
            search_files_recursive(&path, root, query, extensions, max_results, results);
            continue;
        }

        let name_lower = name.to_lowercase();
        if !name_lower.contains(query) {
            continue;
        }

        // 检查扩展名过滤
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if !extensions.is_empty() && !extensions.iter().any(|e| e.to_lowercase() == ext) {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let modified = metadata
            .modified()
            .ok()
            .map(|t| {
                let dt: chrono::DateTime<chrono::Utc> = t.into();
                dt.to_rfc3339()
            })
            .unwrap_or_default();

        results.push(SearchResult {
            path: relative,
            name,
            extension: ext,
            size: metadata.len(),
            modified,
            match_type: "name".to_string(),
            match_preview: None,
            line_number: None,
        });
    }
}

/// 统计目录中的文件数量
fn count_files_in_dir(dir: &PathBuf) -> Result<u32, CommandError> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0u32;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                count += 1;
            }
        }
    }
    Ok(count)
}
