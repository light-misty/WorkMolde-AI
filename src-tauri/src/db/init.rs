use crate::errors::CommandError;
use rusqlite::Connection;

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
            tool_call_id      TEXT        DEFAULT NULL,
            thinking_content  TEXT        DEFAULT NULL,
            reasoning_content TEXT        DEFAULT NULL,
            attachments       TEXT        DEFAULT NULL,
            metadata          TEXT        DEFAULT NULL,
            created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );",
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
        );",
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
        );",
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
        );",
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
        );",
    )?;

    // permission_rules 权限规则表
    // 存储用户配置的权限规则,支持全局、项目、会话三个作用域
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS permission_rules (
            id                TEXT        NOT NULL PRIMARY KEY,
            scope             TEXT        NOT NULL CHECK (scope IN ('global', 'project', 'session')),
            workspace_id      TEXT        DEFAULT NULL,
            session_id        TEXT        DEFAULT NULL,
            permission_type   TEXT        NOT NULL,
            pattern           TEXT        NOT NULL DEFAULT '*',
            action            TEXT        NOT NULL CHECK (action IN ('allow', 'deny', 'ask')),
            description       TEXT        NOT NULL DEFAULT '',
            enabled           INTEGER     NOT NULL DEFAULT 1,
            created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );",
    )?;

    // todo_lists Todo 列表表(按 session_id 隔离)
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS todo_lists (
            session_id        TEXT        NOT NULL PRIMARY KEY,
            items_json        TEXT        NOT NULL DEFAULT '[]',
            updated_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );",
    )?;

    // skill_overrides Skill 覆盖配置表(用户可禁用/启用 Skill)
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS skill_overrides (
            id                TEXT        NOT NULL PRIMARY KEY,
            skill_name        TEXT        NOT NULL,
            workspace_id      TEXT        NOT NULL DEFAULT '',
            enabled           INTEGER     NOT NULL DEFAULT 1,
            custom_config     TEXT        DEFAULT NULL,
            created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            UNIQUE(skill_name, workspace_id)
        );",
    )?;

    // sub_agent_messages 子Agent消息表（持久化子Agent执行期间产生的消息）
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sub_agent_messages (
            id                  TEXT        NOT NULL PRIMARY KEY,
            parent_session_id   TEXT        NOT NULL,
            agent_id            TEXT        NOT NULL,
            role                TEXT        NOT NULL,
            content             TEXT,
            tool_name           TEXT        DEFAULT NULL,
            tool_args           TEXT        DEFAULT NULL,
            tool_result         TEXT        DEFAULT NULL,
            tool_call_id        TEXT        DEFAULT NULL,
            reasoning_content   TEXT        DEFAULT NULL,
            attachments         TEXT        DEFAULT NULL,
            metadata            TEXT        DEFAULT NULL,
            created_at          INTEGER     NOT NULL,
            seq                 INTEGER     NOT NULL
        );",
    )?;

    // message_branches 消息分支表（支持对话分支管理）
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS message_branches (
            id                TEXT        NOT NULL PRIMARY KEY,
            session_id        TEXT        NOT NULL,
            parent_branch_id  TEXT        DEFAULT NULL,
            fork_message_id   TEXT        DEFAULT NULL,
            branch_group_id   TEXT        DEFAULT NULL,
            name              TEXT        NOT NULL,
            sort_order        INTEGER     NOT NULL DEFAULT 0,
            created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );",
    )?;

    // 迁移：为已有数据库的 session_messages 表添加 metadata 字段（新字段已存在时忽略错误）
    let _ = conn.execute(
        "ALTER TABLE session_messages ADD COLUMN metadata TEXT DEFAULT NULL",
        [],
    );

    // 迁移：为分支功能添加新字段（新字段已存在时忽略错误）
    let _ = conn.execute(
        "ALTER TABLE session_messages ADD COLUMN branch_id TEXT DEFAULT NULL",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE session_messages ADD COLUMN branch_group_id TEXT DEFAULT NULL",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE sessions ADD COLUMN active_branch_id TEXT DEFAULT NULL",
        [],
    );

    // 老数据迁移：为每个会话创建默认 main 分支
    {
        let mut stmt = conn.prepare("SELECT id FROM sessions")?;
        let session_ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        for session_id in &session_ids {
            let branch_id = format!("branch_{}_main", session_id);
            // 检查是否已有分支记录
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM message_branches WHERE session_id = ?1",
                    rusqlite::params![session_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            if exists == 0 {
                // 创建 main 分支
                conn.execute(
                    "INSERT INTO message_branches (id, session_id, parent_branch_id, fork_message_id, branch_group_id, name, sort_order) VALUES (?1, ?2, NULL, NULL, NULL, 'main', 0)",
                    rusqlite::params![branch_id, session_id],
                )?;
                // 回填消息 branch_id
                conn.execute(
                    "UPDATE session_messages SET branch_id = ?1 WHERE session_id = ?2 AND branch_id IS NULL",
                    rusqlite::params![branch_id, session_id],
                )?;
                // 设置 session.active_branch_id
                conn.execute(
                    "UPDATE sessions SET active_branch_id = ?1 WHERE id = ?2 AND active_branch_id IS NULL",
                    rusqlite::params![branch_id, session_id],
                )?;
                log::info!("已为会话 {} 创建默认 main 分支", session_id);
            }
        }
    }

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
            ON user_preferences (category);

        CREATE INDEX IF NOT EXISTS idx_permission_rules_scope
            ON permission_rules (scope);
        CREATE INDEX IF NOT EXISTS idx_permission_rules_workspace_id
            ON permission_rules (workspace_id);
        CREATE INDEX IF NOT EXISTS idx_permission_rules_session_id
            ON permission_rules (session_id);
        CREATE INDEX IF NOT EXISTS idx_permission_rules_permission_type
            ON permission_rules (permission_type);

        CREATE INDEX IF NOT EXISTS idx_sub_agent_messages_agent
            ON sub_agent_messages (agent_id);
        CREATE INDEX IF NOT EXISTS idx_sub_agent_messages_session
            ON sub_agent_messages (parent_session_id);",
    )?;

    // 分支相关索引
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_session_messages_branch_id ON session_messages (branch_id, created_at ASC)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_message_branches_session_id ON message_branches (session_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_message_branches_branch_group_id ON message_branches (branch_group_id)",
        [],
    )?;

    log::info!("索引创建完成");
    Ok(())
}

/// 插入内置模板种子数据
fn seed_builtin_templates(conn: &Connection) -> Result<(), CommandError> {
    let now = chrono::Utc::now().to_rfc3339();

    // 内置模板列表
    let builtin_templates: Vec<(&str, &str, &str, &str, &str)> = vec![
        (
            "builtin-weekly-report",
            "周报生成",
            "根据本周工作内容自动生成结构化周报文档",
            "Please generate a professional weekly report Word document (.docx) with clear structure and traceable data.\n\n## Content Structure\n1. Summary Table — task name, priority (High/Mid/Low), status (Done/In Progress/Not Started), completion %, remarks\n2. Key Progress — highlight 2-3 core milestone achievements, use data where applicable\n3. Issues Encountered — table: issue description, impact scope, severity, current status (Resolved/In Progress/Pending)\n4. Risks & Mitigation — identify key risks, mitigation measures, owners\n5. Next Week Plan — prioritized detailed plan with deliverables and deadlines\n\n## Document Format\n- Title: bold 18pt centered; H1: bold 14pt; H2: bold 12pt\n- Body: 10.5pt, 1.25 line spacing\n- Tables: styled with bold header row and light gray fill\n- Margins: top/bottom 2.54cm, left/right 3.17cm (Word default)\n- Page size: A4\n\n## Output\n- Save to workspace, filename format: Weekly_Report_YYYY-MM-DD.docx\n- Run validator to verify document quality after generation\n\nWork content:\n{{content}}",
            "document",
        ),
        (
            "builtin-meeting-minutes",
            "会议纪要",
            "根据会议信息生成规范的会议纪要文档",
            "Please generate a professional meeting minutes Word document (.docx) with complete information, traceable decisions, and trackable action items.\n\n## Content Structure\n1. Basic Info — meeting topic, time, location, chair, minutes taker, attendees, absentees\n2. Agenda — list of topics covered\n3. Discussion — per topic: topic name, discussion points, opinions from all parties, conclusions\n4. Resolutions — list all resolutions (ID, content, voting result)\n5. Action Items Table — ID, description, owner, priority (High/Mid/Low), due date, status\n6. Next Meeting — time, location, proposed topics (if any)\n\n## Document Format\n- Title: bold 18pt centered; H1: bold 14pt; H2: bold 12pt\n- Body: 10.5pt, 1.25 line spacing\n- Action items table should include auto-numbering column\n- Margins: Word default standard\n\n## Output\n- Save to workspace, filename format: Meeting_Minutes_YYYY-MM-DD_Topic.docx\n- Run validator to verify document quality after generation\n\nMeeting information:\n{{content}}",
            "document",
        ),
        (
            "builtin-data-analysis",
            "数据分析",
            "对Excel数据进行统计分析并生成分析报告",
            "Please perform a professional statistical analysis on the specified data file and generate a complete data analysis report.\n\n## Analysis Workflow\n1. Data Overview — rows, columns, data types, missing values, descriptive statistics\n2. Data Cleaning — document any missing value handling, outlier treatment, transformations\n3. Key Metrics — compute core statistics (mean, median, std dev, quartiles, growth rates) based on analysis focus\n4. Trend Analysis — identify trends, cycles, seasonality; use appropriate charts\n5. Comparative Analysis — group comparisons, before/after, or benchmark comparisons\n6. Anomaly Detection — flag outliers, analyze potential causes and impact\n7. Conclusions & Recommendations — actionable insights based on data\n\n## Document Format\n- Cover page (title, date, analyst), table of contents, body, appendix\n- Charts must have figure numbers and captions (e.g., Figure 1: Quarterly Sales Trend)\n- Body: 10.5pt, 1.25 line spacing\n- Reference statistical methods in appendix\n\n## Output\n- Save as Word document to workspace\n- Filename format: Data_Analysis_Report_YYYY-MM-DD.docx\n- Use matplotlib for visualizations, embed charts in docx\n- Run validator to verify quality\n\nFile path: {{filePath}}\nAnalysis focus: {{focus}}",
            "analysis",
        ),
        (
            "builtin-format-convert",
            "格式转换",
            "将文档从一种格式转换为另一种格式",
            "Please convert the specified file from source format to target format, ensuring conversion quality and content integrity.\n\n## Conversion Workflow\n1. Source Inspection — read source file to confirm format and content integrity\n2. Execute Conversion — use appropriate document processing tool\n3. Post-Validation — read target file to verify content completeness and format correctness\n4. Limitations Note — document any features not supported by target format (e.g., Excel formulas to PDF)\n\n## Quality Requirements\n- Text: fully preserved, no garbled characters or loss\n- Tables: row/column structure and merged cells preserved\n- Images: preserved where possible, note position changes\n- Styles: formatting preserved as much as possible (font, size, color, alignment)\n\n## Output\n- Provide conversion summary (source size, target size, duration, notes)\n\nSource file: {{inputPath}}\nSource format: {{sourceFormat}}\nTarget format: {{targetFormat}}\nOutput path: {{outputPath}}",
            "conversion",
        ),
        (
            "builtin-doc-review",
            "文档审阅",
            "审阅文档内容，提出修改建议",
            "Please perform a professional review of the specified document, evaluating quality across multiple dimensions with specific recommendations.\n\n## Review Dimensions & Scoring\nRate each dimension (1-5) with comments:\n1. Accuracy — facts, data, citations correct?\n2. Logic — clear structure, evidence supporting claims, sound reasoning\n3. Completeness — all required elements present, no significant gaps\n4. Standards Compliance — format, terminology, citation standards met\n5. Readability — clear and concise language suitable for target audience\n\n## Issue Format\nFor each issue found, report:\n- Location: page/section\n- Type: Accuracy/Logic/Completeness/Standards/Readability\n- Severity: Critical/Major/Minor\n- Description: specific problem identified\n- Suggestion: actionable fix recommendation\n\n## Summary\nProvide overall assessment: total score, key strengths, priority improvement areas, critical issues list\n\nFile path: {{filePath}}\nReview focus: {{focus}}",
            "analysis",
        ),
        (
            "builtin-ppt-outline",
            "PPT大纲生成",
            "根据主题生成PPT大纲和内容",
            "Please generate a professional PowerPoint presentation (.pptx) based on the topic below, with substantial content, professional visuals, suitable for formal presentation.\n\n## Content Structure\n1. Title slide — title, subtitle (optional), date, presenter\n2. Table of Contents — list of sections\n3. Content slides — each with title, 3-5 bullet points, optional diagrams or tables\n4. Section divider slides — before each major section\n5. Summary slide — key takeaways, next steps\n6. Appendix slides (optional) — supporting data, references\n\n## Design Guidelines\n- Consistent color scheme and font style throughout\n- Title font size >= 28pt, body font size >= 18pt\n- Keep slides uncluttered, follow \"less is more\" principle\n- Use charts for data rather than text tables\n- Add speaker notes for key slides\n\n## Output\n- Save as PPTX to workspace\n- Filename format: Presentation_Topic.pptx\n- Run validator to verify quality\n\nTopic: {{topic}}\nTarget slide count: about {{pageCount}} pages",
            "document",
        ),
        (
            "builtin-tech-proposal",
            "技术方案文档",
            "生成专业技术方案文档",
            "Please generate a complete technical proposal Word document (.docx) for project review and technical decision-making.\n\n## Content Structure\n1. Cover page — project name, document version, date, classification\n2. Revision history — table: version, date, changes, author\n3. Glossary — terms and abbreviations used\n4. Project Overview — background, objectives, constraints\n5. Requirements Analysis — functional summary, non-functional requirements (performance, security, availability)\n6. Architecture Design — architecture description, technology choices with rationale, module breakdown\n7. Detailed Design — core design points, interface definitions, data model\n8. Implementation Plan — phases, milestones, resource needs, deliverables\n9. Risk Assessment — technical risks, mitigation strategies\n\n## Document Format\n- Title: bold 18pt centered; H1: bold 14pt; H2: bold 12pt\n- Body: 10.5pt, 1.25 line spacing\n- Code blocks in monospace font with gray background\n- Header: project name centered; Footer: page number centered\n\n## Output\n- Save to workspace, filename format: Technical_Proposal_ProjectName_YYYY-MM-DD.docx\n- Run validator to verify quality\n\nProject name: {{projectName}}\nBackground: {{background}}\nCore requirements: {{requirements}}",
            "document",
        ),
        (
            "builtin-business-letter",
            "商务公函",
            "生成正式商务信函文档",
            "Please generate a formal business letter Word document (.docx) with proper formatting and appropriate language.\n\n## Content Structure\n1. Letterhead — organization name, logo placeholder, address, contact info\n2. Title — Regarding: [Subject], centered bold\n3. Reference number — organization acronym [Year] No.X, right-aligned\n4. Recipient — full organization name, flush left\n5. Body — opening (context), main content (structured points), closing (thanks or reply request)\n6. Signature — organization name, date, contact person\n7. Attachment note (if any) — name and quantity\n\n## Style Requirements\n- Formal, concise, accurate language with standard business wording\n- Clear paragraphs, one topic per paragraph\n- Cite sources for data or references\n- Avoid vague expressions; specify time, quantities clearly\n\n## Document Format\n- Title: 16pt bold centered; Body: 12pt, 1.5 line spacing\n- Margins: top/bottom 3.7cm, left/right 2.8cm (official document standard)\n- Page number: bottom center\n\n## Output\n- Save to workspace, filename format: Letter_Subject_YYYY-MM-DD.docx\n- Run validator to verify quality\n\nRecipient: {{recipient}}\nPurpose: {{purpose}}\nDetails: {{details}}",
            "document",
        ),
        (
            "builtin-project-plan",
            "项目计划书",
            "生成项目计划文档",
            "Please generate a complete project plan Word document (.docx) for project initiation and execution tracking.\n\n## Content Structure\n1. Cover — project name, version, date, authoring organization\n2. Revision history — version, date, changes, author\n3. Project Overview — background, SMART objectives, scope, key stakeholders\n4. WBS — work breakdown by phase/module using multi-level lists\n5. Milestone Plan — table: milestone name, deliverable, acceptance criteria, target date\n6. Detailed Schedule — table: task ID, name, dependencies, owner, start/end dates, duration\n7. Resource Plan — human resources, equipment/tools, budget estimate\n8. Quality Management — quality standards, review mechanism, acceptance process\n9. Risk Management — risk register: description, probability, impact, strategy, owner\n10. Communication Plan — audience, method, frequency, content\n\n## Document Format\n- Title: bold 18pt centered; H1: bold 14pt; H2: bold 12pt\n- Body: 10.5pt, 1.25 line spacing\n- Professional styled tables with color-coded key rows/columns\n- Header: project name; Footer: page number\n\n## Output\n- Save to workspace, filename format: Project_Plan_ProjectName_YYYY-MM-DD.docx\n- Run validator to verify quality\n\nProject name: {{projectName}}\nObjectives: {{objectives}}\nTimeline: {{timeline}}",
            "document",
        ),
    ];

    // 全量重新插入：先删除所有内置模板，再重新写入种子列表
    // 这样用户从种子列表中移除的模板会自动消失，新增的会自动出现
    conn.execute("DELETE FROM prompt_templates WHERE is_builtin = 1", [])?;

    for (id, name, description, content, category) in &builtin_templates {
        conn.execute(
            "INSERT INTO prompt_templates (id, name, description, content, category, is_builtin, variables, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, NULL, ?6, ?7)",
            rusqlite::params![id, name, description, content, category, now, now],
        )?;
    }

    log::info!("已同步 {} 个内置模板", builtin_templates.len());
    Ok(())
}
