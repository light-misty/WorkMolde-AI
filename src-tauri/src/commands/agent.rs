use std::collections::HashMap;
use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::db::session_repo;
use crate::db::sub_agent_message_repo;
use crate::errors::{
    CommandError, AGENT_ALREADY_RUNNING, AGENT_NOT_RUNNING, AGENT_OPERATION_REJECTED,
    AGENT_SESSION_NOT_FOUND,
};
use crate::events::types;
use crate::events::AgentEmitter;
use crate::models::llm::{ChatMessage, ContentPart, ToolDefinition};
use crate::models::message::AttachmentMeta;
use crate::models::message::Message;
use crate::services::agent::context::{AgentContext, AgentMode};
use crate::services::agent::executor::{AgentExecutor, ContextUsagePersistFn};
use crate::services::attachment::AttachmentService;
use crate::services::llm::router::LlmRouter;
use crate::services::tool::builtin::question::QuestionAnswer;
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
            // 在 Drop 中调用 blocking_lock 需要 block_in_place
            // 避免 "Cannot block the current thread from within a runtime" panic
            let mut guard = tokio::task::block_in_place(|| agents.blocking_lock());
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
    log::info!(
        "start_agent 请求: session_id={}, prompt长度={}",
        session_id,
        prompt.len()
    );

    // 检查是否已有 Agent 在该会话中运行，并注册为活跃 Agent（单次加锁避免 TOCTOU 竞态）
    {
        let mut active = state.active_agents.lock().await;
        if active.contains_key(&session_id) {
            log::error!(
                "start_agent 失败: 会话 '{}' 已有 Agent 正在运行",
                session_id
            );
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
    let handler_registry = Arc::clone(&state.handler_registry);
    let active_agents = Arc::clone(&state.active_agents);
    let db = Arc::clone(&state.db);
    let confirm_channels = Arc::clone(&state.confirm_channels);
    let scratchpad_states = Arc::clone(&state.scratchpad_states);
    // 权限系统组件
    let permission_channels = Arc::clone(&state.permission_channels);
    let permission_registry = Arc::clone(&state.permission_registry);
    let doom_loop_detector = Arc::clone(&state.doom_loop_detector);
    let agent_mode_manager = Arc::clone(&state.agent_mode_manager);
    let skill_registry = Arc::clone(&state.skill_registry);

    let max_iterations = options
        .as_ref()
        .and_then(|o| o.get("maxIterations"))
        .and_then(|v| v.as_u64())
        .unwrap_or(100) as u32;

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

    // 从 options 中提取用户首选 Provider ID（空字符串表示未指定）
    let provider_id = options
        .as_ref()
        .and_then(|o| o.get("providerId"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // 从 options 提取 Agent 模式（待机状态切换模式后需通过此参数同步到后端）
    let agent_mode_str = options
        .as_ref()
        .and_then(|o| o.get("agentMode"))
        .and_then(|v| v.as_str())
        .unwrap_or("build")
        .to_string();

    let agent_mode = match agent_mode_str.as_str() {
        "plan" => AgentMode::Plan,
        "build" => AgentMode::Build,
        "document" => AgentMode::Document,
        other => {
            log::warn!("未知的 agentMode: {}，使用默认 Build", other);
            AgentMode::Build
        }
    };

    // 校验工作区目录是否存在（仅当指定了非默认工作区路径时检查）
    if !workspace_path.is_empty() && workspace_path != "." {
        let ws_path = std::path::Path::new(&workspace_path);
        if !ws_path.exists() || !ws_path.is_dir() {
            log::error!("start_agent 失败: 工作区目录已被删除: {}", workspace_path);
            // 从 active_agents 中移除注册
            {
                let mut active = state.active_agents.lock().await;
                active.remove(&session_id);
            }
            return Err(CommandError::fs(
                crate::errors::FS_PATH_NOT_FOUND,
                format!(
                    "工作区目录已被删除: {}，请移除该工作区后重新选择",
                    workspace_path
                ),
            ));
        }
    }

    // 从 options 中提取附件列表
    let attachments: Vec<AttachmentMeta> = options
        .as_ref()
        .and_then(|o| o.get("attachments"))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let config = Arc::clone(&state.config);
    let doc_service = Arc::clone(&state.doc_service);

    // 同步前端传入的 Agent 模式，确保待机状态切换后新会话使用正确模式
    state.agent_mode_manager.set_mode(&session_id, agent_mode).await;
    log::info!(
        "start_agent: 会话 '{}' Agent 模式已设置为 {:?}",
        session_id, agent_mode
    );

    tokio::spawn(async move {
        // 清理守卫：确保 active_agents 在 panic 时也被清理
        let mut _guard = AgentCleanupGuard::new(Arc::clone(&active_agents), sid.clone());

        let active_agents_for_check = Arc::clone(&active_agents);

        let should_stop: Arc<dyn Fn(&str) -> bool + Send + Sync> = Arc::new(
            move |session_id: &str| match active_agents_for_check.try_lock() {
                Ok(guard) => !guard.get(session_id).copied().unwrap_or(false),
                Err(_) => false,
            },
        );

        // 读取当前 LlmRouter 快照
        let router_snapshot = {
            let guard = llm_router.read().await;
            Arc::clone(&guard)
        };

        let result = run_agent(
            &sid,
            &prompt_clone,
            &attachments,
            &router_snapshot,
            &tool_registry,
            &handler_registry,
            &emitter,
            max_iterations,
            &workspace_path,
            &workspace_id,
            &provider_id,
            should_stop,
            &db,
            &confirm_channels,
            &config,
            &doc_service,
            &scratchpad_states,
            // 权限系统组件
            &permission_channels,
            &permission_registry,
            &doom_loop_detector,
            &agent_mode_manager,
            &skill_registry,
        )
        .await;

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
                Ok(conn) => match session_repo::get_session(&conn, &title_sid) {
                    Ok(session) => {
                        session.title.starts_with("新会话") || session.title.starts_with("New Session")
                    },
                    Err(_) => false,
                },
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
                        log::warn!(
                            "更新会话标题失败: session_id={}, 错误: {}",
                            title_sid,
                            e.message
                        );
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

            log::info!(
                "会话标题已自动生成: session_id={}, title={}",
                title_sid,
                title
            );
        });
    }

    log::info!("start_agent 成功: session_id={}", session_id);
    Ok(())
}

/// 获取当前上下文窗口使用信息
/// 需要传入 session_id，返回该会话的上下文窗口使用情况
/// 优先从数据库读取持久化的 JSON（与实时事件数据完全一致），若无则回退到重新计算
#[tauri::command]
pub async fn get_context_usage(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<crate::models::llm::ContextUsageInfo, CommandError> {
    // 优先从数据库读取持久化的上下文窗口使用信息（与实时事件数据完全一致）
    if let Ok(conn) = state.db.conn() {
        if let Some(usage) = crate::db::session_repo::load_context_usage(&conn, &session_id) {
            return Ok(usage);
        }
    }

    // 回退：数据库中无持久化数据时，重新计算（首次加载或旧版本数据库）
    use crate::services::agent::context::AgentContext;
    use crate::services::agent::prompts::task_type::TaskType;
    use crate::services::agent::prompts::token_budget::TokenBudgetManager;

    // 获取当前活跃 Provider 的上下文窗口大小、模型名称和缓存类型
    // 通过 Router 的主 Provider ID 精确查找（避免 list_providers 顺序不确定）
    let (context_window, model_name, provider_cache_type) = {
        let router = state.llm_router.read().await;
        let providers = router.list_providers();
        let main_provider_id = router.default_provider_id();
        let main_provider = main_provider_id
            .and_then(|id| providers.iter().find(|p| p.id == id))
            .or_else(|| providers.first());
        let cache_type = router.current_cache_type();
        match main_provider {
            Some(p) => (p.context_window, p.model.clone(), cache_type.to_string()),
            None => (128_000, String::new(), cache_type.to_string()),
        }
    };

    // 获取当前工作区路径（用于构建系统提示词）
    let workspace_path = {
        let cfg_manager = state.config.lock().await;
        let settings = cfg_manager.load_app_settings().ok();
        let ws_config = cfg_manager.load_workspaces().ok();
        settings
            .as_ref()
            .and_then(|s| {
                ws_config
                    .as_ref()?
                    .workspaces
                    .iter()
                    .find(|w| w.id == s.workspace.default_workspace_id)
            })
            .map(|ws| ws.path.clone())
            .unwrap_or_else(|| ".".to_string())
    };

    // 使用与 Agent 运行时相同的方法构建系统提示词并估算 Token 数
    let tool_count = state.tool_registry.tool_definitions().len();
    let handler_count = {
        let reg = state.handler_registry.lock().await;
        reg.tool_definitions().len()
    };
    let budget = TokenBudgetManager::new(context_window);
    // 检测执行环境信息，注入系统提示词避免智能体浪费迭代搜索环境
    let git_bash_path = state
        .config
        .lock()
        .await
        .load_app_settings()
        .ok()
        .map(|s| s.git_bash_path)
        .unwrap_or_default();
    let env_info = crate::services::agent::context::EnvironmentInfo::detect(&git_bash_path);
    let system_prompt = AgentContext::build_system_prompt_with_task(
        &workspace_path,
        &TaskType::Unknown,
        tool_count,
        handler_count,
        &budget,
        None,
        &env_info,
        None,              // agents_md_content（T1.07 会更新）
        &AgentMode::Build, // agent_mode
    );
    let system_prompt_tokens = TokenBudgetManager::estimate_tokens(&system_prompt);

    // 估算工具定义 Token 数（与 executor 中的计算方式一致）
    let function_definitions_tokens = {
        let tool_defs = state.tool_registry.tool_definitions();
        let handler_defs = {
            let reg = state.handler_registry.lock().await;
            reg.tool_definitions()
        };
        let all_defs = [tool_defs, handler_defs].concat();
        let defs_str = all_defs
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        TokenBudgetManager::estimate_tokens(&defs_str)
    };

    // 从数据库加载该会话的消息来估算对话历史 Token 数
    let (conversation_tokens, total_message_count) = {
        match state.db.conn() {
            Ok(conn) => {
                let messages = crate::db::message_repo::list_messages(&conn, &session_id);
                let count = messages.len();
                let conv_tokens = TokenBudgetManager::estimate_tokens(
                    &messages
                        .iter()
                        .map(|m| m.content.as_str())
                        .collect::<String>(),
                );
                (conv_tokens, count)
            }
            Err(_) => (0, 0),
        }
    };

    let total_used_tokens =
        system_prompt_tokens + function_definitions_tokens + conversation_tokens;

    Ok(crate::models::llm::ContextUsageInfo {
        context_window,
        system_prompt_tokens,
        function_definitions_tokens,
        conversation_tokens,
        response_tokens: 0,
        total_used_tokens,
        model_name,
        total_message_count,
        cache_hit_tokens: 0,
        cache_miss_tokens: 0,
        cache_creation_tokens: 0,
        lifetime_cache_hit_tokens: 0,
        lifetime_cache_miss_tokens: 0,
        cache_hit_rate: 0.0,
        provider_cache_type,
    })
}

/// 使用 LLM 自动生成会话标题
/// 根据用户的首条消息，调用 LLM 生成简短准确的标题
async fn generate_session_title(
    user_message: &str,
    llm_router: &Arc<LlmRouter>,
) -> Result<String, CommandError> {
    // 系统提示词包含标题生成的完整指令，严格区分系统消息与用户消息
    let system_prompt = "You are a session title generator. Generate a short, accurate session title based on the user's message. Rules:\n\
        1) No more than 20 characters\n\
        2) Output the title text directly without quotes or extra explanation\n\
        3) Use the same language as the user's message (e.g., use Chinese if the user writes in Chinese)\n\
        4) Do not use emoji";

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            attachments: None,
            metadata: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!("<user_message>\n{}\n</user_message>", user_message),
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            attachments: None,
            metadata: None,
        },
    ];

    let tools: Vec<ToolDefinition> = vec![];
    let response = llm_router.chat(&messages, &tools).await?;

    // 从响应中提取标题
    let title = response
        .choices
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
        log::debug!(
            "用户目标为空，跳过会话摘要持久化: session_id={}",
            ctx.session_id
        );
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
        ctx.session_id,
        summary_id,
        user_goal.len()
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
                    // 从文档 Handler 的参数提取文档格式偏好
                    if tc.name == "docx_handler"
                        || tc.name == "xlsx_handler"
                        || tc.name == "pptx_handler"
                        || tc.name == "pdf_handler"
                    {
                        if let Some(action) = params["action"].as_str() {
                            let pref_id = format!("pref_{}", uuid::Uuid::new_v4());
                            let format_name = tc.name.replace("_handler", "");
                            crate::db::user_preference_repo::upsert_preference(
                                &conn,
                                &pref_id,
                                "format",
                                "preferred_document_format",
                                &format_name,
                            )?;
                            // 转换操作时提取目标格式偏好
                            if action == "convert" {
                                if let Some(target) = params["target_format"].as_str() {
                                    let pref_id2 = format!("pref_{}", uuid::Uuid::new_v4());
                                    crate::db::user_preference_repo::upsert_preference(
                                        &conn,
                                        &pref_id2,
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
        }
    }

    Ok(())
}

/// 检查指定会话的 Agent 是否正在运行
#[tauri::command]
pub async fn is_agent_running(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<bool, CommandError> {
    let active = state.active_agents.lock().await;
    Ok(active.contains_key(&session_id))
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
    log::info!(
        "confirm_operation 请求: session_id={}, operation_id={}, approved={}",
        session_id,
        operation_id,
        approved
    );

    let sender = {
        let mut channels = state.confirm_channels.lock().await;
        channels.remove(&operation_id)
    };

    match sender {
        Some(tx) => {
            let decision = crate::ConfirmDecision { approved, feedback };
            if tx.send(decision).is_err() {
                log::warn!(
                    "confirm_operation: 接收端已关闭, operation_id={}",
                    operation_id
                );
                return Err(CommandError::agent(
                    AGENT_SESSION_NOT_FOUND,
                    "Agent 执行已结束，无法确认操作".to_string(),
                ));
            }
            log::info!(
                "confirm_operation: 确认结果已发送, operation_id={}, approved={}",
                operation_id,
                approved
            );
            Ok(())
        }
        None => {
            log::error!(
                "confirm_operation 失败: 未找到操作确认通道, operation_id={}",
                operation_id
            );
            Err(CommandError::agent(
                AGENT_SESSION_NOT_FOUND,
                format!("未找到操作确认通道: {}", operation_id),
            ))
        }
    }
}

/// 权限审批回复命令（双态 once/reject）
/// 优先查找 permission_channels（双态权限通道）
/// 未命中时回退到 confirm_channels（兼容旧版 confirm_operation 调用）
#[tauri::command]
pub async fn permission_respond(
    session_id: String,
    operation_id: String,
    response: String,
    feedback: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "permission_respond 请求: session_id={}, operation_id={}, response={}",
        session_id,
        operation_id,
        response
    );

    // 解析用户回复字符串为 PermissionResponse
    let perm_response = crate::services::permission::types::PermissionResponse::from_str(&response)
        .ok_or_else(|| {
            CommandError::agent(
                AGENT_OPERATION_REJECTED,
                format!("无效的权限回复: {}", response),
            )
        })?;

    // 优先查找 permission_channels（双态权限通道）
    let perm_sender = {
        let mut channels = state.permission_channels.lock().await;
        channels.remove(&operation_id)
    };

    if let Some(tx) = perm_sender {
        // 权限审批通道命中，发送 PermissionDecision
        let decision = crate::PermissionDecision {
            response: perm_response,
            feedback: feedback.clone(),
        };
        if tx.send(decision).is_err() {
            log::warn!(
                "permission_respond: 接收端已关闭, operation_id={}",
                operation_id
            );
            return Err(CommandError::agent(
                AGENT_SESSION_NOT_FOUND,
                "Agent 执行已结束，无法回复权限审批".to_string(),
            ));
        }
        log::info!(
            "permission_respond: 权限回复已发送, operation_id={}, response={}",
            operation_id,
            response
        );
        return Ok(());
    }

    // 回退到 confirm_channels（兼容旧版 confirm_operation 机制）
    log::info!(
        "permission_respond: permission_channels 未命中, 回退到 confirm_channels, operation_id={}",
        operation_id
    );
    let confirm_sender = {
        let mut channels = state.confirm_channels.lock().await;
        channels.remove(&operation_id)
    };

    match confirm_sender {
        Some(tx) => {
            // 将 PermissionResponse 转换为旧 ConfirmDecision
            // Reject → approved=false, Once → approved=true
            let approved = !matches!(
                perm_response,
                crate::services::permission::types::PermissionResponse::Reject
            );
            let decision = crate::ConfirmDecision { approved, feedback };
            if tx.send(decision).is_err() {
                log::warn!(
                    "permission_respond: 接收端已关闭, operation_id={}",
                    operation_id
                );
                return Err(CommandError::agent(
                    AGENT_SESSION_NOT_FOUND,
                    "Agent 执行已结束，无法确认操作".to_string(),
                ));
            }
            log::info!(
                "permission_respond: 兼容模式确认结果已发送, operation_id={}, approved={}",
                operation_id,
                approved
            );
            Ok(())
        }
        None => {
            log::error!(
                "permission_respond 失败: 未找到权限审批通道, operation_id={}",
                operation_id
            );
            Err(CommandError::agent(
                AGENT_SESSION_NOT_FOUND,
                format!("未找到权限审批通道: {}", operation_id),
            ))
        }
    }
}

/// 切换 Agent 模式（Plan/Build/Document）
/// 由前端按钮触发，调用 AgentModeManager 切换会话模式
/// 模式切换仅通过前端按钮实现，不提供让 LLM 自主切换的工具
#[tauri::command]
pub async fn switch_agent_mode(
    session_id: String,
    mode: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "switch_agent_mode 请求: session_id={}, mode={}",
        session_id,
        mode
    );

    // 将字符串参数解析为 AgentMode 枚举
    let agent_mode = match mode.as_str() {
        "plan" => AgentMode::Plan,
        "build" => AgentMode::Build,
        "document" => AgentMode::Document,
        _ => {
            return Err(CommandError::agent(
                AGENT_OPERATION_REJECTED,
                format!("无效的 Agent 模式: {}", mode),
            ));
        }
    };

    // 调用 AgentModeManager 切换会话模式
    state
        .agent_mode_manager
        .set_mode(&session_id, agent_mode)
        .await;
    log::info!(
        "switch_agent_mode 成功: session_id={}, mode={:?}",
        session_id,
        agent_mode
    );
    Ok(())
}

/// 提交 question 工具的用户答案
/// 前端用户回答问题后调用此命令，通过 question_id 找到 oneshot Sender 发送答案
#[tauri::command]
pub async fn submit_question_answer(
    question_id: String,
    answers: Vec<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "submit_question_answer 请求: question_id={}, answers_count={}",
        question_id,
        answers.len()
    );

    // 从 question_channels 取出 Sender
    let sender = {
        let mut channels = state.question_channels.lock().await;
        channels.remove(&question_id)
    };

    match sender {
        Some(tx) => {
            // 构造 QuestionAnswer
            let answer = QuestionAnswer {
                question_id: question_id.clone(),
                answers: answers
                    .iter()
                    .filter_map(|a| {
                        Some(
                            crate::services::tool::builtin::question::QuestionItemAnswer {
                                question_index: a.get("questionIndex")?.as_u64()? as usize,
                                selected_options: a
                                    .get("selectedOptions")?
                                    .as_array()?
                                    .iter()
                                    .filter_map(|o| o.as_str().map(|s| s.to_string()))
                                    .collect(),
                            },
                        )
                    })
                    .collect(),
            };

            if tx.send(answer).is_err() {
                log::warn!(
                    "submit_question_answer: 接收端已关闭, question_id={}",
                    question_id
                );
                return Err(CommandError::agent(
                    AGENT_SESSION_NOT_FOUND,
                    "Agent 执行已结束，无法提交答案".to_string(),
                ));
            }
            log::info!(
                "submit_question_answer: 答案已发送, question_id={}",
                question_id
            );
            Ok(())
        }
        None => {
            log::error!(
                "submit_question_answer 失败: 未找到问题通道, question_id={}",
                question_id
            );
            Err(CommandError::agent(
                AGENT_SESSION_NOT_FOUND,
                format!("未找到问题通道: {}", question_id),
            ))
        }
    }
}

/// 查询指定子 Agent 的所有持久化消息
#[tauri::command]
pub async fn list_sub_agent_messages(
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Message>, CommandError> {
    let conn = state.db.conn()?;
    sub_agent_message_repo::list_sub_agent_messages(&conn, &agent_id)
        .map_err(|e| CommandError::db(4001, format!("查询子 Agent 消息失败: {}", e)))
}

/// 将消息列表持久化到数据库
/// 支持多 tool_calls：将所有 tool_calls 序列化为 JSON 数组存储
/// assistant 消息的 tool_call_id 字段存储所有 tool_call 的 id 列表（JSON 数组）
/// tool 消息的 tool_call_id 字段存储对应的单个 tool_call_id
fn persist_messages_to_db(
    db: &Arc<crate::db::Database>,
    session_id: &str,
    messages: &[ChatMessage],
) -> Result<(), CommandError> {
    let conn = db.conn()?;
    for msg in messages {
        let msg_id = format!("msg_{}", uuid::Uuid::new_v4());

        // 对于包含 tool_calls 的消息，将所有 tool_calls 序列化为 JSON 存储
        let (tool_name, tool_args, tool_result, tool_call_id) = if let Some(tool_calls) =
            &msg.tool_calls
        {
            if tool_calls.is_empty() {
                (None, None, None as Option<String>, None as Option<String>)
            } else if tool_calls.len() == 1 {
                // 单个 tool_call：保持原有格式，同时保存 tool_call id
                let tc = &tool_calls[0];
                (
                    Some(tc.name.clone()),
                    Some(tc.arguments.clone()),
                    None,
                    Some(tc.id.clone()),
                )
            } else {
                // 多个 tool_calls：将所有调用信息序列化为 JSON 数组
                let ids: Vec<&str> = tool_calls.iter().map(|tc| tc.id.as_str()).collect();
                let names: Vec<&str> = tool_calls.iter().map(|tc| tc.name.as_str()).collect();
                let args: Vec<&str> = tool_calls.iter().map(|tc| tc.arguments.as_str()).collect();
                (
                    Some(serde_json::to_string(&names).unwrap_or_default()),
                    Some(serde_json::to_string(&args).unwrap_or_default()),
                    None,
                    Some(serde_json::to_string(&ids).unwrap_or_default()),
                )
            }
        } else if msg.role == "tool" {
            // tool 消息：保存 tool_call_id 以确保历史消息加载时能正确匹配
            (
                None,
                None,
                Some(msg.content.clone()),
                msg.tool_call_id.clone(),
            )
        } else {
            (None, None, None, None)
        };

        let tool_name_ref = tool_name.as_deref();
        let tool_args_ref = tool_args.as_deref();
        let tool_result_ref = tool_result.as_deref();
        let tool_call_id_ref = tool_call_id.as_deref();

        // 将 metadata 序列化为 JSON 字符串存储
        let metadata_str = msg
            .metadata
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_default());

        crate::db::message_repo::create_message(
            &conn,
            &msg_id,
            session_id,
            &msg.role,
            &msg.content,
            tool_name_ref,
            tool_args_ref,
            tool_result_ref,
            tool_call_id_ref,
            None,
            msg.reasoning_content.as_deref(),
            msg.attachments.as_deref(),
            metadata_str.as_deref(),
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
    attachments: &[AttachmentMeta],
    llm_router: &Arc<crate::services::llm::router::LlmRouter>,
    tool_registry: &Arc<crate::services::tool::registry::ToolRegistry>,
    handler_registry: &Arc<tokio::sync::Mutex<crate::services::handler::registry::HandlerRegistry>>,
    emitter: &AgentEmitter<tauri::Wry>,
    max_iterations: u32,
    workspace_path: &str,
    workspace_id: &str,
    provider_id: &str,
    should_stop: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    db: &Arc<crate::db::Database>,
    confirm_channels: &Arc<
        tokio::sync::Mutex<
            std::collections::HashMap<String, tokio::sync::oneshot::Sender<crate::ConfirmDecision>>,
        >,
    >,
    config: &Arc<tokio::sync::Mutex<crate::config::ConfigManager>>,
    doc_service: &Arc<crate::services::document::DocumentService>,
    scratchpad_states: &crate::services::tool::builtin::SharedScratchpadStates,
    // 权限系统组件
    permission_channels: &Arc<
        tokio::sync::Mutex<
            std::collections::HashMap<
                String,
                tokio::sync::oneshot::Sender<crate::PermissionDecision>,
            >,
        >,
    >,
    permission_registry: &Arc<crate::services::permission::registry::PermissionRegistry>,
    doom_loop_detector: &Arc<crate::services::permission::doom_loop::DoomLoopDetector>,
    agent_mode_manager: &Arc<crate::services::agent::AgentModeManager>,
    skill_registry: &Arc<crate::services::skill::registry::SkillRegistry>,
) -> Result<(), CommandError> {
    log::info!(
        "run_agent 开始: session_id={}, workspace={}",
        session_id,
        workspace_path
    );

    if llm_router.is_empty().await {
        let error_msg = "未配置 LLM Provider，请在设置中添加至少一个 Provider";
        log::error!("run_agent 失败: {}", error_msg);
        emitter
            .emit_error(crate::events::types::ErrorPayload {
                session_id: session_id.to_string(),
                code: 1002,
                message: error_msg.to_string(),
                recoverable: true,
            })
            .ok();
        return Err(CommandError::llm(1002, error_msg.to_string()));
    }

    // 从配置中解析作者信息（工作区覆盖优先于全局设置）
    let author_info = {
        let cfg = tokio::task::block_in_place(|| config.blocking_lock());
        let app_settings = cfg.load_app_settings().ok();
        let ws_config = cfg.load_workspaces().ok();
        app_settings.and_then(|settings| {
            let ws_entry = ws_config
                .as_ref()
                .and_then(|wc| wc.workspaces.iter().find(|w| w.id == workspace_id));
            let info = crate::services::agent::context::AuthorInfo::resolve(&settings, ws_entry);
            if info.has_any() {
                log::info!(
                    "已解析作者信息: name={}, email={}, company={}",
                    info.name,
                    info.email,
                    info.company
                );
                Some(info)
            } else {
                None
            }
        })
    };

    // 从配置中读取操作确认级别
    let confirmation_level = {
        let cfg = tokio::task::block_in_place(|| config.blocking_lock());
        cfg.load_app_settings()
            .map(|s| s.general.confirmation_level.clone())
            .unwrap_or_default()
    };
    log::info!("操作确认级别: {:?}", confirmation_level);

    // 从配置中读取上下文压缩配置
    let compaction_config = {
        let cfg = tokio::task::block_in_place(|| config.blocking_lock());
        cfg.load_app_settings()
            .map(|s| s.general.compaction.clone())
            .unwrap_or_default()
    };
    log::info!(
        "上下文压缩配置: enabled={}, trigger_threshold={}, keep_recent={}",
        compaction_config.enabled,
        compaction_config.trigger_threshold,
        compaction_config.keep_recent_messages
    );

    // 从当前活跃 Provider 解析上下文窗口大小
    let context_window = {
        let cfg = tokio::task::block_in_place(|| config.blocking_lock());
        match cfg.load_llm_config() {
            Ok(llm_config) => match llm_config.providers.first() {
                Some(provider) => {
                    let cw = provider.resolve_context_window();
                    log::info!(
                        "从主 Provider 解析上下文窗口: {} tokens (模型: {})",
                        cw,
                        provider.model
                    );
                    cw
                }
                None => {
                    log::warn!("无可用 Provider，使用默认上下文窗口 128K");
                    128_000
                }
            },
            Err(e) => {
                log::warn!("加载 LLM 配置失败: {}, 使用默认上下文窗口 128K", e.message);
                128_000
            }
        }
    };

    let mut ctx = AgentContext::new(
        session_id.to_string(),
        crate::services::agent::context::AgentContext::build_system_prompt(workspace_path),
        context_window,
    );
    ctx.max_iterations = max_iterations;
    ctx.workspace_path = workspace_path.to_string();
    ctx.workspace_id = workspace_id.to_string();
    ctx.preferred_provider_id = provider_id.to_string();
    // 注入 Scratchpad 共享状态（与 ScratchpadTool 持有同一 Arc）
    // executor 每轮迭代开始时会调用 refresh_scratchpad_summary 读取笔记摘要
    ctx.set_scratchpad_states(scratchpad_states.clone());
    // 注入 Skill 注册表（executor 在首次 LLM 调用前将 Skill 清单追加到系统提示词）
    ctx.set_skill_registry(Arc::clone(skill_registry));
    // T3.13: 注入数据库连接（executor 每轮迭代从数据库读取 TodoList 并追加摘要到系统提示词）
    ctx.set_db(Arc::clone(db));

    // 根据用户首条消息识别任务类型，动态重建系统提示词
    let task_type = crate::services::agent::prompts::task_type::TaskType::from_user_message(prompt);
    let tool_count = tool_registry.list_tools().len();
    let handler_count = {
        let reg = handler_registry.lock().await;
        reg.list_handlers().len()
    };
    // 检测执行环境信息（Python路径、Git Bash路径、OS等），注入系统提示词
    // 避免智能体浪费迭代次数搜索 Python 路径等环境信息
    let git_bash_path = {
        let cfg = tokio::task::block_in_place(|| config.blocking_lock());
        cfg.load_app_settings()
            .map(|s| s.git_bash_path)
            .unwrap_or_default()
    };
    let env_info = crate::services::agent::context::EnvironmentInfo::detect(&git_bash_path);

    // 加载 AGENTS.md 自定义规则
    let agents_md_content = {
        let config_dir = {
            let cfg = tokio::task::block_in_place(|| config.blocking_lock());
            cfg.data_dir().to_path_buf()
        };
        let agents_md = crate::services::agent::prompts::agents_md_loader::load_agents_md(
            workspace_path,
            Some(&config_dir),
        );
        if !agents_md.is_empty() {
            log::info!(
                "已加载 AGENTS.md 规则,项目级 {} 个,全局 {} 个",
                agents_md.project_rules.len(),
                agents_md.global_rules.is_some() as usize
            );
            Some(agents_md.merge())
        } else {
            None
        }
    };

    // 从 AgentModeManager 获取当前会话的实际模式（替代硬编码 Build）
    let agent_mode = agent_mode_manager.get_mode(session_id).await;
    log::info!(
        "Agent 模式: session_id={}, mode={:?}",
        session_id,
        agent_mode
    );

    let dynamic_prompt = AgentContext::build_system_prompt_with_task(
        workspace_path,
        &task_type,
        tool_count,
        handler_count,
        ctx.token_budget(),
        author_info.as_ref(),
        &env_info,
        agents_md_content.as_deref(), // AGENTS.md 自定义规则
        &agent_mode,                  // agent_mode（从 AgentModeManager 获取）
    );
    ctx.system_prompt = dynamic_prompt;
    log::info!("任务类型: {:?}, 系统提示词已动态构建", task_type);

    // 从数据库加载该会话的历史消息，使 Agent 能感知之前的对话内容
    let history_messages = {
        match db.conn() {
            Ok(conn) => {
                let db_messages = crate::db::message_repo::list_messages(&conn, session_id);
                db_messages
                    .into_iter()
                    .filter_map(|m| m.to_chat_message())
                    .collect::<Vec<ChatMessage>>()
            }
            Err(e) => {
                log::warn!(
                    "获取数据库连接失败，无法加载历史消息: {}, 将以空上下文启动",
                    e.message
                );
                Vec::new()
            }
        }
    };

    // 记录当前会话是否为新会话（无历史消息）
    let is_new_session = history_messages.is_empty();

    // 注入历史消息到上下文（在添加当前用户消息之前）
    if !is_new_session {
        log::info!(
            "加载历史消息: session_id={}, 历史消息数={}",
            session_id,
            history_messages.len()
        );
        ctx.load_history_messages(history_messages);
    }

    // 历史会话摘要注入已禁用
    // 用户明确要求：新对话中不应该存在上文，每个会话应该是完全独立的
    // 历史消息在同一会话续写时已通过上下文加载提供，无需额外注入跨会话摘要

    // 加载高置信度用户偏好（语义记忆）
    let user_preferences_text = {
        match db.conn() {
            Ok(conn) => {
                let prefs =
                    crate::db::user_preference_repo::list_high_confidence_preferences(&conn, 0.7);
                if prefs.is_empty() {
                    String::new()
                } else {
                    let text = prefs
                        .iter()
                        .map(|p| format!("- {}: {}", p.key, p.value))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!(
                        "\n<user_preferences>\n## 用户偏好\n{}\n</user_preferences>",
                        text
                    )
                }
            }
            Err(_) => String::new(),
        }
    };

    // 将用户偏好追加到系统提示词（历史会话摘要已禁用注入）
    if !user_preferences_text.is_empty() {
        ctx.system_prompt = format!("{}{}", ctx.system_prompt, user_preferences_text);
        log::info!("已注入用户偏好到系统提示词, session_id={}", session_id);
    }

    // 解析附件为 ContentPart 列表
    let content_parts = if !attachments.is_empty() {
        match AttachmentService::resolve_attachments(attachments, workspace_path, doc_service).await
        {
            Ok(parts) if !parts.is_empty() => Some(parts),
            Ok(_) => None,
            Err(e) => {
                log::warn!("附件解析失败: {}, 将忽略附件继续执行", e.message);
                None
            }
        }
    } else {
        None
    };

    // 检查是否包含图片附件，用于幻觉防护
    let has_image_attachments = AttachmentService::has_image_attachments(attachments);

    // 获取当前 Provider 是否支持视觉（提前获取，用于数据层面处理）
    // 通过 Router 的主 Provider ID 精确查找（避免 list_providers 顺序不确定）
    let supports_vision = if has_image_attachments {
        let providers = llm_router.list_providers();
        let main_provider_id = llm_router.default_provider_id();
        let main_provider = main_provider_id
            .and_then(|id| providers.iter().find(|p| p.id == id))
            .or_else(|| providers.first());
        main_provider.map(|p| p.supports_vision).unwrap_or(false)
    } else {
        true
    };

    // 如果有图片附件但当前 Provider 不支持视觉，在数据层面剥离图片 ContentPart
    let filtered_content_parts = if !supports_vision && has_image_attachments {
        let mut text_parts = Vec::new();
        let mut image_count = 0usize;

        if let Some(ref parts) = content_parts {
            for part in parts {
                match part {
                    ContentPart::Image { .. } => {
                        image_count += 1;
                    }
                    ContentPart::Text { text } => {
                        text_parts.push(ContentPart::Text { text: text.clone() });
                    }
                }
            }
        }

        if image_count > 0 {
            log::warn!(
                "当前 Provider 不支持视觉，已剥离 {} 张图片 ContentPart: session_id={}",
                image_count,
                session_id
            );

            // 剥离图片后，如果剩余的只有文本 ContentPart，将它们合并回 content 字段，
            // 设 content_parts 为 None，避免适配器使用 content_parts 而忽略 content 字段
            // （用户实际输入的文字在 content 字段中，content_parts 非空时会被适配器优先使用）
            if text_parts.is_empty() {
                None
            } else {
                // 仍有文档/文本附件的 ContentPart，保留它们
                Some(text_parts)
            }
        } else {
            // content_parts 中没有图片（可能图片解析失败），保持原样
            content_parts.clone()
        }
    } else {
        content_parts.clone()
    };

    ctx.add_user_message_with_attachments(
        prompt,
        filtered_content_parts,
        attachments,
        supports_vision,
    );

    // 如果有图片附件，注入视觉相关提示词
    if has_image_attachments {
        if !supports_vision {
            // 不支持视觉：注入增强版约束提示词
            ctx.system_prompt = format!(
                "{}\n\n<vision_constraint>\nThe current model does not support image understanding. The user has sent image attachments, but the image data has been removed by the system — you cannot see the image content.\n\nYou MUST follow these rules:\n1. NEVER pretend you can see or analyze image content\n2. NEVER guess the user's intent or fabricate image content based on attachments\n3. NEVER perform unrelated operations (such as generating documents, calling tools, etc.) just because you cannot view images\n4. If the user's question depends on image content, tell them directly that you cannot view images and suggest describing the content in text\n5. If the user only sends images without text, ask them what they would like you to do with the images\n</vision_constraint>",
                ctx.system_prompt
            );
            log::warn!(
                "当前 Provider 不支持视觉，已注入增强版幻觉防护提示: session_id={}",
                session_id
            );
        } else {
            // 支持视觉时注入图片可见性提示
            ctx.system_prompt = format!(
                "{}\n\n<image_visibility_warning>\nThe user has sent image attachments. You can see these images. Answer based on what you actually see in the images — do not guess or fabricate image content.\n</image_visibility_warning>",
                ctx.system_prompt
            );
        }
    }

    // 创建增量持久化回调，每轮迭代后自动持久化新增消息
    let db_for_persist = Arc::clone(db);
    #[allow(clippy::type_complexity)]
    let persist_fn: Arc<
        dyn Fn(&str, &[ChatMessage]) -> Result<(), CommandError> + Send + Sync,
    > = Arc::new(move |sid: &str, messages: &[ChatMessage]| {
        persist_messages_to_db(&db_for_persist, sid, messages)
    });

    // 创建版本快照回调，在文件修改/删除前自动创建快照
    let db_for_snapshot = Arc::clone(db);
    let config_for_snapshot = Arc::clone(config);
    let workspace_path_for_snapshot = workspace_path.to_string();
    #[allow(clippy::type_complexity)]
    let snapshot_fn: Arc<
        dyn Fn(&str, &str, &str, &str) -> Result<(), CommandError> + Send + Sync,
    > = Arc::new(
        move |wid: &str, sid: &str, file_path: &str, operation: &str| {
            create_version_snapshot(
                &db_for_snapshot,
                &config_for_snapshot,
                &workspace_path_for_snapshot,
                wid,
                sid,
                file_path,
                operation,
            )
        },
    );

    // 创建上下文窗口使用信息持久化回调，每次发射事件时持久化到数据库
    let db_for_context_usage = Arc::clone(db);
    let context_usage_persist_fn: ContextUsagePersistFn = Arc::new(
        move |sid: &str, usage_info: &crate::models::llm::ContextUsageInfo| {
            if let Ok(json) = serde_json::to_string(usage_info) {
                if let Ok(conn) = db_for_context_usage.conn() {
                    if let Err(e) = crate::db::session_repo::save_context_usage(&conn, sid, &json) {
                        log::warn!(
                            "持久化上下文窗口使用信息失败: session_id={}, 错误: {}",
                            sid,
                            e.message
                        );
                    }
                }
            }
        },
    );

    let executor = AgentExecutor::new(
        Arc::clone(llm_router),
        Arc::clone(tool_registry),
        Arc::clone(handler_registry),
        emitter.clone(),
        Arc::clone(confirm_channels),
        // 权限系统组件
        Arc::clone(permission_channels),
        Arc::clone(permission_registry),
        Arc::clone(doom_loop_detector),
        Arc::clone(agent_mode_manager),
    )
    .with_stop_check(should_stop)
    .with_max_iterations(max_iterations)
    .with_persist_fn(persist_fn)
    .with_context_usage_persist_fn(context_usage_persist_fn)
    .with_snapshot_fn(snapshot_fn)
    .with_confirmation_level(confirmation_level)
    .with_compactor(compaction_config);

    match executor.execute(&mut ctx).await {
        Ok(result) => {
            log::info!(
                "Agent 执行成功: session_id={}, 摘要长度={}",
                session_id,
                result.summary.len()
            );

            // 持久化可能残留的未持久化消息（兜底保护）
            let unpersisted = ctx.get_unpersisted_messages();
            if !unpersisted.is_empty() {
                log::info!(
                    "持久化残留消息: session_id={}, 数量={}",
                    session_id,
                    unpersisted.len()
                );
                if let Err(e) = persist_messages_to_db(db, session_id, unpersisted) {
                    log::warn!(
                        "残留消息持久化失败: session_id={}, 错误: {}",
                        session_id,
                        e.message
                    );
                }
                ctx.mark_persisted();
            }

            // 生成会话摘要并持久化（情景记忆）
            if let Err(e) = persist_session_summary(db, &ctx) {
                log::warn!(
                    "会话摘要持久化失败: session_id={}, 错误: {}",
                    session_id,
                    e.message
                );
            }

            // 从工具调用参数中提取用户偏好并持久化（语义记忆）
            if let Err(e) = extract_and_persist_preferences(db, &ctx) {
                log::warn!(
                    "用户偏好提取失败: session_id={}, 错误: {}",
                    session_id,
                    e.message
                );
            }

            Ok(())
        }
        Err(e) => {
            log::error!(
                "Agent 执行失败: session_id={}, 错误: {}",
                session_id,
                e.message
            );

            // 执行失败时也尝试持久化已有消息
            let unpersisted = ctx.get_unpersisted_messages();
            if !unpersisted.is_empty() {
                log::info!(
                    "执行失败后持久化已有消息: session_id={}, 数量={}",
                    session_id,
                    unpersisted.len()
                );
                if let Err(persist_err) = persist_messages_to_db(db, session_id, unpersisted) {
                    log::warn!(
                        "失败后消息持久化失败: session_id={}, 错误: {}",
                        session_id,
                        persist_err.message
                    );
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
        // block_in_place 避免在 async 上下文中 blocking_lock 导致 panic
        let cfg = tokio::task::block_in_place(|| config.blocking_lock());
        cfg.data_dir().join("snapshots")
    };

    // 确保快照目录存在
    std::fs::create_dir_all(&snapshot_dir)?;

    // 生成快照文件名：使用 UUID + 原始扩展名
    let snapshot_id = uuid::Uuid::new_v4().to_string();
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("bin");
    let snapshot_file_name = format!("{}.{}", snapshot_id, extension);
    let snapshot_path = snapshot_dir.join(&snapshot_file_name);

    // 复制当前文件到快照目录
    std::fs::copy(&abs_path, &snapshot_path)?;

    log::info!(
        "版本快照文件已创建: file={}, snapshot={}, operation={}",
        file_path,
        snapshot_file_name,
        operation
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
        let cfg = tokio::task::block_in_place(|| config.blocking_lock());
        match cfg.load_app_settings() {
            Ok(settings) => {
                let policy_str = match settings.version_snapshot.retention_policy {
                    crate::config::app_settings::RetentionPolicy::ByCount => "byCount",
                    crate::config::app_settings::RetentionPolicy::ByDays => "byDays",
                    crate::config::app_settings::RetentionPolicy::Both => "both",
                };
                (
                    policy_str.to_string(),
                    settings.version_snapshot.max_count,
                    settings.version_snapshot.max_days,
                )
            }
            Err(_) => ("byCount".to_string(), 50, 30),
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
