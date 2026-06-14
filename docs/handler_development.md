# DocAgent Handler 系统开发规范

> 版本: 1.0.0
> 适用项目: DocAgent AI 文档处理桌面应用
> 最后更新: 2026-05-14

---

## 目录

1. [概述](#1-概述)
2. [Handler 接口规范](#2-handler-接口规范)
3. [内置 Handler 详细实现规范](#3-内置-handler-详细实现规范)
4. [自定义 Handler 开发指南](#4-自定义-handler-开发指南)
5. [Handler 与 LLM 的 Tool Calling 交互协议](#5-handler-与-llm-的-tool-calling-交互协议)
6. [附录](#6-附录)

---

## 1. 概述

### 1.1 什么是 Handler

Handler 是 DocAgent 中可被 LLM 通过 Tool Calling 机制调用的原子化能力单元。每个 Handler 封装了一项具体的文档操作能力（如生成、修改、转换、分析等），并通过统一的接口规范与 LLM 进行交互。

### 1.2 设计原则

- **原子性**: 每个 Handler 只负责一项明确的操作，避免职责混淆
- **可组合性**: Handler 之间可以组合使用，batch_process 即为组合调用的体现
- **安全性**: 涉及文件修改/删除的操作必须提供快照/备份机制
- **可观测性**: 所有 Handler 执行结果均包含结构化的 display 信息，便于前端展示
- **可扩展性**: 支持用户通过标准接口开发自定义 Handler

### 1.3 架构总览

```
+------------------+     Tool Calling     +------------------+
|                  | <------------------> |                  |
|   LLM (大模型)   |   tool_calls/result  |  Handler Registry  |
|                  |                      |                  |
+------------------+                      +------------------+
                                                  |
                                          +-------+-------+
                                          |               |
                                    +-----+-----+   +-----+-----+
                                    | 内置 Handler |   | 自定义Handler |
                                    +-----------+   +-----------+
                                    | generate   |   | custom_1   |
                                    | modify     |   | custom_2   |
                                    | delete     |   | ...        |
                                    | convert    |   +-----------+
                                    | read       |
                                    | search     |
                                    | analyze    |
                                    | list       |
                                    | batch      |
                                    +-----------+
                                          |
                                    +-----+-----+
                                    |  Sidecar  |
                                    | (文档引擎) |
                                    +-----------+
```

---

## 2. Handler 接口规范

### 2.1 核心 Trait 定义（Rust）

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Handler 执行结果中的展示信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayInfo {
    /// 简短摘要，用于对话气泡中展示
    pub summary: String,
    /// 详细信息，用于展开面板展示
    pub details: Option<Value>,
}

/// Handler 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandlerResult {
    /// 是否执行成功
    pub success: bool,
    /// 结果数据（结构化 JSON）
    pub data: Option<Value>,
    /// 错误信息（仅在失败时填充）
    pub error: Option<String>,
    /// 展示信息
    pub display: DisplayInfo,
}

/// Handler 参数的 JSON Schema 定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterSchema {
    /// 参数名称
    pub name: String,
    /// 参数类型（JSON Schema 类型）
    pub param_type: String,
    /// 参数描述
    pub description: String,
    /// 是否必填
    pub required: bool,
    /// 默认值
    pub default: Option<Value>,
    /// 枚举值（如果参数为枚举类型）
    pub enum_values: Option<Vec<String>>,
}

/// Handler 接口 Trait
#[async_trait::async_trait]
pub trait Handler: Send + Sync {
    /// Handler 唯一标识名称（如 "generate_document"）
    fn handler_name(&self) -> &str;

    /// Handler 功能描述（供 LLM 理解何时调用此 Handler）
    fn description(&self) -> &str;

    /// 参数定义（JSON Schema 格式）
    fn parameters(&self) -> Value;

    /// 执行 Handler
    ///
    /// # 参数
    /// - `params`: LLM 传递的调用参数（已通过 JSON Schema 验证）
    ///
    /// # 返回
    /// - `HandlerResult`: 执行结果
    async fn execute(&self, params: Value) -> HandlerResult;
}
```

### 2.2 HandlerResult 详细规范

```rust
impl HandlerResult {
    /// 创建成功结果
    pub fn ok(data: Value, summary: &str, details: Option<Value>) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            display: DisplayInfo {
                summary: summary.to_string(),
                details,
            },
        }
    }

    /// 创建失败结果
    pub fn err(error: &str, summary: &str) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.to_string()),
            display: DisplayInfo {
                summary: summary.to_string(),
                details: None,
            },
        }
    }
}
```

### 2.3 JSON Schema 参数规范

每个 Handler 的 `parameters()` 方法返回符合 JSON Schema Draft-07 规范的对象，格式如下：

```json
{
  "type": "object",
  "properties": {
    "param_name": {
      "type": "string",
      "description": "参数描述"
    }
  },
  "required": ["param_name"]
}
```

该 Schema 同时用于：
1. **LLM Tool Calling**: 作为 `tools[].function.parameters` 传递给模型
2. **参数验证**: 在执行前对 LLM 返回的参数进行校验
3. **文档生成**: 自动生成 Handler 使用文档

### 2.4 Handler 注册表

```rust
use std::collections::HashMap;
use std::sync::Arc;

/// Handler 注册表，管理所有已注册的 Handler
pub struct HandlerRegistry {
    handlers: HashMap<String, Arc<dyn Handler>>,
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// 注册一个 Handler
    pub fn register(&mut self, handler: Arc<dyn Handler>) {
        let name = handler.handler_name().to_string();
        self.handlers.insert(name, handler);
    }

    /// 根据名称获取 Handler
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Handler>> {
        self.handlers.get(name)
    }

    /// 获取所有 Handler 的 Tool Calling 定义
    pub fn tool_definitions(&self) -> Vec<Value> {
        self.handlers.values().map(|handler| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": handler.handler_name(),
                    "description": handler.description(),
                    "parameters": handler.parameters(),
                }
            })
        }).collect()
    }

    /// 执行指定 Handler
    pub async fn execute(&self, name: &str, params: Value) -> HandlerResult {
        match self.handlers.get(name) {
            Some(handler) => handler.execute(params).await,
            None => HandlerResult::err(
                &format!("未找到 Handler: {}", name),
                &format!("Handler '{}' 不存在", name),
            ),
        }
    }
}
```

---

## 3. 内置 Handler 详细实现规范

### 3.1 generate_document - 生成文档

#### 基本信息

| 项目 | 值 |
|------|-----|
| 名称 | `generate_document` |
| 描述 | 根据指定格式和内容生成文档文件 |
| 风险等级 | 低（仅创建新文件） |
| 需要确认 | 否 |

#### 参数定义

```json
{
  "type": "object",
  "properties": {
    "document_type": {
      "type": "string",
      "description": "文档格式类型",
      "enum": ["docx", "xlsx", "pptx", "pdf", "md", "csv", "html"]
    },
    "filename": {
      "type": "string",
      "description": "输出文件名（不含路径，自动保存到当前工作区）"
    },
    "content": {
      "type": "object",
      "description": "文档内容结构",
      "properties": {
        "title": {
          "type": "string",
          "description": "文档标题"
        },
        "sections": {
          "type": "array",
          "description": "文档章节列表",
          "items": {
            "type": "object",
            "properties": {
              "heading": {
                "type": "string",
                "description": "章节标题"
              },
              "body": {
                "type": "string",
                "description": "章节正文内容"
              },
              "level": {
                "type": "integer",
                "description": "标题级别（1-6）",
                "default": 1
              }
            },
            "required": ["heading", "body"]
          }
        },
        "author": {
          "type": "string",
          "description": "文档作者（可选，默认使用当前用户名）"
        }
      },
      "required": ["title"]
    },
    "template_path": {
      "type": "string",
      "description": "模板文件路径（可选，指定后将基于模板生成文档）"
    }
  },
  "required": ["document_type", "filename", "content"]
}
```

#### 处理流程

```
1. 参数验证
   |-- 检查 document_type 是否为支持的格式
   |-- 检查 filename 合法性（不含路径分隔符、特殊字符）
   |-- 检查 content.title 非空
   |-- 检查 template_path 指向的文件存在（如提供）

2. 内容准备
   |-- 若 content.author 为空，填充当前系统用户名
   |-- 若提供 template_path，加载模板
   |-- 构建 Sidecar 生成请求体

3. 调用 Sidecar 生成文档
   |-- POST /api/generate
   |-- 请求体: { document_type, filename, content, template_path? }
   |-- 等待生成完成

4. 结果处理
   |-- 验证生成的文件存在
   |-- 获取文件大小
   |-- 构建返回结果
```

#### 输出结构

```json
{
  "success": true,
  "data": {
    "file_path": "/workspace/output/报告.docx",
    "file_size": 24576
  },
  "error": null,
  "display": {
    "summary": "已生成文档: 报告.docx (24KB)",
    "details": {
      "document_type": "docx",
      "title": "项目报告",
      "sections_count": 3,
      "file_path": "/workspace/output/报告.docx",
      "file_size": 24576
    }
  }
}
```

#### 错误场景

| 错误码 | 描述 | 处理建议 |
|--------|------|----------|
| `INVALID_FORMAT` | 不支持的文档格式 | 提示用户选择支持的格式 |
| `FILENAME_INVALID` | 文件名不合法 | 提示用户修改文件名 |
| `TEMPLATE_NOT_FOUND` | 模板文件不存在 | 提示用户检查模板路径 |
| `GENERATION_FAILED` | Sidecar 生成失败 | 检查 Sidecar 服务状态 |

---

### 3.2 modify_document - 修改文档

#### 基本信息

| 项目 | 值 |
|------|-----|
| 名称 | `modify_document` |
| 描述 | 根据指令修改已有文档内容 |
| 风险等级 | 高（修改已有文件） |
| 需要确认 | 是 |

#### 参数定义

```json
{
  "type": "object",
  "properties": {
    "file_path": {
      "type": "string",
      "description": "待修改文档的完整路径"
    },
    "instructions": {
      "type": "string",
      "description": "修改指令（自然语言描述需要进行的修改）"
    },
    "create_backup": {
      "type": "boolean",
      "description": "是否创建备份（默认 true）",
      "default": true
    }
  },
  "required": ["file_path", "instructions"]
}
```

#### 处理流程

```
1. 参数验证
   |-- 检查 file_path 指向的文件存在
   |-- 检查文件格式受支持
   |-- 检查 instructions 非空

2. 读取原始文档
   |-- 调用 Sidecar 读取文档内容
   |-- 保存原始内容到内存

3. 创建版本快照
   |-- 若 create_backup 为 true:
       |-- 生成快照 ID（UUID）
       |-- 将原始文件复制到 .docagent/snapshots/{snapshot_id}/
       |-- 记录快照元数据（时间戳、原路径、操作类型）

4. 执行修改
   |-- 将 instructions 和原始内容发送给 Sidecar
   |-- POST /api/modify
   |-- 请求体: { file_path, instructions, original_content }
   |-- 等待修改完成

5. 保存修改后的文档
   |-- 将修改结果写回原路径
   |-- 生成变更摘要

6. 结果处理
   |-- 构建返回结果（包含变更摘要）
```

#### 输出结构

```json
{
  "success": true,
  "data": {
    "modified_path": "/workspace/output/报告.docx",
    "changes_summary": "修改了第2章标题，新增了第4章'总结与展望'，更新了作者信息"
  },
  "error": null,
  "display": {
    "summary": "已修改文档: 报告.docx",
    "details": {
      "file_path": "/workspace/output/报告.docx",
      "changes_summary": "修改了第2章标题，新增了第4章'总结与展望'，更新了作者信息",
      "snapshot_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      "backup_created": true
    }
  }
}
```

#### 错误场景

| 错误码 | 描述 | 处理建议 |
|--------|------|----------|
| `FILE_NOT_FOUND` | 文件不存在 | 提示用户检查文件路径 |
| `UNSUPPORTED_FORMAT` | 文件格式不受支持 | 提示用户转换格式后重试 |
| `SNAPSHOT_FAILED` | 快照创建失败 | 建议关闭备份选项或检查磁盘空间 |
| `MODIFY_FAILED` | 修改操作失败 | 检查指令合法性，可从快照恢复 |

---

### 3.3 delete_document - 删除文档

#### 基本信息

| 项目 | 值 |
|------|-----|
| 名称 | `delete_document` |
| 描述 | 删除指定文档文件（支持快照恢复） |
| 风险等级 | 极高（删除文件） |
| 需要确认 | 是（强制） |

#### 参数定义

```json
{
  "type": "object",
  "properties": {
    "file_path": {
      "type": "string",
      "description": "待删除文档的完整路径"
    },
    "create_snapshot": {
      "type": "boolean",
      "description": "是否在删除前创建快照（默认 true）",
      "default": true
    }
  },
  "required": ["file_path"]
}
```

#### 处理流程

```
1. 参数验证
   |-- 检查 file_path 指向的文件存在
   |-- 检查文件在工作区范围内（禁止删除工作区外的文件）

2. 创建快照
   |-- 若 create_snapshot 为 true:
       |-- 生成快照 ID（UUID）
       |-- 将文件复制到 .docagent/snapshots/{snapshot_id}/
       |-- 记录快照元数据（时间戳、原路径、文件大小）

3. 执行删除
   |-- 删除原文件
   |-- 验证文件已不存在

4. 结果处理
   |-- 构建返回结果（包含快照 ID，用于恢复）
```

#### 输出结构

```json
{
  "success": true,
  "data": {
    "deleted_path": "/workspace/output/旧报告.docx",
    "snapshot_id": "f0e1d2c3-b4a5-6789-0abc-def123456789"
  },
  "error": null,
  "display": {
    "summary": "已删除文档: 旧报告.docx（快照已保存，可恢复）",
    "details": {
      "deleted_path": "/workspace/output/旧报告.docx",
      "snapshot_id": "f0e1d2c3-b4a5-6789-0abc-def123456789",
      "snapshot_available": true,
      "recover_command": "可通过快照 ID 恢复此文件"
    }
  }
}
```

#### 错误场景

| 错误码 | 描述 | 处理建议 |
|--------|------|----------|
| `FILE_NOT_FOUND` | 文件不存在 | 提示用户检查文件路径 |
| `OUT_OF_WORKSPACE` | 文件在工作区外 | 禁止删除，提示路径限制 |
| `SNAPSHOT_FAILED` | 快照创建失败 | 建议关闭快照选项或检查磁盘空间 |
| `DELETE_FAILED` | 删除操作失败 | 检查文件是否被占用或权限不足 |

---

### 3.4 convert_format - 格式转换

#### 基本信息

| 项目 | 值 |
|------|-----|
| 名称 | `convert_format` |
| 描述 | 将文档从一种格式转换为另一种格式 |
| 风险等级 | 低（不修改源文件） |
| 需要确认 | 否 |

#### 参数定义

```json
{
  "type": "object",
  "properties": {
    "source_path": {
      "type": "string",
      "description": "源文件路径"
    },
    "target_format": {
      "type": "string",
      "description": "目标格式",
      "enum": ["docx", "xlsx", "pptx", "pdf", "md", "csv", "html"]
    },
    "output_path": {
      "type": "string",
      "description": "输出文件路径（可选，默认与源文件同目录，仅扩展名不同）"
    }
  },
  "required": ["source_path", "target_format"]
}
```

#### 支持的转换矩阵

| 源格式 \ 目标格式 | docx | xlsx | pptx | pdf | md | csv | html |
|-------------------|------|------|------|-----|----|-----|------|
| docx              | -    | x    | x    | o   | o  | x   | o    |
| xlsx              | x    | -    | x    | o   | x  | o   | o    |
| pptx              | x    | x    | -    | o   | o  | x   | o    |
| pdf               | x    | x    | x    | -   | o  | x   | o    |
| md                | o    | x    | x    | o   | -  | x   | o    |
| csv               | x    | o    | x    | x   | x  | -   | o    |
| html              | o    | x    | x    | o   | o  | x   | -    |

> `o` = 支持, `x` = 不支持, `-` = 相同格式无需转换

#### 处理流程

```
1. 参数验证
   |-- 检查 source_path 指向的文件存在
   |-- 检查源文件格式与 target_format 不同
   |-- 检查转换路径受支持（参考转换矩阵）

2. 确定输出路径
   |-- 若提供 output_path，使用指定路径
   |-- 否则基于 source_path 替换扩展名生成

3. 调用 Sidecar 转换
   |-- POST /api/convert
   |-- 请求体: { source_path, target_format, output_path }
   |-- 等待转换完成

4. 结果处理
   |-- 验证输出文件存在
   |-- 获取输出文件大小
   |-- 构建返回结果
```

#### 输出结构

```json
{
  "success": true,
  "data": {
    "output_path": "/workspace/output/报告.pdf",
    "output_size": 156672
  },
  "error": null,
  "display": {
    "summary": "已转换: 报告.docx -> 报告.pdf (153KB)",
    "details": {
      "source_path": "/workspace/output/报告.docx",
      "target_format": "pdf",
      "output_path": "/workspace/output/报告.pdf",
      "output_size": 156672
    }
  }
}
```

#### 错误场景

| 错误码 | 描述 | 处理建议 |
|--------|------|----------|
| `FILE_NOT_FOUND` | 源文件不存在 | 提示用户检查文件路径 |
| `CONVERSION_NOT_SUPPORTED` | 转换路径不受支持 | 提示用户查看支持的转换矩阵 |
| `SAME_FORMAT` | 源格式与目标格式相同 | 无需转换 |
| `CONVERSION_FAILED` | 转换过程失败 | 检查文件是否损坏或格式是否正确 |

---

### 3.5 read_document - 读取文档

#### 基本信息

| 项目 | 值 |
|------|-----|
| 名称 | `read_document` |
| 描述 | 读取文档内容并提取文本和元数据 |
| 风险等级 | 低（只读操作） |
| 需要确认 | 否 |

#### 参数定义

```json
{
  "type": "object",
  "properties": {
    "file_path": {
      "type": "string",
      "description": "文档文件路径"
    },
    "max_length": {
      "type": "integer",
      "description": "最大返回字符数（默认 50000）",
      "default": 50000
    },
    "page": {
      "type": "integer",
      "description": "指定读取的页码（仅 PDF 有效，从 1 开始）"
    }
  },
  "required": ["file_path"]
}
```

#### 处理流程

```
1. 参数验证
   |-- 检查 file_path 指向的文件存在
   |-- 检查 max_length 为正整数
   |-- 检查 page 参数仅在 PDF 格式时使用

2. 读取文档
   |-- 调用 Sidecar 读取
   |-- GET /api/read?path={file_path}&max_length={max_length}&page={page}
   |-- 提取文本内容和元数据

3. 内容截断处理
   |-- 若内容超过 max_length，截断并标记 truncated = true
   |-- 保留元数据完整

4. 结果处理
   |-- 构建返回结果（包含内容、元数据、截断标记）
```

#### 输出结构

```json
{
  "success": true,
  "data": {
    "content": "# 项目报告\n\n## 第一章 概述\n\n本项目旨在...",
    "metadata": {
      "title": "项目报告",
      "author": "张三",
      "page_count": 12,
      "word_count": 3520
    },
    "truncated": false
  },
  "error": null,
  "display": {
    "summary": "已读取文档: 项目报告 (12页, 3520字)",
    "details": {
      "file_path": "/workspace/output/报告.docx",
      "title": "项目报告",
      "author": "张三",
      "page_count": 12,
      "word_count": 3520,
      "truncated": false
    }
  }
}
```

#### 错误场景

| 错误码 | 描述 | 处理建议 |
|--------|------|----------|
| `FILE_NOT_FOUND` | 文件不存在 | 提示用户检查文件路径 |
| `UNSUPPORTED_FORMAT` | 文件格式不受支持 | 提示用户支持的格式列表 |
| `READ_FAILED` | 读取失败 | 检查文件是否损坏或被占用 |
| `PAGE_OUT_OF_RANGE` | 页码超出范围 | 提示有效页码范围 |

---

### 3.6 search_documents - 搜索文档

#### 基本信息

| 项目 | 值 |
|------|-----|
| 名称 | `search_documents` |
| 描述 | 在工作区文档中进行全文搜索 |
| 风险等级 | 低（只读操作） |
| 需要确认 | 否 |

#### 参数定义

```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "搜索关键词或短语"
    },
    "file_types": {
      "type": "array",
      "description": "限定搜索的文件类型（可选，默认搜索所有支持的格式）",
      "items": {
        "type": "string",
        "enum": ["docx", "xlsx", "pptx", "pdf", "md", "csv", "html"]
      }
    },
    "directory": {
      "type": "string",
      "description": "限定搜索的目录（可选，默认整个工作区）"
    },
    "max_results": {
      "type": "integer",
      "description": "最大返回结果数（默认 20）",
      "default": 20
    }
  },
  "required": ["query"]
}
```

#### 处理流程

```
1. 参数验证
   |-- 检查 query 非空
   |-- 检查 directory 存在（如提供）
   |-- 检查 max_results 为正整数

2. 遍历工作区
   |-- 根据 directory 和 file_types 过滤文件
   |-- 收集候选文件列表

3. 全文搜索
   |-- 逐文件提取文本内容
   |-- 对每个文件执行关键词匹配
   |-- 计算相关度评分（基于匹配频率和位置）

4. 结果排序与截断
   |-- 按相关度降序排列
   |-- 截取 max_results 条结果
   |-- 为每条结果提取上下文片段（snippet）

5. 结果处理
   |-- 构建返回结果列表
```

#### 输出结构

```json
{
  "success": true,
  "data": {
    "results": [
      {
        "file_path": "/workspace/output/报告.docx",
        "line_number": 42,
        "snippet": "...本项目采用了**微服务架构**，将系统拆分为多个独立服务...",
        "relevance": 0.95
      },
      {
        "file_path": "/workspace/notes/会议记录.md",
        "line_number": 15,
        "snippet": "...讨论了微服务架构的优缺点，决定采用该方案...",
        "relevance": 0.82
      }
    ]
  },
  "error": null,
  "display": {
    "summary": "找到 2 条匹配结果",
    "details": {
      "query": "微服务架构",
      "total_matches": 2,
      "results": [
        {
          "file_path": "/workspace/output/报告.docx",
          "line_number": 42,
          "snippet": "...本项目采用了**微服务架构**，将系统拆分为多个独立服务...",
          "relevance": 0.95
        }
      ]
    }
  }
}
```

#### 错误场景

| 错误码 | 描述 | 处理建议 |
|--------|------|----------|
| `EMPTY_QUERY` | 搜索关键词为空 | 提示用户输入搜索内容 |
| `DIRECTORY_NOT_FOUND` | 指定目录不存在 | 提示用户检查目录路径 |
| `SEARCH_FAILED` | 搜索过程出错 | 检查文件访问权限 |

---

### 3.7 analyze_document - 分析文档

#### 基本信息

| 项目 | 值 |
|------|-----|
| 名称 | `analyze_document` |
| 描述 | 对文档进行多维度分析 |
| 风险等级 | 低（只读操作） |
| 需要确认 | 否 |

#### 参数定义

```json
{
  "type": "object",
  "properties": {
    "file_path": {
      "type": "string",
      "description": "待分析的文档路径"
    },
    "dimensions": {
      "type": "array",
      "description": "分析维度列表",
      "items": {
        "type": "string",
        "enum": ["summary", "structure", "data_stats", "keywords"]
      },
      "default": ["summary", "structure", "keywords"]
    }
  },
  "required": ["file_path"]
}
```

#### 分析维度说明

| 维度 | 标识 | 描述 | 输出内容 |
|------|------|------|----------|
| 摘要 | `summary` | 生成文档内容摘要 | 核心观点、主要结论 |
| 结构 | `structure` | 分析文档组织结构 | 章节层级、标题树、段落分布 |
| 数据统计 | `data_stats` | 统计文档中的数据信息 | 表格数量、数值范围、图表描述 |
| 关键词 | `keywords` | 提取文档关键词 | 关键词列表及权重 |

#### 处理流程

```
1. 参数验证
   |-- 检查 file_path 指向的文件存在
   |-- 检查 dimensions 非空

2. 读取文档
   |-- 调用 read_document 获取全文内容
   |-- 提取元数据

3. 按维度分析
   |-- summary: 调用 LLM 生成摘要
   |-- structure: 解析文档结构（标题层级、段落划分）
   |-- data_stats: 提取表格数据，计算统计指标
   |-- keywords: 使用 TF-IDF 或 LLM 提取关键词

4. 结果处理
   |-- 汇总各维度分析结果
   |-- 构建返回结果
```

#### 输出结构

```json
{
  "success": true,
  "data": {
    "analysis": {
      "summary": {
        "core_points": [
          "项目采用微服务架构进行系统设计",
          "已完成核心模块的开发与测试",
          "预计下季度完成全部功能交付"
        ],
        "conclusion": "项目进展顺利，架构选型合理，需关注性能优化"
      },
      "structure": {
        "heading_tree": [
          {
            "level": 1,
            "text": "项目报告",
            "children": [
              { "level": 2, "text": "第一章 概述", "children": [] },
              { "level": 2, "text": "第二章 架构设计", "children": [
                { "level": 3, "text": "2.1 微服务架构", "children": [] },
                { "level": 3, "text": "2.2 数据库设计", "children": [] }
              ]},
              { "level": 2, "text": "第三章 进度与计划", "children": [] }
            ]
          }
        ],
        "paragraph_count": 28,
        "total_sections": 6
      },
      "data_stats": {
        "table_count": 3,
        "tables": [
          {
            "rows": 5,
            "columns": 4,
            "description": "各模块开发进度表"
          }
        ],
        "numeric_fields_count": 12
      },
      "keywords": [
        { "word": "微服务", "weight": 0.92 },
        { "word": "架构设计", "weight": 0.85 },
        { "word": "性能优化", "weight": 0.78 },
        { "word": "数据库", "weight": 0.71 },
        { "word": "测试", "weight": 0.65 }
      ]
    }
  },
  "error": null,
  "display": {
    "summary": "文档分析完成: 6个章节, 3个表格, 核心关键词: 微服务、架构设计",
    "details": {
      "file_path": "/workspace/output/报告.docx",
      "dimensions": ["summary", "structure", "data_stats", "keywords"],
      "summary": "项目进展顺利，架构选型合理",
      "section_count": 6,
      "table_count": 3,
      "top_keywords": ["微服务", "架构设计", "性能优化"]
    }
  }
}
```

#### 错误场景

| 错误码 | 描述 | 处理建议 |
|--------|------|----------|
| `FILE_NOT_FOUND` | 文件不存在 | 提示用户检查文件路径 |
| `UNSUPPORTED_FORMAT` | 文件格式不受支持 | 提示用户支持的格式列表 |
| `ANALYSIS_FAILED` | 分析过程失败 | 检查文件内容是否可解析 |

---

### 3.8 list_workspace - 列出工作区文件

#### 基本信息

| 项目 | 值 |
|------|-----|
| 名称 | `list_workspace` |
| 描述 | 列出工作区中的文件和目录 |
| 风险等级 | 低（只读操作） |
| 需要确认 | 否 |

#### 参数定义

```json
{
  "type": "object",
  "properties": {
    "directory": {
      "type": "string",
      "description": "目标目录（可选，默认为工作区根目录）"
    },
    "recursive": {
      "type": "boolean",
      "description": "是否递归列出子目录（默认 false）",
      "default": false
    },
    "file_types": {
      "type": "array",
      "description": "限定列出的文件类型（可选，默认所有类型）",
      "items": {
        "type": "string",
        "enum": ["docx", "xlsx", "pptx", "pdf", "md", "csv", "html"]
      }
    }
  },
  "required": []
}
```

#### 处理流程

```
1. 参数验证
   |-- 检查 directory 存在（如提供）
   |-- 检查 directory 在工作区范围内

2. 遍历目录
   |-- 根据 recursive 决定遍历深度
   |-- 根据 file_types 过滤文件
   |-- 收集文件信息（名称、路径、类型、大小、修改时间）

3. 结果排序
   |-- 目录优先，然后按名称排序

4. 结果处理
   |-- 构建返回结果列表
```

#### 输出结构

```json
{
  "success": true,
  "data": {
    "files": [
      {
        "name": "报告.docx",
        "path": "/workspace/output/报告.docx",
        "type": "docx",
        "size": 24576,
        "modified_at": "2026-05-14T10:30:00Z"
      },
      {
        "name": "数据表.xlsx",
        "path": "/workspace/output/数据表.xlsx",
        "type": "xlsx",
        "size": 15360,
        "modified_at": "2026-05-13T16:45:00Z"
      },
      {
        "name": "notes",
        "path": "/workspace/notes",
        "type": "directory",
        "size": 0,
        "modified_at": "2026-05-12T09:00:00Z"
      }
    ]
  },
  "error": null,
  "display": {
    "summary": "工作区包含 3 个项目（2个文件, 1个目录）",
    "details": {
      "directory": "/workspace",
      "recursive": false,
      "total_files": 2,
      "total_directories": 1,
      "total_size": 39936
    }
  }
}
```

#### 错误场景

| 错误码 | 描述 | 处理建议 |
|--------|------|----------|
| `DIRECTORY_NOT_FOUND` | 目录不存在 | 提示用户检查目录路径 |
| `OUT_OF_WORKSPACE` | 目录在工作区外 | 禁止访问，提示路径限制 |
| `LIST_FAILED` | 遍历失败 | 检查目录权限 |

---

### 3.9 batch_process - 批量处理

#### 基本信息

| 项目 | 值 |
|------|-----|
| 名称 | `batch_process` |
| 描述 | 对多个文件批量执行同一操作 |
| 风险等级 | 中（取决于具体操作类型） |
| 需要确认 | 视操作类型而定 |

#### 参数定义

```json
{
  "type": "object",
  "properties": {
    "file_paths": {
      "type": "array",
      "description": "待处理的文件路径列表",
      "items": {
        "type": "string"
      }
    },
    "operation": {
      "type": "string",
      "description": "批量操作类型",
      "enum": ["generate", "modify", "convert"]
    },
    "params": {
      "type": "object",
      "description": "操作参数（根据 operation 类型不同而不同）",
      "properties": {
        "document_type": {
          "type": "string",
          "description": "[generate] 文档格式"
        },
        "instructions": {
          "type": "string",
          "description": "[modify] 修改指令"
        },
        "target_format": {
          "type": "string",
          "description": "[convert] 目标格式"
        }
      }
    }
  },
  "required": ["file_paths", "operation", "params"]
}
```

#### 处理流程

```
1. 参数验证
   |-- 检查 file_paths 非空
   |-- 检查 operation 合法
   |-- 检查 params 与 operation 匹配
   |-- 检查 file_paths 中的文件存在（modify/convert 时）

2. 确认机制
   |-- 若 operation 为 modify，触发用户确认
   |-- 展示批量操作预览（文件列表 + 操作类型）

3. 逐个执行
   |-- 遍历 file_paths
   |-- 对每个文件调用对应的 Handler
       |-- generate -> generate_document
       |-- modify -> modify_document
       |-- convert -> convert_format
   |-- 收集每个文件的执行结果
   |-- 单个文件失败不影响后续文件处理

4. 汇总结果
   |-- 统计成功/失败数量
   |-- 构建汇总返回结果
```

#### 输出结构

```json
{
  "success": true,
  "data": {
    "results": [
      {
        "file_path": "/workspace/output/报告.docx",
        "success": true,
        "data": {
          "output_path": "/workspace/output/报告.pdf",
          "output_size": 156672
        },
        "error": null
      },
      {
        "file_path": "/workspace/output/数据表.xlsx",
        "success": false,
        "data": null,
        "error": "转换路径 xlsx->pdf 不受支持"
      }
    ]
  },
  "error": null,
  "display": {
    "summary": "批量处理完成: 1 成功, 1 失败",
    "details": {
      "operation": "convert",
      "target_format": "pdf",
      "total": 2,
      "succeeded": 1,
      "failed": 1,
      "failed_files": ["/workspace/output/数据表.xlsx"]
    }
  }
}
```

#### 错误场景

| 错误码 | 描述 | 处理建议 |
|--------|------|----------|
| `EMPTY_FILE_LIST` | 文件列表为空 | 提示用户添加文件 |
| `INVALID_OPERATION` | 操作类型不合法 | 提示用户选择 generate/modify/convert |
| `PARAMS_MISMATCH` | 参数与操作不匹配 | 检查 params 是否包含对应操作所需参数 |

---

## 5. Handler 与 LLM 的 Tool Calling 交互协议

### 5.1 交互流程总览

```
用户输入
   |
   v
+------------------+
| 构建系统提示词    |  <-- 包含可用 Handler 列表（tool_definitions）
+------------------+
   |
   v
+------------------+
| 发送至 LLM       |  <-- messages + tools
+------------------+
   |
   v
+------------------+
| LLM 返回响应     |
+------------------+
   |
   +--- 纯文本响应 --> 直接展示给用户
   |
   +--- tool_calls --> 进入 Tool Calling 循环
           |
           v
   +------------------+
   | 解析 tool_calls  |
   +------------------+
           |
           v
   +------------------+
   | Handler 选择与     |
   | 参数映射         |
   +------------------+
           |
           v
   +------------------+     +------------------+
   | 确认机制检查     | --> | 用户确认/拒绝    |
   +------------------+     +------------------+
           |                        |
           | (确认通过)             | (拒绝)
           v                        v
   +------------------+     返回拒绝消息给 LLM
   | 执行 Handler       |
   +------------------+
           |
           v
   +------------------+
   | 构建工具结果消息 |
   +------------------+
           |
           v
   +------------------+
   | 追加到消息历史   |
   +------------------+
           |
           v
   +------------------+
   | 再次调用 LLM     |  <-- 包含工具执行结果
   +------------------+
           |
           v
   (循环，直到 LLM 不再返回 tool_calls)
```

### 5.2 LLM 返回 tool_calls 时的解析

#### 5.2.1 请求格式

发送给 LLM 的请求中包含 `tools` 字段，由 `HandlerRegistry::tool_definitions()` 生成：

```json
{
  "model": "gpt-4o",
  "messages": [
    {
      "role": "system",
      "content": "你是 DocAgent 文档处理助手..."
    },
    {
      "role": "user",
      "content": "帮我生成一份项目报告"
    }
  ],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "generate_document",
        "description": "根据指定格式和内容生成文档文件",
        "parameters": {
          "type": "object",
          "properties": {
            "document_type": { "type": "string", "enum": ["docx", "xlsx", "pptx", "pdf", "md", "csv", "html"] },
            "filename": { "type": "string" },
            "content": { "type": "object" }
          },
          "required": ["document_type", "filename", "content"]
        }
      }
    }
  ]
}
```

#### 5.2.2 响应解析

LLM 返回 `tool_calls` 时的响应格式：

```json
{
  "id": "chatcmpl-abc123",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": null,
        "tool_calls": [
          {
            "id": "call_001",
            "type": "function",
            "function": {
              "name": "generate_document",
              "arguments": "{\"document_type\":\"docx\",\"filename\":\"项目报告.docx\",\"content\":{\"title\":\"项目报告\",\"sections\":[{\"heading\":\"概述\",\"body\":\"本项目旨在...\"}]}}"
            }
          }
        ]
      },
      "finish_reason": "tool_calls"
    }
  ]
}
```

解析步骤：

1. 检查 `choices[0].message.tool_calls` 是否存在
2. 遍历 `tool_calls` 数组，提取每个调用的：
   - `id`: 工具调用 ID（用于结果关联）
   - `function.name`: Handler 名称
   - `function.arguments`: 参数 JSON 字符串

### 5.3 Handler 选择与参数映射

#### 5.3.1 Handler 选择

```rust
/// 根据 LLM 返回的 tool_call 选择并执行对应的 Handler
async fn handle_tool_call(
    registry: &HandlerRegistry,
    tool_call: &ToolCall,
) -> (String, HandlerResult) {
    let handler_name = &tool_call.function.name;

    // 解析参数 JSON
    let params: Value = match serde_json::from_str(&tool_call.function.arguments) {
        Ok(p) => p,
        Err(e) => {
            return (
                tool_call.id.clone(),
                HandlerResult::err(
                    &format!("参数 JSON 解析失败: {}", e),
                    "参数格式错误",
                ),
            );
        }
    };

    // 从注册表查找并执行 Handler
    let result = registry.execute(handler_name, params).await;
    (tool_call.id.clone(), result)
}
```

#### 5.3.2 参数映射规则

| LLM 参数类型 | Handler 参数类型 | 映射规则 |
|-------------|---------------|----------|
| string | string | 直接映射 |
| integer | integer | 直接映射 |
| number | number | 直接映射 |
| boolean | boolean | 直接映射 |
| array | array | 直接映射 |
| object | object | 直接映射 |
| null | - | 移除该字段，使用 Handler 默认值 |
| 缺失字段 | - | 使用 handler.json 中定义的 default 值 |

#### 5.3.3 参数补全

对于 LLM 未提供的可选参数，框架应自动补全默认值：

```rust
/// 补全缺失的可选参数默认值
fn fill_defaults(params: &mut Value, schema: &Value) {
    let properties = schema.get("properties").unwrap();
    for (key, prop_schema) in properties.as_object().unwrap() {
        if params.get(key).is_none() {
            if let Some(default) = prop_schema.get("default") {
                params.as_object_mut().unwrap().insert(
                    key.clone(),
                    default.clone(),
                );
            }
        }
    }
}
```

### 5.4 执行结果返回格式

Handler 执行完成后，需要将结果以 `tool` 角色消息返回给 LLM：

```json
{
  "role": "tool",
  "tool_call_id": "call_001",
  "content": "{\"success\":true,\"data\":{\"file_path\":\"/workspace/output/项目报告.docx\",\"file_size\":24576},\"error\":null,\"display\":{\"summary\":\"已生成文档: 项目报告.docx (24KB)\",\"details\":null}}"
}
```

#### 5.4.1 结果序列化规则

1. 将完整的 `HandlerResult` 序列化为 JSON 字符串
2. `content` 字段为 JSON 字符串（非 JSON 对象）
3. `tool_call_id` 必须与 LLM 返回的 `tool_calls[].id` 一致
4. 即使 Handler 执行失败，也必须返回 `tool` 消息（包含 error 信息）

#### 5.4.2 失败结果返回

```json
{
  "role": "tool",
  "tool_call_id": "call_002",
  "content": "{\"success\":false,\"data\":null,\"error\":\"FILE_NOT_FOUND: 文件不存在\",\"display\":{\"summary\":\"文件不存在\",\"details\":null}}"
}
```

LLM 收到失败结果后，可以：
- 向用户解释失败原因
- 建议替代方案
- 使用其他 Handler 重试

### 5.5 多轮 Tool Calling 循环控制

#### 5.5.1 循环机制

LLM 可能在收到工具结果后继续发起 `tool_calls`，形成多轮调用循环：

```
轮次1: 用户 -> LLM -> tool_calls[generate_document]
轮次2: tool_result -> LLM -> tool_calls[convert_format]  (LLM 决定进一步转换格式)
轮次3: tool_result -> LLM -> 纯文本响应  (循环结束)
```

#### 5.5.2 循环控制参数

| 参数 | 默认值 | 描述 |
|------|--------|------|
| `max_tool_rounds` | 10 | 最大 Tool Calling 轮次 |
| `max_tools_per_round` | 3 | 单轮最大并行工具调用数 |
| `tool_call_timeout` | 30s | 单个 Handler 执行超时时间 |

#### 5.5.3 循环终止条件

循环在以下任一条件满足时终止：

1. LLM 返回纯文本响应（无 `tool_calls`）
2. 达到 `max_tool_rounds` 上限
3. 连续 3 次工具调用失败
4. 用户主动中断

#### 5.5.4 循环实现

```rust
/// Tool Calling 循环控制器
pub struct ToolCallingLoop {
    registry: Arc<HandlerRegistry>,
    max_rounds: usize,
    max_tools_per_round: usize,
    timeout: Duration,
    consecutive_failures: usize,
    max_consecutive_failures: usize,
}

impl ToolCallingLoop {
    pub fn new(registry: Arc<HandlerRegistry>) -> Self {
        Self {
            registry,
            max_rounds: 10,
            max_tools_per_round: 3,
            timeout: Duration::from_secs(30),
            consecutive_failures: 0,
            max_consecutive_failures: 3,
        }
    }

    /// 执行 Tool Calling 循环
    pub async fn run(
        &mut self,
        client: &LlmClient,
        messages: &mut Vec<Value>,
    ) -> Result<String, LoopError> {
        for round in 0..self.max_rounds {
            // 调用 LLM
            let response = client.chat(messages).await?;

            let choice = &response.choices[0];
            let assistant_message = &choice.message;

            // 将助手消息追加到历史
            messages.push(serde_json::to_value(assistant_message)?);

            // 检查是否有 tool_calls
            let tool_calls = match &assistant_message.tool_calls {
                Some(calls) => calls,
                None => {
                    // 无 tool_calls，返回文本内容
                    return Ok(assistant_message.content.clone().unwrap_or_default());
                }
            };

            // 限制单轮并行调用数
            let calls_to_execute: Vec<_> = tool_calls
                .iter()
                .take(self.max_tools_per_round)
                .collect();

            // 执行所有工具调用
            let mut round_had_failure = false;
            for tool_call in calls_to_execute {
                let (call_id, result) =
                    handle_tool_call(&self.registry, tool_call).await;

                if !result.success {
                    round_had_failure = true;
                }

                // 构建工具结果消息
                let tool_message = serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": serde_json::to_string(&result)?
                });
                messages.push(tool_message);
            }

            // 更新连续失败计数
            if round_had_failure {
                self.consecutive_failures += 1;
                if self.consecutive_failures >= self.max_consecutive_failures {
                    return Err(LoopError::TooManyFailures(
                        self.consecutive_failures,
                    ));
                }
            } else {
                self.consecutive_failures = 0;
            }
        }

        Err(LoopError::MaxRoundsExceeded(self.max_rounds))
    }
}
```

### 5.6 确认机制的集成

#### 5.6.1 确认触发条件

以下情况需要用户确认后才能执行 Handler：

1. Handler 声明了 `requires_confirmation: true`
2. Handler 的 `risk_level` 为 `high` 或 `critical`
3. `batch_process` 操作中包含 `modify` 类型
4. `delete_document` 操作（强制确认）

#### 5.6.2 确认流程

```
LLM 返回 tool_calls
       |
       v
+------------------+
| 遍历 tool_calls  |
+------------------+
       |
       v
+------------------+
| 检查确认需求     |
+------------------+
       |
       +--- 无需确认 --> 直接执行
       |
       +--- 需要确认 --> 展示确认对话框
               |
               +--- 用户确认 --> 执行 Handler
               |        |
               |        v
               |   返回正常结果给 LLM
               |
               +--- 用户拒绝 --> 返回拒绝消息给 LLM
                        |
                        v
                   {
                     "role": "tool",
                     "tool_call_id": "call_xxx",
                     "content": "{\"success\":false,\"data\":null,
                       \"error\":\"USER_REJECTED: 用户拒绝执行此操作\",
                       \"display\":{\"summary\":\"操作已取消\",\"details\":null}}"
                   }
```

#### 5.6.3 确认对话框内容

确认对话框应包含以下信息：

```
+------------------------------------------+
|  操作确认                                 |
+------------------------------------------+
|                                           |
|  即将执行: modify_document                |
|  描述: 修改已有文档                       |
|                                           |
|  参数:                                    |
|    文件: /workspace/output/报告.docx      |
|    指令: 将第三章标题改为"技术方案"        |
|    创建备份: 是                           |
|                                           |
|  [确认执行]  [取消]                       |
+------------------------------------------+
```

#### 5.6.4 批量操作的确认

对于 `batch_process`，确认对话框应展示批量预览：

```
+------------------------------------------+
|  批量操作确认                             |
+------------------------------------------+
|                                           |
|  操作类型: convert (格式转换)             |
|  目标格式: pdf                            |
|                                           |
|  涉及文件 (3个):                          |
|    1. /workspace/output/报告.docx         |
|    2. /workspace/output/方案.docx         |
|    3. /workspace/output/总结.md           |
|                                           |
|  [确认全部执行]  [取消]                   |
+------------------------------------------+
```

#### 5.6.5 确认超时

若用户在 60 秒内未响应确认对话框，视为拒绝操作，返回超时拒绝消息：

```json
{
  "success": false,
  "data": null,
  "error": "CONFIRMATION_TIMEOUT: 确认超时，操作已自动取消",
  "display": {
    "summary": "操作确认超时已取消",
    "details": null
  }
}
```

---

## 6. 附录

### 6.1 内置 Handler 速查表

| Handler | 名称 | 风险等级 | 需确认 | 核心参数 |
|-------|------|----------|--------|----------|
| 生成文档 | `generate_document` | 低 | 否 | document_type, filename, content |
| 修改文档 | `modify_document` | 高 | 是 | file_path, instructions |
| 删除文档 | `delete_document` | 极高 | 是(强制) | file_path |
| 格式转换 | `convert_format` | 低 | 否 | source_path, target_format |
| 读取文档 | `read_document` | 低 | 否 | file_path |
| 搜索文档 | `search_documents` | 低 | 否 | query |
| 分析文档 | `analyze_document` | 低 | 否 | file_path, dimensions |
| 列出文件 | `list_workspace` | 低 | 否 | directory, recursive |
| 批量处理 | `batch_process` | 中 | 视操作 | file_paths, operation, params |

### 6.2 错误码汇总

| 错误码 | 类别 | 适用 Handler |
|--------|------|-----------|
| `INVALID_FORMAT` | 参数 | generate_document |
| `FILENAME_INVALID` | 参数 | generate_document |
| `TEMPLATE_NOT_FOUND` | 资源 | generate_document |
| `GENERATION_FAILED` | 内部 | generate_document |
| `FILE_NOT_FOUND` | 资源 | modify, delete, read, analyze |
| `UNSUPPORTED_FORMAT` | 参数 | modify, read, convert, analyze |
| `SNAPSHOT_FAILED` | 内部 | modify_document, delete_document |
| `MODIFY_FAILED` | 内部 | modify_document |
| `OUT_OF_WORKSPACE` | 权限 | delete_document, list_workspace |
| `DELETE_FAILED` | 内部 | delete_document |
| `CONVERSION_NOT_SUPPORTED` | 参数 | convert_format |
| `SAME_FORMAT` | 参数 | convert_format |
| `CONVERSION_FAILED` | 内部 | convert_format |
| `READ_FAILED` | 内部 | read_document |
| `PAGE_OUT_OF_RANGE` | 参数 | read_document |
| `EMPTY_QUERY` | 参数 | search_documents |
| `DIRECTORY_NOT_FOUND` | 资源 | search_documents, list_workspace |
| `SEARCH_FAILED` | 内部 | search_documents |
| `ANALYSIS_FAILED` | 内部 | analyze_document |
| `LIST_FAILED` | 内部 | list_workspace |
| `EMPTY_FILE_LIST` | 参数 | batch_process |
| `INVALID_OPERATION` | 参数 | batch_process |
| `PARAMS_MISMATCH` | 参数 | batch_process |
| `PARAM_VALIDATION_FAILED` | 参数 | 所有 Handler |
| `USER_REJECTED` | 确认 | 需确认的 Handler |
| `CONFIRMATION_TIMEOUT` | 确认 | 需确认的 Handler |
| `SIDECAR_UNAVAILABLE` | 外部 | 所有 Handler |
| `SIDECAR_TIMEOUT` | 外部 | 所有 Handler |

### 6.3 JSON Schema 常用模式

#### 枚举类型参数

```json
{
  "document_type": {
    "type": "string",
    "description": "文档格式",
    "enum": ["docx", "xlsx", "pptx", "pdf", "md", "csv", "html"]
  }
}
```

#### 嵌套对象参数

```json
{
  "content": {
    "type": "object",
    "description": "文档内容",
    "properties": {
      "title": { "type": "string", "description": "标题" },
      "sections": {
        "type": "array",
        "description": "章节列表",
        "items": {
          "type": "object",
          "properties": {
            "heading": { "type": "string" },
            "body": { "type": "string" }
          },
          "required": ["heading", "body"]
        }
      }
    },
    "required": ["title"]
  }
}
```

#### 带默认值的可选参数

```json
{
  "recursive": {
    "type": "boolean",
    "description": "是否递归遍历",
    "default": false
  }
}
```

### 6.4 Handler 生命周期

```
注册阶段:
  Handler 实现 Trait / 编写 handler.json
       |
       v
  注册到 HandlerRegistry
       |
       v
  生成 tool_definitions 供 LLM 使用

调用阶段:
  LLM 返回 tool_calls
       |
       v
  参数验证 (Schema 校验 + 默认值补全)
       |
       v
  确认检查 (risk_level / requires_confirmation)
       |
       v
  执行 Handler (execute 方法)
       |
       v
  结果序列化 (HandlerResult -> JSON)
       |
       v
  返回给 LLM (tool 角色消息)

卸载阶段:
  从 HandlerRegistry 移除
       |
       v
  释放资源
```

### 6.5 版本兼容性

| Handler 版本 | DocAgent 版本 | 变更说明 |
|-----------|--------------|----------|
| 1.0.0 | >= 0.1.0 | 初始版本，9 个内置 Handler |

> 自定义 Handler 的 `version` 字段遵循语义化版本规范（SemVer）。
> 主版本号变更表示不兼容的 API 变更，次版本号变更表示向后兼容的功能新增，修订号变更表示向后兼容的问题修复。
