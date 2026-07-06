#[cfg(desktop)]
use crate::errors::{
    CommandError, UPDATE_CHECK_FAILED, UPDATE_DOWNLOAD_FAILED, UPDATE_INSTALL_FAILED,
    UPDATE_NO_UPDATE_AVAILABLE,
};
#[cfg(desktop)]
use serde::Serialize;
#[cfg(desktop)]
use tauri_plugin_updater::UpdaterExt;

/// 更新端点列表（GitHub 主，Gitee 备）
/// Tauri Updater 默认按顺序尝试端点，国内用户访问 GitHub 可能极慢
/// 因此通过 select_endpoint_order() 并行探测两个端点，动态调整顺序
#[cfg(desktop)]
const UPDATE_ENDPOINTS: &[&str] = &[
    "https://github.com/XuMingKe-06/DocAgent/releases/latest/download/latest.json",
    "https://gitee.com/xumingke-06/doc-agent/raw/main/updater/latest.json",
];

/// 端点探测超时时间（秒）
/// 设置较短的超时，让慢速端点尽快被淘汰
#[cfg(desktop)]
const ENDPOINT_PROBE_TIMEOUT_SECS: u64 = 8;

/// 并行探测两个更新端点，返回按响应速度排序的端点列表
/// 第一个元素为先响应的端点，第二个为备用端点
/// 用于解决国内用户访问 GitHub 慢导致更新下载卡住的问题
#[cfg(desktop)]
async fn select_endpoint_order() -> Vec<String> {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(ENDPOINT_PROBE_TIMEOUT_SECS))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            log::warn!("构建探测 HTTP 客户端失败: {}, 使用默认端点顺序", e);
            return UPDATE_ENDPOINTS.iter().map(|s| s.to_string()).collect();
        }
    };

    // 使用常量数组中的 &'static str 构造 String，避免 async 闭包借用局部变量导致的 move 冲突
    let github_url = UPDATE_ENDPOINTS[0].to_string();
    let gitee_url = UPDATE_ENDPOINTS[1].to_string();

    // 探测单个端点：发送 GET 请求并读取响应体（latest.json 很小，约 1KB）
    // 直接使用常量 UPDATE_ENDPOINTS[i]（&'static str），避免借用局部变量
    let probe_github = async {
        let resp = client.get(UPDATE_ENDPOINTS[0]).send().await?;
        resp.bytes().await?;
        Ok::<_, reqwest::Error>(())
    };
    let probe_gitee = async {
        let resp = client.get(UPDATE_ENDPOINTS[1]).send().await?;
        resp.bytes().await?;
        Ok::<_, reqwest::Error>(())
    };

    // 使用 futures::future::select 竞速：先完成（成功或失败）的 future 被返回
    // 未完成的 future 被保留，可在后续继续 await
    let result = match futures::future::select(Box::pin(probe_github), Box::pin(probe_gitee)).await {
        // GitHub 先完成
        futures::future::Either::Left((Ok(()), _)) => {
            log::info!("更新端点探测: GitHub 先响应，优先使用");
            vec![github_url, gitee_url]
        }
        futures::future::Either::Left((Err(e), gitee_future)) => {
            log::warn!("更新端点探测: GitHub 探测失败 ({}), 等待 Gitee", e);
            match gitee_future.await {
                Ok(()) => {
                    log::info!("更新端点探测: Gitee 响应成功，优先使用");
                    vec![gitee_url, github_url]
                }
                Err(e2) => {
                    log::warn!(
                        "更新端点探测: 双端点均失败 (GitHub: {}, Gitee: {}), 使用默认顺序",
                        e, e2
                    );
                    vec![github_url, gitee_url]
                }
            }
        }
        // Gitee 先完成
        futures::future::Either::Right((Ok(()), _)) => {
            log::info!("更新端点探测: Gitee 先响应，优先使用");
            vec![gitee_url, github_url]
        }
        futures::future::Either::Right((Err(e), github_future)) => {
            log::warn!("更新端点探测: Gitee 探测失败 ({}), 等待 GitHub", e);
            match github_future.await {
                Ok(()) => {
                    log::info!("更新端点探测: GitHub 响应成功，优先使用");
                    vec![github_url, gitee_url]
                }
                Err(e2) => {
                    log::warn!(
                        "更新端点探测: 双端点均失败 (Gitee: {}, GitHub: {}), 使用默认顺序",
                        e, e2
                    );
                    vec![github_url, gitee_url]
                }
            }
        }
    };
    result
}

/// 使用动态选择的端点顺序构建 Updater
/// 先并行探测两个端点，按响应速度排序后传给 UpdaterBuilder
/// Tauri Updater 会按顺序尝试端点，第一个成功响应的端点被使用
#[cfg(desktop)]
async fn build_updater(
    app: &tauri::AppHandle,
) -> Result<tauri_plugin_updater::Updater, CommandError> {
    let endpoints = select_endpoint_order().await;

    // 将端点字符串解析为 Url 对象
    let endpoint_urls: Vec<reqwest::Url> = endpoints
        .iter()
        .map(|url| reqwest::Url::parse(url))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| CommandError::update(UPDATE_CHECK_FAILED, format!("无效的端点 URL: {}", e)))?;

    log::info!("构建 Updater, 端点顺序: {:?}", endpoints);

    app.updater_builder()
        .endpoints(endpoint_urls)
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
/// 先并行探测 GitHub/Gitee 端点响应速度，使用先响应的端点进行检查
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

/// 下载并安装更新（通过 Channel 推送进度）
/// 下载失败时最多重试2次，间隔3秒，重试时重新检查更新
/// 端点探测只在开始时执行一次，重试时复用相同的端点顺序
#[cfg(desktop)]
#[tauri::command]
pub async fn download_and_install_update(
    app: tauri::AppHandle,
    on_event: tauri::ipc::Channel<DownloadEvent>,
) -> Result<(), CommandError> {
    let max_retries: u32 = 2;

    // 探测端点顺序（只在开始时探测一次，重试时复用，避免每次重试都重新探测）
    let endpoint_urls = select_endpoint_order()
        .await
        .iter()
        .map(|url| reqwest::Url::parse(url).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| CommandError::update(UPDATE_DOWNLOAD_FAILED, format!("无效的端点 URL: {}", e)))?;
    log::info!("download_and_install_update 端点顺序: {:?}", endpoint_urls);

    for retry in 0..=max_retries {
        if retry > 0 {
            log::info!("更新下载重试, 第{}次重试, 等待3秒", retry);
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }

        let updater = app
            .updater_builder()
            .endpoints(endpoint_urls.clone())
            .map_err(|e| CommandError::update(UPDATE_DOWNLOAD_FAILED, e.to_string()))?
            .build()
            .map_err(|e| CommandError::update(UPDATE_DOWNLOAD_FAILED, e.to_string()))?;

        let update = updater.check().await.map_err(|e| {
            CommandError::update(UPDATE_DOWNLOAD_FAILED, e.to_string())
        })?;

        let update = match update {
            Some(u) => u,
            None => {
                return Err(CommandError::update(UPDATE_NO_UPDATE_AVAILABLE, "没有可用的更新"));
            }
        };

        let mut downloaded: u64 = 0;
        let mut content_length: Option<u64> = None;

        match update
            .download_and_install(
                |chunk_length, content_len| {
                    downloaded += chunk_length as u64;
                    content_length = content_len;
                    let _ = on_event.send(DownloadEvent::Progress {
                        downloaded,
                        content_length: content_len,
                    });
                },
                || {
                    let _ = on_event.send(DownloadEvent::Finished);
                },
            )
            .await
        {
            Ok(()) => {
                log::info!("更新安装完成，准备重启");
                return Ok(());
            }
            Err(e) => {
                log::error!("更新下载/安装失败 (第{}次尝试): {}", retry + 1, e);
                if retry >= max_retries {
                    return Err(CommandError::update(UPDATE_INSTALL_FAILED, e.to_string()));
                }
            }
        }
    }

    Err(CommandError::update(UPDATE_INSTALL_FAILED, "更新下载失败，重试耗尽".to_string()))
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
/// 端点探测只在开始时执行一次，重试时复用相同的端点顺序
#[cfg(desktop)]
#[tauri::command]
pub async fn download_update(
    app: tauri::AppHandle,
    on_event: tauri::ipc::Channel<DownloadEvent>,
) -> Result<DownloadUpdateResult, CommandError> {
    let max_retries: u32 = 2;

    // 探测端点顺序（只在开始时探测一次，重试时复用，避免每次重试都重新探测）
    let endpoint_urls = select_endpoint_order()
        .await
        .iter()
        .map(|url| reqwest::Url::parse(url).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| CommandError::update(UPDATE_DOWNLOAD_FAILED, format!("无效的端点 URL: {}", e)))?;
    log::info!("download_update 端点顺序: {:?}", endpoint_urls);

    for retry in 0..=max_retries {
        if retry > 0 {
            log::info!("更新下载重试, 第{}次重试, 等待3秒", retry);
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }

        let updater = app
            .updater_builder()
            .endpoints(endpoint_urls.clone())
            .map_err(|e| CommandError::update(UPDATE_DOWNLOAD_FAILED, e.to_string()))?
            .build()
            .map_err(|e| CommandError::update(UPDATE_DOWNLOAD_FAILED, e.to_string()))?;

        let update = updater.check().await.map_err(|e| {
            CommandError::update(UPDATE_DOWNLOAD_FAILED, e.to_string())
        })?;

        let update = match update {
            Some(u) => u,
            None => {
                return Err(CommandError::update(UPDATE_NO_UPDATE_AVAILABLE, "没有可用的更新"));
            }
        };

        let mut downloaded: u64 = 0;
        let mut content_length: Option<u64> = None;

        match update
            .download(
                |chunk_length, content_len| {
                    downloaded += chunk_length as u64;
                    content_length = content_len;
                    let _ = on_event.send(DownloadEvent::Progress {
                        downloaded,
                        content_length: content_len,
                    });
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
                let installer_path = temp_dir.join(format!("docagent_update_{}.exe", timestamp));
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

    Err(CommandError::update(UPDATE_DOWNLOAD_FAILED, "更新下载失败，重试耗尽".to_string()))
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
        return Err(CommandError::update(UPDATE_INSTALL_FAILED, "更新安装文件不存在"));
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
