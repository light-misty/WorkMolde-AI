use rusqlite::{params, Connection};

use crate::models::{Branch, BranchGroupInfo, BranchInfo};

/// 创建分支记录
pub fn create_branch(conn: &Connection, branch: &Branch) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO message_branches (id, session_id, parent_branch_id, fork_message_id, branch_group_id, name, sort_order, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            branch.id,
            branch.session_id,
            branch.parent_branch_id,
            branch.fork_message_id,
            branch.branch_group_id,
            branch.name,
            branch.sort_order,
            branch.created_at,
        ],
    )?;
    Ok(())
}

/// 根据 ID 获取分支
pub fn get_branch(conn: &Connection, id: &str) -> Result<Option<Branch>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, parent_branch_id, fork_message_id, branch_group_id, name, sort_order, created_at
         FROM message_branches WHERE id = ?1"
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(Branch {
            id: row.get(0)?,
            session_id: row.get(1)?,
            parent_branch_id: row.get(2)?,
            fork_message_id: row.get(3)?,
            branch_group_id: row.get(4)?,
            name: row.get(5)?,
            sort_order: row.get(6)?,
            created_at: row.get(7)?,
        }));
    }
    Ok(None)
}

/// 列出会话的所有分支
pub fn list_branches_by_session(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<Branch>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, parent_branch_id, fork_message_id, branch_group_id, name, sort_order, created_at
         FROM message_branches WHERE session_id = ?1 ORDER BY sort_order ASC, created_at ASC"
    )?;
    let branches = stmt
        .query_map(params![session_id], |row| {
            Ok(Branch {
                id: row.get(0)?,
                session_id: row.get(1)?,
                parent_branch_id: row.get(2)?,
                fork_message_id: row.get(3)?,
                branch_group_id: row.get(4)?,
                name: row.get(5)?,
                sort_order: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(branches)
}

/// 列出同一分支组内的所有分支（用于 UI 切换器）
pub fn list_branches_by_group(
    conn: &Connection,
    branch_group_id: &str,
) -> Result<Vec<Branch>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, parent_branch_id, fork_message_id, branch_group_id, name, sort_order, created_at
         FROM message_branches WHERE branch_group_id = ?1 ORDER BY sort_order ASC, created_at ASC"
    )?;
    let branches = stmt
        .query_map(params![branch_group_id], |row| {
            Ok(Branch {
                id: row.get(0)?,
                session_id: row.get(1)?,
                parent_branch_id: row.get(2)?,
                fork_message_id: row.get(3)?,
                branch_group_id: row.get(4)?,
                name: row.get(5)?,
                sort_order: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(branches)
}

/// 列出会话内所有分支组（用于前端渲染切换器）
/// 每个 branch_group_id 对应一组从同一分叉点产生的分支
pub fn list_branch_groups(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<BranchGroupInfo>, rusqlite::Error> {
    // 查询会话内所有非空的 branch_group_id 及其 fork_message_id 和 parent_branch_id
    // parent_branch_id 用于把原分支（如 main）加入分支组，确保切换器能显示所有相关分支
    let mut stmt = conn.prepare(
        "SELECT DISTINCT branch_group_id, fork_message_id, parent_branch_id
         FROM message_branches
         WHERE session_id = ?1 AND branch_group_id IS NOT NULL
         ORDER BY created_at ASC",
    )?;
    let groups: Vec<(String, Option<String>, Option<String>)> = stmt
        .query_map(params![session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    let mut result = Vec::new();
    for (group_id, fork_message_id, parent_branch_id) in groups {
        let mut branches = list_branches_by_group(conn, &group_id)?;

        // 将原分支（parent_branch_id）加入组，确保切换器能显示所有相关分支
        // 原分支的 branch_group_id 为 NULL，但它是该分支组的"父分支"
        if let Some(parent_id) = parent_branch_id {
            let already_in_group = branches.iter().any(|b| b.id == parent_id);
            if !already_in_group {
                if let Some(parent_branch) = get_branch(conn, &parent_id)? {
                    branches.push(parent_branch);
                } else {
                    log::warn!(
                        "[list_branch_groups] parent_branch 不存在: parent_id={}, group_id={}",
                        parent_id, group_id
                    );
                }
            }
        }

        // 按 sort_order 排序，确保切换器显示顺序一致（main 分支 sort_order=0 排首位）
        branches.sort_by_key(|b| b.sort_order);

        let branch_infos: Vec<BranchInfo> = branches
            .iter()
            .map(|b| BranchInfo {
                branch_id: b.id.clone(),
                name: b.name.clone(),
                sort_order: b.sort_order,
            })
            .collect();
        result.push(BranchGroupInfo {
            branch_group_id: group_id,
            fork_message_id,
            branches: branch_infos,
        });
    }
    Ok(result)
}

/// 更新消息的 branch_group_id（用于分叉点原消息打标）
pub fn update_message_branch_group_id(
    conn: &Connection,
    message_id: &str,
    branch_group_id: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE session_messages SET branch_group_id = ?1 WHERE id = ?2",
        params![branch_group_id, message_id],
    )?;
    Ok(())
}

/// 设置会话的活跃分支
pub fn set_session_active_branch(
    conn: &Connection,
    session_id: &str,
    branch_id: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE sessions SET active_branch_id = ?1 WHERE id = ?2",
        params![branch_id, session_id],
    )?;
    Ok(())
}

/// 获取会话的活跃分支 ID（无记录时返回 "main" 兜底）
/// 注意：返回的格式为分支 ID（如 "branch_<session_id>_main"），不是分支名称
pub fn get_session_active_branch(
    conn: &Connection,
    session_id: &str,
) -> Result<String, rusqlite::Error> {
    let active_branch_id: Option<String> = conn
        .query_row(
            "SELECT active_branch_id FROM sessions WHERE id = ?1",
            params![session_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    if let Some(id) = active_branch_id {
        if !id.is_empty() {
            return Ok(id);
        }
    }

    // 兜底：查询会话的第一个分支（按 sort_order）
    let first_branch_id: Option<String> = conn.query_row(
        "SELECT id FROM message_branches WHERE session_id = ?1 ORDER BY sort_order ASC, created_at ASC LIMIT 1",
        params![session_id],
        |row| row.get(0),
    ).ok().flatten();

    Ok(first_branch_id.unwrap_or_else(|| format!("branch_{}_main", session_id)))
}
