# DocAgent Handler & Tool 系统开发规范

> 版本: 0.1.6
> 适用项目: DocAgent AI 文档处理桌面应用
> 最后更新: 2026-06-14

---

## 目录

1. [概述](#1-概述)
2. [Handler 接口规范](#2-handler-接口规范)
3. [内置 Handler 详细实现规范](#3-内置-handler-详细实现规范)
4. [Tool 系统](#4-tool-系统)
5. [Handler 与 LLM 的 Tool Calling 交互协议](#5-handler-与-llm-的-tool-calling-交互协议)
6. [附录](#6-附录)

---

## 1. 概述

DocAgent 包含两套平行的可调用单元：

| 系统 | 数量 | 实现语言 | 实现方式 | 用途 |
|------|------|---------|---------|------|
| Handler | 4 个 | Python | Sidecar 进程 | 文档读取/转换/分析（read/convert/analyze） |
| Tool | 10 个 | Rust | 原生实现 | 文件系统操作（列表/搜索/读写/删除/信息/存在检查/创建目录）+ 脚本写入与命令执行 |

**架构总览**：

```
                    LLM (Tool Calling)
                           │
              ┌────────────┴────────────┐
              │                        │
         Handler                    Tool
        Registry                  Registry
              │                        │
              ▼                        ▼
        Python Sidecar            Rust 原生
        (read/convert/            (list/search/read/
         analyze)                 write/delete/...)
```

### 关键设计决策

- **文档生成/修改**：通过 write_script + run_command Tool 编写脚本并执行（使用 python-docx/openpyxl/python-pptx/reportlab 等库）
- **文档读取/转换/分析**：通过 Handler 直接调用 Sidecar 的对应 action
- **文件系统操作**：通过 Rust 原生 Tool 实现，不依赖 Python
- Handler 和 Tool 均**始终启用**，不可由用户禁用

---

## 2. Handler 接口规范

### 2.1 Rust Trait 定义

```rust
#[async_trait]
pub trait Handler: Send + Sync {
    fn handler_name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(&self, params: Value) -> HandlerResult;
}
```

其中 Handler 的 `execute` 方法通过 `SidecarManager` 向 Python 进程发送请求并接收响应。

### 2.2 HandlerResult

```rust
pub struct HandlerResult {
    pub success: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
}
```

### 2.3 HandlerRegistry

```rust
pub struct HandlerRegistry {
    handlers: HashMap<String, Arc<dyn Handler>>,
}
```

注册表在初始化时注册 4 个内置 Handler，每个 Handler 对应一个文档类型（docx/xlsx/pptx/pdf）。

---

## 3. 内置 Handler 详细实现规范

4 个文档类型 Handler，每个 Handler 支持 3 种 action：

| Action | 说明 | 风险等级 | 需确认 |
|--------|------|----------|--------|
| `read` | 读取文档内容（文本+元数据） | 低 | 否 |
| `convert` | 格式转换 | 低 | 否 |
| `analyze` | 文档分析（统计+结构） | 低 | 否 |

### 3.1 docx_handler

用于 Word 文档的处理。

**参数**：
- `action` (string, required): read / convert / analyze
- `input_path` (string, required): 文档文件路径
- `params` (object, optional): 附加参数（如 convert 时的 target_format）

**示例调用**：
```json
{
  "action": "read",
  "input_path": "/workspace/doc.docx",
  "params": {}
}
```

### 3.2 xlsx_handler

用于 Excel 文档的处理。

### 3.3 pptx_handler

用于 PPT 文档的处理。

### 3.4 pdf_handler

用于 PDF 文档的处理（基于 PyMuPDF/fitz）。

---

## 4. Tool 系统

### 4.1 Rust Trait 定义

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn tool_name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(&self, params: Value, workspace_root: &Path) -> ToolResult;
}
```

与 Handler 不同的是，Tool 的 `execute` 方法接收 `workspace_root` 参数（由 executor 注入），用于路径安全校验。

### 4.2 内置 Tool 列表

| 工具名 | 功能 | 参数 |
|--------|------|------|
| `list_directory` | 列出目录内容 | path, max_depth?, extensions?, sort_by? |
| `search_files` | 搜索文件 | query, path?, extensions?, max_results?, include_content? |
| `read_file` | 读取纯文本文件 | path, encoding? |
| `write_text_file` | 写入文本文件 | path, content, append? |
| `delete_file` | 删除文件 | path, create_backup? |
| `file_info` | 获取文件元数据 | path |
| `file_exists` | 检查文件/目录是否存在 | path |
| `create_directory` | 创建目录 | path, recursive? |

### 4.3 路径安全机制

所有 Tool 通过 executor 注入的 `workspace_root` 进行路径校验：

```rust
fn resolve_path(workspace_root: &Path, input_path: &str) -> Result<PathBuf, ToolError> {
    let resolved = workspace_root.join(input_path).canonicalize()?;
    if !resolved.starts_with(workspace_root) {
        return Err(ToolError::PathOutOfBounds);
    }
    Ok(resolved)
}
```

拒绝路径遍历攻击（如 `../../etc/passwd`）。

---

## 5. Handler 与 LLM 的 Tool Calling 交互协议

### 5.1 交互流程

```
1. LLM 分析用户意图
2. LLM 返回 tool_calls（Handler/Tool）
3. AgentExecutor 解析 tool_calls
4. 高风险操作触发用户确认
5. 执行 Handler/Tool
6. 结果返回给 LLM
7. 循环直到 LLM 返回纯文本
```

### 5.2 循环控制

| 参数 | 默认值 | 说明 |
|------|--------|------|
| 最大迭代次数 | 20 | 达到后返回错误 |
| 确认超时 | 5 分钟 | 超时后自动取消 |
| Sidecar 超时 | 60 秒（文档操作） |
| run_command 超时 | 60 秒（可在设置中配置） |

### 5.3 确认机制的集成

- 操作前通过 `confirm_channels` oneshot channel 同步等待用户确认
- 前端 `ConfirmNode` 组件展示确认弹窗
- 用户确认/拒绝后通过 `confirm_operation` 命令返回结果
- 超时后返回 `AGENT_CONFIRMATION_TIMEOUT` 错误
- run_command 执行高风险命令前需要用户确认（如 rm -rf、format 等）

---

## 6. 附录

### 6.1 Handler/Tool 速查表

| 名称 | 类型 | 语言 | 说明 |
|------|------|------|------|
| docx_handler | Handler | Python | Word 文档 read/convert/analyze |
| xlsx_handler | Handler | Python | Excel 文档 read/convert/analyze |
| pptx_handler | Handler | Python | PPT 文档 read/convert/analyze |
| pdf_handler | Handler | Python | PDF 文档 read/convert/analyze |
| list_directory | Tool | Rust | 列出目录内容 |
| search_files | Tool | Rust | 搜索文件 |
| read_file | Tool | Rust | 读取文本文件 |
| write_text_file | Tool | Rust | 写入文本文件 |
| delete_file | Tool | Rust | 删除文件 |
| file_info | Tool | Rust | 获取文件元数据 |
| file_exists | Tool | Rust | 检查文件存在 |
| create_directory | Tool | Rust | 创建目录 |
| write_script | Tool | Rust | 将智能体生成的脚本写入系统临时目录 |
| run_command | Tool | Rust | 通过 Git Bash 执行命令（运行脚本） |

### 6.2 Sidecar 协议

请求格式：
```json
{"id": "uuid", "action": "read|convert|analyze|ping|validate", "type": "docx|xlsx|pptx|pdf|md|txt", "params": {}}
```

响应格式：
```json
{"id": "uuid", "success": true|false, "data": {}, "error": null}
```
