use tauri::{AppHandle, State};

use crate::db::message_repo;
use crate::db::session_repo;
use crate::errors::CommandError;
use crate::events::types;
use crate::events::AgentEmitter;
use crate::models::session::{
    CreateSessionParams, Session, SessionDetail, SessionFilter, SessionSummary,
};
use crate::AppState;

/// 创建新会话
#[tauri::command]
pub async fn create_session(
    params: CreateSessionParams,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Session, CommandError> {
    log::info!(
        "create_session 请求: title={:?}, workspace_id={:?}, provider_id={:?}",
        params.title,
        params.workspace_id,
        params.provider_id
    );
    let id = uuid::Uuid::new_v4().to_string();
    let title = params.title.unwrap_or_else(|| "新会话".to_string());
    let workspace_id = params.workspace_id.unwrap_or_default();
    let provider_id = params.provider_id.unwrap_or_default();

    let conn = state.db.conn()?;
    session_repo::create_session(&conn, &id, &workspace_id, &title, &provider_id, "")?;

    let session = session_repo::get_session(&conn, &id)?;
    log::info!(
        "create_session 成功: session_id={}, title={}",
        session.id,
        session.title
    );

    // 发射会话更新事件
    let emitter = AgentEmitter::new(app_handle);
    let _ = emitter.emit_session_updated(types::SessionUpdatePayload {
        session_id: session.id.clone(),
        change_type: "created".to_string(),
        data: Some(serde_json::to_value(&session).unwrap_or_default()),
    });

    Ok(session)
}

/// 列出会话，支持筛选
#[tauri::command]
pub async fn list_sessions(
    filter: Option<SessionFilter>,
    state: State<'_, AppState>,
) -> Result<Vec<SessionSummary>, CommandError> {
    log::info!("list_sessions 请求: filter={:?}", filter);
    let conn = state.db.conn()?;

    let workspace_id = filter.as_ref().and_then(|f| f.workspace_id.as_deref());
    let status = filter.as_ref().and_then(|f| f.status.as_deref());
    let search = filter.as_ref().and_then(|f| f.search.as_deref());
    let limit = filter.as_ref().and_then(|f| f.limit).unwrap_or(50);
    let offset = filter.as_ref().and_then(|f| f.offset).unwrap_or(0);

    log::debug!(
        "list_sessions 查询条件: workspace_id={:?}, status={:?}, search={:?}, limit={}, offset={}",
        workspace_id,
        status,
        search,
        limit,
        offset
    );
    let result = session_repo::list_sessions(&conn, workspace_id, status, search, limit, offset);
    log::info!("list_sessions 成功: 返回 {} 条记录", result.len());
    Ok(result)
}

/// 获取会话详情，包含消息历史
#[tauri::command]
pub async fn get_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<SessionDetail, CommandError> {
    log::info!("get_session 请求: session_id={}", session_id);
    let conn = state.db.conn()?;
    let session = session_repo::get_session(&conn, &session_id)?;

    // 获取当前活跃分支 ID（无记录时 branch_repo 兜底返回 main 分支 ID）
    let active_branch_id = crate::db::branch_repo::get_session_active_branch(&conn, &session_id)?;

    // 加载当前分支的消息
    let messages = message_repo::list_messages(&conn, &session_id, &active_branch_id);

    log::info!(
        "get_session 成功: session_id={}, 消息数={}",
        session_id,
        messages.len()
    );

    // 加载会话的所有分支列表（供前端渲染切换器）
    let branches = crate::db::branch_repo::list_branches_by_session(&conn, &session_id)?;

    Ok(SessionDetail {
        session,
        messages,
        branches,
        active_branch_id,
    })
}

/// 删除会话
#[tauri::command]
pub async fn delete_session(
    session_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("delete_session 请求: session_id={}", session_id);

    // 检查会话是否有 Agent 正在运行，防止数据丢失
    {
        let active = state.active_agents.lock().await;
        if active.contains_key(&session_id) {
            log::warn!(
                "delete_session 失败: 会话 '{}' 的 Agent 正在运行",
                session_id
            );
            return Err(CommandError::agent(
                crate::errors::AGENT_ALREADY_RUNNING,
                format!("会话 '{}' 的 Agent 正在运行，无法删除", session_id),
            ));
        }
    }

    let conn = state.db.conn()?;
    session_repo::delete_session(&conn, &session_id)?;
    log::info!("delete_session 成功: session_id={}", session_id);

    // 发射会话更新事件
    let emitter = AgentEmitter::new(app_handle);
    let _ = emitter.emit_session_updated(types::SessionUpdatePayload {
        session_id: session_id.clone(),
        change_type: "deleted".to_string(),
        data: None,
    });

    Ok(())
}

/// 更新会话标题
#[tauri::command]
pub async fn update_session_title(
    session_id: String,
    title: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "update_session_title 请求: session_id={}, title={}",
        session_id,
        title
    );
    let conn = state.db.conn()?;
    session_repo::update_session_title(&conn, &session_id, &title)?;
    log::info!(
        "update_session_title 成功: session_id={}, title={}",
        session_id,
        title
    );

    // 发射会话更新事件
    let emitter = AgentEmitter::new(app_handle);
    let _ = emitter.emit_session_updated(types::SessionUpdatePayload {
        session_id: session_id.clone(),
        change_type: "updated".to_string(),
        data: Some(serde_json::json!({ "title": title })),
    });

    Ok(())
}

/// 清除指定工作区下的所有会话
#[tauri::command]
pub async fn clear_workspace_sessions(
    workspace_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<u64, CommandError> {
    log::info!(
        "clear_workspace_sessions 请求: workspace_id={}",
        workspace_id
    );
    let conn = state.db.conn()?;
    let session_ids = session_repo::delete_sessions_by_workspace(&conn, &workspace_id)?;
    let count = session_ids.len() as u64;
    log::info!(
        "clear_workspace_sessions 成功: workspace_id={}, 已删除 {} 条会话",
        workspace_id,
        count
    );

    // 发射会话更新事件，通知前端刷新列表
    let emitter = AgentEmitter::new(app_handle);
    for sid in &session_ids {
        let _ = emitter.emit_session_updated(types::SessionUpdatePayload {
            session_id: sid.clone(),
            change_type: "deleted".to_string(),
            data: None,
        });
    }

    Ok(count)
}

/// 清除所有会话数据
#[tauri::command]
pub async fn clear_all_sessions(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<u64, CommandError> {
    log::info!("clear_all_sessions 请求");
    let conn = state.db.conn()?;
    let count = session_repo::clear_all_sessions(&conn)?;
    log::info!("clear_all_sessions 成功: 已删除 {} 条会话", count);

    // 发射会话更新事件，通知前端刷新列表
    let emitter = AgentEmitter::new(app_handle);
    let _ = emitter.emit_session_updated(types::SessionUpdatePayload {
        session_id: String::new(),
        change_type: "cleared".to_string(),
        data: None,
    });

    Ok(count)
}

/// 批量删除会话中的指定消息
#[tauri::command]
pub async fn delete_session_messages(
    session_id: String,
    message_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "delete_session_messages 请求: session_id={}, ids={:?}",
        session_id,
        message_ids
    );

    if message_ids.is_empty() {
        return Ok(());
    }

    // 检查会话是否有 Agent 正在运行，防止数据不一致
    {
        let active = state.active_agents.lock().await;
        if active.contains_key(&session_id) {
            log::warn!(
                "delete_session_messages 失败: 会话 '{}' 的 Agent 正在运行",
                session_id
            );
            return Err(CommandError::agent(
                crate::errors::AGENT_ALREADY_RUNNING,
                format!("会话 '{}' 的 Agent 正在运行，无法删除消息", session_id),
            ));
        }
    }

    let conn = state.db.conn()?;
    // 获取当前活跃分支 ID，作为防御性过滤条件（确保不会跨分支删除消息）
    let active_branch_id = crate::db::branch_repo::get_session_active_branch(&conn, &session_id)?;
    message_repo::delete_messages_by_ids(&conn, &session_id, &message_ids, &active_branch_id)?;
    log::info!(
        "delete_session_messages 成功: session_id={}, 已删除 {} 条消息",
        session_id,
        message_ids.len()
    );

    Ok(())
}

/// 更新会话的工作区 ID（用于修复旧数据中 workspace_id 为空的会话）
#[tauri::command]
pub async fn update_session_workspace(
    session_id: String,
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "update_session_workspace 请求: session_id={}, workspace_id={}",
        session_id,
        workspace_id
    );
    let conn = state.db.conn()?;
    session_repo::update_session_workspace(&conn, &session_id, &workspace_id)?;
    log::info!(
        "update_session_workspace 成功: session_id={}, workspace_id={}",
        session_id,
        workspace_id
    );
    Ok(())
}

/// 创建分支命令
/// 在指定用户消息节点处分叉出新分支：
/// 1. 复制原分支截至该消息之前（不含）的所有消息到新分支
/// 2. 为原分叉点消息设置 branch_group_id（用于 UI 切换器定位）
/// 3. 设置会话活跃分支为新分支
/// 不在此处创建 user 消息也不触发 Agent，由前端调用 start_agent 时创建 user 消息
/// 并通过 branchGroupId 参数让 run_agent 在持久化时为新 user 消息设置 branch_group_id
#[tauri::command]
pub async fn create_branch(
    session_id: String,
    fork_message_id: String,
    state: State<'_, AppState>,
) -> Result<crate::models::CreateBranchResult, CommandError> {
    // 1. 检查 Agent 未运行
    {
        let active = state.active_agents.lock().await;
        if active.contains_key(&session_id) {
            return Err(CommandError::agent(
                crate::errors::AGENT_ALREADY_RUNNING,
                format!("会话 '{}' 有 Agent 正在运行，无法创建分支", session_id),
            ));
        }
    }

    let mut conn = state.db.conn()?;

    // 2. 获取当前活跃分支（原分支）
    let source_branch_id = crate::db::branch_repo::get_session_active_branch(&conn, &session_id)?;

    // 3. 生成新分支 ID 和分支组 ID
    let new_branch_id = format!("branch_{}", uuid::Uuid::new_v4());
    let branch_group_id = format!("bg_{}", uuid::Uuid::new_v4());
    let now = chrono::Utc::now().to_rfc3339();

    // 4. 查询同分支组内的最大 sort_order（如果是首次分叉则为 1）
    // 注意：原 fork_message_id 对应的消息可能已有 branch_group_id（之前已分叉过）
    // 此时新分支应加入同一 branch_group_id，sort_order 在该组内递增
    let (final_branch_group_id, sort_order) = {
        // 检查原消息是否已有 branch_group_id
        let existing_group_id: Option<String> = conn
            .query_row(
                "SELECT branch_group_id FROM session_messages WHERE id = ?1",
                rusqlite::params![fork_message_id],
                |row| row.get(0),
            )
            .ok()
            .flatten();

        if let Some(existing_id) = existing_group_id {
            // 已有分支组，加入该组，sort_order 在组内递增
            let existing_branches =
                crate::db::branch_repo::list_branches_by_group(&conn, &existing_id)?;
            let max_sort = existing_branches
                .iter()
                .map(|b| b.sort_order)
                .max()
                .unwrap_or(0);
            (existing_id, max_sort + 1)
        } else {
            // 首次分叉，使用新生成的 branch_group_id，sort_order = 1
            // 原分支 main 不属于任何 branch_group（它的 fork_message_id 为 NULL）
            (branch_group_id.clone(), 1)
        }
    };

    // 5. 在事务中执行所有数据库操作，保证原子性
    let tx = conn.transaction()?;

    // 5.1 创建 Branch 记录
    let branch = crate::models::Branch {
        id: new_branch_id.clone(),
        session_id: session_id.clone(),
        parent_branch_id: Some(source_branch_id.clone()),
        fork_message_id: Some(fork_message_id.clone()),
        branch_group_id: Some(final_branch_group_id.clone()),
        name: format!("分支 {}", sort_order + 1), // sort_order=1 时显示"分支 2"（main 是"分支 1"）
        sort_order,
        created_at: now.clone(),
    };
    crate::db::branch_repo::create_branch(&tx, &branch)?;

    // 5.2 复制原分支截至 fork_message_id 之前（不含）的消息到新分支
    let copied_count = crate::db::message_repo::copy_messages_to_branch(
        &tx,
        &session_id,
        &source_branch_id,
        &fork_message_id,
        &new_branch_id,
    )?;
    log::info!(
        "创建分支 {}: 从原分支 {} 复制了 {} 条前缀消息",
        new_branch_id,
        source_branch_id,
        copied_count
    );

    // 5.3 若原 fork_message_id 对应消息的 branch_group_id 为空，则更新为新生成的 branch_group_id
    if final_branch_group_id == branch_group_id {
        // 首次分叉，需要为原消息打标
        crate::db::branch_repo::update_message_branch_group_id(
            &tx,
            &fork_message_id,
            &final_branch_group_id,
        )?;
    }

    // 5.4 设置会话活跃分支为新分支
    // 注意：不在此处创建 user 消息，由前端调用 startAgent 时创建
    // 这样避免 user 消息被重复创建（create_branch + startAgent 各创建一次）
    // 新 user 消息的 branch_group_id 由 run_agent 从活跃分支记录中获取并设置
    crate::db::branch_repo::set_session_active_branch(&tx, &session_id, &new_branch_id)?;

    tx.commit()?;

    log::info!(
        "创建分支成功: session_id={}, new_branch_id={}, branch_group_id={}",
        session_id,
        new_branch_id,
        final_branch_group_id
    );

    Ok(crate::models::CreateBranchResult {
        branch_id: new_branch_id,
        branch_group_id: final_branch_group_id,
    })
}

/// 切换会话的活跃分支
#[tauri::command]
pub async fn switch_branch(
    session_id: String,
    branch_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    // 检查 Agent 未运行
    {
        let active = state.active_agents.lock().await;
        if active.contains_key(&session_id) {
            return Err(CommandError::agent(
                crate::errors::AGENT_ALREADY_RUNNING,
                format!("会话 '{}' 有 Agent 正在运行，无法切换分支", session_id),
            ));
        }
    }

    let conn = state.db.conn()?;

    // 验证目标分支存在且属于该会话
    let branch = crate::db::branch_repo::get_branch(&conn, &branch_id)?.ok_or_else(|| {
        CommandError::db(
            crate::errors::DB_RECORD_NOT_FOUND,
            format!("分支 '{}' 不存在", branch_id),
        )
    })?;
    if branch.session_id != session_id {
        return Err(CommandError::db(
            crate::errors::DB_CONSTRAINT_VIOLATION,
            format!("分支 '{}' 不属于会话 '{}'", branch_id, session_id),
        ));
    }

    // 设置活跃分支
    crate::db::branch_repo::set_session_active_branch(&conn, &session_id, &branch_id)?;

    log::info!(
        "切换分支: session_id={}, branch_id={}",
        session_id,
        branch_id
    );
    Ok(())
}

/// 列出会话内所有分支组（用于前端渲染切换器）
#[tauri::command]
pub async fn list_branch_groups(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::BranchGroupInfo>, CommandError> {
    let conn = state.db.conn()?;
    let groups = crate::db::branch_repo::list_branch_groups(&conn, &session_id)?;
    Ok(groups)
}
