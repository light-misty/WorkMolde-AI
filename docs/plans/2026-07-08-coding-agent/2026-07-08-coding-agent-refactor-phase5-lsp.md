# 阶段 5:LSP 集成 详细改造文档

> **文档版本**:v1.1(2026-07-08 修订:补充 Document 模式说明,LSP 工具在所有模式下可用)
> **创建日期**:2026-07-08
> **阶段目标**:实现 LSP(Language Server Protocol)客户端,支持与外部语言服务器通信,提供代码跳转、引用查找、诊断信息、悬停信息等能力,让 Agent 获得现代 IDE 级别的代码理解能力
> **依赖阶段**:[阶段 1:核心架构与工具链](./2026-07-08-coding-agent-refactor-phase1-core.md)、[阶段 2:权限系统与 Agent 模式](./2026-07-08-coding-agent-refactor-phase2-permission.md)、[阶段 3:Skill 系统与上下文管理](./2026-07-08-coding-agent-refactor-phase3-skill-context.md)
> **预计任务数**:20 个(T5.01-T5.20)
> **v1.1 修订**:LSP 工具为只读代码理解工具,在 Plan/Build/Document 三种模式下均可用,无需按模式过滤

---

## 一、阶段概述

### 1.1 改造背景

OpenCode 是第一个将 LSP(Language Server Protocol)视为"一等公民"的 AI Agent。通过 LSP 集成,Agent 能够在编辑代码前获取精确的符号定义、引用位置、类型信息,大幅提升代码理解和修改的准确性:

1. **LSP 协议**:微软 2016 年提出的开放协议,通过 JSON-RPC 2.0 标准化编辑器与语言服务器的通信。支持定义跳转、引用查找、悬停信息、诊断、补全等功能。

2. **OpenCode 的 LSP 增强**:不仅转发 LSP 请求,还在此基础上做了两层增强:
   - **上下文注入**:在发送请求前,主动提取调用栈、参数类型等元数据,让语言服务器返回更精准的结果
   - **多语言协同**:内置 LSP 路由层,根据文件路径自动选择对应语言服务器,支持跨语言跳转

3. **LSP 工具链**:OpenCode 暴露以下 LSP 工具给 Agent:
   - `lsp_definition`:跳转到符号定义
   - `lsp_references`:查找符号引用
   - `lsp_diagnostics`:获取文件诊断信息
   - `lsp_hover`:获取悬停信息(类型、文档)

### 1.2 DocAgent 现状

- **无 LSP 集成**:Agent 仅能通过 `read`/`source_code` 工具进行文本级或语法树级的代码理解
- **无符号定位**:无法精确定位函数/变量的定义位置
- **无诊断能力**:无法获取编译错误、类型错误等诊断信息
- **无引用查找**:无法查找某个函数被哪些地方调用

### 1.3 改造目标

1. **实现 LSP 客户端**:支持与外部 LSP 服务器通过 JSON-RPC 2.0 通信
2. **支持主流语言**:Rust(rust-analyzer)、Python(pylsp)、TypeScript(typescript-language-server)、Go(gopls)
3. **实现 LSP 工具链**:goto_definition、find_references、diagnostics、hover
4. **LSP 服务器管理**:自动启动/停止,健康检查,结果缓存
5. **权限系统集成**:LSP 工具受权限系统控制(默认 allow)

### 1.4 设计原则

- **按需启动**:LSP 服务器仅在首次需要时启动,避免资源浪费
- **语言路由**:根据文件扩展名自动选择对应的 LSP 服务器
- **结果缓存**:高频请求(definition、hover)结果缓存,避免重复计算
- **优雅降级**:LSP 服务器不可用时,降级为 SourceCode 工具
- **进程隔离**:LSP 服务器作为子进程运行,崩溃不影响主应用

---

## 二、任务依赖图

```
T5.01 (LSP 依赖) ── T5.02 (LSP 类型) ── T5.03 (LSP 客户端)
                                            │
                                            ├── T5.04 (LSP 服务器管理器)
                                            │       │
                                            │       ├── T5.05 (语言路由)
                                            │       │
                                            │       └── T5.06 (结果缓存)
                                            │
                                            ├── T5.07 (goto_definition 工具)
                                            │
                                            ├── T5.08 (find_references 工具)
                                            │
                                            ├── T5.09 (diagnostics 工具)
                                            │
                                            └── T5.10 (hover 工具)

T5.11 (LSP 配置) ── T5.12 (LSP 权限) ── T5.13 (前端 LSP 状态)

T5.14 (健康检查) ── T5.15 (优雅降级) ── T5.16 (多语言协同)

T5.17 (集成测试) ── T5.18 (文档更新) ── T5.19 (性能优化) ── T5.20 (验收测试)
```

---

## 三、任务清单

### T5.01:新增 LSP 所需依赖

**文件**:
- 修改:`src-tauri/Cargo.toml`

**实施内容**:
```toml
[dependencies]
# 现有依赖...

# LSP 集成:JSON-RPC 2.0 协议
lsp-types = "0.95"      # LSP 类型定义
jsonrpc-core = "18.0"    # JSON-RPC 核心实现
tokio-util = "0.7"       # 异步工具(用于帧编码)
```

**验证**:
- `cargo build -p docagent_lib` 成功

---

### T5.02:定义 LSP 类型与配置

**文件**:
- 创建:`src-tauri/src/models/lsp.rs`
- 修改:`src-tauri/src/models/mod.rs`(添加 `pub mod lsp;`)

**实施内容**:
```rust
//! LSP 模型定义
//! 定义 LSP 服务器配置、客户端状态等类型

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// LSP 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspServerConfig {
    /// 语言名称(如 "rust", "python", "typescript")
    pub language: String,
    /// 启动命令(如 ["rust-analyzer"], ["pylsp"], ["typescript-language-server", "--stdio"])
    pub command: Vec<String>,
    /// 根目录标识文件(如 ["Cargo.toml"], ["pyproject.toml"], ["tsconfig.json"])
    pub root_patterns: Vec<String>,
    /// 初始化选项(可选,传递给 LSP 服务器的 initializationOptions)
    #[serde(default)]
    pub initialization_options: Option<serde_json::Value>,
}

/// LSP 服务器状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LspServerStatus {
    /// 未启动
    Stopped,
    /// 启动中
    Starting,
    /// 已就绪(初始化完成)
    Ready,
    /// 错误状态
    Error,
    /// 已停止(手动或崩溃)
    Terminated,
}

/// LSP 服务器运行时信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspServerInfo {
    /// 语言名称
    pub language: String,
    /// 服务器名称(从 initialize 响应获取)
    pub server_name: Option<String>,
    /// 服务器版本
    pub server_version: Option<String>,
    /// 工作区根目录
    pub workspace_root: PathBuf,
    /// 当前状态
    pub status: LspServerStatus,
    /// 支持的能力(从 initialize 响应获取)
    pub capabilities: Option<serde_json::Value>,
    /// 启动时间(UNIX 时间戳,毫秒)
    pub started_at: u64,
    /// 最后活动时间(UNIX 时间戳,毫秒)
    pub last_activity_at: u64,
    /// 错误信息(状态为 Error 时)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// LSP 位置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspLocation {
    /// 文件 URI(如 "file:///path/to/file.rs")
    pub uri: String,
    /// 文件路径(从 URI 解析)
    pub file_path: String,
    /// 起始位置
    pub start_line: u32,
    pub start_character: u32,
    /// 结束位置
    pub end_line: u32,
    pub end_character: u32,
}

/// LSP 诊断信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspDiagnostic {
    /// 诊断来源(如 "rustc", "pylint")
    pub source: Option<String>,
    /// 严重级别: 1=Error, 2=Warning, 3=Information, 4=Hint
    pub severity: u8,
    /// 诊断消息
    pub message: String,
    /// 位置
    pub location: LspLocation,
    /// 诊断代码(可选)
    pub code: Option<String>,
}

/// LSP 悬停信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspHover {
    /// 悬停内容(Markdown 格式)
    pub content: String,
    /// 内容范围(可选)
    pub range: Option<LspLocation>,
}

/// LSP 符号信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspSymbol {
    /// 符号名称
    pub name: String,
    /// 符号类型(1=File, 2=Module, 3=Namespace, 4=Package, 5=Class, 6=Method, 7=Property, 8=Field, 9=Constructor, 10=Enum, 11=Interface, 12=Function, 13=Variable, 14=Constant)
    pub kind: u8,
    /// 符号位置
    pub location: LspLocation,
    /// 详细信息(可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// 文档(可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
}

/// 严重级别名称
pub fn severity_name(severity: u8) -> &'static str {
    match severity {
        1 => "Error",
        2 => "Warning",
        3 => "Information",
        4 => "Hint",
        _ => "Unknown",
    }
}

/// 符号类型名称
pub fn symbol_kind_name(kind: u8) -> &'static str {
    match kind {
        1 => "File",
        2 => "Module",
        3 => "Namespace",
        4 => "Package",
        5 => "Class",
        6 => "Method",
        7 => "Property",
        8 => "Field",
        9 => "Constructor",
        10 => "Enum",
        11 => "Interface",
        12 => "Function",
        13 => "Variable",
        14 => "Constant",
        _ => "Unknown",
    }
}
```

**验证**:
- `cargo build -p docagent_lib` 成功

---

### T5.03:实现 LSP 客户端

**文件**:
- 创建:`src-tauri/src/services/lsp/client.rs`
- 创建:`src-tauri/src/services/lsp/mod.rs`

**实施内容**:

**mod.rs**:
```rust
//! LSP 服务模块入口
pub mod client;
pub mod manager;
pub mod router;
pub mod cache;
pub mod tools;

pub use client::LspClient;
pub use manager::LspServerManager;
pub use router::LanguageRouter;
pub use cache::LspResultCache;
```

**client.rs**:
```rust
//! LSP 客户端:与外部 LSP 服务器通过 JSON-RPC 2.0 通信
//! 基于 stdio 传输(LSP 最常见的传输方式)

use crate::models::lsp::*;
use crate::errors::CommandError;
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
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
    pending_requests: TokioMutex<std::collections::HashMap<u64, oneshot::Sender<Value>>>,
    /// 服务器信息
    server_info: TokioMutex<Option<LspServerInfo>>,
    /// 工作区根目录
    workspace_root: std::path::PathBuf,
}

impl LspClient {
    /// 创建 LSP 客户端(不立即启动)
    pub fn new(language: String, workspace_root: std::path::PathBuf) -> Self {
        Self {
            language,
            process: TokioMutex::new(None),
            stdin: TokioMutex::new(None),
            request_id: AtomicU64::new(1),
            pending_requests: TokioMutex::new(std::collections::HashMap::new()),
            server_info: TokioMutex::new(None),
            workspace_root,
        }
    }

    /// 启动 LSP 服务器并发送 initialize 请求
    pub async fn start(&self, command: &[String]) -> Result<LspServerInfo, CommandError> {
        if command.is_empty() {
            return Err(CommandError::config(
                crate::errors::CONFIG_FIELD_MISSING,
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
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take()
            .ok_or_else(|| CommandError::runtime(7000, "无法获取 LSP stdin"))?;
        let stdout = child.stdout.take()
            .ok_or_else(|| CommandError::runtime(7000, "无法获取 LSP stdout"))?;

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

        let response = self.request("initialize", init_params).await?;

        // 解析服务器信息
        let server_name = response.get("serverInfo")
            .and_then(|s| s.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string());

        let server_version = response.get("serverInfo")
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

        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut content_length: Option<usize> = None;
            let mut buffer = String::new();

            loop {
                let mut line = String::new();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        log::info!("LSP {} stdout 已关闭", language);
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

        // 等待响应(超时 30 秒)
        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => Err(CommandError::runtime(
                7000,
                format!("LSP 请求 {} 的响应通道已关闭", method),
            )),
            Err(_) => {
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&id);
                Err(CommandError::runtime(
                    7000,
                    format!("LSP 请求 {} 超时(30秒)", method),
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
        let content = format!(
            "Content-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        let mut stdin = self.stdin.lock().await;
        if let Some(stdin) = stdin.as_mut() {
            stdin.write_all(content.as_bytes()).await?;
            stdin.flush().await?;
            Ok(())
        } else {
            Err(CommandError::runtime(7000, "LSP stdin 不可用"))
        }
    }

    /// 发送 textDocument/didOpen 通知
    pub async fn did_open(&self, file_path: &Path, language_id: &str, content: &str) -> Result<(), CommandError> {
        let uri = path_to_uri(file_path);
        self.notify("textDocument/didOpen", json!({
            "textDocument": {
                "uri": uri,
                "languageId": language_id,
                "version": 1,
                "text": content,
            }
        })).await
    }

    /// 发送 textDocument/didChange 通知
    pub async fn did_change(&self, file_path: &Path, content: &str, version: u32) -> Result<(), CommandError> {
        let uri = path_to_uri(file_path);
        self.notify("textDocument/didChange", json!({
            "textDocument": { "uri": uri, "version": version },
            "contentChanges": [{ "text": content }]
        })).await
    }

    /// 发送 textDocument/didClose 通知
    pub async fn did_close(&self, file_path: &Path) -> Result<(), CommandError> {
        let uri = path_to_uri(file_path);
        self.notify("textDocument/didClose", json!({
            "textDocument": { "uri": uri }
        })).await
    }

    /// 请求 textDocument/definition
    pub async fn goto_definition(
        &self,
        file_path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Vec<LspLocation>, CommandError> {
        let uri = path_to_uri(file_path);
        let result = self.request("textDocument/definition", json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        })).await?;

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
        let result = self.request("textDocument/references", json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character },
            "context": { "includeDeclaration": include_declaration }
        })).await?;

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
        let result = self.request("textDocument/hover", json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        })).await?;

        if result.is_null() {
            return Ok(None);
        }

        let content = result.get("contents")
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
        let result = self.request("textDocument/diagnostic", json!({
            "textDocument": { "uri": uri }
        })).await?;

        let items = result.get("items")
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default();

        Ok(items.iter().filter_map(|d| {
            let severity = d.get("severity").and_then(|s| s.as_u64()).unwrap_or(3) as u8;
            let message = d.get("message").and_then(|m| m.as_str()).unwrap_or("").to_string();
            let source = d.get("source").and_then(|s| s.as_str()).map(|s| s.to_string());
            let code = d.get("code").and_then(|c| c.as_str()).map(|s| s.to_string());

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
                    start_character: start.get("character").and_then(|c| c.as_u64()).unwrap_or(0) as u32,
                    end_line: end.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32,
                    end_character: end.get("character").and_then(|c| c.as_u64()).unwrap_or(0) as u32,
                },
            })
        }).collect())
    }

    /// 停止 LSP 服务器
    pub async fn shutdown(&self) -> Result<(), CommandError> {
        // 发送 shutdown 请求
        let _ = self.request("shutdown", Value::Null).await;
        // 发送 exit 通知
        let _ = self.notify("exit", Value::Null).await;

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

    let file_path = uri.strip_prefix("file:///")
        .unwrap_or(&uri)
        .to_string();

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
```

**验证**:
- `cargo build -p docagent_lib` 成功
- 单元测试:创建 LspClient,验证消息序列化

---

### T5.04:实现 LSP 服务器管理器

**文件**:
- 创建:`src-tauri/src/services/lsp/manager.rs`

**实施内容**:
```rust
//! LSP 服务器管理器:管理多个语言的 LSP 服务器
//! 按需启动、自动停止、健康检查

use crate::models::lsp::*;
use crate::services::lsp::client::LspClient;
use crate::errors::CommandError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// LSP 服务器管理器
pub struct LspServerManager {
    /// 已启动的 LSP 客户端(按语言名称索引)
    clients: RwLock<HashMap<String, Arc<LspClient>>>,
    /// LSP 服务器配置(按语言名称索引)
    configs: RwLock<HashMap<String, LspServerConfig>>,
    /// 工作区根目录
    workspace_root: RwLock<PathBuf>,
}

impl LspServerManager {
    /// 创建 LSP 服务器管理器
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            configs: RwLock::new(HashMap::new()),
            workspace_root: RwLock::new(workspace_root),
        }
    }

    /// 注册 LSP 服务器配置
    pub async fn register_config(&self, config: LspServerConfig) {
        let mut configs = self.configs.write().await;
        configs.insert(config.language.clone(), config);
    }

    /// 获取或启动指定语言的 LSP 服务器
    pub async fn get_or_start(&self, language: &str) -> Result<Arc<LspClient>, CommandError> {
        // 检查是否已启动
        {
            let clients = self.clients.read().await;
            if let Some(client) = clients.get(language) {
                if client.get_status().await == LspServerStatus::Ready {
                    return Ok(client.clone());
                }
            }
        }

        // 启动新的 LSP 服务器
        let config = {
            let configs = self.configs.read().await;
            configs.get(language).cloned().ok_or_else(|| {
                CommandError::config(
                    crate::errors::CONFIG_FIELD_MISSING,
                    format!("语言 {} 未配置 LSP 服务器", language),
                )
            })?
        };

        let workspace_root = self.workspace_root.read().await.clone();
        let client = Arc::new(LspClient::new(language.to_string(), workspace_root));

        // 启动服务器
        client.start(&config.command).await?;

        // 存储客户端
        {
            let mut clients = self.clients.write().await;
            clients.insert(language.to_string(), client.clone());
        }

        Ok(client)
    }

    /// 停止指定语言的 LSP 服务器
    pub async fn stop(&self, language: &str) -> Result<(), CommandError> {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.remove(language) {
            client.shutdown().await?;
        }
        Ok(())
    }

    /// 停止所有 LSP 服务器
    pub async fn stop_all(&self) -> Result<(), CommandError> {
        let mut clients = self.clients.write().await;
        for (_, client) in clients.drain() {
            let _ = client.shutdown().await;
        }
        Ok(())
    }

    /// 获取所有 LSP 服务器状态
    pub async fn get_all_status(&self) -> Vec<LspServerInfo> {
        let clients = self.clients.read().await;
        let mut statuses = Vec::new();
        for (_, client) in clients.iter() {
            // 简化:返回基本信息
            let status = client.get_status().await;
            statuses.push(LspServerInfo {
                language: String::new(), // 实际应从 client 获取
                server_name: None,
                server_version: None,
                workspace_root: PathBuf::new(),
                status,
                capabilities: None,
                started_at: 0,
                last_activity_at: 0,
                error: None,
            });
        }
        statuses
    }

    /// 更新工作区根目录(切换工作区时调用)
    pub async fn update_workspace_root(&self, new_root: PathBuf) -> Result<(), CommandError> {
        // 停止所有现有服务器
        self.stop_all().await?;

        // 更新工作区根目录
        let mut root = self.workspace_root.write().await;
        *root = new_root;

        Ok(())
    }
}
```

**验证**:
- `cargo build -p docagent_lib` 成功
- 单元测试:注册配置、启动/停止服务器

---

### T5.05:实现语言路由

**文件**:
- 创建:`src-tauri/src/services/lsp/router.rs`

**实施内容**:
```rust
//! 语言路由:根据文件扩展名自动选择对应的 LSP 服务器

use std::path::Path;

/// 语言路由器
pub struct LanguageRouter {
    /// 扩展名到语言名称的映射
    extension_map: std::collections::HashMap<String, String>,
}

impl Default for LanguageRouter {
    fn default() -> Self {
        let mut map = std::collections::HashMap::new();
        
        // Rust
        map.insert("rs".to_string(), "rust".to_string());
        
        // Python
        map.insert("py".to_string(), "python".to_string());
        
        // TypeScript / JavaScript
        map.insert("ts".to_string(), "typescript".to_string());
        map.insert("tsx".to_string(), "typescript".to_string());
        map.insert("js".to_string(), "javascript".to_string());
        map.insert("jsx".to_string(), "javascript".to_string());
        
        // Go
        map.insert("go".to_string(), "go".to_string());
        
        // Java
        map.insert("java".to_string(), "java".to_string());
        
        // C / C++
        map.insert("c".to_string(), "c".to_string());
        map.insert("h".to_string(), "c".to_string());
        map.insert("cpp".to_string(), "cpp".to_string());
        map.insert("cxx".to_string(), "cpp".to_string());
        map.insert("cc".to_string(), "cpp".to_string());
        map.insert("hpp".to_string(), "cpp".to_string());

        Self { extension_map: map }
    }
}

impl LanguageRouter {
    pub fn new() -> Self {
        Self::default()
    }

    /// 根据文件路径推断语言
    pub fn detect_language(&self, file_path: &Path) -> Option<String> {
        let ext = file_path.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())?;

        self.extension_map.get(&ext).cloned()
    }

    /// 获取语言的 language_id(用于 LSP didOpen)
    pub fn get_language_id(&self, language: &str) -> &str {
        match language {
            "rust" => "rust",
            "python" => "python",
            "typescript" => "typescript",
            "javascript" => "javascript",
            "go" => "go",
            "java" => "java",
            "c" => "c",
            "cpp" => "cpp",
            _ => "plaintext",
        }
    }

    /// 添加自定义扩展名映射
    pub fn add_extension(&mut self, ext: &str, language: &str) {
        self.extension_map.insert(ext.to_lowercase(), language.to_string());
    }
}
```

**验证**:
- `cargo build -p docagent_lib` 成功
- 单元测试:各种文件扩展名正确路由到语言

---

### T5.06:实现结果缓存

**文件**:
- 创建:`src-tauri/src/services/lsp/cache.rs`

**实施内容**:
```rust
//! LSP 结果缓存:缓存高频请求结果,避免重复计算
//! 缓存 definition、hover 等请求结果,设置 TTL

use crate::models::lsp::*;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 缓存键
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    /// 请求方法
    method: String,
    /// 文件路径
    file_path: String,
    /// 行号
    line: u32,
    /// 字符位置
    character: u32,
}

/// 缓存条目
struct CacheEntry<T> {
    /// 缓存值
    value: T,
    /// 过期时间
    expires_at: Instant,
}

/// LSP 结果缓存
pub struct LspResultCache {
    /// definition 缓存
    definition_cache: RwLock<HashMap<CacheKey, CacheEntry<Vec<LspLocation>>>>,
    /// hover 缓存
    hover_cache: RwLock<HashMap<CacheKey, CacheEntry<Option<LspHover>>>>,
    /// references 缓存
    references_cache: RwLock<HashMap<CacheKey, CacheEntry<Vec<LspLocation>>>>,
    /// 缓存 TTL
    ttl: Duration,
    /// 最大缓存条目数
    max_entries: usize,
}

impl Default for LspResultCache {
    fn default() -> Self {
        Self {
            definition_cache: RwLock::new(HashMap::new()),
            hover_cache: RwLock::new(HashMap::new()),
            references_cache: RwLock::new(HashMap::new()),
            ttl: Duration::from_secs(300), // 5 分钟
            max_entries: 500,
        }
    }
}

impl LspResultCache {
    pub fn new(ttl_seconds: u64, max_entries: usize) -> Self {
        Self {
            definition_cache: RwLock::new(HashMap::new()),
            hover_cache: RwLock::new(HashMap::new()),
            references_cache: RwLock::new(HashMap::new()),
            ttl: Duration::from_secs(ttl_seconds),
            max_entries,
        }
    }

    /// 获取 definition 缓存
    pub async fn get_definition(
        &self,
        file_path: &str,
        line: u32,
        character: u32,
    ) -> Option<Vec<LspLocation>> {
        let key = CacheKey {
            method: "definition".to_string(),
            file_path: file_path.to_string(),
            line,
            character,
        };
        let cache = self.definition_cache.read().await;
        cache.get(&key)
            .filter(|e| e.expires_at > Instant::now())
            .map(|e| e.value.clone())
    }

    /// 存储 definition 缓存
    pub async fn set_definition(
        &self,
        file_path: &str,
        line: u32,
        character: u32,
        locations: Vec<LspLocation>,
    ) {
        let key = CacheKey {
            method: "definition".to_string(),
            file_path: file_path.to_string(),
            line,
            character,
        };
        let mut cache = self.definition_cache.write().await;
        
        // 检查缓存大小
        if cache.len() >= self.max_entries {
            self.evict_expired(&mut cache);
        }
        
        cache.insert(key, CacheEntry {
            value: locations,
            expires_at: Instant::now() + self.ttl,
        });
    }

    /// 获取 hover 缓存
    pub async fn get_hover(
        &self,
        file_path: &str,
        line: u32,
        character: u32,
    ) -> Option<Option<LspHover>> {
        let key = CacheKey {
            method: "hover".to_string(),
            file_path: file_path.to_string(),
            line,
            character,
        };
        let cache = self.hover_cache.read().await;
        cache.get(&key)
            .filter(|e| e.expires_at > Instant::now())
            .map(|e| e.value.clone())
    }

    /// 存储 hover 缓存
    pub async fn set_hover(
        &self,
        file_path: &str,
        line: u32,
        character: u32,
        hover: Option<LspHover>,
    ) {
        let key = CacheKey {
            method: "hover".to_string(),
            file_path: file_path.to_string(),
            line,
            character,
        };
        let mut cache = self.hover_cache.write().await;
        
        if cache.len() >= self.max_entries {
            self.evict_expired(&mut cache);
        }
        
        cache.insert(key, CacheEntry {
            value: hover,
            expires_at: Instant::now() + self.ttl,
        });
    }

    /// 清除指定文件的缓存(文件修改时调用)
    pub async fn invalidate_file(&self, file_path: &str) {
        let target = file_path.to_string();
        
        self.definition_cache.write().await.retain(|k, _| k.file_path != target);
        self.hover_cache.write().await.retain(|k, _| k.file_path != target);
        self.references_cache.write().await.retain(|k, _| k.file_path != target);
    }

    /// 清除所有缓存
    pub async fn clear_all(&self) {
        self.definition_cache.write().await.clear();
        self.hover_cache.write().await.clear();
        self.references_cache.write().await.clear();
    }

    /// 驱逐过期条目
    fn evict_expired<T>(&self, cache: &mut HashMap<CacheKey, CacheEntry<T>>) {
        let now = Instant::now();
        cache.retain(|_, e| e.expires_at > now);
    }
}
```

**验证**:
- `cargo build -p docagent_lib` 成功
- 单元测试:缓存命中、过期、失效

---

### T5.07:实现 lsp_definition 工具

**文件**:
- 创建:`src-tauri/src/services/tool/builtin/lsp_tools.rs`
- 修改:`src-tauri/src/services/tool/builtin.rs`(注册 LSP 工具)

**实施内容**:
```rust
//! LSP 工具集:暴露 LSP 能力给 Agent
//! 包含 goto_definition、find_references、diagnostics、hover 四个工具

use async_trait::async_trait;
use crate::services::tool::trait_def::Tool;
use crate::services::lsp::manager::LspServerManager;
use crate::services::lsp::router::LanguageRouter;
use crate::services::lsp::cache::LspResultCache;
use crate::models::tool::ToolResult;
use crate::models::lsp::*;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

/// lsp_definition 工具:跳转到符号定义
pub struct LspGotoDefinitionTool {
    /// LSP 服务器管理器
    manager: Arc<LspServerManager>,
    /// 语言路由器
    router: Arc<LanguageRouter>,
    /// 结果缓存
    cache: Arc<LspResultCache>,
}

impl LspGotoDefinitionTool {
    pub fn new(
        manager: Arc<LspServerManager>,
        router: Arc<LanguageRouter>,
        cache: Arc<LspResultCache>,
    ) -> Self {
        Self { manager, router, cache }
    }
}

#[async_trait]
impl Tool for LspGotoDefinitionTool {
    fn tool_name(&self) -> &str {
        "lsp_definition"
    }

    fn description(&self) -> &str {
        "跳转到符号的定义位置。基于 LSP 协议,精确定位函数、类、变量等符号的定义。比文本搜索更准确,能理解代码语义。支持 Rust、Python、TypeScript、Go、Java 等语言。"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filePath": {
                    "type": "string",
                    "description": "文件路径(相对于工作区根目录)"
                },
                "line": {
                    "type": "integer",
                    "description": "光标所在行号(从 0 开始)"
                },
                "character": {
                    "type": "integer",
                    "description": "光标所在列号(从 0 开始)"
                }
            },
            "required": ["filePath", "line", "character"]
        })
    }

    async fn execute(
        &self,
        params: Value,
        workspace_root: &str,
    ) -> Result<ToolResult, crate::errors::CommandError> {
        let file_path_str = params.get("filePath")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                "缺少 filePath 参数",
            ))?;

        let line = params.get("line")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let character = params.get("character")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        // 解析文件路径
        let file_path = PathBuf::from(file_path_str);
        let abs_path = if file_path.is_absolute() {
            file_path
        } else {
            PathBuf::from(workspace_root).join(&file_path)
        };

        // 检查缓存
        if let Some(cached) = self.cache.get_definition(
            &abs_path.to_string_lossy(),
            line,
            character,
        ).await {
            return Ok(ToolResult {
                success: true,
                result: json!({
                    "locations": cached,
                    "total": cached.len(),
                    "cached": true,
                }),
                error: None,
                metadata: Some(json!({"cached": true})),
            });
        }

        // 检测语言
        let language = self.router.detect_language(&abs_path)
            .ok_or_else(|| crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                format!("无法识别文件语言: {}", file_path_str),
            ))?;

        // 获取或启动 LSP 服务器
        let client = self.manager.get_or_start(&language).await?;

        // 读取文件内容并发送 didOpen
        let content = std::fs::read_to_string(&abs_path)?;
        let language_id = self.router.get_language_id(&language);
        client.did_open(&abs_path, language_id, &content).await?;

        // 请求 definition
        let locations = client.goto_definition(&abs_path, line, character).await?;

        // 缓存结果
        self.cache.set_definition(
            &abs_path.to_string_lossy(),
            line,
            character,
            locations.clone(),
        ).await;

        Ok(ToolResult {
            success: true,
            result: json!({
                "locations": locations,
                "total": locations.len(),
                "language": language,
            }),
            error: None,
            metadata: Some(json!({
                "language": language,
                "filePath": abs_path.to_string_lossy(),
                "position": { "line": line, "character": character },
            })),
        })
    }
}
```

**验证**:
- `cargo build -p docagent_lib` 成功
- 集成测试:对测试代码执行 goto_definition

---

### T5.08:实现 lsp_references 工具

**文件**:
- 修改:`src-tauri/src/services/tool/builtin/lsp_tools.rs`

**实施内容**:
```rust
/// lsp_references 工具:查找符号的所有引用
pub struct LspFindReferencesTool {
    manager: Arc<LspServerManager>,
    router: Arc<LanguageRouter>,
    cache: Arc<LspResultCache>,
}

impl LspFindReferencesTool {
    pub fn new(
        manager: Arc<LspServerManager>,
        router: Arc<LanguageRouter>,
        cache: Arc<LspResultCache>,
    ) -> Self {
        Self { manager, router, cache }
    }
}

#[async_trait]
impl Tool for LspFindReferencesTool {
    fn tool_name(&self) -> &str {
        "lsp_references"
    }

    fn description(&self) -> &str {
        "查找符号的所有引用位置。基于 LSP 协议,返回该符号在整个项目中被使用的位置列表。用于分析代码依赖关系、评估修改影响范围。"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filePath": {
                    "type": "string",
                    "description": "文件路径"
                },
                "line": {
                    "type": "integer",
                    "description": "符号所在行号(从 0 开始)"
                },
                "character": {
                    "type": "integer",
                    "description": "符号所在列号(从 0 开始)"
                },
                "includeDeclaration": {
                    "type": "boolean",
                    "default": true,
                    "description": "是否包含声明位置"
                }
            },
            "required": ["filePath", "line", "character"]
        })
    }

    async fn execute(
        &self,
        params: Value,
        workspace_root: &str,
    ) -> Result<ToolResult, crate::errors::CommandError> {
        let file_path_str = params.get("filePath")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                "缺少 filePath 参数",
            ))?;

        let line = params.get("line")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let character = params.get("character")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let include_declaration = params.get("includeDeclaration")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // 解析文件路径
        let file_path = PathBuf::from(file_path_str);
        let abs_path = if file_path.is_absolute() {
            file_path
        } else {
            PathBuf::from(workspace_root).join(&file_path)
        };

        // 检测语言
        let language = self.router.detect_language(&abs_path)
            .ok_or_else(|| crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                format!("无法识别文件语言: {}", file_path_str),
            ))?;

        // 获取或启动 LSP 服务器
        let client = self.manager.get_or_start(&language).await?;

        // 发送 didOpen
        let content = std::fs::read_to_string(&abs_path)?;
        let language_id = self.router.get_language_id(&language);
        client.did_open(&abs_path, language_id, &content).await?;

        // 请求 references
        let locations = client.find_references(&abs_path, line, character, include_declaration).await?;

        Ok(ToolResult {
            success: true,
            result: json!({
                "references": locations,
                "total": locations.len(),
                "language": language,
            }),
            error: None,
            metadata: Some(json!({
                "language": language,
                "filePath": abs_path.to_string_lossy(),
                "includeDeclaration": include_declaration,
            })),
        })
    }
}
```

**验证**:
- `cargo build -p docagent_lib` 成功

---

### T5.09:实现 lsp_diagnostics 工具

**文件**:
- 修改:`src-tauri/src/services/tool/builtin/lsp_tools.rs`

**实施内容**:
```rust
/// lsp_diagnostics 工具:获取文件诊断信息(错误、警告)
pub struct LspDiagnosticsTool {
    manager: Arc<LspServerManager>,
    router: Arc<LanguageRouter>,
}

impl LspDiagnosticsTool {
    pub fn new(
        manager: Arc<LspServerManager>,
        router: Arc<LanguageRouter>,
    ) -> Self {
        Self { manager, router }
    }
}

#[async_trait]
impl Tool for LspDiagnosticsTool {
    fn tool_name(&self) -> &str {
        "lsp_diagnostics"
    }

    fn description(&self) -> &str {
        "获取文件的诊断信息(编译错误、类型错误、警告等)。基于 LSP 协议,返回语言服务器检测到的问题列表。用于代码质量检查、错误定位。"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filePath": {
                    "type": "string",
                    "description": "文件路径"
                }
            },
            "required": ["filePath"]
        })
    }

    async fn execute(
        &self,
        params: Value,
        workspace_root: &str,
    ) -> Result<ToolResult, crate::errors::CommandError> {
        let file_path_str = params.get("filePath")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                "缺少 filePath 参数",
            ))?;

        // 解析文件路径
        let file_path = PathBuf::from(file_path_str);
        let abs_path = if file_path.is_absolute() {
            file_path
        } else {
            PathBuf::from(workspace_root).join(&file_path)
        };

        // 检测语言
        let language = self.router.detect_language(&abs_path)
            .ok_or_else(|| crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                format!("无法识别文件语言: {}", file_path_str),
            ))?;

        // 获取或启动 LSP 服务器
        let client = self.manager.get_or_start(&language).await?;

        // 发送 didOpen
        let content = std::fs::read_to_string(&abs_path)?;
        let language_id = self.router.get_language_id(&language);
        client.did_open(&abs_path, language_id, &content).await?;

        // 请求 diagnostics
        let diagnostics = client.diagnostics(&abs_path).await?;

        // 统计严重级别
        let error_count = diagnostics.iter().filter(|d| d.severity == 1).count();
        let warning_count = diagnostics.iter().filter(|d| d.severity == 2).count();
        let info_count = diagnostics.iter().filter(|d| d.severity == 3).count();

        Ok(ToolResult {
            success: true,
            result: json!({
                "diagnostics": diagnostics,
                "total": diagnostics.len(),
                "errors": error_count,
                "warnings": warning_count,
                "informations": info_count,
                "language": language,
            }),
            error: None,
            metadata: Some(json!({
                "language": language,
                "filePath": abs_path.to_string_lossy(),
                "totalDiagnostics": diagnostics.len(),
            })),
        })
    }
}
```

**验证**:
- `cargo build -p docagent_lib` 成功

---

### T5.10:实现 lsp_hover 工具

**文件**:
- 修改:`src-tauri/src/services/tool/builtin/lsp_tools.rs`

**实施内容**:
```rust
/// lsp_hover 工具:获取符号的悬停信息(类型、文档)
pub struct LspHoverTool {
    manager: Arc<LspServerManager>,
    router: Arc<LanguageRouter>,
    cache: Arc<LspResultCache>,
}

impl LspHoverTool {
    pub fn new(
        manager: Arc<LspServerManager>,
        router: Arc<LanguageRouter>,
        cache: Arc<LspResultCache>,
    ) -> Self {
        Self { manager, router, cache }
    }
}

#[async_trait]
impl Tool for LspHoverTool {
    fn tool_name(&self) -> &str {
        "lsp_hover"
    }

    fn description(&self) -> &str {
        "获取符号的悬停信息(类型签名、文档说明)。基于 LSP 协议,返回符号的详细类型信息和文档。用于理解符号的用法和含义。"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filePath": {
                    "type": "string",
                    "description": "文件路径"
                },
                "line": {
                    "type": "integer",
                    "description": "符号所在行号(从 0 开始)"
                },
                "character": {
                    "type": "integer",
                    "description": "符号所在列号(从 0 开始)"
                }
            },
            "required": ["filePath", "line", "character"]
        })
    }

    async fn execute(
        &self,
        params: Value,
        workspace_root: &str,
    ) -> Result<ToolResult, crate::errors::CommandError> {
        let file_path_str = params.get("filePath")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                "缺少 filePath 参数",
            ))?;

        let line = params.get("line")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let character = params.get("character")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        // 解析文件路径
        let file_path = PathBuf::from(file_path_str);
        let abs_path = if file_path.is_absolute() {
            file_path
        } else {
            PathBuf::from(workspace_root).join(&file_path)
        };

        // 检查缓存
        if let Some(cached) = self.cache.get_hover(
            &abs_path.to_string_lossy(),
            line,
            character,
        ).await {
            return Ok(ToolResult {
                success: true,
                result: json!({
                    "hover": cached,
                    "cached": true,
                }),
                error: None,
                metadata: Some(json!({"cached": true})),
            });
        }

        // 检测语言
        let language = self.router.detect_language(&abs_path)
            .ok_or_else(|| crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                format!("无法识别文件语言: {}", file_path_str),
            ))?;

        // 获取或启动 LSP 服务器
        let client = self.manager.get_or_start(&language).await?;

        // 发送 didOpen
        let content = std::fs::read_to_string(&abs_path)?;
        let language_id = self.router.get_language_id(&language);
        client.did_open(&abs_path, language_id, &content).await?;

        // 请求 hover
        let hover = client.hover(&abs_path, line, character).await?;

        // 缓存结果
        self.cache.set_hover(
            &abs_path.to_string_lossy(),
            line,
            character,
            hover.clone(),
        ).await;

        Ok(ToolResult {
            success: true,
            result: json!({
                "hover": hover,
                "language": language,
            }),
            error: None,
            metadata: Some(json!({
                "language": language,
                "filePath": abs_path.to_string_lossy(),
                "position": { "line": line, "character": character },
            })),
        })
    }
}
```

**验证**:
- `cargo build -p docagent_lib` 成功

---

### T5.11:定义 LSP 配置

**文件**:
- 修改:`src-tauri/src/config/app_settings.rs`

**实施内容**:
```rust
/// LSP 集成配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspConfig {
    /// 是否启用 LSP 集成
    pub enabled: bool,
    /// LSP 服务器配置列表
    pub servers: Vec<LspServerConfigEntry>,
    /// 缓存配置
    pub cache: LspCacheConfig,
    /// 请求超时时间(秒)
    pub request_timeout_seconds: u64,
    /// 健康检查间隔(秒,0 表示禁用)
    pub health_check_interval_seconds: u64,
}

/// LSP 服务器配置项(用于配置文件)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspServerConfigEntry {
    /// 语言名称
    pub language: String,
    /// 启动命令
    pub command: Vec<String>,
    /// 根目录标识文件
    #[serde(default)]
    pub root_patterns: Vec<String>,
    /// 初始化选项
    #[serde(default)]
    pub initialization_options: Option<serde_json::Value>,
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// LSP 缓存配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspCacheConfig {
    /// 是否启用缓存
    pub enabled: bool,
    /// 缓存 TTL(秒)
    pub ttl_seconds: u64,
    /// 最大缓存条目数
    pub max_entries: usize,
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            servers: vec![
                LspServerConfigEntry {
                    language: "rust".to_string(),
                    command: vec!["rust-analyzer".to_string()],
                    root_patterns: vec!["Cargo.toml".to_string()],
                    initialization_options: None,
                    enabled: true,
                },
                LspServerConfigEntry {
                    language: "python".to_string(),
                    command: vec!["pylsp".to_string()],
                    root_patterns: vec!["pyproject.toml".to_string(), "setup.py".to_string()],
                    initialization_options: None,
                    enabled: true,
                },
                LspServerConfigEntry {
                    language: "typescript".to_string(),
                    command: vec!["typescript-language-server".to_string(), "--stdio".to_string()],
                    root_patterns: vec!["tsconfig.json".to_string(), "package.json".to_string()],
                    initialization_options: None,
                    enabled: true,
                },
                LspServerConfigEntry {
                    language: "go".to_string(),
                    command: vec!["gopls".to_string()],
                    root_patterns: vec!["go.mod".to_string()],
                    initialization_options: None,
                    enabled: true,
                },
            ],
            cache: LspCacheConfig {
                enabled: true,
                ttl_seconds: 300,
                max_entries: 500,
            },
            request_timeout_seconds: 30,
            health_check_interval_seconds: 60,
        }
    }
}

fn default_true() -> bool {
    true
}
```

**验证**:
- `cargo build -p docagent_lib` 成功
- 配置可正确序列化/反序列化

---

### T5.12:LSP 权限集成

**文件**:
- 修改:`src-tauri/src/services/permission/types.rs`(添加 Lsp 权限类型)
- 修改:`src-tauri/src/services/agent/executor.rs`

**实施内容**:

**在 PermissionType 中添加 Lsp 变体**:
```rust
// 在 types.rs 的 PermissionType 枚举中添加
pub enum PermissionType {
    // ... 现有变体
    /// LSP 工具(goto_definition, find_references, diagnostics, hover)
    Lsp,
}
```

**在 AgentExecutor 中添加 LSP 工具权限检查**:
```rust
// 在 execute_tool 方法中
let permission_type = match tool_name {
    "lsp_definition" | "lsp_references" | "lsp_diagnostics" | "lsp_hover" => {
        PermissionType::Lsp
    }
    // ... 其他工具
};

// LSP 工具默认 allow(只读操作,安全)
```

**默认权限规则**:
```json
{
  "lsp": "allow"
}
```

**验证**:
- `cargo build -p docagent_lib` 成功
- 权限检查:LSP 工具默认允许,可配置为 ask

---

### T5.13:前端 LSP 状态展示

**文件**:
- 创建:`src/components/settings/LspStatusPanel.tsx`
- 修改:`src/components/settings/SettingsDialog.tsx`(添加 LSP 标签页)

**实施内容**:
```tsx
import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { LspServerInfo } from '@/types';

export function LspStatusPanel() {
  const [servers, setServers] = useState<LspServerInfo[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadLspStatus();
  }, []);

  const loadLspStatus = async () => {
    try {
      const status = await invoke<LspServerInfo[]>('lsp_get_status');
      setServers(status);
    } catch (e) {
      console.error('加载 LSP 状态失败:', e);
    } finally {
      setLoading(false);
    }
  };

  const handleRestart = async (language: string) => {
    try {
      await invoke('lsp_restart_server', { language });
      loadLspStatus();
    } catch (e) {
      console.error('重启 LSP 服务器失败:', e);
    }
  };

  if (loading) {
    return <div>加载中...</div>;
  }

  return (
    <div className="lsp-status-panel">
      <h3>LSP 服务器状态</h3>
      {servers.length === 0 ? (
        <p>暂无已启动的 LSP 服务器</p>
      ) : (
        <table>
          <thead>
            <tr>
              <th>语言</th>
              <th>服务器</th>
              <th>版本</th>
              <th>状态</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {servers.map((server, idx) => (
              <tr key={idx}>
                <td>{server.language}</td>
                <td>{server.serverName || '-'}</td>
                <td>{server.serverVersion || '-'}</td>
                <td>
                  <span className={`status-badge ${server.status}`}>
                    {server.status}
                  </span>
                </td>
                <td>
                  <button onClick={() => handleRestart(server.language)}>
                    重启
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
```

**验证**:
- `npx tsc -b` 成功
- 设置弹窗中可见 LSP 状态面板

---

### T5.14:LSP 服务器健康检查

**文件**:
- 修改:`src-tauri/src/services/lsp/manager.rs`
- 修改:`src-tauri/src/lib.rs`(启动健康检查任务)

**实施内容**:

**在 LspServerManager 中添加健康检查**:
```rust
impl LspServerManager {
    /// 执行健康检查
    pub async fn health_check(&self) -> Result<(), CommandError> {
        let clients = self.clients.read().await;
        for (language, client) in clients.iter() {
            let status = client.get_status().await;
            if status != LspServerStatus::Ready {
                log::warn!(
                    "LSP 服务器 {} 状态异常: {:?}",
                    language,
                    status
                );
                // 自动重启逻辑
                // ...
            }
        }
        Ok(())
    }
}

// 在 lib.rs 中启动健康检查任务
tokio::spawn(async move {
    let interval = std::time::Duration::from_secs(
        lsp_config.health_check_interval_seconds
    );
    loop {
        tokio::time::sleep(interval).await;
        if let Err(e) = lsp_manager.health_check().await {
            log::warn!("LSP 健康检查失败: {}", e);
        }
    }
});
```

**验证**:
- `cargo build -p docagent_lib` 成功
- LSP 服务器崩溃后自动检测并重启

---

### T5.15:优雅降级机制

**文件**:
- 修改:`src-tauri/src/services/tool/builtin/lsp_tools.rs`

**实施内容**:

当 LSP 服务器不可用时,降级为 SourceCode 工具:

```rust
// 在 LspGotoDefinitionTool 的 execute 方法中
async fn execute(
    &self,
    params: Value,
    workspace_root: &str,
) -> Result<ToolResult, crate::errors::CommandError> {
    // 尝试使用 LSP
    match self.try_lsp_definition(&params, workspace_root).await {
        Ok(result) => Ok(result),
        Err(e) => {
            log::warn!(
                "LSP goto_definition 失败,降级为 SourceCode 搜索: {}",
                e
            );
            
            // 降级:使用 SourceCode 工具搜索符号定义
            self.fallback_to_source_code(&params, workspace_root).await
        }
    }
}

/// 降级为 SourceCode 搜索
async fn fallback_to_source_code(
    &self,
    params: Value,
    workspace_root: &str,
) -> Result<ToolResult, crate::errors::CommandError> {
    // 使用 SourceCodeSearcher 搜索符号
    // ... 降级逻辑
    Ok(ToolResult {
        success: true,
        result: json!({
            "fallback": true,
            "message": "LSP 不可用,使用 SourceCode 搜索",
            "locations": [], // SourceCode 搜索结果
        }),
        error: None,
        metadata: Some(json!({"fallback": true})),
    })
}
```

**验证**:
- `cargo build -p docagent_lib` 成功
- LSP 服务器未安装时,自动降级为 SourceCode

---

### T5.16:多语言协同(跨语言跳转)

**文件**:
- 修改:`src-tauri/src/services/lsp/router.rs`

**实施内容**:

扩展 LanguageRouter 支持跨语言跳转:

```rust
impl LanguageRouter {
    /// 检测文件可能涉及的语言(基于导入/依赖)
    pub fn detect_related_languages(&self, file_path: &Path) -> Vec<String> {
        let mut related = Vec::new();
        
        // 读取文件内容,分析导入语句
        if let Ok(content) = std::fs::read_to_string(file_path) {
            // Rust: use 语句
            if content.contains("use ") {
                related.push("rust".to_string());
            }
            // Python: import 语句
            if content.contains("import ") {
                related.push("python".to_string());
            }
            // TypeScript: import 语句
            if content.contains("import ") && content.contains("from ") {
                related.push("typescript".to_string());
            }
        }
        
        related
    }
}
```

**说明**:跨语言跳转是高级特性,本阶段仅实现基础框架,完整实现可作为后续扩展。

**验证**:
- `cargo build -p docagent_lib` 成功

---

### T5.17:编写集成测试

**文件**:
- 创建:`src-tauri/tests/lsp_integration_test.rs`

**实施内容**:
```rust
//! 阶段 5 集成测试:LSP 集成

use docagent_lib::services::lsp::{LanguageRouter, LspResultCache};
use docagent_lib::models::lsp::*;
use std::path::PathBuf;

/// 测试:语言路由器正确识别文件语言
#[tokio::test]
async fn test_language_router_detects_rust() {
    let router = LanguageRouter::new();
    
    assert_eq!(
        router.detect_language(&PathBuf::from("src/main.rs")),
        Some("rust".to_string())
    );
    assert_eq!(
        router.detect_language(&PathBuf::from("lib/utils.py")),
        Some("python".to_string())
    );
    assert_eq!(
        router.detect_language(&PathBuf::from("src/index.ts")),
        Some("typescript".to_string())
    );
    assert_eq!(
        router.detect_language(&PathBuf::from("main.go")),
        Some("go".to_string())
    );
}

/// 测试:语言路由器对未知扩展名返回 None
#[tokio::test]
async fn test_language_router_returns_none_for_unknown() {
    let router = LanguageRouter::new();
    
    assert_eq!(
        router.detect_language(&PathBuf::from("README.md")),
        None
    );
    assert_eq!(
        router.detect_language(&PathBuf::from("config.json")),
        None
    );
}

/// 测试:语言路由器获取 language_id
#[tokio::test]
async fn test_language_router_gets_language_id() {
    let router = LanguageRouter::new();
    
    assert_eq!(router.get_language_id("rust"), "rust");
    assert_eq!(router.get_language_id("python"), "python");
    assert_eq!(router.get_language_id("typescript"), "typescript");
    assert_eq!(router.get_language_id("unknown"), "plaintext");
}

/// 测试:LSP 缓存命中和过期
#[tokio::test]
async fn test_lsp_cache_hit_and_expiry() {
    let cache = LspResultCache::new(1, 100); // TTL 1 秒
    
    let locations = vec![LspLocation {
        uri: "file:///test.rs".to_string(),
        file_path: "/test.rs".to_string(),
        start_line: 10,
        start_character: 0,
        end_line: 20,
        end_character: 0,
    }];
    
    // 存储缓存
    cache.set_definition("/test.rs", 5, 10, locations.clone()).await;
    
    // 立即读取应命中
    let cached = cache.get_definition("/test.rs", 5, 10).await;
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().len(), 1);
    
    // 等待 2 秒(超过 TTL)
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    
    // 再次读取应未命中(已过期)
    let expired = cache.get_definition("/test.rs", 5, 10).await;
    assert!(expired.is_none());
}

/// 测试:LSP 缓存文件失效
#[tokio::test]
async fn test_lsp_cache_invalidation_by_file() {
    let cache = LspResultCache::new(300, 100);
    
    // 存储多个文件的缓存
    cache.set_definition("/file1.rs", 0, 0, vec![]).await;
    cache.set_definition("/file2.rs", 0, 0, vec![]).await;
    cache.set_hover("/file1.rs", 0, 0, None).await;
    
    // 失效 file1 的缓存
    cache.invalidate_file("/file1.rs").await;
    
    // file1 的缓存应被清除
    assert!(cache.get_definition("/file1.rs", 0, 0).await.is_none());
    assert!(cache.get_hover("/file1.rs", 0, 0).await.is_none());
    
    // file2 的缓存应保留
    assert!(cache.get_definition("/file2.rs", 0, 0).await.is_some());
}

/// 测试:严重级别名称转换
#[tokio::test]
async fn test_severity_name_conversion() {
    assert_eq!(severity_name(1), "Error");
    assert_eq!(severity_name(2), "Warning");
    assert_eq!(severity_name(3), "Information");
    assert_eq!(severity_name(4), "Hint");
    assert_eq!(severity_name(99), "Unknown");
}

/// 测试:符号类型名称转换
#[tokio::test]
async fn test_symbol_kind_name_conversion() {
    assert_eq!(symbol_kind_name(5), "Class");
    assert_eq!(symbol_kind_name(6), "Method");
    assert_eq!(symbol_kind_name(12), "Function");
    assert_eq!(symbol_kind_name(13), "Variable");
}
```

**验证**:
- `cargo test` 全部通过

---

### T5.18:更新文档与工具注册

**文件**:
- 修改:`src-tauri/src/services/tool/builtin.rs`(注册 LSP 工具)
- 修改:`src-tauri/src/lib.rs`(初始化 LSP 管理器)
- 修改:`CLAUDE.md`(更新工具列表)

**实施内容**:

**在 register_builtin_tools 中注册 LSP 工具**:
```rust
pub fn register_builtin_tools(
    registry: &mut ToolRegistry,
    // ... 现有参数
    lsp_manager: Arc<LspServerManager>,
    lsp_router: Arc<LanguageRouter>,
    lsp_cache: Arc<LspResultCache>,
) -> SharedScratchpadStates {
    // ... 现有工具注册
    
    // 阶段 5 新增 LSP 工具
    registry.register(Box::new(LspGotoDefinitionTool::new(
        lsp_manager.clone(),
        lsp_router.clone(),
        lsp_cache.clone(),
    )));
    registry.register(Box::new(LspFindReferencesTool::new(
        lsp_manager.clone(),
        lsp_router.clone(),
        lsp_cache.clone(),
    )));
    registry.register(Box::new(LspDiagnosticsTool::new(
        lsp_manager.clone(),
        lsp_router.clone(),
    )));
    registry.register(Box::new(LspHoverTool::new(
        lsp_manager.clone(),
        lsp_router.clone(),
        lsp_cache.clone(),
    )));
    
    // ...
}
```

**在 lib.rs 中初始化 LSP 管理器**:
```rust
// 在 setup 函数中初始化 LSP
let lsp_config = app_settings.lsp.clone();
let lsp_manager = Arc::new(LspServerManager::new(workspace_root.clone()));
let lsp_router = Arc::new(LanguageRouter::new());
let lsp_cache = Arc::new(LspResultCache::new(
    lsp_config.cache.ttl_seconds,
    lsp_config.cache.max_entries,
));

// 注册 LSP 服务器配置
for server_config in &lsp_config.servers {
    if server_config.enabled {
        lsp_manager.register_config(LspServerConfig {
            language: server_config.language.clone(),
            command: server_config.command.clone(),
            root_patterns: server_config.root_patterns.clone(),
            initialization_options: server_config.initialization_options.clone(),
        }).await;
    }
}

// 启动健康检查任务
if lsp_config.health_check_interval_seconds > 0 {
    let manager = lsp_manager.clone();
    let interval = lsp_config.health_check_interval_seconds;
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
            if let Err(e) = manager.health_check().await {
                log::warn!("LSP 健康检查失败: {}", e);
            }
        }
    });
}
```

**更新 CLAUDE.md**:
```markdown
## 内置工具

- `lsp_definition`: 跳转到符号定义(LSP)
- `lsp_references`: 查找符号引用(LSP)
- `lsp_diagnostics`: 获取文件诊断信息(LSP)
- `lsp_hover`: 获取符号悬停信息(LSP)
```

**验证**:
- `cargo build -p docagent_lib` 成功
- 应用启动后,工具列表包含 4 个 LSP 工具

---

### T5.19:性能优化

**文件**:
- 修改:`src-tauri/src/services/lsp/client.rs`
- 修改:`src-tauri/src/services/lsp/manager.rs`

**实施内容**:

1. **连接复用**:LSP 客户端启动后保持长连接,避免重复初始化
2. **批量请求**:支持一次请求多个位置的定义(部分 LSP 服务器支持)
3. **预热**:应用启动时预启动常用语言的 LSP 服务器
4. **文件监听**:文件修改时自动发送 didChange 并失效缓存

```rust
// 在 LspServerManager 中添加预热方法
impl LspServerManager {
    /// 预热常用语言的 LSP 服务器
    pub async fn warmup(&self, languages: &[&str]) {
        for lang in languages {
            if let Err(e) = self.get_or_start(lang).await {
                log::debug!("预热 LSP {} 失败: {}", lang, e);
            }
        }
    }
}

// 在文件变更事件处理中,失效 LSP 缓存
if let Some(lsp_cache) = &lsp_cache {
    lsp_cache.invalidate_file(&changed_path).await;
}
```

**验证**:
- `cargo build -p docagent_lib` 成功
- 首次 LSP 请求响应时间 < 1 秒(预热后)

---

### T5.20:验收测试

**文件**:
- 创建:`src-tauri/tests/lsp_acceptance_test.rs`

**实施内容**:
```rust
//! 阶段 5 验收测试:端到端 LSP 功能验证
//! 注意:这些测试需要对应的 LSP 服务器已安装

use docagent_lib::services::lsp::*;
use docagent_lib::services::tool::trait_def::Tool;
use docagent_lib::services::tool::builtin::lsp_tools::*;
use serde_json::json;
use std::path::PathBuf;

/// 验收测试:Rust 项目 LSP 集成
/// 前置条件:rust-analyzer 已安装
#[tokio::test]
#[ignore = "需要 rust-analyzer 已安装"]
async fn acceptance_rust_lsp_integration() {
    // 初始化 LSP 管理器
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manager = std::sync::Arc::new(LspServerManager::new(workspace_root.clone()));
    
    manager.register_config(LspServerConfig {
        language: "rust".to_string(),
        command: vec!["rust-analyzer".to_string()],
        root_patterns: vec!["Cargo.toml".to_string()],
        initialization_options: None,
    }).await;
    
    let router = std::sync::Arc::new(LanguageRouter::new());
    let cache = std::sync::Arc::new(LspResultCache::new(300, 500));
    
    // 测试 goto_definition
    let goto_tool = LspGotoDefinitionTool::new(
        manager.clone(),
        router.clone(),
        cache.clone(),
    );
    
    // 对 src/lib.rs 中的某行执行 goto_definition
    let result = goto_tool.execute(
        json!({
            "filePath": "src/lib.rs",
            "line": 0,
            "character": 0
        }),
        workspace_root.to_str().unwrap(),
    ).await;
    
    // 验证结果(不断言具体值,因为代码可能变化)
    assert!(result.is_ok());
}

/// 验收测试:LSP 服务器管理
#[tokio::test]
async fn acceptance_lsp_server_lifecycle() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manager = LspServerManager::new(workspace_root);
    
    // 获取状态(初始应为空)
    let initial_status = manager.get_all_status().await;
    assert!(initial_status.is_empty());
    
    // 注册配置(但不启动)
    manager.register_config(LspServerConfig {
        language: "rust".to_string(),
        command: vec!["rust-analyzer".to_string()],
        root_patterns: vec!["Cargo.toml".to_string()],
        initialization_options: None,
    }).await;
    
    // 停止所有服务器(应无错误)
    let stop_result = manager.stop_all().await;
    assert!(stop_result.is_ok());
}
```

**验证**:
- `cargo test` 通过(非 ignore 测试)
- 手动验收测试:在真实项目中使用 LSP 工具

---

## 四、数据库迁移

本阶段无新增数据库表。LSP 配置存储在 JSON 配置文件中,缓存为内存数据。

---

## 五、配置变更

### 5.1 新增配置项

| 配置项 | 位置 | 默认值 | 说明 |
|--------|------|--------|------|
| `lsp.enabled` | `LspConfig` | `true` | 是否启用 LSP 集成 |
| `lsp.servers` | `LspConfig` | 内置 4 语言 | LSP 服务器配置列表 |
| `lsp.cache.enabled` | `LspCacheConfig` | `true` | 是否启用缓存 |
| `lsp.cache.ttlSeconds` | `LspCacheConfig` | `300` | 缓存 TTL |
| `lsp.cache.maxEntries` | `LspCacheConfig` | `500` | 最大缓存条目数 |
| `lsp.requestTimeoutSeconds` | `LspConfig` | `30` | 请求超时时间 |
| `lsp.healthCheckIntervalSeconds` | `LspConfig` | `60` | 健康检查间隔 |

### 5.2 默认 LSP 服务器

| 语言 | 命令 | 根目录标识 |
|------|------|-----------|
| Rust | `rust-analyzer` | `Cargo.toml` |
| Python | `pylsp` | `pyproject.toml`, `setup.py` |
| TypeScript | `typescript-language-server --stdio` | `tsconfig.json`, `package.json` |
| Go | `gopls` | `go.mod` |

---

## 六、事件清单

### 6.1 新增事件

本阶段无新增系统事件。LSP 状态变更通过现有的 `system:network_change` 类似机制处理。

### 6.2 Tauri 命令

| 命令名 | 说明 |
|--------|------|
| `lsp_get_status` | 获取所有 LSP 服务器状态 |
| `lsp_restart_server` | 重启指定语言的 LSP 服务器 |
| `lsp_stop_all` | 停止所有 LSP 服务器 |

---

## 七、参考资源

### 7.1 OpenCode 相关源码

- **LSP 集成**: `packages/opencode/src/lsp/`
  - `client.ts`:LSP 客户端实现
  - `manager.ts`:LSP 服务器管理
  - `router.ts`:语言路由
- **LSP 工具**: `packages/opencode/src/tool/lsp_*.ts`

### 7.2 LSP 协议规范

- **LSP 官方规范**:https://microsoft.github.io/language-server-protocol/
- **JSON-RPC 2.0**:https://www.jsonrpc.org/specification
- **LSP 类型定义**:https://docs.rs/lsp-types

### 7.3 LSP 服务器

- **rust-analyzer**:https://rust-analyzer.github.io/
- **pylsp**:https://github.com/python-lsp/python-lsp-server
- **typescript-language-server**:https://github.com/typescript-language-server/typescript-language-server
- **gopls**:https://pkg.go.dev/golang.org/x/tools/gopls

### 7.4 相关文档

- [阶段 1:核心架构与工具链](./2026-07-08-coding-agent-refactor-phase1-core.md)
- [阶段 2:权限系统与 Agent 模式](./2026-07-08-coding-agent-refactor-phase2-permission.md)
- [阶段 3:Skill 系统与上下文管理](./2026-07-08-coding-agent-refactor-phase3-skill-context.md)
- [阶段 4:子 Agent 与高级工具](./2026-07-08-coding-agent-refactor-phase4-subagent-tools.md)
- [总体改造计划](./2026-07-08-coding-agent-refactor-overview.md)

---

## 八、任务完成状态追踪

| 任务 ID | 任务名称 | 状态 | 完成时间 | 备注 |
|---------|---------|------|---------|------|
| T5.01 | 新增 LSP 所需依赖 | 待实施 | - | |
| T5.02 | 定义 LSP 类型与配置 | 待实施 | - | |
| T5.03 | 实现 LSP 客户端 | 待实施 | - | |
| T5.04 | 实现 LSP 服务器管理器 | 待实施 | - | |
| T5.05 | 实现语言路由 | 待实施 | - | |
| T5.06 | 实现结果缓存 | 待实施 | - | |
| T5.07 | 实现 lsp_definition 工具 | 待实施 | - | |
| T5.08 | 实现 lsp_references 工具 | 待实施 | - | |
| T5.09 | 实现 lsp_diagnostics 工具 | 待实施 | - | |
| T5.10 | 实现 lsp_hover 工具 | 待实施 | - | |
| T5.11 | 定义 LSP 配置 | 待实施 | - | |
| T5.12 | LSP 权限集成 | 待实施 | - | |
| T5.13 | 前端 LSP 状态展示 | 待实施 | - | |
| T5.14 | LSP 服务器健康检查 | 待实施 | - | |
| T5.15 | 优雅降级机制 | 待实施 | - | |
| T5.16 | 多语言协同 | 待实施 | - | |
| T5.17 | 编写集成测试 | 待实施 | - | |
| T5.18 | 更新文档与工具注册 | 待实施 | - | |
| T5.19 | 性能优化 | 待实施 | - | |
| T5.20 | 验收测试 | 待实施 | - | |

---

## 九、风险与回滚策略

### 9.1 主要风险点

1. **LSP 服务器未安装**:用户环境未安装 rust-analyzer 等服务器
   - 缓解:优雅降级,自动回退到 SourceCode 工具
   - 回滚:禁用 LSP(`lsp.enabled = false`)

2. **LSP 服务器崩溃**:子进程异常退出
   - 缓解:健康检查 + 自动重启
   - 回滚:降级为 SourceCode 工具

3. **性能影响**:LSP 服务器占用较多内存
   - 缓解:按需启动,闲置超时自动停止
   - 回滚:禁用 LSP 或减少支持的语言

4. **跨平台兼容性**:Windows 下 LSP 服务器行为可能不同
   - 缓解:充分测试 Windows 环境;隐藏控制台窗口
   - 回滚:针对特定平台禁用 LSP

5. **缓存一致性问题**:文件修改后缓存未失效
   - 缓解:文件监听器自动失效缓存;设置合理 TTL
   - 回滚:禁用缓存(`lsp.cache.enabled = false`)

### 9.2 验收标准

- 所有 20 个任务(T5.01-T5.20)实施完成
- `cargo test` 全部通过(包括 7 个新增集成测试)
- `cargo clippy` 无警告
- `npx tsc -b` 无类型错误
- 手动测试:在 Rust 项目中调用 `lsp_definition`,正确定位函数定义
- 手动测试:调用 `lsp_references`,返回所有引用位置
- 手动测试:调用 `lsp_diagnostics`,返回编译错误和警告
- 手动测试:调用 `lsp_hover`,返回符号类型和文档
- 手动测试:LSP 服务器不可用时,自动降级为 SourceCode
- 手动测试:设置弹窗中可见 LSP 状态面板,可重启服务器
- v1.1 验证:LSP 工具在 Plan/Build/Document 三种模式下均可用(只读工具,无需按模式过滤)

---

## 十、阶段总结与后续展望

### 10.1 阶段总结

本阶段实现了完整的 LSP 集成,让 DocAgent 获得了现代 IDE 级别的代码理解能力:

1. **LSP 客户端**:基于 JSON-RPC 2.0 协议,支持 stdio 传输
2. **服务器管理**:按需启动、自动停止、健康检查、结果缓存
3. **语言路由**:根据文件扩展名自动选择 LSP 服务器
4. **工具链**:4 个 LSP 工具(goto_definition、find_references、diagnostics、hover)
5. **优雅降级**:LSP 不可用时自动回退到 SourceCode
6. **权限集成**:LSP 工具受权限系统控制

### 10.2 后续扩展方向

1. **更多语言支持**:添加 Ruby、PHP、Swift、Kotlin 等语言的 LSP 服务器配置
2. **补全能力**:实现 `textDocument/completion`,让 Agent 获得代码补全能力
3. **重命名**:实现 `textDocument/rename`,支持符号重命名
4. **代码操作**:实现 `textDocument/codeAction`,支持快速修复
5. **跨语言跳转**:完善多语言协同,支持 TS 调用 Go API 的跨语言跳转
6. **LSP 服务器自动安装**:检测并自动安装缺失的 LSP 服务器

### 10.3 全部阶段完成

至此,DocAgent 编程 Agent 改造的 5 个阶段全部完成:

| 阶段 | 主题 | 任务数 | 状态 |
|------|------|--------|------|
| 阶段 1 | 核心架构与工具链 | 15 | 完成 |
| 阶段 2 | 权限系统与 Agent 模式 | 19 | 完成 |
| 阶段 3 | Skill 系统与上下文管理 | 22 | 完成 |
| 阶段 4 | 子 Agent 与高级工具 | 18 | 完成 |
| 阶段 5 | LSP 集成 | 20 | 完成 |
| **总计** | | **94** | **全部完成** |

DocAgent 已从文档处理 Agent 成功改造为编程 Agent,具备:
- 完整的代码生成与执行能力(阶段 1)
- 精细的权限控制与 Plan/Build/Document 三态模式(阶段 2)
- Skill 系统、TodoWrite、上下文压缩、代码语义搜索(阶段 3)
- 子 Agent 委托、WebFetch、WebSearch(阶段 4)
- LSP 集成,获得 IDE 级代码理解能力(阶段 5)
