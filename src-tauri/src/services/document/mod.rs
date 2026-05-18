/// 文档服务
/// 通过 Python Sidecar 执行文档处理操作
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::errors::CommandError;
use serde_json::{json, Value};

/// 默认请求超时时间（秒）
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 120;

/// Sidecar 进程管理器
pub struct SidecarManager {
    /// Sidecar 进程
    process: Arc<Mutex<Option<Child>>>,
    /// Python 可执行文件路径
    python_path: String,
    /// Sidecar 脚本路径
    script_path: String,
    /// 请求超时时间
    request_timeout: Duration,
}

impl SidecarManager {
    pub fn new(python_path: String, script_path: String) -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
            python_path,
            script_path,
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
        }
    }

    /// 启动 Sidecar 进程
    pub async fn start(&self) -> Result<(), CommandError> {
        log::info!("启动 Sidecar 进程: python={}, script={}", self.python_path, self.script_path);
        let mut guard = self.process.lock().await;
        if guard.is_some() {
            log::warn!("Sidecar 进程已在运行, 跳过启动");
            return Ok(());
        }

        let mut child = Command::new(&self.python_path)
            .arg(&self.script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                log::error!("启动 Sidecar 失败: {}", e);
                CommandError::doc(3001, format!("启动 Sidecar 失败: {}", e))
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

        *guard = Some(child);
        log::info!("Sidecar 进程启动成功");
        Ok(())
    }

    /// 停止 Sidecar 进程
    pub async fn stop(&self) -> Result<(), CommandError> {
        log::info!("停止 Sidecar 进程");
        let mut guard = self.process.lock().await;
        if let Some(ref mut child) = *guard {
            // 尝试终止进程，即使失败也要清理进程对象，避免残留导致 start() 跳过启动
            if let Err(e) = child.kill().await {
                log::warn!("终止 Sidecar 进程失败（可能已退出）: {}", e);
            } else {
                log::info!("Sidecar 进程已停止");
            }
        } else {
            log::debug!("Sidecar 进程未运行, 无需停止");
        }
        // 始终清理进程对象，确保后续 start() 能正常启动新进程
        *guard = None;
        Ok(())
    }

    /// 检查 Sidecar 进程是否仍在运行
    async fn is_running(&self) -> bool {
        let mut guard = self.process.lock().await;
        if let Some(ref mut child) = *guard {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    // 进程已退出
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
        // 先清理可能残留的旧进程对象，避免 start() 检测到 is_some() 而跳过启动
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
                    // 重启成功后重试一次
                    self.send_request_inner(request).await
                } else {
                    Err(e)
                }
            }
            Err(_) => {
                // 请求超时
                log::error!("Sidecar 请求超时（{}秒）", self.request_timeout.as_secs());
                // 超时后重启 Sidecar
                let _ = self.stop().await;
                Err(CommandError::doc(3009, format!(
                    "Sidecar 请求超时（{}秒）", self.request_timeout.as_secs()
                )))
            }
        }
    }

    /// 内部发送请求实现（无超时、无重试）
    async fn send_request_inner(&self, request: Value) -> Result<Value, CommandError> {
        let mut guard = self.process.lock().await;

        let child = guard.as_mut().ok_or_else(|| {
            log::error!("Sidecar 未启动, 无法发送请求");
            CommandError::doc(3003, "Sidecar 未启动".to_string())
        })?;

        // 写入请求
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            log::error!("无法写入 Sidecar stdin");
            CommandError::doc(3004, "无法写入 Sidecar stdin".to_string())
        })?;

        let request_str = serde_json::to_string(&request).unwrap_or_default();
        stdin.write_all(format!("{}\n", request_str).as_bytes()).await.map_err(|e| {
            log::error!("写入 Sidecar 失败: {}", e);
            CommandError::doc(3005, format!("写入 Sidecar 失败: {}", e))
        })?;
        stdin.flush().await.map_err(|e| {
            log::error!("刷新 Sidecar stdin 失败: {}", e);
            CommandError::doc(3005, format!("刷新 Sidecar stdin 失败: {}", e))
        })?;
        log::debug!("请求已写入 Sidecar");

        // 读取响应
        let stdout = child.stdout.as_mut().ok_or_else(|| {
            log::error!("无法读取 Sidecar stdout");
            CommandError::doc(3006, "无法读取 Sidecar stdout".to_string())
        })?;

        let mut reader = BufReader::new(stdout);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await.map_err(|e| {
            log::error!("读取 Sidecar 响应失败: {}", e);
            CommandError::doc(3007, format!("读取 Sidecar 响应失败: {}", e))
        })?;

        let response: Value = serde_json::from_str(response_line.trim()).map_err(|e| {
            log::error!("解析 Sidecar 响应失败: {}, 原始内容: {}", e, response_line.trim());
            CommandError::doc(3008, format!("解析 Sidecar 响应失败: {}", e))
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
            Err(CommandError::doc(3000, error.to_string()))
        }
    }
}
