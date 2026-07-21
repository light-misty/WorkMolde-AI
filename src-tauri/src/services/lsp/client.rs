//! LSP 客户端:与外部 LSP 服务器通过 JSON-RPC 2.0 通信
//! 基于 stdio 传输(LSP 最常见的传输方式)

use crate::errors::CommandError;
use crate::models::lsp::*;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child as TokioChild, Command as TokioCommand};
use tokio::sync::{oneshot, Mutex as TokioMutex};

/// LSP 客户端
pub struct LspClient {
    /// 语言名称
    language: String,
    /// 子进程
    process: TokioMutex<Option<TokioChild>>,
    /// stdin 写入句柄
    stdin: TokioMutex<Option<tokio::process::ChildStdin>>,
    /// 请求 ID 生成器
    request_id: AtomicU64,
    /// 等待响应的请求映射(request_id -> sender)
    pending_requests: Arc<TokioMutex<std::collections::HashMap<u64, oneshot::Sender<Value>>>>,
    /// 服务器信息(使用 Arc 以便 reader task 更新崩溃状态)
    server_info: Arc<TokioMutex<Option<LspServerInfo>>>,
    /// 工作区根目录
    workspace_root: std::path::PathBuf,
    /// 请求超时时间(从 LspConfig.request_timeout_seconds 读取)
    request_timeout: Duration,
}

impl LspClient {
    /// 创建 LSP 客户端(不立即启动)
    pub fn new(
        language: String,
        workspace_root: std::path::PathBuf,
        request_timeout: Duration,
    ) -> Self {
        Self {
            language,
            process: TokioMutex::new(None),
            stdin: TokioMutex::new(None),
            request_id: AtomicU64::new(1),
            pending_requests: Arc::new(TokioMutex::new(std::collections::HashMap::new())),
            server_info: Arc::new(TokioMutex::new(None)),
            workspace_root,
            request_timeout,
        }
    }

    /// 启动 LSP 服务器并发送 initialize 请求
    pub async fn start(&self, command: &[String]) -> Result<LspServerInfo, CommandError> {
        if command.is_empty() {
            return Err(CommandError::config(
                crate::errors::CONFIG_MISSING_FIELD,
                "LSP 启动命令为空",
            ));
        }

        log::info!(
            "启动 LSP 服务器: language={}, command={:?}",
            self.language,
            command
        );

        // 启动子进程
        let mut cmd = TokioCommand::new(&command[0]);
        if command.len() > 1 {
            cmd.args(&command[1..]);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(&self.workspace_root);

        // Windows 下隐藏控制台窗口
        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| CommandError::runtime(7001, "无法获取 LSP stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CommandError::runtime(7001, "无法获取 LSP stdout"))?;

        // 存储进程和 stdin
        *self.process.lock().await = Some(child);
        *self.stdin.lock().await = Some(stdin);

        // 启动 stdout 读取任务
        self.start_reader_task(stdout).await;

        // 发送 initialize 请求
        let init_params = json!({
            "processId": std::process::id(),
            "rootUri": format!("file:///{}", self.workspace_root.to_string_lossy().replace('\\', "/")),
            "capabilities": {
                "textDocument": {
                    "definition": { "linkSupport": false },
                    "hover": { "contentFormat": ["markdown", "plaintext"] },
                    "references": {},
                    "diagnostic": {}
                }
            },
            "workspaceFolders": [{
                "uri": format!("file:///{}", self.workspace_root.to_string_lossy().replace('\\', "/")),
                "name": self.workspace_root.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("workspace")
            }]
        });

        // 发送 initialize 请求,失败时清理已 spawn 的子进程,防止泄漏
        let response = match self.request("initialize", init_params).await {
            Ok(resp) => resp,
            Err(e) => {
                // initialize 失败,清理已 spawn 的子进程,防止泄漏
                log::warn!(
                    "LSP {} initialize 失败,清理子进程: {}",
                    self.language,
                    e.message
                );
                let mut process = self.process.lock().await;
                if let Some(mut child) = process.take() {
                    let _ = child.kill().await;
                }
                *self.stdin.lock().await = None;
                return Err(e);
            }
        };

        // 解析服务器信息
        let server_name = response
            .get("serverInfo")
            .and_then(|s| s.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string());

        let server_version = response
            .get("serverInfo")
            .and_then(|s| s.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let capabilities = response.get("capabilities").cloned();

        // 发送 initialized 通知
        self.notify("initialized", json!({})).await?;

        let info = LspServerInfo {
            language: self.language.clone(),
            server_name,
            server_version,
            workspace_root: self.workspace_root.clone(),
            status: LspServerStatus::Ready,
            capabilities,
            started_at: current_timestamp_ms(),
            last_activity_at: current_timestamp_ms(),
            error: None,
        };

        *self.server_info.lock().await = Some(info.clone());

        log::info!(
            "LSP 服务器已就绪: language={}, server={:?}",
            self.language,
            info.server_name
        );

        Ok(info)
    }

    /// 启动 stdout 读取任务
    async fn start_reader_task(&self, stdout: tokio::process::ChildStdout) {
        let pending_requests = self.pending_requests.clone();
        let language = self.language.clone();
        let server_info = self.server_info.clone();

        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut content_length: Option<usize> = None;
            let mut buffer = String::new();

            loop {
                let mut line = String::new();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        log::error!("LSP {} stdout 已关闭,服务器进程可能已崩溃", language);
                        // 更新服务器状态为 Terminated(stdout 关闭表示子进程已退出)
                        {
                            let mut info = server_info.lock().await;
                            if let Some(ref mut info) = *info {
                                info.status = LspServerStatus::Terminated;
                                info.error = Some("LSP 服务器进程已退出(stdout 关闭)".to_string());
                            }
                        } // 释放 server_info 锁,避免嵌套持锁
                          // 清空 pending_requests,让所有等待响应的请求立即失败(sender drop 后 rx 收到 Err)
                        pending_requests.lock().await.clear();
                        break;
                    }
                    Ok(_) => {
                        // 解析 LSP 消息头
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            // 空行分隔头和正文,开始读取正文
                            if let Some(len) = content_length {
                                let mut body = vec![0u8; len];
                                if reader.read_exact(&mut body).await.is_ok() {
                                    if let Ok(body_str) = String::from_utf8(body) {
                                        Self::handle_message(&body_str, &pending_requests).await;
                                    }
                                }
                                content_length = None;
                                buffer.clear();
                            }
                        } else if let Some(len_str) = line.strip_prefix("Content-Length: ") {
                            content_length = len_str.trim().parse().ok();
                        }
                    }
                    Err(e) => {
                        log::error!("LSP {} 读取 stdout 失败: {}", language, e);
                        // 读取失败也更新状态为 Terminated
                        {
                            let mut info = server_info.lock().await;
                            if let Some(ref mut info) = *info {
                                info.status = LspServerStatus::Terminated;
                                info.error = Some(format!("LSP 读取 stdout 失败: {}", e));
                            }
                        } // 释放 server_info 锁,避免嵌套持锁
                          // 清空 pending_requests,让所有等待响应的请求立即失败(sender drop 后 rx 收到 Err)
                        pending_requests.lock().await.clear();
                        break;
                    }
                }
            }
        });
    }

    /// 处理收到的 LSP 消息
    async fn handle_message(
        body: &str,
        pending_requests: &TokioMutex<std::collections::HashMap<u64, oneshot::Sender<Value>>>,
    ) {
        match serde_json::from_str::<Value>(body) {
            Ok(msg) => {
                if let Some(id) = msg.get("id").and_then(|v| v.as_u64()) {
                    // 响应消息
                    let result = msg.get("result").cloned().unwrap_or(Value::Null);
                    let mut pending = pending_requests.lock().await;
                    if let Some(sender) = pending.remove(&id) {
                        let _ = sender.send(result);
                    }
                } else if let Some(method) = msg.get("method").and_then(|v| v.as_str()) {
                    // 通知或请求(暂不处理服务器端请求)
                    log::debug!("LSP 通知: method={}", method);
                }
            }
            Err(e) => {
                log::warn!("LSP 消息解析失败: {}", e);
            }
        }
    }

    /// 发送请求并等待响应
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, CommandError> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(id, tx);
        }

        self.send_message(&message).await?;

        // 等待响应(使用配置的超时时间)
        match tokio::time::timeout(self.request_timeout, rx).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => Err(CommandError::runtime(
                7001,
                format!("LSP 请求 {} 的响应通道已关闭", method),
            )),
            Err(_) => {
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&id);
                Err(CommandError::runtime(
                    7001,
                    format!(
                        "LSP 请求 {} 超时({}秒)",
                        method,
                        self.request_timeout.as_secs()
                    ),
                ))
            }
        }
    }

    /// 发送通知(不等待响应)
    pub async fn notify(&self, method: &str, params: Value) -> Result<(), CommandError> {
        let message = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.send_message(&message).await
    }

    /// 发送消息到 stdin
    async fn send_message(&self, message: &Value) -> Result<(), CommandError> {
        let body = serde_json::to_string(message)?;
        let content = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        let mut stdin = self.stdin.lock().await;
        if let Some(stdin) = stdin.as_mut() {
            stdin.write_all(content.as_bytes()).await?;
            stdin.flush().await?;
            Ok(())
        } else {
            Err(CommandError::runtime(7001, "LSP stdin 不可用"))
        }
    }

    /// 发送 textDocument/didOpen 通知
    pub async fn did_open(
        &self,
        file_path: &Path,
        language_id: &str,
        content: &str,
    ) -> Result<(), CommandError> {
        let uri = path_to_uri(file_path);
        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": content,
                }
            }),
        )
        .await
    }

    /// 发送 textDocument/didChange 通知
    pub async fn did_change(
        &self,
        file_path: &Path,
        content: &str,
        version: u32,
    ) -> Result<(), CommandError> {
        let uri = path_to_uri(file_path);
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": content }]
            }),
        )
        .await
    }

    /// 发送 textDocument/didClose 通知
    pub async fn did_close(&self, file_path: &Path) -> Result<(), CommandError> {
        let uri = path_to_uri(file_path);
        self.notify(
            "textDocument/didClose",
            json!({
                "textDocument": { "uri": uri }
            }),
        )
        .await
    }

    /// 请求 textDocument/definition
    pub async fn goto_definition(
        &self,
        file_path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<LspLocation>, CommandError> {
        let uri = path_to_uri(file_path);
        let result = self
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character }
                }),
            )
            .await?;

        Ok(parse_locations(&result))
    }

    /// 请求 textDocument/references
    pub async fn find_references(
        &self,
        file_path: &Path,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> Result<Vec<LspLocation>, CommandError> {
        let uri = path_to_uri(file_path);
        let result = self
            .request(
                "textDocument/references",
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character },
                    "context": { "includeDeclaration": include_declaration }
                }),
            )
            .await?;

        Ok(parse_locations(&result))
    }

    /// 请求 textDocument/hover
    pub async fn hover(
        &self,
        file_path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Option<LspHover>, CommandError> {
        let uri = path_to_uri(file_path);
        let result = self
            .request(
                "textDocument/hover",
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character }
                }),
            )
            .await?;

        if result.is_null() {
            return Ok(None);
        }

        let content = result
            .get("contents")
            .and_then(|c| c.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(Some(LspHover {
            content,
            range: None, // 简化,不解析 range
        }))
    }

    /// 请求 textDocument/diagnostic
    pub async fn diagnostics(&self, file_path: &Path) -> Result<Vec<LspDiagnostic>, CommandError> {
        let uri = path_to_uri(file_path);
        // diagnostics 通常通过推送通知获取,这里请求全量诊断
        let result = self
            .request(
                "textDocument/diagnostic",
                json!({
                    "textDocument": { "uri": uri }
                }),
            )
            .await?;

        let items = result
            .get("items")
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default();

        Ok(items
            .iter()
            .filter_map(|d| {
                let severity = d.get("severity").and_then(|s| s.as_u64()).unwrap_or(3) as u8;
                let message = d
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("")
                    .to_string();
                let source = d
                    .get("source")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string());
                let code = d
                    .get("code")
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string());

                let range = d.get("range")?;
                let start = range.get("start")?;
                let end = range.get("end")?;

                Some(LspDiagnostic {
                    source,
                    severity,
                    message,
                    code,
                    location: LspLocation {
                        uri: uri.clone(),
                        file_path: file_path.to_string_lossy().to_string(),
                        start_line: start.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32,
                        start_character: start
                            .get("character")
                            .and_then(|c| c.as_u64())
                            .unwrap_or(0) as u32,
                        end_line: end.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32,
                        end_character: end.get("character").and_then(|c| c.as_u64()).unwrap_or(0)
                            as u32,
                    },
                })
            })
            .collect())
    }

    /// 请求 textDocument/documentSymbol(获取文档符号列表)
    /// 解析 DocumentSymbol[] 响应(含递归 children)为扁平的 LspSymbol 列表
    pub async fn document_symbol(&self, file_path: &Path) -> Result<Vec<LspSymbol>, CommandError> {
        let uri = path_to_uri(file_path);
        let result = self
            .request(
                "textDocument/documentSymbol",
                json!({
                    "textDocument": { "uri": uri }
                }),
            )
            .await?;

        // DocumentSymbol 包含: name, kind, range, selectionRange, detail?, children?
        // 递归解析(含 children),展平为 LspSymbol 列表
        Ok(parse_document_symbols(&result, file_path))
    }

    /// 请求 workspace/symbol(搜索工作区符号)
    /// 解析 SymbolInformation[] 响应为 LspSymbol 列表
    pub async fn workspace_symbol(&self, query: &str) -> Result<Vec<LspSymbol>, CommandError> {
        let result = self
            .request(
                "workspace/symbol",
                json!({
                    "query": query
                }),
            )
            .await?;

        // SymbolInformation 包含: name, kind, location, containerName?
        Ok(parse_symbol_informations(&result))
    }

    /// 请求 textDocument/implementation(跳转到实现)
    /// 复用 parse_locations 解析响应为 LspLocation 列表
    pub async fn goto_implementation(
        &self,
        file_path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<LspLocation>, CommandError> {
        let uri = path_to_uri(file_path);
        let result = self
            .request(
                "textDocument/implementation",
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character }
                }),
            )
            .await?;

        Ok(parse_locations(&result))
    }

    /// 请求 textDocument/prepareCallHierarchy(准备调用层级)
    /// 返回 CallHierarchyItem 列表,供后续 incoming_calls/outgoing_calls 使用
    pub async fn prepare_call_hierarchy(
        &self,
        file_path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<CallHierarchyItem>, CommandError> {
        let uri = path_to_uri(file_path);
        let result = self
            .request(
                "textDocument/prepareCallHierarchy",
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character }
                }),
            )
            .await?;

        // CallHierarchyItem 包含: name, kind, uri, range, selectionRange, detail?, tags?
        Ok(parse_call_hierarchy_items(&result))
    }

    /// 请求 callHierarchy/incomingCalls(查找谁调用了目标符号)
    /// items 为 prepareCallHierarchy 返回的 CallHierarchyItem 列表
    pub async fn incoming_calls(
        &self,
        items: &[CallHierarchyItem],
    ) -> Result<Vec<CallHierarchyCall>, CommandError> {
        let items_json: Vec<Value> = items
            .iter()
            .map(|item| {
                json!({
                    "name": item.name,
                    "kind": item.kind,
                    "uri": item.uri,
                    "range": range_to_json(&item.range),
                    "selectionRange": range_to_json(&item.selection_range),
                })
            })
            .collect();

        let result = self
            .request(
                "callHierarchy/incomingCalls",
                json!({
                    "items": items_json
                }),
            )
            .await?;

        // CallHierarchyIncomingCall 包含: from (CallHierarchyItem), fromRanges (Range[])
        Ok(parse_call_hierarchy_calls(&result, "incoming"))
    }

    /// 请求 callHierarchy/outgoingCalls(查找目标符号调用了谁)
    /// items 为 prepareCallHierarchy 返回的 CallHierarchyItem 列表
    pub async fn outgoing_calls(
        &self,
        items: &[CallHierarchyItem],
    ) -> Result<Vec<CallHierarchyCall>, CommandError> {
        let items_json: Vec<Value> = items
            .iter()
            .map(|item| {
                json!({
                    "name": item.name,
                    "kind": item.kind,
                    "uri": item.uri,
                    "range": range_to_json(&item.range),
                    "selectionRange": range_to_json(&item.selection_range),
                })
            })
            .collect();

        let result = self
            .request(
                "callHierarchy/outgoingCalls",
                json!({
                    "items": items_json
                }),
            )
            .await?;

        // CallHierarchyOutgoingCall 包含: to (CallHierarchyItem), fromRanges (Range[])
        Ok(parse_call_hierarchy_calls(&result, "outgoing"))
    }

    /// 停止 LSP 服务器
    pub async fn shutdown(&self) -> Result<(), CommandError> {
        // 检查进程是否已退出(stdin 为 None 表示进程已死)
        let stdin_alive = self.stdin.lock().await.is_some();
        if stdin_alive {
            // 进程仍存活,发送 shutdown 请求与 exit 通知
            let _ = self.request("shutdown", Value::Null).await;
            let _ = self.notify("exit", Value::Null).await;
        } else {
            log::debug!("LSP {} 进程已退出,跳过 shutdown 请求", self.language);
        }

        // 终止进程
        let mut process = self.process.lock().await;
        if let Some(mut child) = process.take() {
            let _ = child.kill().await;
        }
        *self.stdin.lock().await = None;
        *self.server_info.lock().await = None;

        log::info!("LSP 服务器已停止: language={}", self.language);
        Ok(())
    }

    /// 获取服务器状态
    pub async fn get_status(&self) -> LspServerStatus {
        let info = self.server_info.lock().await;
        info.as_ref()
            .map(|i| i.status.clone())
            .unwrap_or(LspServerStatus::Stopped)
    }

    /// 获取完整的 LSP 服务器信息(供 LspServerManager.get_all_status 使用)
    pub async fn get_server_info(&self) -> Option<LspServerInfo> {
        let info = self.server_info.lock().await;
        info.as_ref().map(|i| {
            let mut cloned = i.clone();
            // 更新最后活动时间
            cloned.last_activity_at = current_timestamp_ms();
            cloned
        })
    }

    /// 获取语言名称
    pub fn language(&self) -> &str {
        &self.language
    }
}

/// 文件路径转 URI
fn path_to_uri(path: &Path) -> String {
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    format!("file:///{}", abs_path.to_string_lossy().replace('\\', "/"))
}

/// 解析 LSP 位置响应
fn parse_locations(result: &Value) -> Vec<LspLocation> {
    match result {
        Value::Array(arr) => arr.iter().filter_map(parse_single_location).collect(),
        Value::Object(_) => parse_single_location(result).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// 解析单个 LSP 位置
fn parse_single_location(value: &Value) -> Option<LspLocation> {
    let uri = value.get("uri")?.as_str()?.to_string();
    let range = value.get("range")?;
    let start = range.get("start")?;
    let end = range.get("end")?;

    let file_path = uri.strip_prefix("file:///").unwrap_or(&uri).to_string();

    Some(LspLocation {
        uri,
        file_path,
        start_line: start.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32,
        start_character: start.get("character").and_then(|c| c.as_u64()).unwrap_or(0) as u32,
        end_line: end.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32,
        end_character: end.get("character").and_then(|c| c.as_u64()).unwrap_or(0) as u32,
    })
}

/// 获取当前时间戳(毫秒)
fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ============ 符号与调用层级解析辅助函数 ============

/// 从 LSP Range JSON 解析位置(不含 uri/file_path,需从外部传入)
fn parse_range_only(range: &Value, uri: &str, file_path: &str) -> Option<LspLocation> {
    let start = range.get("start")?;
    let end = range.get("end")?;
    Some(LspLocation {
        uri: uri.to_string(),
        file_path: file_path.to_string(),
        start_line: start.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32,
        start_character: start.get("character").and_then(|c| c.as_u64()).unwrap_or(0) as u32,
        end_line: end.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32,
        end_character: end.get("character").and_then(|c| c.as_u64()).unwrap_or(0) as u32,
    })
}

/// 将 LspLocation 转为 LSP Range JSON(用于构建请求参数)
fn range_to_json(loc: &LspLocation) -> Value {
    json!({
        "start": { "line": loc.start_line, "character": loc.start_character },
        "end": { "line": loc.end_line, "character": loc.end_character }
    })
}

/// 解析 Documentsymbol[] 响应为扁平的 LspSymbol 列表(递归处理 children)
fn parse_document_symbols(result: &Value, file_path: &Path) -> Vec<LspSymbol> {
    let uri = path_to_uri(file_path);
    let file_path_str = file_path.to_string_lossy().to_string();
    let mut symbols = Vec::new();

    if let Some(arr) = result.as_array() {
        for doc_symbol in arr {
            collect_document_symbols(doc_symbol, &uri, &file_path_str, &mut symbols);
        }
    }
    symbols
}

/// 递归收集 DocumentSymbol(含 children 展平)
fn collect_document_symbols(
    doc_symbol: &Value,
    uri: &str,
    file_path: &str,
    out: &mut Vec<LspSymbol>,
) {
    let name = match doc_symbol.get("name").and_then(|n| n.as_str()) {
        Some(n) => n.to_string(),
        None => return,
    };
    let kind = doc_symbol.get("kind").and_then(|k| k.as_u64()).unwrap_or(0) as u8;
    let location = doc_symbol
        .get("range")
        .and_then(|r| parse_range_only(r, uri, file_path))
        .unwrap_or(LspLocation {
            uri: uri.to_string(),
            file_path: file_path.to_string(),
            start_line: 0,
            start_character: 0,
            end_line: 0,
            end_character: 0,
        });
    let detail = doc_symbol
        .get("detail")
        .and_then(|d| d.as_str())
        .map(|s| s.to_string());

    out.push(LspSymbol {
        name,
        kind,
        location,
        detail,
        documentation: None, // DocumentSymbol 通常不含 documentation 字段
    });

    // 递归处理 children
    if let Some(children) = doc_symbol.get("children").and_then(|c| c.as_array()) {
        for child in children {
            collect_document_symbols(child, uri, file_path, out);
        }
    }
}

/// 解析 SymbolInformation[] 响应为 LspSymbol 列表
fn parse_symbol_informations(result: &Value) -> Vec<LspSymbol> {
    let arr = match result.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };

    arr.iter()
        .filter_map(|sym_info| {
            let name = sym_info.get("name")?.as_str()?.to_string();
            let kind = sym_info.get("kind").and_then(|k| k.as_u64()).unwrap_or(0) as u8;
            let location = parse_single_location(sym_info.get("location")?)?;
            // containerName 作为 detail 展示
            let detail = sym_info
                .get("containerName")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string());

            Some(LspSymbol {
                name,
                kind,
                location,
                detail,
                documentation: None,
            })
        })
        .collect()
}

/// 解析 CallHierarchyItem[] 响应为 CallHierarchyItem 列表
fn parse_call_hierarchy_items(result: &Value) -> Vec<CallHierarchyItem> {
    let arr = match result.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };

    arr.iter()
        .filter_map(parse_single_call_hierarchy_item)
        .collect()
}

/// 解析单个 CallHierarchyItem(辅助函数,供 items 和 calls 复用)
fn parse_single_call_hierarchy_item(item: &Value) -> Option<CallHierarchyItem> {
    let name = item.get("name")?.as_str()?.to_string();
    let kind = item.get("kind").and_then(|k| k.as_u64()).unwrap_or(0) as u8;
    let uri = item.get("uri")?.as_str()?.to_string();
    let file_path = uri.strip_prefix("file:///").unwrap_or(&uri).to_string();

    let range = parse_range_only(item.get("range")?, &uri, &file_path)?;
    let selection_range = parse_range_only(item.get("selectionRange")?, &uri, &file_path)?;

    let detail = item
        .get("detail")
        .and_then(|d| d.as_str())
        .map(|s| s.to_string());
    let tags = item.get("tags").and_then(|t| t.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_u64().map(|n| n as u8))
            .collect()
    });

    Some(CallHierarchyItem {
        name,
        kind,
        uri,
        file_path,
        range,
        selection_range,
        detail,
        tags,
    })
}

/// 解析 callHierarchy/incomingCalls 或 outgoingCalls 响应为 CallHierarchyCall 列表
/// direction: "incoming" 或 "outgoing"
fn parse_call_hierarchy_calls(result: &Value, direction: &str) -> Vec<CallHierarchyCall> {
    let arr = match result.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };

    // incoming: { from: CallHierarchyItem, fromRanges: Range[] }
    // outgoing: { to: CallHierarchyItem, fromRanges: Range[] }
    let item_key = if direction == "incoming" {
        "from"
    } else {
        "to"
    };

    arr.iter()
        .filter_map(|call| {
            let item = parse_single_call_hierarchy_item(call.get(item_key)?)?;

            let from_ranges: Vec<LspLocation> = call
                .get("fromRanges")
                .and_then(|r| r.as_array())
                .map(|ranges| {
                    ranges
                        .iter()
                        .filter_map(|range| parse_range_only(range, &item.uri, &item.file_path))
                        .collect()
                })
                .unwrap_or_default();

            let (from, to) = if direction == "incoming" {
                (Some(item), None)
            } else {
                (None, Some(item))
            };

            Some(CallHierarchyCall {
                direction: direction.to_string(),
                from,
                to,
                from_ranges,
            })
        })
        .collect()
}
