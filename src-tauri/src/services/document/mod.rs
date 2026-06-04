/// 文档服务
/// 通过 Python Sidecar 执行文档处理操作
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

use crate::errors::CommandError;
use serde_json::{json, Value};

/// Windows 平台 CREATE_NO_WINDOW 标志，防止子进程弹出命令行窗口
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// 默认请求超时时间（秒）
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 120;

/// Sidecar 健康检查请求超时（秒）
const HEALTH_CHECK_TIMEOUT_SECS: u64 = 10;

/// Sidecar 运行状态（进程 + I/O 管道）
/// 将 stdin 和 BufReader<stdout> 持久化存储，避免每次请求重新创建 BufReader
/// 导致缓冲区数据丢失的问题
struct SidecarRunning {
    /// 子进程（仅用于检查运行状态和终止）
    process: Child,
    /// stdin 管道（持久化，避免每次请求重新获取）
    stdin: ChildStdin,
    /// stdout 的 BufReader（持久化，避免缓冲区数据丢失）
    stdout_reader: BufReader<ChildStdout>,
}

/// Sidecar 进程管理器
pub struct SidecarManager {
    /// Sidecar 运行状态（包含进程和 I/O 管道，统一锁保护）
    running: Arc<Mutex<Option<SidecarRunning>>>,
    /// Python 可执行文件路径
    python_path: String,
    /// Sidecar 脚本路径
    script_path: String,
    /// 请求超时时间
    request_timeout: Duration,
    /// 连续健康检查失败次数
    health_check_failures: Arc<Mutex<u32>>,
}

impl SidecarManager {
    pub fn new(python_path: String, script_path: String) -> Self {
        Self {
            running: Arc::new(Mutex::new(None)),
            python_path,
            script_path,
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
            health_check_failures: Arc::new(Mutex::new(0)),
        }
    }

    /// 启动 Sidecar 进程并进行就绪检查
    pub async fn start(&self) -> Result<(), CommandError> {
        log::info!("启动 Sidecar 进程: python={}, script={}", self.python_path, self.script_path);
        let mut guard = self.running.lock().await;
        if guard.is_some() {
            log::warn!("Sidecar 进程已在运行, 跳过启动");
            return Ok(());
        }

        let mut cmd = Command::new(&self.python_path);
        cmd.arg(&self.script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Windows 平台：设置 CREATE_NO_WINDOW 标志，防止 Python 子进程弹出命令行窗口
        #[cfg(target_os = "windows")]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn()
            .map_err(|e| {
                log::error!("启动 Sidecar 失败: {}", e);
                CommandError::doc(3010, format!("启动 Sidecar 失败: {}", e))
            })?;

        // 取出 stderr 并启动后台任务读取日志
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    log::info!("[Sidecar stderr] {}", line);
                }
            });
        }

        // 取出 stdin 和 stdout，持久化存储
        let mut stdin = child.stdin.take().ok_or_else(|| {
            log::error!("无法获取 Sidecar stdin");
            CommandError::doc(3010, "无法获取 Sidecar stdin".to_string())
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            log::error!("无法获取 Sidecar stdout");
            CommandError::doc(3010, "无法获取 Sidecar stdout".to_string())
        })?;

        let mut stdout_reader = BufReader::new(stdout);

        // 就绪检查：发送 ping 请求验证 Sidecar 是否可以正常处理请求
        // 这能检测 Python 进程启动后立即崩溃的情况（如缺少依赖包、脚本路径错误等）
        log::info!("Sidecar 进程已启动，进行就绪检查...");
        let ping_request = json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "action": "ping",
            "type": "health",
            "params": {},
        });

        let ping_str = serde_json::to_string(&ping_request).unwrap_or_default();
        let readiness_result = tokio::time::timeout(
            Duration::from_secs(HEALTH_CHECK_TIMEOUT_SECS),
            async {
                stdin.write_all(format!("{}\n", ping_str).as_bytes()).await?;
                stdin.flush().await?;

                let mut response_line = String::new();
                let bytes_read = stdout_reader.read_line(&mut response_line).await?;

                // EOF 检查：read_line 返回 0 表示流已关闭
                if bytes_read == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "Sidecar 进程已退出，未返回响应",
                    ));
                }

                let trimmed = response_line.trim();
                let trimmed = trimmed.strip_prefix('\u{feff}').unwrap_or(trimmed);
                let _response: Value = serde_json::from_str(trimmed)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

                Ok(())
            },
        ).await;

        match readiness_result {
            Ok(Ok(())) => {
                log::info!("Sidecar 就绪检查通过");
                // 存储运行状态
                *guard = Some(SidecarRunning {
                    process: child,
                    stdin,
                    stdout_reader,
                });
                Ok(())
            }
            Ok(Err(e)) => {
                log::error!("Sidecar 就绪检查失败: {}，清理进程", e);
                // 就绪检查失败，终止进程
                let _ = child.kill().await;
                Err(CommandError::doc(3010, format!(
                    "Sidecar 启动后就绪检查失败（可能 Python 环境缺失或脚本路径错误）: {}", e
                )))
            }
            Err(_) => {
                log::error!("Sidecar 就绪检查超时，清理进程");
                // 就绪检查超时，终止进程
                let _ = child.kill().await;
                Err(CommandError::doc(3010, format!(
                    "Sidecar 启动后就绪检查超时（{}秒）（可能 Python 环境缺失或脚本路径错误）",
                    HEALTH_CHECK_TIMEOUT_SECS
                )))
            }
        }
    }

    /// 停止 Sidecar 进程
    pub async fn stop(&self) -> Result<(), CommandError> {
        log::info!("停止 Sidecar 进程");
        let mut guard = self.running.lock().await;
        if let Some(ref mut running) = *guard {
            // 尝试终止进程，即使失败也要清理运行状态，避免残留导致 start() 跳过启动
            if let Err(e) = running.process.kill().await {
                log::warn!("终止 Sidecar 进程失败（可能已退出）: {}", e);
            } else {
                log::info!("Sidecar 进程已停止");
            }
        } else {
            log::debug!("Sidecar 进程未运行, 无需停止");
        }
        // 始终清理运行状态（包括 stdin 和 stdout_reader），确保后续 start() 能正常启动新进程
        *guard = None;
        Ok(())
    }

    /// 检查 Sidecar 进程是否仍在运行
    async fn is_running(&self) -> bool {
        let mut guard = self.running.lock().await;
        if let Some(ref mut running) = *guard {
            match running.process.try_wait() {
                Ok(Some(_status)) => {
                    // 进程已退出，清理整个运行状态（包括 stdin 和 stdout_reader）
                    log::warn!("Sidecar 进程已退出");
                    *guard = None;
                    false
                }
                Ok(None) => true,
                Err(e) => {
                    log::error!("检查 Sidecar 进程状态失败: {}", e);
                    false
                }
            }
        } else {
            false
        }
    }

    /// 确保 Sidecar 正在运行，如果未运行则自动重启
    async fn ensure_running(&self) -> Result<(), CommandError> {
        if self.is_running().await {
            return Ok(());
        }
        log::info!("Sidecar 未运行，正在重启...");
        // 先清理可能残留的旧运行状态，避免 start() 检测到 is_some() 而跳过启动
        let _ = self.stop().await;
        self.start().await
    }

    /// 发送请求到 Sidecar 并获取响应（带超时和自动重启）
    pub async fn send_request(&self, request: Value) -> Result<Value, CommandError> {
        log::debug!("发送请求到 Sidecar: action={}", request["action"]);

        // 确保 Sidecar 正在运行
        self.ensure_running().await?;

        // 带超时执行请求
        let result = tokio::time::timeout(
            self.request_timeout,
            self.send_request_inner(request.clone()),
        ).await;

        match result {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(e)) => {
                // 请求失败，可能是进程崩溃，先清理旧进程再重启
                log::warn!("Sidecar 请求失败，尝试重启: {}", e.message);
                let _ = self.stop().await;
                if self.start().await.is_ok() {
                    // 重启成功后重试一次（带超时保护）
                    tokio::time::timeout(
                        self.request_timeout,
                        self.send_request_inner(request),
                    ).await.map_err(|_| {
                        log::error!("Sidecar 重试请求超时（{}秒）", self.request_timeout.as_secs());
                        CommandError::doc(3010, format!(
                            "Sidecar 重试请求超时（{}秒）", self.request_timeout.as_secs()
                        ))
                    })?
                } else {
                    Err(e)
                }
            }
            Err(_) => {
                // 请求超时
                log::error!("Sidecar 请求超时（{}秒）", self.request_timeout.as_secs());
                // 超时后重启 Sidecar
                let _ = self.stop().await;
                Err(CommandError::doc(3010, format!(
                    "Sidecar 请求超时（{}秒）", self.request_timeout.as_secs()
                )))
            }
        }
    }

    /// 内部发送请求实现（无超时、无重试）
    pub async fn send_request_inner(&self, request: Value) -> Result<Value, CommandError> {
        let mut guard = self.running.lock().await;

        let running = guard.as_mut().ok_or_else(|| {
            log::error!("Sidecar 未启动, 无法发送请求");
            CommandError::doc(3010, "Sidecar 未启动".to_string())
        })?;

        // 写入请求
        let request_str = serde_json::to_string(&request).unwrap_or_default();
        running.stdin.write_all(format!("{}\n", request_str).as_bytes()).await.map_err(|e| {
            log::error!("写入 Sidecar 失败: {}", e);
            CommandError::doc(3010, format!("写入 Sidecar 失败: {}", e))
        })?;
        running.stdin.flush().await.map_err(|e| {
            log::error!("刷新 Sidecar stdin 失败: {}", e);
            CommandError::doc(3010, format!("刷新 Sidecar stdin 失败: {}", e))
        })?;
        log::debug!("请求已写入 Sidecar");

        // 读取响应
        let mut response_line = String::new();
        let bytes_read = running.stdout_reader.read_line(&mut response_line).await.map_err(|e| {
            log::error!("读取 Sidecar 响应失败: {}", e);
            CommandError::doc(3010, format!("读取 Sidecar 响应失败: {}", e))
        })?;

        // EOF 检查：read_line 返回 0 表示流已关闭（Sidecar 进程已退出）
        if bytes_read == 0 {
            log::error!("Sidecar 进程已退出，未返回响应（可能运行时崩溃）");
            return Err(CommandError::doc(3010, "Sidecar 进程已退出，未返回响应（可能运行时崩溃）".to_string()));
        }

        let trimmed = response_line.trim();
        // 去除 UTF-8 BOM（Python Sidecar 输出可能包含 BOM，trim() 不会移除）
        let trimmed = trimmed.strip_prefix('\u{feff}').unwrap_or(trimmed);

        let response: Value = serde_json::from_str(trimmed).map_err(|e| {
            log::error!("解析 Sidecar 响应失败: {}, 原始内容: {}", e, trimmed);
            CommandError::doc(3010, format!("解析 Sidecar 响应失败: {}", e))
        })?;

        log::debug!("收到 Sidecar 响应: success={}", response["success"].as_bool().unwrap_or(false));
        Ok(response)
    }
}

/// 文档服务
pub struct DocumentService {
    sidecar: SidecarManager,
}

impl DocumentService {
    pub fn new(sidecar: SidecarManager) -> Self {
        Self { sidecar }
    }

    /// 处理文档操作
    /// 如果 Sidecar 未启动，会自动启动
    pub async fn process(
        &self,
        action: &str,
        doc_type: &str,
        params: Value,
    ) -> Result<Value, CommandError> {
        log::info!("处理文档操作: action={}, doc_type={}", action, doc_type);

        let request = json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "action": action,
            "type": doc_type,
            "params": params,
        });

        let response = self.sidecar.send_request(request).await?;

        if response["success"].as_bool().unwrap_or(false) {
            log::info!("文档处理成功: action={}, doc_type={}", action, doc_type);
            Ok(response["data"].clone())
        } else {
            let error = response["error"].as_str().unwrap_or("未知错误");
            log::error!("文档处理失败: action={}, doc_type={}, 错误: {}", action, doc_type, error);
            Err(CommandError::doc(3010, error.to_string()))
        }
    }

    /// 执行 Sidecar 健康检查
    /// 发送 ping 请求，如果 Sidecar 无响应或响应异常则返回 false
    /// 连续失败 3 次会自动重启 Sidecar
    pub async fn health_check(&self) -> bool {
        let request = json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "action": "ping",
            "type": "health",
            "params": {},
        });

        // 先检查进程是否在运行
        if !self.sidecar.is_running().await {
            log::warn!("Sidecar 健康检查: 进程未运行");
            let mut failures = self.sidecar.health_check_failures.lock().await;
            *failures += 1;
            if *failures >= 3 {
                log::warn!("Sidecar 连续 {} 次健康检查失败，尝试重启", *failures);
                *failures = 0;
                let _ = self.sidecar.stop().await;
                if let Err(e) = self.sidecar.start().await {
                    log::error!("Sidecar 重启失败: {}", e.message);
                    return false;
                }
                log::info!("Sidecar 重启成功");
            }
            return false;
        }

        // 发送 ping 请求
        let result = tokio::time::timeout(
            Duration::from_secs(HEALTH_CHECK_TIMEOUT_SECS),
            self.sidecar.send_request_inner(request),
        ).await;

        let mut failures = self.sidecar.health_check_failures.lock().await;

        match result {
            Ok(Ok(response)) => {
                let success = response["success"].as_bool().unwrap_or(false);
                if success {
                    // 健康检查成功，重置失败计数
                    *failures = 0;
                    log::debug!("Sidecar 健康检查通过");
                    true
                } else {
                    *failures += 1;
                    log::warn!("Sidecar 健康检查: 响应异常 (连续失败 {} 次)", *failures);
                    false
                }
            }
            Ok(Err(e)) => {
                *failures += 1;
                log::warn!("Sidecar 健康检查: 请求失败 {} (连续失败 {} 次)", e.message, *failures);
                if *failures >= 3 {
                    log::warn!("Sidecar 连续 {} 次健康检查失败，尝试重启", *failures);
                    *failures = 0;
                    drop(failures); // 释放锁后再操作
                    let _ = self.sidecar.stop().await;
                    if let Err(e) = self.sidecar.start().await {
                        log::error!("Sidecar 重启失败: {}", e.message);
                        return false;
                    }
                    log::info!("Sidecar 重启成功");
                }
                false
            }
            Err(_) => {
                *failures += 1;
                log::warn!("Sidecar 健康检查: 超时 (连续失败 {} 次)", *failures);
                if *failures >= 3 {
                    log::warn!("Sidecar 连续 {} 次健康检查失败，尝试重启", *failures);
                    *failures = 0;
                    drop(failures); // 释放锁后再操作
                    let _ = self.sidecar.stop().await;
                    if let Err(e) = self.sidecar.start().await {
                        log::error!("Sidecar 重启失败: {}", e.message);
                        return false;
                    }
                    log::info!("Sidecar 重启成功");
                }
                false
            }
        }
    }
}
