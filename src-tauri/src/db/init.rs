use rusqlite::Connection;
use crate::errors::CommandError;

/// 执行数据库初始化：建表、创建索引、插入种子数据
pub fn initialize_database(conn: &Connection) -> Result<(), CommandError> {
    log::info!("开始初始化数据库结构");

    create_tables(conn)?;
    create_indexes(conn)?;
    seed_builtin_templates(conn)?;

    log::info!("数据库结构初始化完成");
    Ok(())
}

/// 创建所有数据表
fn create_tables(conn: &Connection) -> Result<(), CommandError> {
    // sessions 会话表
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
            id                  TEXT        NOT NULL PRIMARY KEY,
            workspace_id        TEXT        NOT NULL,
            title               TEXT        NOT NULL DEFAULT '新会话',
            created_at          TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at          TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            llm_provider        TEXT        NOT NULL DEFAULT '',
            llm_model           TEXT        NOT NULL DEFAULT '',
            context_usage_json  TEXT        DEFAULT NULL
        );"
    )?;

    // session_messages 消息表
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS session_messages (
            id                TEXT        NOT NULL PRIMARY KEY,
            session_id        TEXT        NOT NULL,
            role              TEXT        NOT NULL CHECK (role IN ('user', 'assistant', 'tool')),
            content           TEXT        NOT NULL DEFAULT '',
            tool_name         TEXT        DEFAULT NULL,
            tool_args         TEXT        DEFAULT NULL,
            tool_result       TEXT        DEFAULT NULL,
            thinking_content  TEXT        DEFAULT NULL,
            reasoning_content TEXT        DEFAULT NULL,
            attachments       TEXT        DEFAULT NULL,
            created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );"
    )?;

    // version_snapshots 版本快照表
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS version_snapshots (
            id                TEXT        NOT NULL PRIMARY KEY,
            workspace_id      TEXT        NOT NULL,
            session_id        TEXT        NOT NULL,
            file_path         TEXT        NOT NULL,
            snapshot_path     TEXT        NOT NULL,
            operation         TEXT        NOT NULL DEFAULT '',
            created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );"
    )?;

    // prompt_templates Prompt模板表
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS prompt_templates (
            id                TEXT        NOT NULL PRIMARY KEY,
            name              TEXT        NOT NULL,
            description       TEXT        NOT NULL DEFAULT '',
            content           TEXT        NOT NULL DEFAULT '',
            category          TEXT        NOT NULL DEFAULT 'custom',
            is_builtin        INTEGER     NOT NULL DEFAULT 0,
            variables         TEXT        DEFAULT NULL,
            created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );"
    )?;

    // session_summaries 会话摘要表（情景记忆）
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS session_summaries (
            id                TEXT        NOT NULL PRIMARY KEY,
            session_id        TEXT        NOT NULL,
            workspace_id      TEXT        NOT NULL,
            user_goal         TEXT        NOT NULL DEFAULT '',
            result_summary    TEXT        NOT NULL DEFAULT '',
            files_involved    TEXT        NOT NULL DEFAULT '[]',
            tools_used        TEXT        NOT NULL DEFAULT '[]',
            errors_resolved   TEXT        NOT NULL DEFAULT '[]',
            created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );"
    )?;

    // user_preferences 用户偏好表（语义记忆）
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS user_preferences (
            id                TEXT        NOT NULL PRIMARY KEY,
            category          TEXT        NOT NULL,
            key               TEXT        NOT NULL,
            value             TEXT        NOT NULL,
            confidence        REAL        NOT NULL DEFAULT 0.5,
            observation_count INTEGER     NOT NULL DEFAULT 1,
            last_observed_at  TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            UNIQUE(category, key)
        );"
    )?;

    log::info!("数据表创建完成");
    Ok(())
}

/// 创建所有索引
fn create_indexes(conn: &Connection) -> Result<(), CommandError> {
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_sessions_workspace_id
            ON sessions (workspace_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_updated_at
            ON sessions (updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_sessions_created_at
            ON sessions (created_at DESC);

        CREATE INDEX IF NOT EXISTS idx_session_messages_session_id
            ON session_messages (session_id);
        CREATE INDEX IF NOT EXISTS idx_session_messages_session_id_created_at
            ON session_messages (session_id, created_at ASC);
        CREATE INDEX IF NOT EXISTS idx_session_messages_role
            ON session_messages (role);

        CREATE INDEX IF NOT EXISTS idx_version_snapshots_workspace_id
            ON version_snapshots (workspace_id);
        CREATE INDEX IF NOT EXISTS idx_version_snapshots_session_id
            ON version_snapshots (session_id);
        CREATE INDEX IF NOT EXISTS idx_version_snapshots_file_path
            ON version_snapshots (file_path);
        CREATE INDEX IF NOT EXISTS idx_version_snapshots_created_at
            ON version_snapshots (created_at DESC);

        CREATE INDEX IF NOT EXISTS idx_prompt_templates_category
            ON prompt_templates (category);
        CREATE INDEX IF NOT EXISTS idx_prompt_templates_is_builtin
            ON prompt_templates (is_builtin);
        CREATE INDEX IF NOT EXISTS idx_prompt_templates_updated_at
            ON prompt_templates (updated_at DESC);

        CREATE INDEX IF NOT EXISTS idx_session_summaries_workspace
            ON session_summaries (workspace_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_session_summaries_session
            ON session_summaries (session_id);

        CREATE INDEX IF NOT EXISTS idx_user_preferences_category
            ON user_preferences (category);"
    )?;

    log::info!("索引创建完成");
    Ok(())
}

/// 插入内置模板种子数据
fn seed_builtin_templates(conn: &Connection) -> Result<(), CommandError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM prompt_templates WHERE is_builtin = 1",
        [],
        |row| row.get(0),
    )?;

    if count > 0 {
        log::debug!("内置模板已存在 (count={})，跳过种子数据", count);
        return Ok(());
    }

    let now = chrono::Utc::now().to_rfc3339();

    // 内置模板列表
    let builtin_templates: Vec<(&str, &str, &str, &str, &str)> = vec![
        (
            "builtin-weekly-report",
            "周报生成",
            "根据本周工作内容自动生成结构化周报文档",
            "请根据以下工作内容，帮我生成一份结构化的周报文档，保存为Word格式。要求包含：本周工作总结、关键进展、遇到的问题、下周计划。工作内容如下：{{content}}",
            "document",
        ),
        (
            "builtin-meeting-minutes",
            "会议纪要",
            "根据会议信息生成规范的会议纪要文档",
            "请根据以下会议信息，帮我生成一份规范的会议纪要文档，保存为Word格式。要求包含：会议主题、参会人员、会议时间、讨论内容、决议事项、后续行动项。会议信息如下：{{content}}",
            "document",
        ),
        (
            "builtin-data-analysis",
            "数据分析报告",
            "对Excel数据进行统计分析并生成分析报告",
            "请读取以下Excel文件的数据，进行统计分析，并生成一份数据分析报告。要求包含：数据概览、关键指标、趋势分析、异常发现、建议。文件路径：{{filePath}}，分析重点：{{focus}}",
            "analysis",
        ),
        (
            "builtin-format-convert",
            "格式转换",
            "将文档从一种格式转换为另一种格式",
            "请将文件 {{inputPath}} 从 {{sourceFormat}} 格式转换为 {{targetFormat}} 格式，保存到 {{outputPath}}",
            "conversion",
        ),
        (
            "builtin-doc-review",
            "文档审阅",
            "审阅文档内容，提出修改建议",
            "请审阅以下文档，检查内容的准确性、逻辑性和完整性，并提出具体的修改建议。文件路径：{{filePath}}，审阅重点：{{focus}}",
            "analysis",
        ),
        (
            "builtin-ppt-outline",
            "PPT大纲生成",
            "根据主题生成PPT大纲和内容",
            "请根据以下主题，帮我生成一份PPT演示文稿，保存为PPTX格式。要求包含：封面、目录、内容页（每页有标题和要点）、总结页。主题：{{topic}}，页数要求：{{pageCount}}页左右",
            "document",
        ),
    ];

    for (id, name, description, content, category) in &builtin_templates {
        conn.execute(
            "INSERT INTO prompt_templates (id, name, description, content, category, is_builtin, variables, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, NULL, ?6, ?7)",
            rusqlite::params![id, name, description, content, category, now, now],
        )?;
    }

    log::info!("已插入 {} 个内置模板", builtin_templates.len());
    Ok(())
}
