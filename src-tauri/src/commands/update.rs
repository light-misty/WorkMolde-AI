#[cfg(desktop)]
use crate::errors::{
    CommandError, UPDATE_CHECK_FAILED, UPDATE_DOWNLOAD_FAILED, UPDATE_INSTALL_FAILED,
    UPDATE_NO_UPDATE_AVAILABLE,
};
#[cfg(desktop)]
use serde::Serialize;
#[cfg(desktop)]
use tauri_plugin_updater::UpdaterExt;

/// GitHub 更新端点
#[cfg(desktop)]
const UPDATE_ENDPOINT: &str =
    "https://github.com/light-misty/WorkMolde-AI/releases/latest/download/latest.json";

/// 构建 Updater 实例，使用 GitHub 端点
#[cfg(desktop)]
async fn build_updater(
    app: &tauri::AppHandle,
) -> Result<tauri_plugin_updater::Updater, CommandError> {
    let endpoint_url = reqwest::Url::parse(UPDATE_ENDPOINT)
        .map_err(|e| CommandError::update(UPDATE_CHECK_FAILED, format!("无效的端点 URL: {}", e)))?;

    log::info!("构建 Updater, 端点: {}", UPDATE_ENDPOINT);

    app.updater_builder()
        .endpoints(vec![endpoint_url])
        .map_err(|e| CommandError::update(UPDATE_CHECK_FAILED, format!("设置端点失败: {}", e)))?
        .build()
        .map_err(|e| CommandError::update(UPDATE_CHECK_FAILED, format!("构建 Updater 失败: {}", e)))
}

/// 更新信息
#[cfg(desktop)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// 新版本号
    pub version: String,
    /// 当前版本号
    pub current_version: String,
    /// 发布日期
    pub date: Option<String>,
    /// 更新说明
    pub body: Option<String>,
}

/// 下载进度事件
#[cfg(desktop)]
#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data")]
#[serde(rename_all = "camelCase")]
pub enum DownloadEvent {
    /// 下载进度
    Progress {
        downloaded: u64,
        content_length: Option<u64>,
    },
    /// 下载完成
    Finished,
}

/// 检查更新
#[cfg(desktop)]
#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, CommandError> {
    let updater = build_updater(&app).await?;

    let current_version = app.package_info().version.to_string();

    let update = updater.check().await.map_err(|e| {
        log::warn!("更新检查失败: {}", e);
        CommandError::update(UPDATE_CHECK_FAILED, e.to_string())
    })?;

    match update {
        Some(update) => {
            log::info!("发现新版本: {}", update.version);
            Ok(Some(UpdateInfo {
                version: update.version,
                current_version,
                date: update.date.map(|d| d.to_string()),
                body: update.body,
            }))
        }
        None => {
            log::info!("当前已是最新版本");
            Ok(None)
        }
    }
}

/// 下载更新结果
#[cfg(desktop)]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadUpdateResult {
    /// 安装包临时文件路径
    pub installer_path: String,
}

/// 下载更新（保存到临时文件，不安装）
/// 下载失败时最多重试2次，间隔3秒，重试时重新检查更新
#[cfg(desktop)]
#[tauri::command]
pub async fn download_update(
    app: tauri::AppHandle,
    on_event: tauri::ipc::Channel<DownloadEvent>,
) -> Result<DownloadUpdateResult, CommandError> {
    let max_retries: u32 = 2;

    let endpoint_url = reqwest::Url::parse(UPDATE_ENDPOINT).map_err(|e| {
        CommandError::update(UPDATE_DOWNLOAD_FAILED, format!("无效的端点 URL: {}", e))
    })?;
    log::info!("download_update 端点: {}", UPDATE_ENDPOINT);

    for retry in 0..=max_retries {
        if retry > 0 {
            log::info!("更新下载重试, 第{}次重试, 等待3秒", retry);
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }

        let updater = app
            .updater_builder()
            .endpoints(vec![endpoint_url.clone()])
            .map_err(|e| CommandError::update(UPDATE_DOWNLOAD_FAILED, e.to_string()))?
            .build()
            .map_err(|e| CommandError::update(UPDATE_DOWNLOAD_FAILED, e.to_string()))?;

        let update = updater
            .check()
            .await
            .map_err(|e| CommandError::update(UPDATE_DOWNLOAD_FAILED, e.to_string()))?;

        let update = match update {
            Some(u) => u,
            None => {
                return Err(CommandError::update(
                    UPDATE_NO_UPDATE_AVAILABLE,
                    "没有可用的更新",
                ));
            }
        };

        let mut downloaded: u64 = 0;
        // 节流状态: 上次发送进度事件的时间, 以及缓存的 content_length
        // Tauri Updater 的 on_chunk 每个 chunk(约 8-64KB)都调用一次,
        // 86MB 文件会产生上万个事件, 导致 Channel 积压, 前端进度条卡在 0%
        // 通过 100ms 节流, 将事件数量从上万降到几百, 保证进度条实时更新
        let mut last_send_time: Option<std::time::Instant> = None;
        let mut cached_content_length: Option<u64> = None;
        // 节流间隔: 100ms, 平衡实时性与性能
        const PROGRESS_THROTTLE_MS: u128 = 100;

        match update
            .download(
                |chunk_length, content_len| {
                    downloaded += chunk_length as u64;
                    // 缓存 content_length(每个 chunk 都是同一个值)
                    if cached_content_length.is_none() && content_len.is_some() {
                        cached_content_length = content_len;
                    }

                    // 节流: 距离上次发送超过 100ms 才发送, 避免高频事件导致 Channel 积压
                    let now = std::time::Instant::now();
                    let should_send = match last_send_time {
                        Some(last) => now.duration_since(last).as_millis() >= PROGRESS_THROTTLE_MS,
                        None => true, // 第一个 chunk 立即发送, 让前端尽快显示进度
                    };

                    if should_send {
                        last_send_time = Some(now);
                        let _ = on_event.send(DownloadEvent::Progress {
                            downloaded,
                            content_length: cached_content_length,
                        });
                    }
                },
                || {
                    let _ = on_event.send(DownloadEvent::Finished);
                },
            )
            .await
        {
            Ok(bytes) => {
                // 保存到临时文件
                let temp_dir = std::env::temp_dir();
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let installer_path = temp_dir.join(format!("workmolde_update_{}.exe", timestamp));
                std::fs::write(&installer_path, &bytes).map_err(|e| {
                    CommandError::update(UPDATE_INSTALL_FAILED, format!("保存更新文件失败: {}", e))
                })?;

                log::info!("更新已下载到: {:?}", installer_path);

                return Ok(DownloadUpdateResult {
                    installer_path: installer_path.to_string_lossy().to_string(),
                });
            }
            Err(e) => {
                log::error!("更新下载失败 (第{}次尝试): {}", retry + 1, e);
                if retry >= max_retries {
                    return Err(CommandError::update(UPDATE_DOWNLOAD_FAILED, e.to_string()));
                }
            }
        }
    }

    Err(CommandError::update(
        UPDATE_DOWNLOAD_FAILED,
        "更新下载失败，重试耗尽".to_string(),
    ))
}

/// 转义 NSIS 安装器命令行参数
#[cfg(all(desktop, target_os = "windows"))]
fn escape_nsis_arg(arg: &str) -> String {
    if arg.contains(' ') || arg.contains('"') || arg.contains('\t') {
        format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

/// 安装已下载的更新
/// installer_path: 下载的安装包临时文件路径
/// restart: 是否在安装完成后自动重启应用
#[cfg(desktop)]
#[tauri::command]
pub async fn install_downloaded_update(
    installer_path: String,
    restart: bool,
) -> Result<(), CommandError> {
    let path = std::path::Path::new(&installer_path);
    if !path.exists() {
        return Err(CommandError::update(
            UPDATE_INSTALL_FAILED,
            "更新安装文件不存在",
        ));
    }

    log::info!("开始安装更新, restart={}, path={}", restart, installer_path);

    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        // 构建 NSIS 参数
        let mut args = vec!["/P".to_string()]; // Passive 模式，显示进度条
        if restart {
            args.push("/R".to_string()); // 安装完成后自动重启
        }
        args.push("/UPDATE".to_string()); // 标记为更新安装

        // 获取当前进程的命令行参数，传递给 NSIS 安装器
        let current_args: Vec<String> = std::env::args().skip(1).collect();
        if !current_args.is_empty() {
            args.push("/ARGS".to_string());
            for arg in &current_args {
                args.push(escape_nsis_arg(arg));
            }
        }

        let params_str = args.join(" ");

        // 使用 ShellExecuteW 启动安装器（支持 UAC 提权）
        #[link(name = "shell32")]
        extern "system" {
            fn ShellExecuteW(
                hwnd: *mut std::ffi::c_void,
                lpoperation: *const u16,
                lpfile: *const u16,
                lpparameters: *const u16,
                lpdirectory: *const u16,
                nshowcmd: i32,
            ) -> *mut std::ffi::c_void;
        }

        const SW_SHOW: i32 = 5;

        let operation: Vec<u16> = OsStr::new("open")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let file: Vec<u16> = OsStr::new(&installer_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let parameters: Vec<u16> = OsStr::new(&params_str)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                file.as_ptr(),
                parameters.as_ptr(),
                std::ptr::null(),
                SW_SHOW,
            );
        }

        std::process::exit(0);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new(&installer_path).spawn();
        std::process::exit(0);
    }
}
