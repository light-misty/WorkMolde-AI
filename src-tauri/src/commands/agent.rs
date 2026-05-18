use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::errors::{CommandError, AGENT_ALREADY_RUNNING, AGENT_NOT_RUNNING, AGENT_SESSION_NOT_FOUND};
use crate::events::AgentEmitter;
use crate::services::agent::context::AgentContext;
use crate::services::agent::executor::AgentExecutor;
use crate::AppState;

/// 启动 Agent 执行，在后台 spawn 一个 tokio task
#[tauri::command]
pub async fn start_agent(
    session_id: String,
    prompt: String,
    options: Option<serde_json::Value>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("start_agent 请求: session_id={}, prompt长度={}", session_id, prompt.len());

    // 检查是否已有 Agent 在该会话中运行
    {
        let active = state.active_agents.lock().await;
        if active.contains_key(&session_id) {
            log::error!("start_agent 失败: 会话 '{}' 已有 Agent 正在运行", session_id);
            return Err(CommandError::agent(
                AGENT_ALREADY_RUNNING,
                format!("会话 '{}' 已有 Agent 正在运行", session_id),
            ));
        }
    }

    // 注册为活跃 Agent
    {
        let mut active = state.active_agents.lock().await;
        active.insert(session_id.clone(), true);
        log::info!("start_agent: 会话 '{}' 已注册为活跃 Agent", session_id);
    }

    let emitter = AgentEmitter::new(app_handle.clone());
    let sid = session_id.clone();
    let prompt_clone = prompt.clone();

    let llm_router = Arc::clone(&state.llm_router);
    let skill_registry = Arc::clone(&state.skill_registry);
    let active_agents = Arc::clone(&state.active_agents);
    let db = Arc::clone(&state.db);
    let confirm_channels = Arc::clone(&state.confirm_channels);

    let max_iterations = options
        .as_ref()
        .and_then(|o| o.get("maxIterations"))
        .and_then(|v| v.as_u64())
        .unwrap_or(20) as u32;

    let workspace_path = options
        .as_ref()
        .and_then(|o| o.get("workingDirectory"))
        .and_then(|v| v.as_str())
        .unwrap_or(".")
        .to_string();

    tokio::spawn(async move {
        let active_agents_for_check = Arc::clone(&active_agents);

        let should_stop: Arc<dyn Fn(&str) -> bool + Send + Sync> = Arc::new(move |session_id: &str| {
            match active_agents_for_check.try_lock() {
                Ok(guard) => !guard.get(session_id).copied().unwrap_or(false),
                Err(_) => false,
            }
        });

        // 读取当前 LlmRouter 快照
        let router_snapshot = {
            let guard = llm_router.read().await;
            Arc::clone(&guard)
        };

        let result = run_agent(
            &sid,
            &prompt_clone,
            &router_snapshot,
            &skill_registry,
            &emitter,
            max_iterations,
            &workspace_path,
            should_stop,
            &db,
            &confirm_channels,
        ).await;

        if let Err(e) = &result {
            log::error!("Agent 执行失败: session_id={}, 错误: {}", sid, e.message);
        }

        {
            let mut active = active_agents.lock().await;
            let was_running = active.remove(&sid);
            if was_running.is_none() {
                log::warn!("Agent 已从活跃列表移除: session_id={}", sid);
            } else {
                log::info!("Agent 已从活跃列表移除: session_id={}", sid);
            }
        }
    });

    log::info!("start_agent 成功: session_id={}", session_id);
    Ok(())
}

/// 停止 Agent 执行
#[tauri::command]
pub async fn stop_agent(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("stop_agent 请求: session_id={}", session_id);
    let mut active = state.active_agents.lock().await;
    if !active.contains_key(&session_id) {
        log::error!("stop_agent 失败: 会话 '{}' 没有 Agent 在运行", session_id);
        return Err(CommandError::agent(
            AGENT_NOT_RUNNING,
            format!("会话 '{}' 没有 Agent 在运行", session_id),
        ));
    }

    // 标记为停止
    active.insert(session_id.clone(), false);
    log::info!("stop_agent 成功: 会话 '{}' 已标记为停止", session_id);
    Ok(())
}

/// 确认 Agent 操作
#[tauri::command]
pub async fn confirm_operation(
    session_id: String,
    operation_id: String,
    approved: bool,
    feedback: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("confirm_operation 请求: session_id={}, operation_id={}, approved={}", session_id, operation_id, approved);

    let sender = {
        let mut channels = state.confirm_channels.lock().await;
        channels.remove(&operation_id)
    };

    match sender {
        Some(tx) => {
            let decision = crate::ConfirmDecision {
                approved,
                feedback,
            };
            if tx.send(decision).is_err() {
                log::warn!("confirm_operation: 接收端已关闭, operation_id={}", operation_id);
                return Err(CommandError::agent(
                    AGENT_SESSION_NOT_FOUND,
                    "Agent 执行已结束，无法确认操作".to_string(),
                ));
            }
            log::info!("confirm_operation: 确认结果已发送, operation_id={}, approved={}", operation_id, approved);
            Ok(())
        }
        None => {
            log::error!("confirm_operation 失败: 未找到操作确认通道, operation_id={}", operation_id);
            Err(CommandError::agent(
                AGENT_SESSION_NOT_FOUND,
                format!("未找到操作确认通道: {}", operation_id),
            ))
        }
    }
}

/// 真正的 Agent 执行逻辑
/// 使用 AgentExecutor 执行 Tool Calling 循环
async fn run_agent(
    session_id: &str,
    prompt: &str,
    llm_router: &Arc<crate::services::llm::router::LlmRouter>,
    skill_registry: &Arc<tokio::sync::Mutex<crate::services::skill::registry::SkillRegistry>>,
    emitter: &AgentEmitter<tauri::Wry>,
    max_iterations: u32,
    workspace_path: &str,
    should_stop: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    db: &Arc<crate::db::Database>,
    confirm_channels: &Arc<tokio::sync::Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<crate::ConfirmDecision>>>>,
) -> Result<(), CommandError> {
    log::info!("run_agent 开始: session_id={}, workspace={}", session_id, workspace_path);

    if llm_router.is_empty() {
        let error_msg = "未配置 LLM Provider，请在设置中添加至少一个 Provider";
        log::error!("run_agent 失败: {}", error_msg);
        emitter.emit_error(crate::events::types::ErrorPayload {
            session_id: session_id.to_string(),
            code: 1002,
            message: error_msg.to_string(),
            recoverable: true,
        }).ok();
        return Err(CommandError::llm(1002, error_msg.to_string()));
    }

    let system_prompt = crate::services::agent::context::AgentContext::build_system_prompt(workspace_path);
    let mut ctx = AgentContext::new(session_id.to_string(), system_prompt);
    ctx.max_iterations = max_iterations;
    ctx.add_user_message(prompt);

    let executor = AgentExecutor::new(
        Arc::clone(llm_router),
        Arc::clone(skill_registry),
        emitter.clone(),
        Arc::clone(confirm_channels),
    )
    .with_stop_check(should_stop)
    .with_max_iterations(max_iterations);

    match executor.execute(&mut ctx).await {
        Ok(result) => {
            log::info!("Agent 执行成功: session_id={}, 摘要长度={}", session_id, result.summary.len());

            // 持久化消息到数据库
            if let Ok(conn) = db.conn() {
                for msg in &ctx.messages {
                    let msg_id = format!("msg_{}", uuid::Uuid::new_v4());
                    let (tool_name, tool_args, tool_result) = if let Some(tool_calls) = &msg.tool_calls {
                        if let Some(tc) = tool_calls.first() {
                            (Some(tc.name.as_str()), Some(tc.arguments.as_str()), None as Option<&str>)
                        } else {
                            (None, None, None)
                        }
                    } else if msg.role == "tool" {
                        (None, None, Some(msg.content.as_str()))
                    } else {
                        (None, None, None)
                    };

                    if let Err(e) = crate::db::message_repo::create_message(
                        &conn,
                        &msg_id,
                        session_id,
                        &msg.role,
                        &msg.content,
                        tool_name,
                        tool_args,
                        tool_result,
                        None,
                        0,
                        0,
                    ) {
                        log::warn!("消息持久化失败: session_id={}, 错误: {}", session_id, e.message);
                    }
                }
                log::info!("消息持久化完成: session_id={}, 消息数={}", session_id, ctx.messages.len());
            }

            // 发射 Token 用量更新事件
            emitter.emit_token_update(crate::events::types::TokenUpdatePayload {
                session_id: session_id.to_string(),
                provider_id: String::new(),
                prompt_tokens: result.total_input_tokens,
                completion_tokens: result.total_output_tokens,
                total_cost: 0.0,
            }).ok();

            Ok(())
        }
        Err(e) => {
            log::error!("Agent 执行失败: session_id={}, 错误: {}", session_id, e.message);
            Err(e)
        }
    }
}
