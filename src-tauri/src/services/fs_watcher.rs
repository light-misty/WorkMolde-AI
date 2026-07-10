use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::Mutex;

use crate::events::types::{
    FileChangePayload, WorkspaceDirectoryDeletedPayload, FILE_CHANGE, WORKSPACE_DIRECTORY_DELETED,
};
use crate::services::skill::registry::SkillRegistry;

/// 文件系统监听服务，监听活动工作区目录变更并发射事件到前端
pub struct FsWatcherService<R: Runtime> {
    app_handle: AppHandle<R>,
    /// 工作区目录的监听器（递归监听，用于检测文件变更）
    workspace_watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    /// 父目录的监听器（非递归，仅用于检测工作区根目录被删除）
    parent_watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    /// Skill 目录的监听器（递归监听，用于检测 SKILL.md 文件变更触发热重载）
    skill_watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    /// 当前正在监听的工作区 ID、路径和名称
    active_watch: Arc<Mutex<Option<(String, PathBuf, String)>>>,
    /// 标记是否已经发射过目录删除事件，防止重复发射
    deletion_emitted: Arc<AtomicBool>,
    /// LSP 结果缓存（文件变更时联动失效缓存）
    lsp_cache: Option<Arc<crate::services::lsp::cache::LspResultCache>>,
}

impl<R: Runtime> FsWatcherService<R> {
    /// 创建文件监听服务实例
    pub fn new(
        app_handle: AppHandle<R>,
        lsp_cache: Option<Arc<crate::services::lsp::cache::LspResultCache>>,
    ) -> Self {
        Self {
            app_handle,
            workspace_watcher: Arc::new(Mutex::new(None)),
            parent_watcher: Arc::new(Mutex::new(None)),
            skill_watcher: Arc::new(Mutex::new(None)),
            active_watch: Arc::new(Mutex::new(None)),
            deletion_emitted: Arc::new(AtomicBool::new(false)),
            lsp_cache,
        }
    }

    /// 开始监听指定工作区目录
    pub async fn watch(&self, workspace_id: String, workspace_path: String) {
        let workspace_name = PathBuf::from(&workspace_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "未命名工作区".to_string());
        self.watch_with_name(workspace_id, workspace_path, workspace_name)
            .await;
    }

    /// 开始监听指定工作区目录（带名称）
    pub async fn watch_with_name(
        &self,
        workspace_id: String,
        workspace_path: String,
        workspace_name: String,
    ) {
        let path = PathBuf::from(&workspace_path);
        if !path.exists() || !path.is_dir() {
            log::warn!("FsWatcher: 路径无效或不是目录: {}", workspace_path);
            return;
        }

        // 如果已经在监听同一工作区，跳过
        {
            let active = self.active_watch.lock().await;
            if let Some((ref id, _, _)) = *active {
                if id == &workspace_id {
                    log::debug!("FsWatcher: 已在监听工作区 {}, 跳过", workspace_id);
                    return;
                }
            }
        }

        // 重置删除事件标记
        self.deletion_emitted.store(false, Ordering::SeqCst);

        let app_handle = self.app_handle.clone();
        let wid = workspace_id.clone();
        let wname = workspace_name.clone();
        let wpath = workspace_path.clone();
        let wpath_buf = path.clone();
        let deletion_emitted = self.deletion_emitted.clone();

        // === 创建工作区目录监听器（递归，用于检测文件变更）===
        let ws_app_handle = app_handle.clone();
        let ws_wid = wid.clone();
        let ws_wname = wname.clone();
        let ws_wpath = wpath.clone();
        let ws_deletion_emitted = deletion_emitted.clone();
        let ws_lsp_cache = self.lsp_cache.clone();
        let workspace_callback = move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    let change_type = match event.kind {
                        EventKind::Create(_) => "created",
                        EventKind::Modify(_) => "modified",
                        EventKind::Remove(_) => "deleted",
                        EventKind::Any | EventKind::Other => "modified",
                        _ => return,
                    };

                    for event_path in &event.paths {
                        let path_str = event_path.to_string_lossy().to_string();

                        // 过滤日志目录下的文件变更，避免日志写入触发 FsWatcher
                        // 形成"日志写入 -> 文件变更 -> FsWatcher 记录 -> 日志写入"的反馈循环
                        if is_path_in_log_dir(event_path, crate::utils::logger::current_log_dir()) {
                            continue;
                        }

                        log::trace!(
                            "FsWatcher: 检测到文件变更 type={}, path={}",
                            change_type,
                            path_str
                        );

                        // 文件变更时联动失效 LSP 缓存，避免返回过期的定义/悬停信息
                        if let Some(ref cache) = ws_lsp_cache {
                            let cache = Arc::clone(cache);
                            let p = path_str.clone();
                            tauri::async_runtime::spawn(async move {
                                cache.invalidate_file(&p).await;
                            });
                        }

                        // 当收到删除事件时，检查工作区根目录是否仍然存在
                        if change_type == "deleted" && !ws_deletion_emitted.load(Ordering::SeqCst) {
                            let watch_root = PathBuf::from(&ws_wpath);
                            if !watch_root.exists() {
                                log::warn!(
                                    "FsWatcher: 检测到工作区根目录已被删除, workspace_id={}, path={}",
                                    ws_wid,
                                    ws_wpath
                                );
                                ws_deletion_emitted.store(true, Ordering::SeqCst);
                                let deleted_payload = WorkspaceDirectoryDeletedPayload {
                                    workspace_id: ws_wid.clone(),
                                    workspace_name: ws_wname.clone(),
                                    workspace_path: ws_wpath.clone(),
                                };
                                let _ = ws_app_handle
                                    .emit(WORKSPACE_DIRECTORY_DELETED, deleted_payload);
                                return;
                            }
                        }

                        let payload = FileChangePayload {
                            workspace_id: ws_wid.clone(),
                            change_type: change_type.to_string(),
                            path: path_str,
                            old_path: None,
                        };
                        let _ = ws_app_handle.emit(FILE_CHANGE, payload);
                    }
                }
                Err(e) => {
                    log::warn!("FsWatcher: 工作区监听器错误: {:?}", e);
                    // 监听器出错时，检查工作区根目录是否仍然存在
                    if !ws_deletion_emitted.load(Ordering::SeqCst) {
                        let watch_root = PathBuf::from(&ws_wpath);
                        if !watch_root.exists() {
                            log::warn!(
                                "FsWatcher: 监听器出错且工作区根目录已不存在, workspace_id={}, path={}",
                                ws_wid,
                                ws_wpath
                            );
                            ws_deletion_emitted.store(true, Ordering::SeqCst);
                            let deleted_payload = WorkspaceDirectoryDeletedPayload {
                                workspace_id: ws_wid.clone(),
                                workspace_name: ws_wname.clone(),
                                workspace_path: ws_wpath.clone(),
                            };
                            let _ =
                                ws_app_handle.emit(WORKSPACE_DIRECTORY_DELETED, deleted_payload);
                        }
                    }
                }
            }
        };

        let mut ws_watcher = match RecommendedWatcher::new(
            workspace_callback,
            notify::Config::default().with_poll_interval(Duration::from_secs(2)),
        ) {
            Ok(w) => w,
            Err(e) => {
                log::error!("FsWatcher: 创建工作区监听器失败: {:?}", e);
                return;
            }
        };

        if let Err(e) = ws_watcher.watch(&path, RecursiveMode::Recursive) {
            log::error!("FsWatcher: 启动工作区监听失败: {:?}", e);
            return;
        }

        // === 创建父目录监听器（非递归，仅用于检测工作区根目录被删除）===
        // Windows 上 ReadDirectoryChangesW 在被监视的目录自身被删除时不会报告事件，
        // 但父目录的句柄仍然有效，会报告子目录删除事件，实现秒级检测
        if let Some(parent_path) = path.parent() {
            let parent_app_handle = app_handle.clone();
            let parent_wid = wid.clone();
            let parent_wname = wname.clone();
            let parent_wpath = wpath.clone();
            let parent_ws_path = wpath_buf.clone();
            let parent_deletion_emitted = deletion_emitted.clone();

            let parent_callback = move |res: Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        // 只关心删除事件
                        if matches!(event.kind, EventKind::Remove(_)) {
                            for event_path in &event.paths {
                                // 检查被删除的是否是工作区根目录
                                if event_path == &parent_ws_path
                                    && !parent_deletion_emitted.load(Ordering::SeqCst)
                                {
                                    log::warn!(
                                        "FsWatcher(父目录): 检测到工作区根目录被删除, workspace_id={}, path={}",
                                        parent_wid,
                                        parent_wpath
                                    );
                                    parent_deletion_emitted.store(true, Ordering::SeqCst);
                                    let deleted_payload = WorkspaceDirectoryDeletedPayload {
                                        workspace_id: parent_wid.clone(),
                                        workspace_name: parent_wname.clone(),
                                        workspace_path: parent_wpath.clone(),
                                    };
                                    let _ = parent_app_handle
                                        .emit(WORKSPACE_DIRECTORY_DELETED, deleted_payload);
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!("FsWatcher(父目录): 监听错误: {:?}", e);
                    }
                }
            };

            let pt_watcher = match RecommendedWatcher::new(
                parent_callback,
                notify::Config::default().with_poll_interval(Duration::from_secs(1)),
            ) {
                Ok(w) => Some(w),
                Err(e) => {
                    log::warn!("FsWatcher: 创建父目录监听器失败: {:?}", e);
                    None
                }
            };

            if let Some(mut pt_watcher) = pt_watcher {
                if let Err(e) = pt_watcher.watch(parent_path, RecursiveMode::NonRecursive) {
                    log::warn!("FsWatcher: 启动父目录监听失败: {:?}", e);
                    // 父目录监听器启动失败不影响主功能，继续
                } else {
                    let mut parent_guard = self.parent_watcher.lock().await;
                    *parent_guard = Some(pt_watcher);
                    log::info!(
                        "FsWatcher: 父目录监听已启动, parent={}",
                        parent_path.display()
                    );
                }
            }
        }

        // 保存工作区监听器
        {
            let mut watcher_guard = self.workspace_watcher.lock().await;
            *watcher_guard = Some(ws_watcher);
        }
        {
            let mut active_guard = self.active_watch.lock().await;
            *active_guard = Some((workspace_id.clone(), path, workspace_name));
        }

        log::info!(
            "FsWatcher: 开始监听工作区 {} 路径 {}",
            workspace_id,
            workspace_path
        );
    }

    /// 停止监听
    pub async fn stop(&self) {
        {
            let mut watcher_guard = self.workspace_watcher.lock().await;
            *watcher_guard = None;
        }
        {
            let mut parent_guard = self.parent_watcher.lock().await;
            *parent_guard = None;
        }
        {
            let mut active_guard = self.active_watch.lock().await;
            *active_guard = None;
        }
        self.deletion_emitted.store(false, Ordering::SeqCst);
        log::info!("FsWatcher: 已停止监听");
    }

    /// 监听 Skill 目录的文件变更
    /// 当 SKILL.md 文件被创建/修改/删除时，触发 SkillRegistry 热重载
    ///
    /// # 参数
    /// - `dirs`: 待监听的 Skill 目录列表（全局、项目、配置目录等）
    /// - `skill_registry`: Skill 注册表的 Arc 引用，用于触发重载
    ///
    /// 一个 RecommendedWatcher 实例可同时监听多个目录，因此只创建一个 watcher
    pub async fn watch_skill_directories(
        &self,
        dirs: Vec<PathBuf>,
        skill_registry: Arc<SkillRegistry>,
    ) {
        let registry = Arc::clone(&skill_registry);

        // Skill 目录事件回调：仅当 SKILL.md 文件变更时触发重载
        let callback = move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    // 仅处理创建/修改/删除事件，忽略其他事件（如访问、关闭等）
                    match event.kind {
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => (),
                        _ => return,
                    }

                    // 检查变更路径是否包含 SKILL.md 文件
                    // 一个事件可能携带多个路径，任一匹配即触发重载
                    let should_reload = event.paths.iter().any(|p| {
                        p.file_name()
                            .map(|n| n == std::ffi::OsStr::new("SKILL.md"))
                            .unwrap_or(false)
                    });

                    if !should_reload {
                        return;
                    }

                    log::info!("FsWatcher(skill): 检测到 SKILL.md 文件变更，触发热重载");
                    match registry.reload_if_changed() {
                        Ok(count) => {
                            log::info!("FsWatcher(skill): Skill 重载完成，共 {} 个 Skill", count)
                        }
                        Err(e) => {
                            log::warn!("FsWatcher(skill): Skill 重载失败: {}", e.message)
                        }
                    }
                }
                Err(e) => {
                    log::debug!("FsWatcher(skill): 监听错误: {:?}", e);
                }
            }
        };

        // 创建 watcher（使用 2 秒轮询间隔，与工作区监听器保持一致）
        let mut watcher = match RecommendedWatcher::new(
            callback,
            notify::Config::default().with_poll_interval(Duration::from_secs(2)),
        ) {
            Ok(w) => w,
            Err(e) => {
                log::warn!("FsWatcher(skill): 创建监听器失败: {:?}", e);
                return;
            }
        };

        // 对每个目录调用 watch（一个 watcher 可监听多个目录）
        let mut watched_count = 0;
        for dir in &dirs {
            if !dir.exists() || !dir.is_dir() {
                log::debug!("FsWatcher(skill): 跳过不存在的目录: {}", dir.display());
                continue;
            }
            match watcher.watch(dir, RecursiveMode::Recursive) {
                Ok(()) => {
                    log::info!("FsWatcher(skill): 已监听 Skill 目录: {}", dir.display());
                    watched_count += 1;
                }
                Err(e) => {
                    log::warn!("FsWatcher(skill): 监听目录失败 {}: {:?}", dir.display(), e);
                }
            }
        }

        // 仅在至少监听了一个目录时保存 watcher
        if watched_count > 0 {
            let mut guard = self.skill_watcher.lock().await;
            *guard = Some(watcher);
        }
    }

    /// 获取当前监听的工作区信息 (id, path, name)
    pub async fn get_active_watch_info(&self) -> Option<(String, PathBuf, String)> {
        let active = self.active_watch.lock().await;
        active.clone()
    }

    /// 检查是否已经发射过工作区目录删除事件
    pub fn is_deletion_emitted(&self) -> bool {
        self.deletion_emitted.load(Ordering::SeqCst)
    }
}

/// 判断指定路径是否位于日志目录下（应被 FsWatcher 忽略）
///
/// 用于过滤日志目录下的文件变更事件，避免出现反馈循环：
/// 日志写入 -> 文件变更 -> FsWatcher 记录 DEBUG -> 日志写入 -> ...
///
/// - `path`: FsWatcher 收到的事件路径
/// - `log_dir`: 当前日志目录（来自 `logger::current_log_dir()`），None 表示日志系统未初始化
///
/// 返回 true 表示该路径位于日志目录下，应忽略；false 表示正常处理
fn is_path_in_log_dir(path: &Path, log_dir: Option<&Path>) -> bool {
    match log_dir {
        Some(dir) => path.starts_with(dir),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_path_in_log_dir_none_returns_false() {
        // log_dir 为 None（日志系统未初始化或降级模式）时不应过滤任何路径
        let path = Path::new("/workspace/some_file.txt");
        assert!(!is_path_in_log_dir(path, None));
    }

    #[test]
    fn test_is_path_in_log_dir_path_inside_log_dir() {
        // 路径位于日志目录下时应被过滤
        let log_dir = Path::new("/workspace/log");
        let log_file = Path::new("/workspace/log/docagent_20260629_213909.log");
        assert!(is_path_in_log_dir(log_file, Some(log_dir)));
    }

    #[test]
    fn test_is_path_in_log_dir_path_outside_log_dir() {
        // 路径不在日志目录下时不应被过滤
        let log_dir = Path::new("/workspace/log");
        let other_file = Path::new("/workspace/docs/report.docx");
        assert!(!is_path_in_log_dir(other_file, Some(log_dir)));
    }

    #[test]
    fn test_is_path_in_log_dir_log_dir_itself() {
        // 日志目录自身路径也应被过滤（防止监听到目录本身的元数据变更）
        let log_dir = Path::new("/workspace/log");
        assert!(is_path_in_log_dir(log_dir, Some(log_dir)));
    }

    #[test]
    fn test_is_path_in_log_dir_sibling_directory() {
        // 同名前缀的兄弟目录不应被误过滤（例如 /workspace/logs vs /workspace/log）
        let log_dir = Path::new("/workspace/log");
        let sibling_file = Path::new("/workspace/logs/other.log");
        assert!(!is_path_in_log_dir(sibling_file, Some(log_dir)));
    }
}
