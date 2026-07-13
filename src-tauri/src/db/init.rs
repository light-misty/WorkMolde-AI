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

    // 迁移：为已有数据库的 session_messages 表添加 metadata 字段（新字段已存在时忽略错误）
    let _ = conn.execute(
        "ALTER TABLE session_messages ADD COLUMN metadata TEXT DEFAULT NULL",
        [],
    );

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
            "请生成一份专业的周报 Word 文档（.docx），要求结构清晰、数据可追溯。\n\n## 内容结构要求\n1. 本周工作总结 — 使用表格列出：任务名称、优先级（高/中/低）、状态（已完成/进行中/未开始）、完成度（百分比）、备注\n2. 关键进展 — 突出 2-3 个核心里程碑成果，必要时用数据支撑\n3. 遇到的问题 — 表格列出：问题描述、影响范围、严重程度、当前状态（已解决/处理中/待协调）\n4. 风险与应对 — 识别当前主要风险，说明缓解措施和责任人\n5. 下周计划 — 按优先级排列的详细工作计划，含预期交付物和时间节点\n\n## 文档格式要求\n- 标题：黑体 18pt 加粗居中；一级标题：黑体 14pt 加粗；二级标题：黑体 12pt 加粗\n- 正文：宋体 10.5pt，1.25 倍行距\n- 表格：使用带样式的表格，表头加粗并填充浅灰色底纹\n- 页边距：上下 2.54cm，左右 3.17cm（Word 默认标准）\n- 页面设置：A4 纸张\n\n## 输出要求\n- 保存到工作区，文件名格式：周报_YYYY-MM-DD.docx\n- 生成后使用 validator_handler 检查文档质量\n\n工作内容如下：\n{{content}}",
            "document",
        ),
        (
            "builtin-meeting-minutes",
            "会议纪要",
            "根据会议信息生成规范的会议纪要文档",
            "请生成一份规范的会议纪要 Word 文档（.docx），确保信息完整、决策可追溯、行动项可跟踪。\n\n## 内容结构要求\n1. 会议基本信息 — 会议主题、时间、地点、主持人、记录人、参会人员列表、缺席人员\n2. 会议议程 — 列出本次会议的议题清单\n3. 讨论内容 — 按议题逐一记录：议题名称、讨论要点、各方观点、结论\n4. 决议事项 — 列出本次会议通过的所有决议（编号、决议内容、表决结果）\n5. 行动项跟踪表 — 表格列出：编号、行动项描述、负责人、优先级（高/中/低）、截止日期、状态\n6. 下次会议安排 — 时间、地点、拟讨论议题（如有）\n\n## 文档格式要求\n- 标题：黑体 18pt 加粗居中；一级标题：黑体 14pt 加粗；二级标题：黑体 12pt 加粗\n- 正文：宋体 10.5pt，1.25 倍行距\n- 行动项表格需包含自动编号列\n- 页边距：Word 默认标准\n\n## 输出要求\n- 保存到工作区，文件名格式：会议纪要_YYYY-MM-DD_议题.docx\n- 生成后使用 validator_handler 检查文档质量\n\n会议信息如下：\n{{content}}",
            "document",
        ),
        (
            "builtin-data-analysis",
            "数据分析",
            "对Excel数据进行统计分析并生成分析报告",
            "请对指定的数据文件进行专业统计分析，生成一份完整的数据分析报告。\n\n## 分析流程\n1. 数据概览 — 读取数据后先给出整体概览（行数、列数、数据类型、缺失值统计、基本统计量）\n2. 数据清洗说明 — 如有缺失值处理、异常值处理、数据变换等，需说明处理方法和理由\n3. 关键指标分析 — 针对分析重点计算核心统计指标（均值、中位数、标准差、分位数、增长率等）\n4. 趋势分析 — 识别数据中的趋势、周期性、季节性模式，使用适当图表辅助说明\n5. 对比分析 — 按分析重点进行分组对比、前后对比或基准对比\n6. 异常发现 — 标注异常数据点，分析可能原因和影响\n7. 结论与建议 — 基于数据给出可操作的结论和建议\n\n## 文档格式要求\n- 报告需有封面页（标题、日期、分析人）、目录页、正文、附录\n- 正文中的图表需有图号和标题（如图 1：季度销售趋势）\n- 字体：标题黑体，正文宋体 10.5pt，1.25 倍行距\n- 对报告中引用的统计方法在附录中简要说明\n\n## 输出要求\n- 保存为 Word 文档到工作区\n- 文件名格式：数据分析_YYYY-MM-DD.docx\n- 如需可视化图表，使用 matplotlib 生成并嵌入 docx\n- 生成后使用 validator_handler 检查文档质量\n\n文件路径：{{filePath}}\n分析重点：{{focus}}",
            "analysis",
        ),
        (
            "builtin-format-convert",
            "格式转换",
            "将文档从一种格式转换为另一种格式",
            "请将指定文件从源格式转换为目标格式，确保转换质量和内容完整性。\n\n## 转换流程要求\n1. 源文件检测 — 先读取源文件确认其格式和内容完整性\n2. 转换执行 — 使用对应的文档处理工具进行格式转换\n3. 后验证 — 转换后读取目标文件，验证内容完整、格式正确\n4. 差异说明 — 如目标格式不支持源格式的某些特性（如 Excel 公式转 PDF），需在说明中标注\n\n## 质量要求\n- 文本内容：完全保留，不能出现乱码或丢失\n- 表格：保持行列结构和合并单元格信息\n- 图片：尽可能保留，标注位置变化\n- 样式：尽量保留原格式（字体、字号、颜色、对齐方式）\n\n## 输出要求\n- 转换完成时输出转换结果摘要（源文件大小、目标文件大小、耗时、注意事項）\n\n源文件路径：{{inputPath}}\n源格式：{{sourceFormat}}\n目标格式：{{targetFormat}}\n输出路径：{{outputPath}}",
            "conversion",
        ),
        (
            "builtin-doc-review",
            "文档审阅",
            "审阅文档内容，提出修改建议",
            "请对指定文档进行专业审阅，从多个维度评估文档质量并给出具体修改建议。\n\n## 审阅维度与评分标准\n请按以下维度评分（1-5 分）并给出评语：\n1. 准确性 — 事实、数据、引用是否正确无误\n2. 逻辑性 — 结构是否清晰，论点与论据是否匹配，推理是否严密\n3. 完整性 — 是否覆盖了应有的内容要素，有无明显遗漏\n4. 规范性 — 格式、术语、引用标准是否符合规范\n5. 可读性 — 语言表达是否清晰简洁，是否适合目标读者\n\n## 审阅意见格式\n对每个问题按以下格式输出：\n- 位置：页码/章节\n- 类型：准确性/逻辑性/完整性/规范性/可读性\n- 严重程度：严重/中等/轻微\n- 问题描述：具体指出问题\n- 修改建议：给出可操作的修改方案\n\n## 综合评估\n在审阅结束后给出综合评估：总体评分、主要优势、关键改进方向、建议优先处理的高严重度问题清单\n\n文件路径：{{filePath}}\n审阅重点：{{focus}}",
            "analysis",
        ),
        (
            "builtin-ppt-outline",
            "PPT大纲生成",
            "根据主题生成PPT大纲和内容",
            "请根据以下主题生成一份专业的 PPT 演示文稿（.pptx），要求内容充实、视觉规范、适合正式演示场景。\n\n## 内容结构要求\n1. 封面页 — 标题、副标题（如有）、日期、汇报人\n2. 目录页 — 列出各章节标题\n3. 内容页 — 每页包含：标题、要点（3-5 个要点，用项目符号列表）、必要时配图或表格\n4. 过渡页 — 在每个主要章节前插入过渡页\n5. 总结页 — 核心结论回顾、下一步行动\n6. 附录页（可选）— 补充数据、参考资料\n\n## 设计规范\n- 使用统一的配色方案和字体风格\n- 标题字号不低于 28pt，正文字号不低于 18pt\n- 每页内容不宜过满，遵循「少即是多」原则\n- 数据用图表展示优先于文字表格\n- 为关键页面添加演讲者备注\n\n## 输出要求\n- 保存为 PPTX 格式到工作区\n- 文件名格式：演示文稿_主题.pptx\n- 生成后使用 validator_handler 检查文档质量\n\n主题：{{topic}}\n页数要求：{{pageCount}} 页左右",
            "document",
        ),
        (
            "builtin-tech-proposal",
            "技术方案文档",
            "生成专业技术方案文档",
            "请生成一份完整的技术方案 Word 文档（.docx），用于项目评审和技术决策。\n\n## 内容结构要求\n1. 文档封面 — 项目名称、文档版本、编制日期、密级\n2. 修订记录 — 表格列出：版本号、修订日期、修订内容、修订人\n3. 术语和缩写表 — 列出文档中使用的专业术语和缩写\n4. 项目概述 — 项目背景、目标范围、约束条件\n5. 需求分析 — 功能需求摘要、非功能需求（性能、安全、可用性等）\n6. 总体架构设计 — 架构图（用文字描述或使用 mermaid 图）、技术选型及选型理由、系统模块划分\n7. 详细设计 — 各模块的核心设计要点、接口定义、数据模型\n8. 实施计划 — 阶段划分、里程碑、资源需求、交付物清单\n9. 风险评估 — 识别技术风险、应对策略\n\n## 文档格式要求\n- 标题：黑体 18pt 加粗居中；一级标题：黑体 14pt 加粗；二级标题：黑体 12pt 加粗\n- 正文：宋体 10.5pt，1.25 倍行距\n- 代码块使用等宽字体，灰底框显示\n- 页眉：项目名称居中；页脚：页码居中\n\n## 输出要求\n- 保存到工作区，文件名格式：技术方案_项目名称_YYYY-MM-DD.docx\n- 生成后使用 validator_handler 检查文档质量\n\n项目名称：{{projectName}}\n项目背景：{{background}}\n核心需求：{{requirements}}",
            "document",
        ),
        (
            "builtin-business-letter",
            "商务公函",
            "生成正式商务信函文档",
            "请生成一份正式的商务公函 Word 文档（.docx），格式规范、措辞得体。\n\n## 内容结构要求\n1. 信头 — 发函单位名称、Logo 占位、地址、联系方式\n2. 标题 — 关于XXX的函，居中加粗\n3. 函号 — 发函单位简称〔年份〕X号，右对齐\n4. 收函方 — 收函单位全称，顶格左对齐\n5. 正文 — 开头（事由引述）、主体（分条陈述）、结尾（致谢或盼复）\n6. 落款 — 发函单位全称、日期（加盖公章占位）、联系人信息\n7. 附件标注（如有）— 附件名称和份数\n\n## 文风要求\n- 语言正式、简洁、准确，使用规范公文用语\n- 段落分明，一事一段\n- 涉及数据或引用需标注来源\n- 避免使用模糊表达，时间、数量等需具体\n\n## 文档格式要求\n- 标题：宋体 16pt 加粗居中；正文：宋体 12pt，1.5 倍行距\n- 页边距：上下 3.7cm，左右 2.8cm（公文标准）\n- 页码：页面底端居中\n\n## 输出要求\n- 保存到工作区，文件名格式：公函_标题_YYYY-MM-DD.docx\n- 生成后使用 validator_handler 检查文档质量\n\n收函方：{{recipient}}\n发函事由：{{purpose}}\n详细内容：{{details}}",
            "document",
        ),
        (
            "builtin-project-plan",
            "项目计划书",
            "生成项目计划文档",
            "请生成一份完整的项目计划书 Word 文档（.docx），用于项目启动和执行跟踪。\n\n## 内容结构要求\n1. 封面 — 项目名称、版本号、编制日期、编制单位\n2. 修订记录 — 版本、日期、修订内容、修订人\n3. 项目概况 — 项目背景、目标（SMART 原则）、范围界定、关键干系人\n4. 工作分解结构（WBS）— 按阶段/模块分解工作任务，使用多层列表\n5. 里程碑计划 — 表格列出：里程碑名称、交付物、验收标准、计划完成日期\n6. 详细进度安排 — 表格列出：任务编号、任务名称、前置任务、责任人、计划开始/结束日期、持续时间\n7. 资源计划 — 人力资源需求、设备/工具需求、预算概算\n8. 质量管理 — 质量标准、评审机制、验收流程\n9. 风险管理 — 风险识别表：风险描述、概率、影响等级、应对策略、责任人\n10. 沟通计划 — 沟通对象、沟通方式、频率、内容\n\n## 文档格式要求\n- 标题：黑体 18pt 加粗居中；一级标题：黑体 14pt 加粗；二级标题：黑体 12pt 加粗\n- 正文：宋体 10.5pt，1.25 倍行距\n- 表格使用带样式的专业表格，关键行列用颜色区分\n- 页眉：项目名称；页脚：页码\n\n## 输出要求\n- 保存到工作区，文件名格式：项目计划书_项目名称_YYYY-MM-DD.docx\n- 生成后使用 validator_handler 检查文档质量\n\n项目名称：{{projectName}}\n项目目标：{{objectives}}\n时间安排：{{timeline}}",
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
