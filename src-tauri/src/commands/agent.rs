use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::errors::{CommandError, AGENT_ALREADY_RUNNING, AGENT_NOT_RUNNING, AGENT_SESSION_NOT_FOUND};
use crate::events::AgentEmitter;
use crate::models::llm::ChatMessage;
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

    // 检查是否已有 Agent 在该会话中运行，并注册为活跃 Agent（单次加锁避免 TOCTOU 竞态）
    {
        let mut active = state.active_agents.lock().await;
        if active.contains_key(&session_id) {
            log::error!("start_agent 失败: 会话 '{}' 已有 Agent 正在运行", session_id);
            return Err(CommandError::agent(
                AGENT_ALREADY_RUNNING,
                format!("会话 '{}' 已有 Agent 正在运行", session_id),
            ));
        }
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

    let workspace_id = options
        .as_ref()
        .and_then(|o| o.get("workspaceId"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let config = Arc::clone(&state.config);

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
            &workspace_id,
            should_stop,
            &db,
            &confirm_channels,
            &config,
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

/// 将消息列表持久化到数据库
/// 支持多 tool_calls：将所有 tool_calls 序列化为 JSON 数组存储
fn persist_messages_to_db(
    db: &Arc<crate::db::Database>,
    session_id: &str,
    messages: &[ChatMessage],
) -> Result<(), CommandError> {
    let conn = db.conn()?;
    for msg in messages {
        let msg_id = format!("msg_{}", uuid::Uuid::new_v4());

        // 对于包含 tool_calls 的消息，将所有 tool_calls 序列化为 JSON 存储
        // 修复原来只持久化第一个 tool_call 的问题
        let (tool_name, tool_args, tool_result) = if let Some(tool_calls) = &msg.tool_calls {
            if tool_calls.is_empty() {
                (None, None, None as Option<String>)
            } else if tool_calls.len() == 1 {
                // 单个 tool_call：保持原有格式
                let tc = &tool_calls[0];
                (Some(tc.name.clone()), Some(tc.arguments.clone()), None)
            } else {
                // 多个 tool_calls：将所有调用信息序列化为 JSON 数组
                let names: Vec<&str> = tool_calls.iter().map(|tc| tc.name.as_str()).collect();
                let args: Vec<&str> = tool_calls.iter().map(|tc| tc.arguments.as_str()).collect();
                (
                    Some(serde_json::to_string(&names).unwrap_or_default()),
                    Some(serde_json::to_string(&args).unwrap_or_default()),
                    None,
                )
            }
        } else if msg.role == "tool" {
            (None, None, Some(msg.content.clone()))
        } else {
            (None, None, None)
        };

        let tool_name_ref = tool_name.as_deref();
        let tool_args_ref = tool_args.as_deref();
        let tool_result_ref = tool_result.as_deref();

        crate::db::message_repo::create_message(
            &conn,
            &msg_id,
            session_id,
            &msg.role,
            &msg.content,
            tool_name_ref,
            tool_args_ref,
            tool_result_ref,
            None,
            0,
            0,
        )?;
    }
    Ok(())
}

/// 真正的 Agent 执行逻辑
/// 使用 AgentExecutor 执行 Tool Calling 循环
#[allow(clippy::too_many_arguments)]
async fn run_agent(
    session_id: &str,
    prompt: &str,
    llm_router: &Arc<crate::services::llm::router::LlmRouter>,
    skill_registry: &Arc<tokio::sync::Mutex<crate::services::skill::registry::SkillRegistry>>,
    emitter: &AgentEmitter<tauri::Wry>,
    max_iterations: u32,
    workspace_path: &str,
    workspace_id: &str,
    should_stop: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    db: &Arc<crate::db::Database>,
    confirm_channels: &Arc<tokio::sync::Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<crate::ConfirmDecision>>>>,
    config: &Arc<tokio::sync::Mutex<crate::config::ConfigManager>>,
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
    ctx.workspace_path = workspace_path.to_string();
    ctx.workspace_id = workspace_id.to_string();
    ctx.add_user_message(prompt);

    // 创建增量持久化回调，每轮迭代后自动持久化新增消息
    let db_for_persist = Arc::clone(db);
    #[allow(clippy::type_complexity)]
    let persist_fn: Arc<dyn Fn(&str, &[ChatMessage]) -> Result<(), CommandError> + Send + Sync> =
        Arc::new(move |sid: &str, messages: &[ChatMessage]| {
            persist_messages_to_db(&db_for_persist, sid, messages)
        });

    // 创建版本快照回调，在文件修改/删除前自动创建快照
    let db_for_snapshot = Arc::clone(db);
    let config_for_snapshot = Arc::clone(config);
    let workspace_path_for_snapshot = workspace_path.to_string();
    #[allow(clippy::type_complexity)]
    let snapshot_fn: Arc<dyn Fn(&str, &str, &str, &str) -> Result<(), CommandError> + Send + Sync> =
        Arc::new(move |wid: &str, sid: &str, file_path: &str, operation: &str| {
            create_version_snapshot(
                &db_for_snapshot,
                &config_for_snapshot,
                &workspace_path_for_snapshot,
                wid,
                sid,
                file_path,
                operation,
            )
        });

    let executor = AgentExecutor::new(
        Arc::clone(llm_router),
        Arc::clone(skill_registry),
        emitter.clone(),
        Arc::clone(confirm_channels),
    )
    .with_stop_check(should_stop)
    .with_max_iterations(max_iterations)
    .with_persist_fn(persist_fn)
    .with_snapshot_fn(snapshot_fn);

    match executor.execute(&mut ctx).await {
        Ok(result) => {
            log::info!("Agent 执行成功: session_id={}, 摘要长度={}", session_id, result.summary.len());

            // 持久化可能残留的未持久化消息（兜底保护）
            let unpersisted = ctx.get_unpersisted_messages();
            if !unpersisted.is_empty() {
                log::info!("持久化残留消息: session_id={}, 数量={}", session_id, unpersisted.len());
                if let Err(e) = persist_messages_to_db(db, session_id, unpersisted) {
                    log::warn!("残留消息持久化失败: session_id={}, 错误: {}", session_id, e.message);
                }
                ctx.mark_persisted();
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

            // 执行失败时也尝试持久化已有消息
            let unpersisted = ctx.get_unpersisted_messages();
            if !unpersisted.is_empty() {
                log::info!("执行失败后持久化已有消息: session_id={}, 数量={}", session_id, unpersisted.len());
                if let Err(persist_err) = persist_messages_to_db(db, session_id, unpersisted) {
                    log::warn!("失败后消息持久化失败: session_id={}, 错误: {}", session_id, persist_err.message);
                }
            }

            Err(e)
        }
    }
}

/// 创建版本快照
/// 在文件被修改/删除前，将当前文件复制到快照目录，并创建数据库记录
/// 同时根据保留策略清理过期快照
fn create_version_snapshot(
    db: &Arc<crate::db::Database>,
    config: &Arc<tokio::sync::Mutex<crate::config::ConfigManager>>,
    workspace_root: &str,
    workspace_id: &str,
    session_id: &str,
    file_path: &str,
    operation: &str,
) -> Result<(), CommandError> {
    // 解析文件绝对路径
    let abs_path = if std::path::Path::new(file_path).is_absolute() {
        file_path.to_string()
    } else {
        std::path::Path::new(workspace_root)
            .join(file_path)
            .to_string_lossy()
            .to_string()
    };

    let path = std::path::Path::new(&abs_path);

    // 文件不存在则无需创建快照（可能是新建文件）
    if !path.exists() || !path.is_file() {
        log::debug!("跳过快照创建: 文件不存在或不是文件, path={}", file_path);
        return Ok(());
    }

    // 获取应用数据目录，用于存储快照文件
    let snapshot_dir = {
        let cfg = config.blocking_lock();
        cfg.data_dir().join("snapshots")
    };

    // 确保快照目录存在
    std::fs::create_dir_all(&snapshot_dir)?;

    // 生成快照文件名：使用 UUID + 原始扩展名
    let snapshot_id = uuid::Uuid::new_v4().to_string();
    let extension = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let snapshot_file_name = format!("{}.{}", snapshot_id, extension);
    let snapshot_path = snapshot_dir.join(&snapshot_file_name);

    // 复制当前文件到快照目录
    std::fs::copy(&abs_path, &snapshot_path)?;

    log::info!(
        "版本快照文件已创建: file={}, snapshot={}, operation={}",
        file_path, snapshot_file_name, operation
    );

    // 在数据库中创建快照记录
    let conn = db.conn()?;
    crate::db::snapshot_repo::create_snapshot(
        &conn,
        &snapshot_id,
        workspace_id,
        session_id,
        file_path,
        &snapshot_path.to_string_lossy(),
        operation,
    )?;

    // 根据保留策略清理过期快照
    let (policy, max_count, max_days) = {
        let cfg = config.blocking_lock();
        match cfg.load_app_settings() {
            Ok(settings) => {
                let policy_str = match settings.version_snapshot.retention_policy {
                    crate::config::app_settings::RetentionPolicy::ByCount => "byCount",
                    crate::config::app_settings::RetentionPolicy::ByDays => "byDays",
                    crate::config::app_settings::RetentionPolicy::Both => "both",
                };
                (policy_str.to_string(), settings.version_snapshot.max_count, settings.version_snapshot.max_days)
            }
            Err(_) => ("byCount".to_string(), 50, 30)
        }
    };

    // 清理过期快照并删除对应的文件
    let deleted_ids = crate::db::snapshot_repo::cleanup_snapshots(
        &conn,
        workspace_id,
        file_path,
        &policy,
        max_count,
        max_days,
    );

    // 删除被清理快照对应的物理文件
    for id in &deleted_ids {
        // 查找快照文件路径（快照文件名格式为 <id>.<ext>）
        // 直接在 snapshots 目录下按 ID 前缀查找
        if let Ok(entries) = std::fs::read_dir(&snapshot_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&format!("{}.", id)) {
                    if let Err(e) = std::fs::remove_file(entry.path()) {
                        log::warn!("删除快照文件失败: {}, 错误: {}", name, e);
                    } else {
                        log::debug!("已删除过期快照文件: {}", name);
                    }
                    break;
                }
            }
        }
    }

    Ok(())
}
