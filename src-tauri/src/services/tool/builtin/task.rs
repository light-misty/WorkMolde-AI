//! Task 工具：委托子任务给子 Agent 执行
//! 主 Agent 可通过此工具将子任务委托给独立子 Agent，子 Agent 拥有独立上下文但继承父 Agent 配置
//! 支持 single（单个子任务）和 batch（并行批量子任务）两种模式

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::errors::{AGENT_EXECUTION_ERROR, TOOL_INVALID_PARAMS};
use crate::models::sub_agent::{SubAgentConfig, SubAgentResult};
use crate::models::tool::ToolResult;
use crate::services::agent::sub_executor::SubAgentExecTrait;
use crate::services::tool::trait_def::Tool;

/// 检查嵌套深度是否允许执行子任务
/// - current_depth >= 3：超过最大嵌套深度限制
/// - current_depth >= 1 且 allow_nested_task=false：子 Agent 不允许调用 task 工具（防止递归）
///
/// 返回 Ok(()) 表示允许执行，Err(错误信息) 表示拒绝
pub fn check_nesting_depth(
    current_depth: u32,
    allow_nested_task: bool,
) -> Result<(), &'static str> {
    if current_depth >= 3 {
        return Err("子 Agent 嵌套深度超过限制(3 层)");
    }
    if current_depth >= 1 && !allow_nested_task {
        return Err("子 Agent 不允许调用 task 工具");
    }
    Ok(())
}

/// Task 工具：委托子任务给子 Agent 执行
/// 主 Agent 可通过此工具将子任务委托给独立子 Agent，子 Agent 拥有独立上下文但继承父 Agent 配置
/// 支持 single（单个子任务）和 batch（并行批量子任务）两种模式
/// T4.18: 采用延迟注入模式，sub_executor 在应用初始化后通过 set_sub_executor 设置
#[derive(Clone)]
pub struct TaskTool {
    /// 子 Agent 执行器（延迟注入，None 时调用 task 工具返回错误）
    /// 使用 trait 对象避免 SubAgentExecutor 的 Drop glue 在 cdylib 模式下的符号导出问题
    sub_executor: Arc<RwLock<Option<Arc<dyn SubAgentExecTrait>>>>,
    /// 父 Agent 的系统提示词（由 executor 每轮迭代注入）
    parent_system_prompt: Arc<RwLock<String>>,
    /// 当前 Agent 模式（plan/build/document）
    agent_mode: Arc<RwLock<String>>,
    /// 当前嵌套深度（0=主 Agent，1=子 Agent，2=孙 Agent，最大 3）
    nesting_depth: Arc<AtomicU32>,
}

impl TaskTool {
    /// 创建 TaskTool 实例（不包含 sub_executor，需后续调用 set_sub_executor 注入）
    pub fn new() -> Self {
        Self {
            sub_executor: Arc::new(RwLock::new(None)),
            parent_system_prompt: Arc::new(RwLock::new(String::new())),
            agent_mode: Arc::new(RwLock::new("build".to_string())),
            nesting_depth: Arc::new(AtomicU32::new(0)),
        }
    }

    /// 延迟注入子 Agent 执行器（由 lib.rs 在应用初始化后调用）
    pub async fn set_sub_executor(&self, executor: Arc<dyn SubAgentExecTrait>) {
        let mut guard = self.sub_executor.write().await;
        *guard = Some(executor);
    }

    /// 异步更新父 Agent 系统提示词（由 executor 每轮迭代注入）
    pub async fn update_parent_prompt(&self, prompt: String) {
        let mut guard = self.parent_system_prompt.write().await;
        *guard = prompt;
    }

    /// 异步更新 Agent 模式
    pub async fn update_agent_mode(&self, mode: String) {
        let mut guard = self.agent_mode.write().await;
        *guard = mode;
    }

    /// 更新当前嵌套深度
    pub fn update_nesting_depth(&self, depth: u32) {
        self.nesting_depth.store(depth, Ordering::Relaxed);
    }
}

impl Default for TaskTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn tool_name(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "委托子任务给子 Agent 执行。子 Agent 拥有独立上下文，继承父 Agent 的系统提示词、工作区和工具配置。\
         适用于将复杂任务分解为子任务独立执行。支持 single（单个子任务）和 batch（并行批量子任务）两种模式。"
    }

    fn category(&self) -> &str {
        "agent"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["single", "batch"],
                    "description": "操作模式：single=单个子任务，batch=并行批量子任务（最多 5 个）",
                    "default": "single"
                },
                "description": {
                    "type": "string",
                    "description": "子任务描述（action=single 时必填）"
                },
                "tasks": {
                    "type": "array",
                    "description": "批量子任务列表（action=batch 时必填，最多 5 个）",
                    "items": {
                        "type": "object",
                        "properties": {
                            "description": {
                                "type": "string",
                                "description": "子任务描述"
                            },
                            "maxIterations": {
                                "type": "integer",
                                "description": "子 Agent 最大迭代次数（默认 10，最大 50）",
                                "default": 10,
                                "minimum": 1,
                                "maximum": 50
                            },
                            "timeoutSeconds": {
                                "type": "integer",
                                "description": "子 Agent 超时时间（秒，默认 300，最大 600）",
                                "default": 300,
                                "minimum": 1,
                                "maximum": 600
                            }
                        },
                        "required": ["description"]
                    }
                },
                "maxIterations": {
                    "type": "integer",
                    "description": "子 Agent 最大迭代次数（默认 10，最大 50）",
                    "default": 10,
                    "minimum": 1,
                    "maximum": 50
                },
                "timeoutSeconds": {
                    "type": "integer",
                    "description": "子 Agent 超时时间（秒，默认 300，最大 600）",
                    "default": 300,
                    "minimum": 1,
                    "maximum": 600
                },
                "allowedTools": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "子 Agent 可用工具列表（空表示继承所有工具）"
                }
            },
            "required": ["description"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let start = std::time::Instant::now();

        // 提取 action（默认 "single"）
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("single");

        match action {
            // 单个子任务模式
            "single" => self.execute_single(params, start).await,
            // 批量并行子任务模式
            "batch" => self.execute_batch(params, start).await,
            // 未知 action
            _ => ToolResult {
                success: false,
                output: None,
                error: Some(format!("未知 action: {}，支持 single/batch", action)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(TOOL_INVALID_PARAMS),
            },
        }
    }
}

impl TaskTool {
    /// 执行单个子任务
    async fn execute_single(&self, params: Value, start: std::time::Instant) -> ToolResult {
        // 1. 提取 description（必填，缺失返回错误）
        let description = match params.get("description").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("缺少 description 参数".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(TOOL_INVALID_PARAMS),
                };
            }
        };

        // 2. 提取 maxIterations（默认 10，限制 1-50）
        let max_iterations = params
            .get("maxIterations")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .clamp(1, 50) as u32;

        // 3. 提取 timeoutSeconds（默认 300，限制最大 600）
        let timeout_seconds = params
            .get("timeoutSeconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(300)
            .clamp(1, 600);

        // 4. 提取 allowedTools（空表示继承所有工具）
        let allowed_tools: Vec<String> = params
            .get("allowedTools")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // 5. 从 params["_session_id"] 获取 session_id（默认 "default"）
        let session_id = params
            .get("_session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        // 6. 从 params["_workspace_root"] 获取 workspace_root（默认空字符串）
        let workspace_root = params
            .get("_workspace_root")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // 7. 从 params["_nesting_depth"] 获取当前嵌套深度（默认用 self.nesting_depth 的值）
        let current_depth = params
            .get("_nesting_depth")
            .and_then(|v| v.as_u64())
            .map(|d| d as u32)
            .unwrap_or_else(|| self.nesting_depth.load(Ordering::Relaxed));

        // 8. 嵌套深度检查（调用公共函数，allow_nested_task 默认为 false 防止递归调用）
        let allow_nested_task = false;
        if let Err(msg) = check_nesting_depth(current_depth, allow_nested_task) {
            return ToolResult {
                success: false,
                output: None,
                error: Some(msg.to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(AGENT_EXECUTION_ERROR),
            };
        }

        // 9. 读取 parent_system_prompt 和 agent_mode（优先从 params 读取，回退到字段值）
        let parent_prompt = match params.get("_system_prompt").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => self.parent_system_prompt.read().await.clone(),
        };
        let agent_mode = match params.get("_agent_mode").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => self.agent_mode.read().await.clone(),
        };

        // 10. 构建 SubAgentConfig
        let agent_id = Uuid::new_v4().to_string();
        let config = SubAgentConfig {
            agent_id: agent_id.clone(),
            parent_session_id: session_id,
            task_description: description,
            system_prompt: parent_prompt,
            workspace_root,
            max_iterations,
            timeout_seconds,
            allow_nested_task: false,
            allowed_tools,
            agent_mode,
            nesting_depth: current_depth + 1,
        };

        // 11. 调用子 Agent 执行器（从延迟注入的 RwLock 中读取）
        let executor = {
            let guard = self.sub_executor.read().await;
            match guard.clone() {
                Some(exec) => exec,
                None => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some("子 Agent 执行器尚未初始化".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(AGENT_EXECUTION_ERROR),
                    };
                }
            }
        };
        let result: SubAgentResult = executor.exec_sub_agent(config).await;

        let duration_ms = start.elapsed().as_millis() as u64;

        // 12. 构建 ToolResult
        ToolResult {
            success: result.success,
            output: Some(json!({
                "agentId": result.agent_id,
                "success": result.success,
                "result": result.result,
                "error": result.error,
                "iterations": result.iterations,
                "durationMs": result.duration_ms,
                "toolCalls": result.tool_calls,
            })),
            error: result.error.clone(),
            duration_ms,
            error_code: if result.success {
                None
            } else {
                Some(AGENT_EXECUTION_ERROR)
            },
        }
    }

    /// 批量并行执行子任务
    /// 最多支持 5 个子任务并行执行，使用 tokio::spawn 实现并行
    async fn execute_batch(&self, params: Value, start: std::time::Instant) -> ToolResult {
        // 1. 提取 tasks 数组（必填，非空）
        let tasks = match params.get("tasks").and_then(|v| v.as_array()) {
            Some(arr) if !arr.is_empty() => arr,
            _ => {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("缺少 tasks 参数或 tasks 为空".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(TOOL_INVALID_PARAMS),
                };
            }
        };

        // 2. 验证任务数量不超过 5（防止资源耗尽）
        if tasks.len() > 5 {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "批量任务数量超过限制(最多 5 个)，当前: {}",
                    tasks.len()
                )),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(TOOL_INVALID_PARAMS),
            };
        }

        // 3. 提取公共参数：_session_id
        let session_id = params
            .get("_session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        // 4. 提取公共参数：_workspace_root
        let workspace_root = params
            .get("_workspace_root")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // 5. 提取公共参数：_nesting_depth（默认用 self.nesting_depth 的值）
        let current_depth = params
            .get("_nesting_depth")
            .and_then(|v| v.as_u64())
            .map(|d| d as u32)
            .unwrap_or_else(|| self.nesting_depth.load(Ordering::Relaxed));

        // 6. 提取公共参数：allowedTools（空表示继承所有工具）
        let allowed_tools: Vec<String> = params
            .get("allowedTools")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // 7. 嵌套深度检查（调用公共函数，与 execute_single 相同逻辑）
        let allow_nested_task = false;
        if let Err(msg) = check_nesting_depth(current_depth, allow_nested_task) {
            return ToolResult {
                success: false,
                output: None,
                error: Some(msg.to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(AGENT_EXECUTION_ERROR),
            };
        }

        // 8. 读取 parent_system_prompt 和 agent_mode（优先从 params 读取，回退到字段值）
        let parent_prompt = match params.get("_system_prompt").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => self.parent_system_prompt.read().await.clone(),
        };
        let agent_mode = match params.get("_agent_mode").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => self.agent_mode.read().await.clone(),
        };

        // 9. 为每个 task 构建 SubAgentConfig 并使用 tokio::spawn 并行执行
        let mut handles = Vec::with_capacity(tasks.len());
        for task in tasks {
            // 提取单个任务的 description（必填，缺失返回错误）
            let description = match task.get("description").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some("tasks 中存在缺少 description 的任务".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(TOOL_INVALID_PARAMS),
                    };
                }
            };

            // 提取单个任务的 maxIterations（默认 10，限制 1-50）
            let max_iterations = task
                .get("maxIterations")
                .and_then(|v| v.as_u64())
                .unwrap_or(10)
                .clamp(1, 50) as u32;

            // 提取单个任务的 timeoutSeconds（默认 300，限制最大 600）
            let timeout_seconds = task
                .get("timeoutSeconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(300)
                .clamp(1, 600);

            // 构建 SubAgentConfig（agent_id 用 UUID，nesting_depth = current_depth + 1）
            let agent_id = Uuid::new_v4().to_string();
            let config = SubAgentConfig {
                agent_id,
                parent_session_id: session_id.clone(),
                task_description: description,
                system_prompt: parent_prompt.clone(),
                workspace_root: workspace_root.clone(),
                max_iterations,
                timeout_seconds,
                allow_nested_task: false,
                allowed_tools: allowed_tools.clone(),
                agent_mode: agent_mode.clone(),
                nesting_depth: current_depth + 1,
            };

            // sub_executor 从延迟注入的 RwLock 中读取（Arc<dyn SubAgentExecTrait> 可安全 clone 后 move 到 spawn 任务中）
            let executor = {
                let guard = self.sub_executor.read().await;
                match guard.clone() {
                    Some(exec) => exec,
                    None => {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some("子 Agent 执行器尚未初始化".to_string()),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: Some(AGENT_EXECUTION_ERROR),
                        };
                    }
                }
            };
            let handle = tokio::spawn(async move { executor.exec_sub_agent(config).await });
            handles.push(handle);
        }

        // 10. 依次 await 所有 JoinHandle 收集结果
        let mut results: Vec<SubAgentResult> = Vec::with_capacity(handles.len());
        let mut success_count = 0u32;
        let mut failed_count = 0u32;
        for handle in handles {
            match handle.await {
                Ok(result) => {
                    if result.success {
                        success_count += 1;
                    } else {
                        failed_count += 1;
                    }
                    results.push(result);
                }
                // JoinError：子任务 panic 或被取消
                Err(e) => {
                    failed_count += 1;
                    results.push(SubAgentResult {
                        agent_id: String::new(),
                        success: false,
                        result: String::new(),
                        error: Some(format!("子任务执行异常: {}", e)),
                        iterations: 0,
                        duration_ms: 0,
                        tool_calls: 0,
                    });
                }
            }
        }

        let total_count = results.len() as u32;
        let duration_ms = start.elapsed().as_millis() as u64;

        // 11. 构建聚合 ToolResult（至少一个成功即为成功，不返回单一错误）
        let results_json: Vec<Value> = results
            .iter()
            .map(|r| {
                json!({
                    "agentId": r.agent_id,
                    "success": r.success,
                    "result": r.result,
                    "error": r.error,
                    "iterations": r.iterations,
                    "durationMs": r.duration_ms,
                    "toolCalls": r.tool_calls,
                })
            })
            .collect();

        ToolResult {
            success: success_count > 0,
            output: Some(json!({
                "action": "batch",
                "total": total_count,
                "successCount": success_count,
                "failedCount": failed_count,
                "results": results_json,
            })),
            error: None,
            duration_ms,
            error_code: None,
        }
    }
}
