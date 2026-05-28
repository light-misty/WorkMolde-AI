use std::collections::HashMap;
use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::db::session_repo;
use crate::errors::{CommandError, AGENT_ALREADY_RUNNING, AGENT_NOT_RUNNING, AGENT_SESSION_NOT_FOUND};
use crate::events::AgentEmitter;
use crate::events::types;
use crate::models::llm::{ChatMessage, ToolDefinition};
use crate::services::agent::context::AgentContext;
use crate::services::agent::executor::AgentExecutor;
use crate::services::llm::router::LlmRouter;
use crate::AppState;

/// Agent 清理守卫：在 Drop 时自动从 active_agents 移除记录
/// 防止因 panic 或意外退出导致会话残留的"僵尸"状态
struct AgentCleanupGuard {
    active_agents: Option<Arc<tokio::sync::Mutex<HashMap<String, bool>>>>,
    session_id: Option<String>,
}

impl AgentCleanupGuard {
    fn new(
        active_agents: Arc<tokio::sync::Mutex<HashMap<String, bool>>>,
        session_id: String,
    ) -> Self {
        Self {
            active_agents: Some(active_agents),
            session_id: Some(session_id),
        }
    }

    /// 标记为主动完成，跳过 Drop 中的清理（避免重复移除）
    fn disarm(&mut self) {
        self.active_agents = None;
        self.session_id = None;
    }
}

impl Drop for AgentCleanupGuard {
    fn drop(&mut self) {
        if let (Some(agents), Some(sid)) = (self.active_agents.take(), self.session_id.take()) {
            let mut guard = agents.blocking_lock();
            let was_running = guard.remove(&sid);
            if was_running.is_some() {
                log::info!("Agent 清理守卫: 已从活跃列表移除 session_id={}", sid);
            }
        }
    }
}

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
    let tool_registry = Arc::clone(&state.tool_registry);
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
        // 清理守卫：确保 active_agents 在 panic 时也被清理
        let mut _guard = AgentCleanupGuard::new(Arc::clone(&active_agents), sid.clone());

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
            &tool_registry,
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

        // 从 active_agents 中移除（如果正常执行到这里，守卫不再需要代劳）
        {
            let mut active = active_agents.lock().await;
            let was_running = active.remove(&sid);
            if was_running.is_none() {
                log::warn!("Agent 已从活跃列表移除: session_id={}", sid);
            } else {
                log::info!("Agent 已从活跃列表移除: session_id={}", sid);
            }
        }
        _guard.disarm();
    });

    // 自动生成会话标题（后台任务，不阻塞主流程）
    // 仅当会话标题为默认值（"新会话"开头）时才生成
    {
        let title_sid = session_id.clone();
        let title_prompt = prompt.clone();
        let title_db = Arc::clone(&state.db);
        let title_emitter = AgentEmitter::new(app_handle.clone());
        let title_llm_router = Arc::clone(&state.llm_router);

        tokio::spawn(async move {
            // 延迟2秒，避免与主Agent的首次LLM调用竞争
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            // 检查当前会话标题是否为默认标题，仅对默认标题的会话生成新标题
            let should_generate = match title_db.conn() {
                Ok(conn) => {
                    match session_repo::get_session(&conn, &title_sid) {
                        Ok(session) => session.title.starts_with("新会话"),
                        Err(_) => false,
                    }
                }
                Err(_) => false,
            };

            if !should_generate {
                log::debug!("会话标题已自定义，跳过自动生成: session_id={}", title_sid);
                return;
            }

            // 读取当前 LlmRouter 快照
            let router_snapshot = {
                let guard = title_llm_router.read().await;
                Arc::clone(&guard)
            };

            // 尝试使用LLM生成标题，失败时降级为规则生成
            let title = match generate_session_title(&title_prompt, &router_snapshot).await {
                Ok(t) => t,
                Err(e) => {
                    log::warn!("LLM生成标题失败，使用降级方案: {}", e.message);
                    generate_fallback_title(&title_prompt)
                }
            };

            // 更新数据库中的标题
            match title_db.conn() {
                Ok(conn) => {
                    if let Err(e) = session_repo::update_session_title(&conn, &title_sid, &title) {
                        log::warn!("更新会话标题失败: session_id={}, 错误: {}", title_sid, e.message);
                        return;
                    }
                }
                Err(e) => {
                    log::warn!("获取数据库连接失败: {}", e.message);
                    return;
                }
            }

            // 发射会话更新事件，通知前端标题已变更
            let _ = title_emitter.emit_session_updated(types::SessionUpdatePayload {
                session_id: title_sid.clone(),
                change_type: "title_updated".to_string(),
                data: Some(serde_json::json!({ "title": title })),
            });

            log::info!("会话标题已自动生成: session_id={}, title={}", title_sid, title);
        });
    }

    log::info!("start_agent 成功: session_id={}", session_id);
    Ok(())
}

/// 使用 LLM 自动生成会话标题
/// 根据用户的首条消息，调用 LLM 生成简短准确的标题
async fn generate_session_title(
    user_message: &str,
    llm_router: &Arc<LlmRouter>,
) -> Result<String, CommandError> {
    let system_prompt = "你是一个会话标题生成器。根据用户的消息，生成一个简短、准确的会话标题。要求：1) 不超过20个字；2) 直接输出标题文本；3) 不要加引号；4) 不要加任何额外说明；5) 不要使用emoji。";

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!("请为以下对话生成标题：\n\n{}", user_message),
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
        },
    ];

    let tools: Vec<ToolDefinition> = vec![];
    let response = llm_router.chat(&messages, &tools).await?;

    // 从响应中提取标题
    let title = response.choices
        .first()
        .map(|c| c.message.content.trim().to_string())
        .unwrap_or_default();

    if title.is_empty() {
        return Err(CommandError::agent(2001, "LLM 返回空标题".to_string()));
    }

    // 限制标题长度（最多50个字符）
    let title = if title.chars().count() > 50 {
        let truncated: String = title.chars().take(25).collect();
        format!("{}...", truncated)
    } else {
        title
    };

    Ok(title)
}

/// 降级方案：基于规则生成会话标题
/// 去除常见前缀后截取用户消息的前20个字符
fn generate_fallback_title(user_message: &str) -> String {
    // 去除常见中文前缀（长前缀优先匹配）
    let cleaned = user_message
        .trim()
        .trim_start_matches("能不能帮我")
        .trim_start_matches("可以帮我")
        .trim_start_matches("帮我")
        .trim_start_matches("能不能")
        .trim_start_matches("麻烦")
        .trim_start_matches("请")
        .trim();

    // 截取前20个字符
    if cleaned.chars().count() <= 20 {
        cleaned.to_string()
    } else {
        let truncated: String = cleaned.chars().take(20).collect();
        format!("{}...", truncated)
    }
}

/// 生成会话摘要并持久化到数据库（情景记忆）
/// 从 AgentContext 的执行记录中提取结构化摘要，纯规则无额外 LLM 调用
fn persist_session_summary(
    db: &Arc<crate::db::Database>,
    ctx: &AgentContext,
) -> Result<(), CommandError> {
    // 跳过空工作区ID的会话
    if ctx.workspace_id.is_empty() {
        log::debug!("工作区ID为空，跳过会话摘要持久化");
        return Ok(());
    }

    let (user_goal, result_summary, files_involved, tools_used, errors_resolved) =
        ctx.extract_session_summary_info();

    // 如果用户目标为空，说明没有有效对话，跳过摘要
    if user_goal.is_empty() {
        log::debug!("用户目标为空，跳过会话摘要持久化: session_id={}", ctx.session_id);
        return Ok(());
    }

    let summary_id = format!("summary_{}", uuid::Uuid::new_v4());
    let conn = db.conn()?;
    crate::db::session_summary_repo::create_session_summary(
        &conn,
        &summary_id,
        &ctx.session_id,
        &ctx.workspace_id,
        &user_goal,
        &result_summary,
        &files_involved,
        &tools_used,
        &errors_resolved,
    )?;

    log::info!(
        "会话摘要已持久化: session_id={}, summary_id={}, user_goal长度={}",
        ctx.session_id, summary_id, user_goal.len()
    );
    Ok(())
}

/// 从工具调用参数中提取用户偏好并持久化（语义记忆）
/// 纯规则提取，从工具调用参数中识别用户偏好模式
fn extract_and_persist_preferences(
    db: &Arc<crate::db::Database>,
    ctx: &AgentContext,
) -> Result<(), CommandError> {
    let conn = db.conn()?;

    for msg in &ctx.messages {
        if let Some(tool_calls) = &msg.tool_calls {
            for tc in tool_calls {
                if let Ok(params) = serde_json::from_str::<serde_json::Value>(&tc.arguments) {
                    // 从 generate_document 的 format 参数提取文档格式偏好
                    if tc.name == "generate_document" {
                        if let Some(format) = params["format"].as_str() {
                            let pref_id = format!("pref_{}", uuid::Uuid::new_v4());
                            crate::db::user_preference_repo::upsert_preference(
                                &conn,
                                &pref_id,
                                "format",
                                "preferred_document_format",
                                format,
                            )?;
                        }
                    }

                    // 从 convert_format 的 target_format 参数提取格式转换偏好
                    if tc.name == "convert_format" {
                        if let Some(target) = params["target_format"].as_str() {
                            let pref_id = format!("pref_{}", uuid::Uuid::new_v4());
                            crate::db::user_preference_repo::upsert_preference(
                                &conn,
                                &pref_id,
                                "format",
                                "preferred_target_format",
                                target,
                            )?;
                        }
                    }
                }
            }
        }
    }

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
            msg.reasoning_content.as_deref(),
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
    tool_registry: &Arc<crate::services::tool::registry::ToolRegistry>,
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

    // 根据用户首条消息识别任务类型，动态重建系统提示词
    let task_type = crate::services::agent::prompts::task_type::TaskType::from_user_message(prompt);
    let tool_count = tool_registry.list_tools().len();
    let skill_count = {
        let reg = skill_registry.lock().await;
        reg.list_skills().len()
    };
    let dynamic_prompt = AgentContext::build_system_prompt_with_task(
        workspace_path,
        &task_type,
        tool_count,
        skill_count,
    );
    ctx.system_prompt = dynamic_prompt;
    log::info!("任务类型: {:?}, 系统提示词已动态构建", task_type);

    // 从数据库加载该会话的历史消息，使 Agent 能感知之前的对话内容
    let history_messages = {
        match db.conn() {
            Ok(conn) => {
                let db_messages = crate::db::message_repo::list_messages(&conn, session_id);
                db_messages.into_iter()
                    .filter_map(|m| m.to_chat_message())
                    .collect::<Vec<ChatMessage>>()
            }
            Err(e) => {
                log::warn!("获取数据库连接失败，无法加载历史消息: {}, 将以空上下文启动", e.message);
                Vec::new()
            }
        }
    };

    // 注入历史消息到上下文（在添加当前用户消息之前）
    if !history_messages.is_empty() {
        log::info!("加载历史消息: session_id={}, 历史消息数={}", session_id, history_messages.len());
        ctx.load_history_messages(history_messages);
    }

    // 加载同工作区的历史会话摘要（情景记忆）和用户偏好（语义记忆）
    let historical_summaries_text = {
        match db.conn() {
            Ok(conn) => {
                let summaries = crate::db::session_summary_repo::list_summaries_by_workspace(
                    &conn, workspace_id, 3,
                );
                if summaries.is_empty() {
                    String::new()
                } else {
                    let text = summaries.iter()
                        .map(|s| {
                            let files = s.get_files_involved();
                            format!(
                                "- 用户目标: {} | 结果: {} | 涉及文件: {:?}",
                                s.user_goal, s.result_summary, files
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("\n<historical_context>\n## 近期历史会话摘要\n{}\n</historical_context>", text)
                }
            }
            Err(_) => String::new(),
        }
    };

    // 加载高置信度用户偏好（语义记忆）
    let user_preferences_text = {
        match db.conn() {
            Ok(conn) => {
                let prefs = crate::db::user_preference_repo::list_high_confidence_preferences(
                    &conn, 0.7,
                );
                if prefs.is_empty() {
                    String::new()
                } else {
                    let text = prefs.iter()
                        .map(|p| format!("- {}: {}", p.key, p.value))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("\n<user_preferences>\n## 用户偏好\n{}\n</user_preferences>", text)
                }
            }
            Err(_) => String::new(),
        }
    };

    // 将历史摘要和用户偏好追加到系统提示词
    if !historical_summaries_text.is_empty() || !user_preferences_text.is_empty() {
        let mut prompt_extension = String::new();
        if !historical_summaries_text.is_empty() {
            prompt_extension.push_str(&historical_summaries_text);
        }
        if !user_preferences_text.is_empty() {
            prompt_extension.push_str(&user_preferences_text);
        }
        ctx.system_prompt = format!("{}{}", ctx.system_prompt, prompt_extension);
        log::info!("已注入历史摘要和用户偏好到系统提示词, session_id={}", session_id);
    }

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
        Arc::clone(tool_registry),
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

            // 生成会话摘要并持久化（情景记忆）
            if let Err(e) = persist_session_summary(db, &ctx) {
                log::warn!("会话摘要持久化失败: session_id={}, 错误: {}", session_id, e.message);
            }

            // 从工具调用参数中提取用户偏好并持久化（语义记忆）
            if let Err(e) = extract_and_persist_preferences(db, &ctx) {
                log::warn!("用户偏好提取失败: session_id={}, 错误: {}", session_id, e.message);
            }

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
