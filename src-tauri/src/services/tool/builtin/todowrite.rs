//! TodoWrite 工具：结构化任务管理
//! 支持 create/update/list/clear 四种操作，按 session_id 隔离并持久化

use crate::db::todo_repo;
use crate::db::Database;
use crate::errors::{self, CommandError};
use crate::models::todo::{TodoItem, TodoPriority, TodoStatus};
use crate::models::tool::ToolResult;
use crate::services::tool::trait_def::Tool;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// TodoWrite 工具：结构化任务管理
/// 支持创建、更新、列出和清除任务，按 session_id 隔离并持久化到数据库
pub struct TodoWriteTool {
    /// 数据库连接（用于持久化 Todo 列表）
    db: Arc<Database>,
}

impl TodoWriteTool {
    /// 创建 TodoWrite 工具实例
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// 获取当前时间戳（毫秒，与 todo.rs 中 current_timestamp_ms 保持一致）
    fn now_ts() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// 从参数中提取 session_id（executor 注入的 _session_id）
    /// _session_id 以下划线开头，表示是系统注入参数，不暴露给 LLM
    fn extract_session_id(params: &Value) -> Result<String, CommandError> {
        params
            .get("_session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                CommandError::tool(
                    errors::TOOL_INVALID_PARAMS,
                    "Missing _session_id parameter".to_string(),
                )
            })
    }
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn tool_name(&self) -> &str {
        "todowrite"
    }

    fn description(&self) -> &str {
        "Structured task management tool. Supports creating, updating, listing, and clearing tasks. \
         Task statuses: pending / in_progress / completed. At most one in_progress task at a time; \
         setting a new task to in_progress automatically changes other in_progress tasks to pending."
    }

    fn category(&self) -> &str {
        "memory"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "update", "list", "clear"],
                    "description": "Action type"
                },
                "content": {
                    "type": "string",
                    "description": "Task content (required for create, optional for update)"
                },
                "taskId": {
                    "type": "string",
                    "description": "Task ID (required for update)"
                },
                "status": {
                    "type": "string",
                    "enum": ["pending", "in_progress", "completed"],
                    "description": "Task status (optional for update)"
                },
                "priority": {
                    "type": "string",
                    "enum": ["high", "medium", "low"],
                    "description": "Task priority (optional for create, default medium)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let start = std::time::Instant::now();

        // 提取 session_id（由 executor 在调用前注入）
        let session_id = match Self::extract_session_id(&params) {
            Ok(id) => id,
            Err(e) => {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(e.code),
                };
            }
        };

        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");

        let result = match action {
            "create" => self.handle_create(&session_id, &params),
            "update" => self.handle_update(&session_id, &params),
            "list" => self.handle_list(&session_id),
            "clear" => self.handle_clear(&session_id),
            _ => Err(CommandError::tool(
                errors::TOOL_INVALID_PARAMS,
                format!("Unknown action: {}", action),
            )),
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        match result {
            Ok(output) => ToolResult {
                success: true,
                output: Some(output),
                error: None,
                duration_ms,
                error_code: None,
            },
            Err(e) => ToolResult {
                success: false,
                output: None,
                error: Some(e.to_string()),
                duration_ms,
                error_code: Some(e.code),
            },
        }
    }
}

impl TodoWriteTool {
    /// 处理 create 操作：创建新任务
    fn handle_create(&self, session_id: &str, params: &Value) -> Result<Value, CommandError> {
        let content = params
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                CommandError::tool(
                    errors::TOOL_INVALID_PARAMS,
                    "create action requires content parameter".to_string(),
                )
            })?;

        let priority_str = params
            .get("priority")
            .and_then(|v| v.as_str())
            .unwrap_or("medium");
        let priority = TodoPriority::from_str(priority_str).unwrap_or_default();

        let now = Self::now_ts();
        let item = TodoItem {
            id: uuid::Uuid::new_v4().to_string(),
            content: content.to_string(),
            status: TodoStatus::Pending,
            priority,
            created_at: now,
            updated_at: now,
            completed_at: None,
        };

        let conn = self.db.conn()?;
        let mut todo_list = todo_repo::get_todo_list(&conn, session_id)?;
        todo_list.items.push(item.clone());
        todo_list.updated_at = now;
        todo_repo::save_todo_list(&conn, &todo_list)?;

        Ok(json!({
            "created": item,
            "summary": todo_list.build_summary(),
        }))
    }

    /// 处理 update 操作：更新任务状态/内容/优先级
    fn handle_update(&self, session_id: &str, params: &Value) -> Result<Value, CommandError> {
        let task_id = params
            .get("taskId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                CommandError::tool(
                    errors::TOOL_INVALID_PARAMS,
                    "update action requires taskId parameter".to_string(),
                )
            })?;

        let conn = self.db.conn()?;
        let mut todo_list = todo_repo::get_todo_list(&conn, session_id)?;

        // 使用索引而非可变引用，避免 borrow checker 冲突
        // （更新 in_progress 时需要遍历其他 items）
        let idx = todo_list
            .items
            .iter()
            .position(|i| i.id == task_id)
            .ok_or_else(|| {
                CommandError::tool(
                    errors::TOOL_NOT_FOUND,
                    format!("Task not found: {}", task_id),
                )
            })?;

        let now = Self::now_ts();
        let mut updated = false;

        // 更新状态
        if let Some(status_str) = params.get("status").and_then(|v| v.as_str()) {
            if let Some(new_status) = TodoStatus::from_str(status_str) {
                let is_completed = matches!(new_status, TodoStatus::Completed);
                // 如果设为 in_progress，需要将其他 in_progress 改为 pending
                if matches!(new_status, TodoStatus::InProgress) {
                    for other in todo_list.items.iter_mut() {
                        if other.id != task_id && matches!(other.status, TodoStatus::InProgress) {
                            other.status = TodoStatus::Pending;
                            other.updated_at = now;
                        }
                    }
                }
                let item = &mut todo_list.items[idx];
                item.status = new_status;
                item.completed_at = if is_completed { Some(now) } else { None };
                updated = true;
            }
        }

        // 更新内容和优先级
        {
            let item = &mut todo_list.items[idx];
            if let Some(content) = params.get("content").and_then(|v| v.as_str()) {
                item.content = content.to_string();
                updated = true;
            }
            if let Some(priority_str) = params.get("priority").and_then(|v| v.as_str()) {
                if let Some(p) = TodoPriority::from_str(priority_str) {
                    item.priority = p;
                    updated = true;
                }
            }
            if updated {
                item.updated_at = now;
            }
        }

        if updated {
            todo_list.updated_at = now;
            todo_repo::save_todo_list(&conn, &todo_list)?;
        }

        let item = &todo_list.items[idx];
        Ok(json!({
            "updated": item,
            "summary": todo_list.build_summary(),
        }))
    }

    /// 处理 list 操作：列出所有任务
    fn handle_list(&self, session_id: &str) -> Result<Value, CommandError> {
        let conn = self.db.conn()?;
        let todo_list = todo_repo::get_todo_list(&conn, session_id)?;
        Ok(json!({
            "items": todo_list.items,
            "summary": todo_list.build_summary(),
            "pendingCount": todo_list.pending_count(),
            "completedCount": todo_list.completed_count(),
            "totalCount": todo_list.items.len(),
        }))
    }

    /// 处理 clear 操作：清空任务列表
    fn handle_clear(&self, session_id: &str) -> Result<Value, CommandError> {
        let conn = self.db.conn()?;
        todo_repo::delete_todo_list(&conn, session_id)?;
        Ok(json!({
            "cleared": true,
            "summary": null,
        }))
    }
}
