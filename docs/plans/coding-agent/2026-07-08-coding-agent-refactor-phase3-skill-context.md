# 阶段 3:Skill 系统与上下文管理 详细改造文档

> **文档版本**:v1.1(2026-07-08 修订:Skill 模式过滤支持 document 模式)
> **创建日期**:2026-07-08
> **阶段目标**:实现 Skill 系统(领域能力注入)、TodoWrite 工具(结构化任务管理)、SessionCompaction(上下文压缩)、SourceCode 工具(代码语义搜索),全面提升 Agent 的上下文管理与代码理解能力
> **依赖阶段**:[阶段 1:核心架构与工具链](./2026-07-08-coding-agent-refactor-phase1-core.md)、[阶段 2:权限系统与 Agent 模式](./2026-07-08-coding-agent-refactor-phase2-permission.md)
> **预计任务数**:22 个(T3.01-T3.22)
> **v1.1 修订**:Skill 的 `is_applicable_to_mode` 和 `list_by_mode` 需支持 "document" 模式字符串(阶段 2 新增的 Document AgentMode)

---

## 一、阶段概述

### 1.1 改造背景

OpenCode 通过 Skill 系统、TodoWrite 工具、SessionCompaction(上下文压缩)、SourceCode(代码语义搜索)四大模块,构建了强大的上下文管理能力:

1. **Skill 系统**:从 `.opencode/skills/*/SKILL.md` 加载领域能力,通过 frontmatter 声明 Skill 元数据(名称、描述、触发条件),markdown 内容作为 Skill 详细说明。系统提示词注入可用 Skill 清单,Agent 通过 `skill` 工具按需加载,实现"按需加载领域能力"。

2. **TodoWrite 工具**:结构化任务管理工具,支持 `pending`/`in_progress`/`completed` 三种状态,跨迭代保持任务状态,Agent 自主维护任务清单。

3. **SessionCompaction**:上下文接近溢出时触发压缩,生成"继续工作所需摘要"而非原样保留全部历史,对旧工具输出做 prune(保留必要信息,释放 token)。

4. **SourceCode 工具**:基于 tree-sitter 的代码语义搜索,支持按符号类型(函数、类、方法)查询,精准定位代码。

### 1.2 WorkMolde AI 现状

- **Scratchpad 工具**:已有的草稿本工具,按 session_id 隔离,每轮迭代注入摘要,但缺乏结构化任务管理能力
- **无 Skill 系统**:Agent 能力完全依赖 System Prompt,无法按需加载领域能力
- **无上下文压缩**:上下文溢出时直接截断早期消息,可能丢失关键信息
- **无代码语义搜索**:`search` 仅支持文本搜索,无法理解代码结构

### 1.3 改造目标

1. **实现 Skill 系统**:支持从多个目录加载 Skill,系统提示词注入清单,Agent 按需加载
2. **实现 TodoWrite 工具**:替代 Scratchpad,提供结构化任务管理
3. **实现 SessionCompaction**:智能压缩上下文,保留关键信息
4. **实现 SourceCode 工具**:基于 tree-sitter 的代码语义搜索
5. **保留 Scratchpad 兼容性**:作为草稿本保留,但任务管理由 TodoWrite 接管

### 1.4 设计原则

- **渐进式增强**:Skill/TodoWrite/Compaction/SourceCode 互相独立,可分步实施
- **权限系统复用**:Skill 工具受 `PermissionType::Skill` 控制
- **Agent 模式感知**:Plan 模式下 TodoWrite 可用,SourceCode 可用,Skill 可全部加载（只读约束由 Plan 模式的 check_permission 保证）
- **最小侵入**:尽量复用现有 AgentContext、ToolRegistry 架构

---

## 二、任务依赖图

```
T3.01 (依赖) ── T3.02 (类型) ── T3.03 (SkillRepo)
                                    │
                                    ├── T3.04 (SkillLoader)
                                    │       │
                                    │       ├── T3.05 (SkillRegistry)
                                    │       │       │
                                    │       │       ├── T3.06 (SkillTool)
                                    │       │       │       │
                                    │       │       │       └── T3.07 (系统提示词注入)
                                    │       │       │
                                    │       │       └── T3.08 (Skill 权限过滤)
                                    │       │
                                    │       └── T3.09 (Skill 热重载)
                                    │
T3.10 (TodoWrite 模型) ── T3.11 (TodoWriteRepo) ── T3.12 (TodoWriteTool)
                                                            │
                                                            └── T3.13 (集成到 AgentContext)
                                                                    │
                                                                    └── T3.14 (替代 Scratchpad 摘要)

T3.15 (Compaction 配置) ── T3.16 (Compaction 策略) ── T3.17 (Compaction 集成)
                                                            │
                                                            └── T3.18 (Compaction 事件)

T3.19 (tree-sitter 依赖) ── T3.20 (LanguageParser) ── T3.21 (SourceCodeTool)
                                                            │
                                                            └── T3.22 (集成测试)
```

---

## 三、任务清单

### T3.01:新增 Skill 系统所需依赖到 Cargo.toml

**文件**:
- 修改:`src-tauri/Cargo.toml`

**实施内容**:
```toml
[dependencies]
# 现有依赖...

# Skill 系统:YAML frontmatter 解析
serde_yaml = "0.9"
# Skill 系统:markdown 解析(轻量级,仅提取 frontmatter)
yaml-front-matter = "0.1"
```

**验证**:
- `cargo build -p workmolde_lib` 成功

---

### T3.02:定义 Skill 类型与数据模型

**文件**:
- 创建:`src-tauri/src/models/skill.rs`
- 修改:`src-tauri/src/models/mod.rs`(添加 `pub mod skill;`)

**实施内容**:
```rust
//! Skill 模型定义
//! Skill 是可注入的领域能力包,通过 SKILL.md 文件定义

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Skill 来源类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillSource {
    /// 全局目录(~/.agent/skills/)
    Global,
    /// 项目目录(.agent/skills/)
    Project,
    /// 配置路径
    Configured,
}

/// Skill frontmatter 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillFrontmatter {
    /// Skill 名称(唯一标识)
    pub name: String,
    /// 简短描述(用于系统提示词清单)
    pub description: String,
    /// 触发条件(可选,Agent 据此判断是否加载)
    #[serde(default)]
    pub when: Option<String>,
    /// 适用 Agent 模式(可选,默认 ["plan", "build", "document"])
    /// v1.1: 新增 "document" 模式支持
    /// 文档相关 Skill 可设置为 ["document"] 仅在 Document 模式下可见
    #[serde(default)]
    pub modes: Vec<String>,
    /// 标签(可选,用于分类)
    #[serde(default)]
    pub tags: Vec<String>,
    /// 是否为只读 Skill(不修改文件)
    #[serde(default)]
    pub read_only: bool,
}

/// Skill 完整定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    /// Skill 元数据
    pub frontmatter: SkillFrontmatter,
    /// markdown 正文(Skill 详细说明)
    pub content: String,
    /// Skill 来源
    pub source: SkillSource,
    /// SKILL.md 文件路径
    pub file_path: PathBuf,
    /// Skill 目录路径(SKILL.md 所在目录)
    pub dir_path: PathBuf,
    /// 最后修改时间(UNIX 时间戳,秒)
    pub modified_at: u64,
}

impl Skill {
    /// 检查 Skill 是否适用于指定 Agent 模式
    /// v1.1: 支持 "plan" / "build" / "document" 三种模式字符串
    /// 若 modes 为空,默认适用于所有模式(含 document)
    pub fn is_applicable_to_mode(&self, mode: &str) -> bool {
        if self.frontmatter.modes.is_empty() {
            // 默认适用于所有模式(plan/build/document)
            return true;
        }
        self.frontmatter.modes.iter().any(|m| m == mode)
    }

    /// 生成系统提示词中的 Skill 清单条目
    pub fn to_summary_line(&self) -> String {
        let when_hint = self
            .frontmatter
            .when
            .as_ref()
            .map(|w| format!(" (when: {})", w))
            .unwrap_or_default();
        format!(
            "- {}: {}{}",
            self.frontmatter.name, self.frontmatter.description, when_hint
        )
    }
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功

---

### T3.03:实现 Skill 仓库与持久化

**文件**:
- 创建:`src-tauri/src/db/skill_repo.rs`
- 修改:`src-tauri/src/db/mod.rs`(添加 `pub mod skill_repo;`)
- 修改:`src-tauri/src/db/init.rs`(添加 skill_overrides 表)

**实施内容**:

**数据库表**(添加到 `init.rs` 的 `create_tables` 函数):
```rust
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
    );"
)?;
```

**skill_repo.rs**:
```rust
//! Skill 仓库:管理 Skill 的启用/禁用配置
//! Skill 内容本身从文件系统加载,数据库仅存储用户覆盖

use rusqlite::Connection;
use crate::errors::CommandError;
use serde::{Deserialize, Serialize};

/// Skill 覆盖配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillOverride {
    pub id: String,
    pub skill_name: String,
    pub workspace_id: String,
    pub enabled: bool,
    pub custom_config: Option<String>,
}

/// 插入或更新 Skill 覆盖配置
pub fn upsert_override(conn: &Connection, override_config: &SkillOverride) -> Result<(), CommandError> {
    conn.execute(
        "INSERT INTO skill_overrides (id, skill_name, workspace_id, enabled, custom_config, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(skill_name, workspace_id) DO UPDATE SET
            enabled = excluded.enabled,
            custom_config = excluded.custom_config,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        rusqlite::params![
            override_config.id,
            override_config.skill_name,
            override_config.workspace_id,
            override_config.enabled as i32,
            override_config.custom_config,
        ],
    )?;
    Ok(())
}

/// 查询指定工作区的 Skill 覆盖配置
pub fn list_overrides_by_workspace(
    conn: &Connection,
    workspace_id: &str,
) -> Result<Vec<SkillOverride>, CommandError> {
    let mut stmt = conn.prepare(
        "SELECT id, skill_name, workspace_id, enabled, custom_config FROM skill_overrides WHERE workspace_id = ?1"
    )?;
    let overrides = stmt.query_map(rusqlite::params![workspace_id], |row| {
        Ok(SkillOverride {
            id: row.get(0)?,
            skill_name: row.get(1)?,
            workspace_id: row.get(2)?,
            enabled: row.get::<_, i32>(3)? != 0,
            custom_config: row.get(4)?,
        })
    })?;
    overrides.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// 删除 Skill 覆盖配置
pub fn delete_override(conn: &Connection, id: &str) -> Result<(), CommandError> {
    conn.execute("DELETE FROM skill_overrides WHERE id = ?1", rusqlite::params![id])?;
    Ok(())
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 数据库初始化时 `skill_overrides` 表创建成功

---

### T3.04:实现 Skill 加载器

**文件**:
- 创建:`src-tauri/src/services/skill/loader.rs`
- 创建:`src-tauri/src/services/skill/mod.rs`

**实施内容**:

**mod.rs**:
```rust
//! Skill 系统模块入口
pub mod loader;
pub mod registry;
pub mod tool;

pub use loader::SkillLoader;
pub use registry::SkillRegistry;
pub use tool::SkillTool;
```

**loader.rs**:
```rust
//! Skill 加载器:从文件系统扫描并解析 SKILL.md 文件
//! 支持三个加载目录:全局(~/.agent/skills/)、项目(.agent/skills/)、配置路径

use crate::models::skill::{Skill, SkillFrontmatter, SkillSource};
use crate::errors::CommandError;
use std::path::{Path, PathBuf};
use yaml_front_matter::{YamlFrontMatter, Document};

/// Skill 加载器
pub struct SkillLoader {
    /// 全局 Skill 目录(~/.agent/skills/)
    global_dir: PathBuf,
    /// 项目 Skill 目录(.agent/skills/),可空
    project_dir: Option<PathBuf>,
    /// 配置的额外 Skill 目录
    extra_dirs: Vec<PathBuf>,
}

impl SkillLoader {
    /// 创建 Skill 加载器
    pub fn new(
        global_dir: PathBuf,
        project_dir: Option<PathBuf>,
        extra_dirs: Vec<PathBuf>,
    ) -> Self {
        Self {
            global_dir,
            project_dir,
            extra_dirs,
        }
    }

    /// 扫描所有目录并加载 Skill
    /// 返回按 Skill 名称去重后的列表(项目目录覆盖全局目录)
    pub fn load_all(&self) -> Result<Vec<Skill>, CommandError> {
        let mut skills: Vec<Skill> = Vec::new();
        let mut seen_names: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        // 加载顺序:全局 -> 配置 -> 项目(后加载的覆盖先加载的)
        let sources = [
            (SkillSource::Global, &self.global_dir),
        ];

        // 加载全局目录
        self.load_from_dir(&self.global_dir, SkillSource::Global, &mut skills, &mut seen_names)?;

        // 加载配置目录
        for dir in &self.extra_dirs {
            self.load_from_dir(dir, SkillSource::Configured, &mut skills, &mut seen_names)?;
        }

        // 加载项目目录(优先级最高)
        if let Some(project_dir) = &self.project_dir {
            self.load_from_dir(project_dir, SkillSource::Project, &mut skills, &mut seen_names)?;
        }

        log::info!("已加载 {} 个 Skill", skills.len());
        Ok(skills)
    }

    /// 从单个目录加载 Skill
    fn load_from_dir(
        &self,
        dir: &Path,
        source: SkillSource,
        skills: &mut Vec<Skill>,
        seen_names: &mut std::collections::HashMap<String, usize>,
    ) -> Result<(), CommandError> {
        if !dir.exists() || !dir.is_dir() {
            return Ok(());
        }

        log::debug!("扫描 Skill 目录: {} (来源: {:?})", dir.display(), source);

        // 遍历目录下的子目录,每个子目录应包含 SKILL.md
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let skill_md_path = path.join("SKILL.md");
            if !skill_md_path.exists() {
                continue;
            }

            match self.parse_skill_file(&skill_md_path, &path, source.clone()) {
                Ok(skill) => {
                    let name = skill.frontmatter.name.clone();
                    if let Some(idx) = seen_names.get(&name) {
                        // 同名 Skill,后加载的覆盖先加载的
                        log::info!(
                            "Skill '{}' 被 {:?} 来源覆盖(原来源: {:?})",
                            name,
                            source,
                            skills[*idx].source
                        );
                        skills[*idx] = skill;
                    } else {
                        seen_names.insert(name, skills.len());
                        skills.push(skill);
                    }
                }
                Err(e) => {
                    log::warn!("解析 Skill 文件失败: {} - {}", skill_md_path.display(), e);
                }
            }
        }

        Ok(())
    }

    /// 解析单个 SKILL.md 文件
    fn parse_skill_file(
        &self,
        file_path: &Path,
        dir_path: &Path,
        source: SkillSource,
    ) -> Result<Skill, CommandError> {
        let content = std::fs::read_to_string(file_path)?;

        // 解析 frontmatter
        let document: Document<SkillFrontmatter> = YamlFrontMatter::parse(&content)
            .map_err(|e| CommandError::config(
                crate::errors::CONFIG_FORMAT_INVALID,
                format!("Skill frontmatter 解析失败: {}", e),
            ))?;

        let frontmatter = document.metadata;
        let markdown_content = document.content;

        // 获取文件修改时间
        let metadata = std::fs::metadata(file_path)?;
        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(Skill {
            frontmatter,
            content: markdown_content,
            source,
            file_path: file_path.to_path_buf(),
            dir_path: dir_path.to_path_buf(),
            modified_at,
        })
    }
}
```

**验证**:
- 创建测试用 SKILL.md 文件,验证加载成功
- 验证同名 Skill 的覆盖逻辑

---

### T3.05:实现 Skill 注册表

**文件**:
- 创建:`src-tauri/src/services/skill/registry.rs`

**实施内容**:
```rust
//! Skill 注册表:管理已加载的 Skill,提供查询和过滤功能

use crate::models::skill::Skill;
use crate::services::skill::loader::SkillLoader;
use crate::errors::CommandError;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Skill 注册表
pub struct SkillRegistry {
    /// 已加载的 Skill 列表(按名称索引)
    skills: RwLock<HashMap<String, Skill>>,
    /// Skill 加载器
    loader: SkillLoader,
}

impl SkillRegistry {
    /// 创建 Skill 注册表
    pub fn new(loader: SkillLoader) -> Self {
        Self {
            skills: RwLock::new(HashMap::new()),
            loader,
        }
    }

    /// 加载所有 Skill 到注册表
    pub fn reload(&self) -> Result<usize, CommandError> {
        let skills = self.loader.load_all()?;
        let count = skills.len();
        let mut skills_map = self.skills.write().unwrap();
        skills_map.clear();
        for skill in skills {
            skills_map.insert(skill.frontmatter.name.clone(), skill);
        }
        log::info!("Skill 注册表已加载 {} 个 Skill", count);
        Ok(count)
    }

    /// 获取所有 Skill(按名称排序)
    pub fn list_all(&self) -> Vec<Skill> {
        let skills = self.skills.read().unwrap();
        let mut list: Vec<Skill> = skills.values().cloned().collect();
        list.sort_by(|a, b| a.frontmatter.name.cmp(&b.frontmatter.name));
        list
    }

    /// 按 Agent 模式过滤 Skill
    pub fn list_by_mode(&self, mode: &str) -> Vec<Skill> {
        let skills = self.skills.read().unwrap();
        let mut list: Vec<Skill> = skills
            .values()
            .filter(|s| s.is_applicable_to_mode(mode))
            .cloned()
            .collect();
        list.sort_by(|a, b| a.frontmatter.name.cmp(&b.frontmatter.name));
        list
    }

    /// 按名称获取 Skill
    pub fn get_by_name(&self, name: &str) -> Option<Skill> {
        let skills = self.skills.read().unwrap();
        skills.get(name).cloned()
    }

    /// 生成系统提示词中的 Skill 清单
    pub fn build_summary_for_prompt(&self, mode: &str) -> String {
        let skills = self.list_by_mode(mode);
        if skills.is_empty() {
            return String::new();
        }

        let mut summary = String::from("\n\n## 可用 Skill 清单\n");
        summary.push_str("通过 `skill` 工具加载详细内容后再使用:\n\n");
        for skill in skills {
            summary.push_str(&skill.to_summary_line());
            summary.push('\n');
        }
        summary
    }
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 单元测试:加载测试 Skill 后,`list_by_mode` 返回正确结果

---

### T3.06:实现 Skill 工具

**文件**:
- 创建:`src-tauri/src/services/skill/tool.rs`

**实施内容**:
```rust
//! Skill 工具:Agent 通过此工具按需加载 Skill 的详细内容
//! 系统提示词中仅注入 Skill 清单(名称+描述),详细内容需通过此工具加载

use async_trait::async_trait;
use crate::services::tool::trait_def::Tool;
use crate::services::skill::registry::SkillRegistry;
use crate::models::tool::ToolResult;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct SkillTool {
    /// Skill 注册表引用
    registry: Arc<SkillRegistry>,
}

impl SkillTool {
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for SkillTool {
    fn tool_name(&self) -> &str {
        "skill"
    }

    fn description(&self) -> &str {
        "加载指定 Skill 的详细内容。Skill 是领域能力包,包含特定领域的最佳实践、规范和工具使用指南。系统提示词中仅展示 Skill 清单,需通过此工具加载详细内容后才能完整使用 Skill 能力。"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["load", "list"],
                    "description": "操作类型:load=加载指定 Skill 的详细内容;list=列出所有可用 Skill"
                },
                "name": {
                    "type": "string",
                    "description": "要加载的 Skill 名称(action=load 时必填)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        params: Value,
        workspace_root: &str,
    ) -> Result<ToolResult, crate::errors::CommandError> {
        let action = params.get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("list");

        match action {
            "list" => {
                let skills = self.registry.list_all();
                let list: Vec<Value> = skills.iter().map(|s| {
                    json!({
                        "name": s.frontmatter.name,
                        "description": s.frontmatter.description,
                        "when": s.frontmatter.when,
                        "readOnly": s.frontmatter.read_only,
                        "tags": s.frontmatter.tags,
                    })
                }).collect();

                Ok(ToolResult {
                    success: true,
                    result: json!({
                        "skills": list,
                        "total": list.len(),
                    }),
                    error: None,
                    metadata: None,
                })
            }
            "load" => {
                let name = params.get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| crate::errors::CommandError::tool(
                        crate::errors::TOOL_INVALID_PARAMS,
                        "action=load 时必须提供 name 参数",
                    ))?;

                let skill = self.registry.get_by_name(name)
                    .ok_or_else(|| crate::errors::CommandError::tool(
                        crate::errors::TOOL_NOT_FOUND,
                        format!("Skill '{}' 不存在", name),
                    ))?;

                Ok(ToolResult {
                    success: true,
                    result: json!({
                        "name": skill.frontmatter.name,
                        "description": skill.frontmatter.description,
                        "when": skill.frontmatter.when,
                        "modes": skill.frontmatter.modes,
                        "tags": skill.frontmatter.tags,
                        "readOnly": skill.frontmatter.read_only,
                        "content": skill.content,
                        "source": skill.source,
                        "filePath": skill.file_path.to_string_lossy(),
                    }),
                    error: None,
                    metadata: Some(json!({
                        "skillName": name,
                        "loaded": true,
                    })),
                })
            }
            _ => Err(crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                format!("未知 action: {}", action),
            )),
        }
    }
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 单元测试:调用 `skill` 工具加载测试 Skill,验证返回内容

---

### T3.07:Skill 清单注入系统提示词

**文件**:
- 修改:`src-tauri/src/services/agent/context.rs`

**实施内容**:

在 `AgentContext` 中添加 Skill 注册表引用,并在构建系统提示词时注入 Skill 清单:

```rust
// 在 AgentContext 结构体中添加字段
pub struct AgentContext {
    // 现有字段...
    /// Skill 注册表(可选,未加载时为 None)
    pub skill_registry: Option<Arc<SkillRegistry>>,
}

// 在 build_system_prompt 方法中注入 Skill 清单
impl AgentContext {
    pub fn build_system_prompt(&self, mode: &str) -> String {
        let mut prompt = String::new();
        
        // 基础系统提示词...
        
        // 注入 Skill 清单(如果有)
        if let Some(registry) = &self.skill_registry {
            let skill_summary = registry.build_summary_for_prompt(mode);
            if !skill_summary.is_empty() {
                prompt.push_str(&skill_summary);
            }
        }
        
        // 其他上下文...
        prompt
    }
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 启动 Agent 时,系统提示词中包含 Skill 清单

---

### T3.08:Skill 权限过滤（已移除 Plan 模式 read_only 检查）

> **变更说明**:`compaction-skill-refinement` spec 移除了 Plan 模式下对 Skill `read_only` 字段的权限检查。原计划在 executor.rs 中检查 `read_only` 字段并拒绝加载非只读 Skill 的代码已被移除。移除后 Plan 模式下所有 Skill 均可加载，Plan 模式的只读约束由 `check_permission` 中的 `is_modification()` 检查保证（edit/bash/write 等修改类工具在 Plan 模式下被拒绝）。

**文件**:
- 不再需要修改（原 T3.08 代码块已从 executor.rs 移除）

**实施说明**:
- Skill 加载本身是只读操作（仅读取 markdown 内容），不需要在 Plan 模式下限制
- Plan 模式的只读约束由 `check_permission` 统一处理，通过 `is_modification()` 拒绝修改类工具
- SkillTool 的 `execute` 方法仅处理业务逻辑，权限检查由 AgentExecutor 统一管理（`PermissionType::Skill`）

**验证**:
- Plan 模式下所有 Skill 均可被加载（不再检查 `read_only` 字段）
- Plan 模式下 edit/bash/write 等修改类工具仍被 `check_permission` 的 `is_modification()` 检查拒绝
- Build/Document 模式下 Skill 加载行为不变（原本就允许所有 Skill）
- v1.1 新增:Document 模式下,所有 Skill 可正常加载(同 Build 模式)
- v1.1 新增:`modes: ["document"]` 的 Skill 仅在 Document 模式下可见

---

### T3.09:Skill 热重载与文件监听

**文件**:
- 修改:`src-tauri/src/services/fs_watcher.rs`
- 修改:`src-tauri/src/services/skill/registry.rs`

**实施内容**:

**在 FsWatcherService 中添加 Skill 目录监听**:
```rust
// 在 FsWatcherService 中添加方法
impl FsWatcherService {
    /// 添加 Skill 目录监听
    pub fn watch_skill_directories(&self, dirs: Vec<PathBuf>) -> Result<(), CommandError> {
        for dir in dirs {
            if dir.exists() {
                self.watcher.watch(&dir, RecursiveMode::Recursive)?;
                log::info!("已添加 Skill 目录监听: {}", dir.display());
            }
        }
        Ok(())
    }
}
```

**在 SkillRegistry 中添加热重载触发**:
```rust
impl SkillRegistry {
    /// 触发热重载(由 FsWatcherService 调用)
    pub fn reload_if_changed(&self) -> Result<usize, CommandError> {
        self.reload()
    }
}
```

**在 lib.rs 中连接文件监听与 Skill 重载**:
```rust
// 在文件变更事件处理中,检查是否为 Skill 目录变更
if path.starts_with(skill_dir) {
    if let Err(e) = skill_registry.reload_if_changed() {
        log::warn!("Skill 热重载失败: {}", e);
    }
}
```

**验证**:
- 修改 SKILL.md 文件后,无需重启即可生效
- 日志中显示 Skill 重载信息

---

### T3.10:定义 TodoWrite 数据模型

**文件**:
- 创建:`src-tauri/src/models/todo.rs`
- 修改:`src-tauri/src/models/mod.rs`(添加 `pub mod todo;`)

**实施内容**:
```rust
//! TodoWrite 数据模型
//! 结构化任务管理,支持 pending/in_progress/completed 三种状态

use serde::{Deserialize, Serialize};

/// Todo 任务状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TodoStatus {
    /// 待处理
    Pending,
    /// 进行中(同一时间只能有一个任务处于此状态)
    InProgress,
    /// 已完成
    Completed,
}

impl TodoStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TodoStatus::Pending => "pending",
            TodoStatus::InProgress => "in_progress",
            TodoStatus::Completed => "completed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(TodoStatus::Pending),
            "in_progress" => Some(TodoStatus::InProgress),
            "completed" => Some(TodoStatus::Completed),
            _ => None,
        }
    }
}

/// Todo 优先级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TodoPriority {
    High,
    Medium,
    Low,
}

impl Default for TodoPriority {
    fn default() -> Self {
        TodoPriority::Medium
    }
}

/// 单个 Todo 任务
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    /// 任务唯一 ID(UUID)
    pub id: String,
    /// 任务内容(简短描述)
    pub content: String,
    /// 任务状态
    pub status: TodoStatus,
    /// 优先级
    #[serde(default)]
    pub priority: TodoPriority,
    /// 创建时间(UNIX 时间戳,毫秒)
    pub created_at: u64,
    /// 更新时间(UNIX 时间戳,毫秒)
    pub updated_at: u64,
    /// 完成时间(UNIX 时间戳,毫秒,仅 status=completed 时有值)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
}

/// Todo 列表(按 session_id 隔离)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoList {
    /// 会话 ID
    pub session_id: String,
    /// 任务列表
    pub items: Vec<TodoItem>,
    /// 最后更新时间
    pub updated_at: u64,
}

impl TodoList {
    /// 创建空的 Todo 列表
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            items: Vec::new(),
            updated_at: current_timestamp_ms(),
        }
    }

    /// 获取进行中的任务(应最多一个)
    pub fn get_in_progress(&self) -> Option<&TodoItem> {
        self.items.iter().find(|t| t.status == TodoStatus::InProgress)
    }

    /// 获取待处理任务数
    pub fn pending_count(&self) -> usize {
        self.items.iter().filter(|t| t.status == TodoStatus::Pending).count()
    }

    /// 获取已完成任务数
    pub fn completed_count(&self) -> usize {
        self.items.iter().filter(|t| t.status == TodoStatus::Completed).count()
    }

    /// 生成摘要文本(注入到系统提示词)
    pub fn build_summary(&self) -> Option<String> {
        if self.items.is_empty() {
            return None;
        }

        let mut summary = String::from("\n## 当前任务清单\n");
        for item in &self.items {
            let status_icon = match item.status {
                TodoStatus::Pending => "[ ]",
                TodoStatus::InProgress => "[>]",
                TodoStatus::Completed => "[x]",
            };
            summary.push_str(&format!("{} {} ({})\n", status_icon, item.content, item.priority.as_str()));
        }

        let total = self.items.len();
        let completed = self.completed_count();
        let pending = self.pending_count();
        summary.push_str(&format!("\n进度: {}/{} 已完成, {} 待处理\n", completed, total, pending));

        Some(summary)
    }
}

/// 获取当前时间戳(毫秒)
fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 单元测试:TodoList 的 build_summary 方法生成正确格式

---

### T3.11:实现 TodoWrite 仓库

**文件**:
- 创建:`src-tauri/src/db/todo_repo.rs`
- 修改:`src-tauri/src/db/mod.rs`(添加 `pub mod todo_repo;`)
- 修改:`src-tauri/src/db/init.rs`(添加 todo_lists 表)

**实施内容**:

**数据库表**(添加到 `init.rs`):
```rust
// todo_lists Todo 列表表(按 session_id 隔离)
conn.execute_batch(
    "CREATE TABLE IF NOT EXISTS todo_lists (
        session_id        TEXT        NOT NULL PRIMARY KEY,
        items_json        TEXT        NOT NULL DEFAULT '[]',
        updated_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    );"
)?;
```

**todo_repo.rs**:
```rust
//! Todo 仓库:持久化 Todo 列表,支持跨迭代保持任务状态

use rusqlite::Connection;
use crate::errors::CommandError;
use crate::models::todo::{TodoList, TodoItem, TodoStatus, TodoPriority};
use serde_json;

/// 获取指定会话的 Todo 列表
pub fn get_todo_list(conn: &Connection, session_id: &str) -> Result<TodoList, CommandError> {
    let mut stmt = conn.prepare(
        "SELECT session_id, items_json, updated_at FROM todo_lists WHERE session_id = ?1"
    )?;
    let result = stmt.query_row(rusqlite::params![session_id], |row| {
        let items_json: String = row.get(1)?;
        let updated_at_str: String = row.get(2)?;
        Ok((items_json, updated_at_str))
    });

    match result {
        Ok((items_json, _)) => {
            let items: Vec<TodoItem> = serde_json::from_str(&items_json)
                .unwrap_or_default();
            Ok(TodoList {
                session_id: session_id.to_string(),
                items,
                updated_at: current_timestamp_ms(),
            })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            Ok(TodoList::new(session_id.to_string()))
        }
        Err(e) => Err(e.into()),
    }
}

/// 保存 Todo 列表(upsert)
pub fn save_todo_list(conn: &Connection, todo_list: &TodoList) -> Result<(), CommandError> {
    let items_json = serde_json::to_string(&todo_list.items)?;
    conn.execute(
        "INSERT INTO todo_lists (session_id, items_json, updated_at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(session_id) DO UPDATE SET
            items_json = excluded.items_json,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        rusqlite::params![todo_list.session_id, items_json],
    )?;
    Ok(())
}

/// 删除指定会话的 Todo 列表
pub fn delete_todo_list(conn: &Connection, session_id: &str) -> Result<(), CommandError> {
    conn.execute("DELETE FROM todo_lists WHERE session_id = ?1", rusqlite::params![session_id])?;
    Ok(())
}

fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 单元测试:保存后读取 Todo 列表,数据一致

---

### T3.12:实现 TodoWrite 工具

> **命名规则说明**：`todowrite` 工具沿用 OpenCode 原名，不适用"复合词保留下划线"规则。OpenCode 生态中该工具名已约定俗成。

**文件**:
- 创建:`src-tauri/src/services/tool/builtin/todowrite.rs`
- 修改:`src-tauri/src/services/tool/builtin.rs`(注册 TodoWriteTool)

**实施内容**:
```rust
//! TodoWrite 工具:结构化任务管理
//! Agent 通过此工具创建、更新、查询任务,支持跨迭代状态保持

use async_trait::async_trait;
use crate::services::tool::trait_def::Tool;
use crate::models::tool::ToolResult;
use crate::models::todo::{TodoItem, TodoList, TodoStatus, TodoPriority};
use crate::db::todo_repo;
use serde_json::{json, Value};
use uuid::Uuid;

pub struct TodoWriteTool {
    /// 数据库连接(通过 Arc 共享)
    db: std::sync::Arc<crate::db::Database>,
}

impl TodoWriteTool {
    pub fn new(db: std::sync::Arc<crate::db::Database>) -> Self {
        Self { db }
    }

    /// 获取当前时间戳(毫秒)
    fn current_timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn tool_name(&self) -> &str {
        "todowrite"
    }

    fn description(&self) -> &str {
        "结构化任务管理工具。创建、更新、查询任务清单,支持 pending/in_progress/completed 三种状态。同一时间只能有一个任务处于 in_progress 状态。任务清单跨迭代保持,用于跟踪复杂任务的进度。"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "update", "list", "clear"],
                    "description": "操作类型:create=创建新任务;update=更新任务状态;list=列出所有任务;clear=清空任务清单"
                },
                "content": {
                    "type": "string",
                    "description": "任务内容(action=create 时必填)"
                },
                "id": {
                    "type": "string",
                    "description": "任务 ID(action=update 时必填)"
                },
                "status": {
                    "type": "string",
                    "enum": ["pending", "in_progress", "completed"],
                    "description": "任务状态(action=update 时可选)"
                },
                "priority": {
                    "type": "string",
                    "enum": ["high", "medium", "low"],
                    "description": "任务优先级(action=create 时可选,默认 medium)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        params: Value,
        workspace_root: &str,
    ) -> Result<ToolResult, crate::errors::CommandError> {
        let action = params.get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                "缺少 action 参数",
            ))?;

        // 从 params 中获取 session_id(由 executor 注入)
        let session_id = params.get("_session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        let conn = self.db.conn()?;

        match action {
            "create" => {
                let content = params.get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| crate::errors::CommandError::tool(
                        crate::errors::TOOL_INVALID_PARAMS,
                        "action=create 时必须提供 content 参数",
                    ))?;

                let priority = params.get("priority")
                    .and_then(|v| v.as_str())
                    .and_then(TodoPriority::from_str)
                    .unwrap_or_default();

                let mut todo_list = todo_repo::get_todo_list(&conn, session_id)?;
                let now = Self::current_timestamp_ms();
                let item = TodoItem {
                    id: Uuid::new_v4().to_string(),
                    content: content.to_string(),
                    status: TodoStatus::Pending,
                    priority,
                    created_at: now,
                    updated_at: now,
                    completed_at: None,
                };
                todo_list.items.push(item.clone());
                todo_list.updated_at = now;
                todo_repo::save_todo_list(&conn, &todo_list)?;

                Ok(ToolResult {
                    success: true,
                    result: json!({
                        "action": "create",
                        "item": item,
                        "total": todo_list.items.len(),
                    }),
                    error: None,
                    metadata: None,
                })
            }
            "update" => {
                let id = params.get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| crate::errors::CommandError::tool(
                        crate::errors::TOOL_INVALID_PARAMS,
                        "action=update 时必须提供 id 参数",
                    ))?;

                let mut todo_list = todo_repo::get_todo_list(&conn, session_id)?;
                let now = Self::current_timestamp_ms();

                let item = todo_list.items.iter_mut()
                    .find(|t| t.id == id)
                    .ok_or_else(|| crate::errors::CommandError::tool(
                        crate::errors::TOOL_NOT_FOUND,
                        format!("任务 {} 不存在", id),
                    ))?;

                if let Some(status_str) = params.get("status").and_then(|v| v.as_str()) {
                    let new_status = TodoStatus::from_str(status_str)
                        .ok_or_else(|| crate::errors::CommandError::tool(
                            crate::errors::TOOL_INVALID_PARAMS,
                            format!("无效状态: {}", status_str),
                        ))?;

                    // 如果设为 in_progress,先将其他 in_progress 任务改为 pending
                    if new_status == TodoStatus::InProgress {
                        for other in todo_list.items.iter_mut() {
                            if other.id != id && other.status == TodoStatus::InProgress {
                                other.status = TodoStatus::Pending;
                                other.updated_at = now;
                            }
                        }
                    }

                    item.status = new_status.clone();
                    if new_status == TodoStatus::Completed {
                        item.completed_at = Some(now);
                    } else {
                        item.completed_at = None;
                    }
                }

                if let Some(content) = params.get("content").and_then(|v| v.as_str()) {
                    item.content = content.to_string();
                }

                item.updated_at = now;
                todo_list.updated_at = now;
                todo_repo::save_todo_list(&conn, &todo_list)?;

                Ok(ToolResult {
                    success: true,
                    result: json!({
                        "action": "update",
                        "item": item,
                        "total": todo_list.items.len(),
                    }),
                    error: None,
                    metadata: None,
                })
            }
            "list" => {
                let todo_list = todo_repo::get_todo_list(&conn, session_id)?;
                Ok(ToolResult {
                    success: true,
                    result: json!({
                        "items": todo_list.items,
                        "total": todo_list.items.len(),
                        "pending": todo_list.pending_count(),
                        "completed": todo_list.completed_count(),
                    }),
                    error: None,
                    metadata: None,
                })
            }
            "clear" => {
                todo_repo::delete_todo_list(&conn, session_id)?;
                Ok(ToolResult {
                    success: true,
                    result: json!({
                        "action": "clear",
                        "message": "任务清单已清空",
                    }),
                    error: None,
                    metadata: None,
                })
            }
            _ => Err(crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                format!("未知 action: {}", action),
            )),
        }
    }
}
```

**步骤 3:在 register_builtin_tools 中注册 TodoWriteTool**

> **接口对齐说明**:本阶段需要修改 `register_builtin_tools` 签名,增加 `db` 参数。
> 完整签名见 overview 4.4.1 节统一接口定义。

修改 `src-tauri/src/services/tool/builtin.rs` 中的 `register_builtin_tools`:

```rust
// Phase 3 阶段签名(渐进式扩展:在 Phase 2 基础上增加 db 参数)
pub fn register_builtin_tools(
    registry: &mut ToolRegistry,
    git_bash_path: String,
    agent_mode_manager: Arc<AgentModeManager>,       // Phase 2 引入
    db: Arc<Database>,                                // [本阶段新增]
) -> SharedScratchpadStates {
    // ... 现有工具注册 ...

    // [本阶段新增] TodoWrite 工具
    registry.register(Box::new(TodoWriteTool::new(db.clone())));

    // ... 其他工具 ...
}
```

在 `lib.rs` 中调用时传入 `db` 参数:

```rust
// 签名参照 overview 4.4.1 节统一接口定义(Phase 3 阶段:渐进式扩展,增加 db 参数)
let scratchpad_states = register_builtin_tools(
    &mut tool_registry,
    git_bash_path,
    agent_mode_manager.clone(),
    db.clone(),                   // [本阶段新增]
);
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 单元测试:create -> update -> list -> clear 流程

---

### T3.13:集成 TodoWrite 到 AgentContext

**文件**:
- 修改:`src-tauri/src/services/agent/context.rs`
- 修改:`src-tauri/src/services/agent/executor.rs`

**实施内容**:

> **架构变更说明**(参照 overview 4.4.4):
> 本任务将 `build_system_prompt` 从关联函数(静态方法)改为实例方法,需要在 `AgentContext` 结构体中新增 `db` 字段。
> 现有 `AgentContext`(见 [context.rs:186](file:///d:/DeskTop/WorkMolde-AI/src-tauri/src/services/agent/context.rs#L186))没有 `db` 字段,本任务需要添加。

**步骤 1:在 AgentContext 结构体中新增 db 字段**

修改 `src-tauri/src/services/agent/context.rs` 中的 `AgentContext` 结构体:

```rust
pub struct AgentContext {
    // ... 现有字段保持不变 ...

    // [新增] 数据库连接(用于读取 TodoList、SessionSummary 等)
    // 为 Option 类型,测试场景下可为 None
    pub db: Option<std::sync::Arc<crate::db::Database>>,
}
```

在 `new` 和 `new_default` 方法中初始化:

```rust
impl AgentContext {
    pub fn new(session_id: String, system_prompt: String, context_window: usize) -> Self {
        Self {
            // ... 现有字段初始化保持不变 ...
            db: None,  // 默认 None,由 executor 在初始化时注入
        }
    }

    /// 设置数据库连接(由 executor 在初始化时注入)
    pub fn set_db(&mut self, db: std::sync::Arc<crate::db::Database>) {
        self.db = Some(db);
    }
}
```

**步骤 2:在 AgentContext 中添加 TodoList 读取**

```rust
impl AgentContext {
    /// 构建系统提示词时注入 TodoList 摘要
    /// 注意:此方法从静态方法改为实例方法(参照 overview 4.4.4)
    pub fn build_system_prompt(&self, session_id: &str, mode: &str) -> String {
        let mut prompt = String::new();

        // 基础提示词...(调用 build_system_prompt_with_task 静态方法构建基础部分)
        // let base = Self::build_system_prompt_with_task(...);
        // prompt.push_str(&base);

        // 注入 TodoList 摘要(如果有)
        if let Some(db) = &self.db {
            if let Ok(conn) = db.conn() {
                if let Ok(todo_list) = crate::db::todo_repo::get_todo_list(&conn, session_id) {
                    if let Some(summary) = todo_list.build_summary() {
                        prompt.push_str(&summary);
                    }
                }
            }
        }

        prompt
    }
}
```

**在 AgentExecutor 中每轮迭代刷新 TodoList**:
```rust
// 在 execute_iteration 方法中,每轮迭代开始时刷新系统提示词
async fn execute_iteration(&self, session_id: &str, ...) -> ... {
    // 构建上下文(包含 TodoList 摘要)
    let system_prompt = self.context.build_system_prompt(session_id, mode);
    
    // ... 调用 LLM
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- Agent 执行时,系统提示词中包含当前 TodoList

---

### T3.14:替代 Scratchpad 摘要功能

**文件**:
- 修改:`src-tauri/src/services/agent/context.rs`
- 修改:`src-tauri/src/services/tool/builtin.rs`

**实施内容**:

**调整 Scratchpad 角色**:Scratchpad 保留为草稿本,但不再注入系统提示词(由 TodoWrite 接管任务管理):

```rust
// 在 AgentContext 中,移除 format_scratchpad_summary 的注入
// Scratchpad 仍可通过 scratchpad 工具使用,但不再自动注入摘要
// TodoList 摘要取代 Scratchpad 摘要的位置

impl AgentContext {
    pub fn build_system_prompt(&self, session_id: &str, mode: &str) -> String {
        let mut prompt = String::new();
        
        // 基础提示词...
        
        // 注入 TodoList 摘要(替代原 Scratchpad 摘要)
        // ... TodoList 注入逻辑
        
        // 移除:format_scratchpad_summary(&self.scratchpad_states, session_id)
        
        prompt
    }
}
```

**保留 Scratchpad 工具**:Scratchpad 工具仍可用,但定位为"自由格式草稿本",Agent 可自行决定是否使用。

**验证**:
- `cargo build -p workmolde_lib` 成功
- Agent 执行时,系统提示词中不再包含 Scratchpad 摘要
- TodoList 摘要正常注入

---

### T3.15:定义 SessionCompaction 配置

**文件**:
- 修改:`src-tauri/src/config/app_settings.rs`

**实施内容**:
```rust
/// SessionCompaction 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionConfig {
    /// 是否启用上下文压缩
    pub enabled: bool,
    /// 触发压缩的 token 阈值(占上下文窗口的百分比,如 0.8 表示 80%)
    pub trigger_threshold: f32,
    /// 压缩后保留的最近消息数
    pub keep_recent_messages: usize,
    /// 压缩时保留的系统提示词 token 数
    pub keep_system_tokens: usize,
    /// 是否压缩工具输出(旧工具输出会被 prune)
    pub compact_tool_outputs: bool,
    /// 工具输出保留的最大字符数(超过则截断)
    pub tool_output_max_chars: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            trigger_threshold: 0.8,
            keep_recent_messages: 10,
            keep_system_tokens: 4000,
            compact_tool_outputs: true,
            tool_output_max_chars: 2000,
        }
    }
}

// 在 GeneralSettings 或新建 AgentSettings 中添加 CompactionConfig 字段
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 配置可正确序列化/反序列化

---

### T3.16:实现 Compaction 策略

**文件**:
- 创建:`src-tauri/src/services/agent/compaction.rs`
- 修改:`src-tauri/src/services/agent/mod.rs`(添加 `pub mod compaction;`)

**实施内容**:
```rust
//! SessionCompaction:上下文压缩策略
//! 当上下文接近溢出时,压缩旧消息为摘要,保留最近消息和关键信息

use crate::config::app_settings::CompactionConfig;
use crate::models::llm::{ChatMessage, MessageRole};
use crate::services::llm::provider::LlmProvider;
use serde_json::{json, Value};
use std::sync::Arc;

/// 上下文压缩器
pub struct ContextCompactor {
    /// 压缩配置
    config: CompactionConfig,
}

/// 压缩结果
pub struct CompactionResult {
    /// 压缩后的消息列表
    pub messages: Vec<ChatMessage>,
    /// 压缩摘要(注入到系统提示词)
    pub compaction_summary: String,
    /// 压缩前 token 数
    pub tokens_before: u64,
    /// 压缩后 token 数
    pub tokens_after: u64,
    /// 是否实际执行了压缩
    pub compacted: bool,
}

impl ContextCompactor {
    pub fn new(config: CompactionConfig) -> Self {
        Self { config }
    }

    /// 检查是否需要压缩
    pub fn should_compact(&self, current_tokens: u64, context_window: u64) -> bool {
        if !self.config.enabled {
            return false;
        }
        let threshold = (context_window as f32 * self.config.trigger_threshold) as u64;
        current_tokens >= threshold
    }

    /// 执行上下文压缩
    /// 1. 保留最近 N 条消息
    /// 2. 对旧消息生成摘要(通过 LLM)
    /// 3. 对旧工具输出做 prune(截断)
    pub async fn compact(
        &self,
        messages: &[ChatMessage],
        llm_provider: &dyn LlmProvider,
    ) -> Result<CompactionResult, crate::errors::CommandError> {
        let keep_recent = self.config.keep_recent_messages;

        // 若消息数不超过保留数,不压缩
        if messages.len() <= keep_recent {
            return Ok(CompactionResult {
                messages: messages.to_vec(),
                compaction_summary: String::new(),
                tokens_before: 0,
                tokens_after: 0,
                compacted: false,
            });
        }

        // 分割消息:old_messages + recent_messages
        let split_point = messages.len() - keep_recent;
        let old_messages = &messages[..split_point];
        let mut recent_messages: Vec<ChatMessage> = messages[split_point..].to_vec();

        // 对旧消息的工具输出做 prune(减少摘要请求的 token 消耗)
        let mut old_messages_pruned: Vec<ChatMessage> = old_messages.to_vec();
        if self.config.compact_tool_outputs {
            for msg in &mut old_messages_pruned {
                self.prune_tool_output(msg);
            }
        }

        // 对旧消息生成摘要
        let summary = self
            .generate_summary(&old_messages_pruned, llm_provider)
            .await?;

        // 对 recent_messages 中的工具输出做 prune
        if self.config.compact_tool_outputs {
            for msg in &mut recent_messages {
                self.prune_tool_output(msg);
            }
        }

        // 构建结果:摘要 system 消息 + recent_messages
        let summary_system_msg = ChatMessage {
            role: "system".to_string(),
            content: format!("[Context Compaction Summary]\n{}", summary),
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            attachments: None,
        };

        let mut result_messages = Vec::with_capacity(recent_messages.len() + 1);
        result_messages.push(summary_system_msg);
        result_messages.extend(recent_messages);

        Ok(CompactionResult {
            messages: result_messages,
            compaction_summary: summary,
            tokens_before: 0,
            tokens_after: 0,
            compacted: true,
        })
    }

    /// 生成旧消息的摘要
    async fn generate_summary(
        &self,
        messages: &[ChatMessage],
        llm_provider: &dyn LlmProvider,
    ) -> Result<String, crate::errors::CommandError> {
        // 摘要请求的系统提示词(英文,提取关键信息)
        let system_prompt =
            "You are a conversation summarization assistant. Carefully read the following conversation history and extract the following key information:\n\n\
1. The user's core requirements and goals\n\
2. Completed work and results\n\
3. Unfinished tasks and pending items\n\
4. Key decisions and rationale\n\
5. File paths and important data involved\n\
6. Obstacles encountered and solutions\n\n\
Requirements:\n\
- Output a concise structured summary in English\n\
- Preserve all key file paths, function names, variable names and other technical details\n\
- Preserve important code snippets and data\n\
- Do not omit any key information to facilitate subsequent conversation reference";

        // 构建摘要请求消息:系统提示 + 旧消息
        let mut summary_messages = Vec::with_capacity(messages.len() + 1);
        summary_messages.push(ChatMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            attachments: None,
        });
        summary_messages.extend(messages.iter().cloned());

        // 调用 LLM 生成摘要
        let response = llm_provider.chat(&summary_messages, &[]).await?;

        let summary = response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        Ok(summary)
    }

    /// 对工具输出做 prune(截断过长的内容)
    fn prune_tool_output(&self, msg: &mut ChatMessage) {
        if msg.role != MessageRole::Tool {
            return;
        }

        if let Some(ref mut result) = msg.tool_result {
            if let Value::String(ref mut s) = result {
                if s.len() > self.config.tool_output_max_chars {
                    let truncated = &s[..self.config.tool_output_max_chars];
                    *s = format!("{}...[已截断,原始长度 {} 字符]", truncated, s.len());
                }
            }
        }
    }
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 单元测试:`should_compact` 在不同 token 数下返回正确结果

---

### T3.17:集成 Compaction 到 AgentExecutor（使用 compact_messages 方法）

**文件**:
- 修改:`src-tauri/src/services/agent/executor.rs`
- 修改:`src-tauri/src/services/llm/router.rs`

**实施内容**:

在 AgentExecutor 的迭代循环中,每轮迭代前通过 router 的 `compact_messages` 方法检查是否需要压缩:

```rust
impl AgentExecutor {
    /// 执行单轮迭代
    async fn execute_iteration(&self, session_id: &str, ctx: &AgentContext, ...) -> ... {
        // 获取当前上下文消息
        let all_messages = self.context.get_messages(session_id).await?;
        
        // 计算当前 token 数
        let current_tokens = self.estimate_tokens(&all_messages);
        let context_window = self.get_context_window();
        
        // 检查是否需要压缩
        if let Some(compactor) = &self.compactor {
            if compactor.should_compact(current_tokens, context_window) {
                log::info!(
                    "触发上下文压缩:当前 {} tokens,上下文窗口 {}",
                    current_tokens,
                    context_window
                );
                
                // 发射压缩开始事件
                self.emitter.emit_compaction_start(...).ok();
                
                // 解析 preferred_provider_id(与 chat_stream 的 provider 解析模式一致)
                let preferred = if ctx.preferred_provider_id.is_empty() {
                    None
                } else {
                    Some(ctx.preferred_provider_id.as_str())
                };
                
                // 通过 router.compact_messages 执行压缩
                // compact_messages 方法签名:
                // pub async fn compact_messages(
                //     &self,
                //     messages: &[ChatMessage],
                //     compactor: &ContextCompactor,
                //     preferred_provider_id: Option<&str>,  // 新增参数
                // ) -> Result<CompactionResult, CommandError>
                match self.router.compact_messages(
                    &all_messages, compactor, preferred
                ).await {
                    Ok(result) => {
                        if result.compacted {
                            // 更新消息列表和上下文
                            self.context.replace_messages(session_id, result.messages).await?;
                            
                            // 发射压缩完成事件
                            self.emitter.emit_compaction_done(...).ok();
                        }
                    }
                    Err(e) => {
                        log::warn!("上下文压缩失败: {}", e);
                    }
                }
            }
        }
        
        // 继续执行 LLM 调用...
    }
}
    
    /// 估算消息列表的 token 数(简化实现)
    fn estimate_tokens(&self, messages: &[ChatMessage]) -> u64 {
        // 简化:按字符数 / 4 估算(英文约 4 字符/token,中文约 2 字符/token)
        let total_chars: usize = messages.iter()
            .map(|m| m.content.len())
            .sum();
        (total_chars as f64 / 3.0) as u64
    }
    
    /// 获取当前 Provider 的上下文窗口大小
    fn get_context_window(&self) -> u64 {
        // 从 LLM Router 获取当前 Provider 的 context_window
        128_000 // 默认值,实际应从 Provider 配置读取
    }
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 模拟长对话,验证压缩在阈值触发

---

### T3.18:Compaction 事件与前端通知

**文件**:
- 修改:`src-tauri/src/events/types.rs`
- 修改:`src/components/workflow/WorkflowTimeline.tsx`(展示压缩事件)

**实施内容**:

**新增事件常量**:
```rust
// 在 events/types.rs 中添加
pub const AGENT_COMPACTION_START: &str = "agent:compaction_start";
pub const AGENT_COMPACTION_DONE: &str = "agent:compaction_done";

/// 上下文压缩开始事件
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompactionStartPayload {
    pub session_id: String,
    pub tokens_before: u64,
}

/// 上下文压缩完成事件
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompactionDonePayload {
    pub session_id: String,
    pub tokens_before: u64,
    pub tokens_after: u64,
    pub compacted: bool,
}
```

**前端展示压缩事件**:
```tsx
// 在 WorkflowTimeline 中添加压缩事件节点
// 当收到 agent:compaction_done 事件时,展示:
// "上下文已压缩:12000 -> 4500 tokens (Context Compaction Summary)"

// 在 WorkflowNode 组件中添加 compaction 类型
case 'compaction':
  return (
    <div className="workflow-node compaction-node">
      <Icon name="compress" />
      <span>上下文压缩: {data.tokensBefore} -> {data.tokensAfter} tokens [Context Compaction Summary]</span>
    </div>
  );
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- `npx tsc -b` 成功
- 前端正确展示压缩事件

---

### T3.19:新增 tree-sitter 依赖

**文件**:
- 修改:`src-tauri/Cargo.toml`

**实施内容**:
```toml
[dependencies]
# 现有依赖...

# 代码语义搜索:tree-sitter
tree-sitter = "0.22"
tree-sitter-c = "0.20"
tree-sitter-cpp = "0.21"
tree-sitter-rust = "0.21"
tree-sitter-python = "0.21"
tree-sitter-javascript = "0.21"
tree-sitter-typescript = "0.21"
tree-sitter-go = "0.21"
tree-sitter-java = "0.21"
```

**验证**:
- `cargo build -p workmolde_lib` 成功

---

### T3.20:实现 LanguageParser

**文件**:
- 创建:`src-tauri/src/services/code/parser.rs`
- 创建:`src-tauri/src/services/code/mod.rs`

**实施内容**:

**mod.rs**:
```rust
//! 代码理解模块入口
pub mod parser;
pub mod search;

pub use parser::LanguageParser;
pub use search::SourceCodeSearcher;
```

**parser.rs**:
```rust
//! 代码解析器:基于 tree-sitter 解析多种语言的语法树
//! 提取函数、类、方法等符号信息

use tree_sitter::{Language, Node, Parser, Query, QueryCursor};
use std::collections::HashMap;

/// 支持的编程语言
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProgrammingLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    C,
    Cpp,
    Unknown,
}

impl ProgrammingLanguage {
    /// 从文件扩展名推断语言
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs" => ProgrammingLanguage::Rust,
            "py" => ProgrammingLanguage::Python,
            "js" | "jsx" | "mjs" => ProgrammingLanguage::JavaScript,
            "ts" | "tsx" => ProgrammingLanguage::TypeScript,
            "go" => ProgrammingLanguage::Go,
            "java" => ProgrammingLanguage::Java,
            "c" | "h" => ProgrammingLanguage::C,
            "cpp" | "cxx" | "cc" | "hpp" => ProgrammingLanguage::Cpp,
            _ => ProgrammingLanguage::Unknown,
        }
    }

    /// 获取 tree-sitter 语言定义
    pub fn to_tree_sitter_language(&self) -> Option<Language> {
        match self {
            ProgrammingLanguage::Rust => Some(tree_sitter_rust::language()),
            ProgrammingLanguage::Python => Some(tree_sitter_python::language()),
            ProgrammingLanguage::JavaScript => Some(tree_sitter_javascript::language()),
            ProgrammingLanguage::TypeScript => Some(tree_sitter_typescript::language_typescript()),
            ProgrammingLanguage::Go => Some(tree_sitter_go::language()),
            ProgrammingLanguage::Java => Some(tree_sitter_java::language()),
            ProgrammingLanguage::C => Some(tree_sitter_c::language()),
            ProgrammingLanguage::Cpp => Some(tree_sitter_cpp::language()),
            ProgrammingLanguage::Unknown => None,
        }
    }
}

/// 符号类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolType {
    Function,
    Method,
    Class,
    Struct,
    Interface,
    Enum,
    Variable,
    Constant,
    Module,
    Other(String),
}

impl SymbolType {
    pub fn as_str(&self) -> &str {
        match self {
            SymbolType::Function => "function",
            SymbolType::Method => "method",
            SymbolType::Class => "class",
            SymbolType::Struct => "struct",
            SymbolType::Interface => "interface",
            SymbolType::Enum => "enum",
            SymbolType::Variable => "variable",
            SymbolType::Constant => "constant",
            SymbolType::Module => "module",
            SymbolType::Other(s) => s.as_str(),
        }
    }
}

/// 代码符号
#[derive(Debug, Clone)]
pub struct CodeSymbol {
    /// 符号名称
    pub name: String,
    /// 符号类型
    pub symbol_type: SymbolType,
    /// 起始行(从 0 开始)
    pub start_line: usize,
    /// 结束行
    pub end_line: usize,
    /// 起始列
    pub start_col: usize,
    /// 文档注释(如有)
    pub doc_comment: Option<String>,
}

/// 代码解析器
pub struct LanguageParser {
    parser: Parser,
}

impl LanguageParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        Self { parser }
    }

    /// 解析文件内容,提取所有符号
    pub fn parse_symbols(
        &mut self,
        content: &str,
        language: ProgrammingLanguage,
    ) -> Result<Vec<CodeSymbol>, String> {
        let ts_language = language.to_tree_sitter_language()
            .ok_or_else(|| format!("不支持的语言: {:?}", language))?;

        self.parser.set_language(&ts_language)
            .map_err(|e| format!("设置语言失败: {}", e))?;

        let tree = self.parser.parse(content, None)
            .ok_or("解析失败")?;

        let root_node = tree.root_node();
        let mut symbols = Vec::new();

        // 遍历语法树,提取符号
        self.extract_symbols(&root_node, content, &mut symbols);

        Ok(symbols)
    }

    /// 递归提取符号
    fn extract_symbols(&self, node: &Node, content: &str, symbols: &mut Vec<CodeSymbol>) {
        // 检查当前节点是否为符号定义
        if let Some(symbol) = self.node_to_symbol(node, content) {
            symbols.push(symbol);
        }

        // 递归处理子节点
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_symbols(&child, content, symbols);
        }
    }

    /// 将语法节点转换为符号
    fn node_to_symbol(&self, node: &Node, content: &str) -> Option<CodeSymbol> {
        let node_kind = node.kind();

        let (symbol_type, name_node) = match node_kind {
            "function_definition" | "function_declaration" => {
                (SymbolType::Function, node.child_by_field_name("name"))
            }
            "method_definition" | "method_declaration" => {
                (SymbolType::Method, node.child_by_field_name("name"))
            }
            "class_definition" | "class_declaration" => {
                (SymbolType::Class, node.child_by_field_name("name"))
            }
            "struct_specifier" | "struct_item" => {
                (SymbolType::Struct, node.child_by_field_name("name"))
            }
            "interface_declaration" | "trait_item" => {
                (SymbolType::Interface, node.child_by_field_name("name"))
            }
            "enum_specifier" | "enum_item" => {
                (SymbolType::Enum, node.child_by_field_name("name"))
            }
            _ => return None,
        };

        let name_node = name_node?;
        let name = name_node.utf8_text(content.as_bytes()).ok()?.to_string();

        let start_pos = node.start_position();
        let end_pos = node.end_position();

        // 提取文档注释(前一个兄弟节点)
        let doc_comment = self.extract_doc_comment(node, content);

        Some(CodeSymbol {
            name,
            symbol_type,
            start_line: start_pos.row,
            end_line: end_pos.row,
            start_col: start_pos.column,
            doc_comment,
        })
    }

    /// 提取文档注释
    fn extract_doc_comment(&self, node: &Node, content: &str) -> Option<String> {
        let prev = node.prev_sibling()?;
        let kind = prev.kind();

        if kind.contains("comment") || kind.contains("doc") {
            prev.utf8_text(content.as_bytes())
                .ok()
                .map(|s| s.to_string())
        } else {
            None
        }
    }
}
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 单元测试:解析 Rust 文件,正确提取函数、结构体等符号

---

### T3.21:实现 SourceCode 工具

**文件**:
- 创建:`src-tauri/src/services/code/search.rs`
- 创建:`src-tauri/src/services/tool/builtin/sourcecode.rs`
- 修改:`src-tauri/src/services/tool/builtin.rs`(注册 SourceCodeTool)

**实施内容**:

**search.rs**:
```rust
//! 代码搜索器:基于 tree-sitter 的语义搜索
//! 支持按符号类型、名称模式、正则表达式查询

use crate::services::code::parser::{LanguageParser, ProgrammingLanguage, CodeSymbol, SymbolType};
use std::path::{Path, PathBuf};

/// 搜索查询
pub struct SearchQuery {
    /// 搜索目录
    pub directory: PathBuf,
    /// 符号名称(支持通配符,如 "get_*")
    pub symbol_name: Option<String>,
    /// 符号类型过滤(如 Function, Class)
    pub symbol_type: Option<SymbolType>,
    /// 文件扩展名过滤(如 ["rs", "py"])
    pub extensions: Vec<String>,
    /// 是否递归搜索子目录
    pub recursive: bool,
    /// 最大返回结果数
    pub max_results: usize,
}

/// 代码搜索结果(SourceCode 工具专用)
///
/// 命名说明:此 `SearchResult` 与 Phase 4 的 `SearchResultItem`(WebSearch 工具)是不同的类型:
/// - `SearchResult`(本类型):代码符号搜索结果,包含 file_path/symbol/line_content
/// - `SearchResultItem`(Phase 4):网络搜索结果,包含 title/url/snippet
/// 两者属于不同模块(`services/code/search.rs` vs `services/web/searcher.rs`),不会冲突
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub file_path: PathBuf,
    pub symbol: CodeSymbol,
    /// 符号所在行的内容
    pub line_content: String,
}

/// 代码搜索器
pub struct SourceCodeSearcher {
    parser: LanguageParser,
}

impl SourceCodeSearcher {
    pub fn new() -> Self {
        Self {
            parser: LanguageParser::new(),
        }
    }

    /// 执行搜索
    pub fn search(&mut self, query: &SearchQuery) -> Result<Vec<SearchResult>, String> {
        let mut results = Vec::new();
        self.search_dir(&query.directory, query, &mut results)?;
        
        // 限制结果数量
        results.truncate(query.max_results);
        Ok(results)
    }

    /// 递归搜索目录
    fn search_dir(
        &mut self,
        dir: &Path,
        query: &SearchQuery,
        results: &mut Vec<SearchResult>,
    ) -> Result<(), String> {
        if !dir.is_dir() {
            return Err(format!("目录不存在: {}", dir.display()));
        }

        for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

            if path.is_dir() {
                // 跳过常见忽略目录
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if matches!(name, "node_modules" | ".git" | "target" | "dist" | "build" | "__pycache__") {
                        continue;
                    }
                }
                if query.recursive {
                    self.search_dir(&path, query, results)?;
                }
            } else if path.is_file() {
                // 检查扩展名
                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                
                if !query.extensions.is_empty() && !query.extensions.iter().any(|e| e == ext) {
                    continue;
                }

                // 解析文件并搜索符号
                if let Err(e) = self.search_file(&path, query, results) {
                    log::debug!("解析文件失败 {}: {}", path.display(), e);
                }
            }
        }

        Ok(())
    }

    /// 搜索单个文件
    fn search_file(
        &mut self,
        path: &Path,
        query: &SearchQuery,
        results: &mut Vec<SearchResult>,
    ) -> Result<(), String> {
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        
        let language = ProgrammingLanguage::from_extension(ext);
        if language == ProgrammingLanguage::Unknown {
            return Ok(());
        }

        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let symbols = self.parser.parse_symbols(&content, language)?;

        for symbol in symbols {
            // 符号类型过滤
            if let Some(ref filter_type) = query.symbol_type {
                if &symbol.symbol_type != filter_type {
                    continue;
                }
            }

            // 符号名称匹配(通配符)
            if let Some(ref name_pattern) = query.symbol_name {
                if !wildmatch::Wildmatch::new(name_pattern).matches(&symbol.name) {
                    continue;
                }
            }

            // 提取所在行内容
            let line_content = content.lines()
                .nth(symbol.start_line)
                .unwrap_or("")
                .to_string();

            results.push(SearchResult {
                file_path: path.to_path_buf(),
                symbol,
                line_content,
            });
        }

        Ok(())
    }
}
```

**sourcecode.rs**:
```rust
//! SourceCode 工具:代码语义搜索
//! 基于 tree-sitter 解析代码语法树,支持按符号类型、名称模式查询

use async_trait::async_trait;
use crate::services::tool::trait_def::Tool;
use crate::services::code::search::{SourceCodeSearcher, SearchQuery};
use crate::models::tool::ToolResult;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct SourceCodeTool {
    /// 搜索器(使用 Mutex 保护可变状态)
    searcher: Mutex<SourceCodeSearcher>,
}

impl SourceCodeTool {
    pub fn new() -> Self {
        Self {
            searcher: Mutex::new(SourceCodeSearcher::new()),
        }
    }
}

#[async_trait]
impl Tool for SourceCodeTool {
    fn tool_name(&self) -> &str {
        "source_code"
    }

    fn description(&self) -> &str {
        "代码语义搜索工具。基于 tree-sitter 解析代码语法树,支持按符号类型(函数、类、方法、结构体等)和名称模式查询。比文本搜索更精准,能理解代码结构。支持 Rust/Python/JavaScript/TypeScript/Go/Java/C/C++ 等语言。"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["search", "list_symbols"],
                    "description": "操作类型:search=搜索符号;list_symbols=列出文件中所有符号"
                },
                "path": {
                    "type": "string",
                    "description": "搜索目录或文件路径"
                },
                "symbolName": {
                    "type": "string",
                    "description": "符号名称(支持通配符,如 'get_*')"
                },
                "symbolType": {
                    "type": "string",
                    "enum": ["function", "method", "class", "struct", "interface", "enum", "variable", "constant", "module"],
                    "description": "符号类型过滤"
                },
                "extensions": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "文件扩展名过滤(如 ['rs', 'py'])"
                },
                "recursive": {
                    "type": "boolean",
                    "default": true,
                    "description": "是否递归搜索子目录"
                },
                "maxResults": {
                    "type": "integer",
                    "default": 50,
                    "description": "最大返回结果数"
                }
            },
            "required": ["action", "path"]
        })
    }

    async fn execute(
        &self,
        params: Value,
        workspace_root: &str,
    ) -> Result<ToolResult, crate::errors::CommandError> {
        let action = params.get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("search");

        let path_str = params.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                "缺少 path 参数",
            ))?;

        // 解析路径(支持相对路径)
        let path = if std::path::Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else {
            PathBuf::from(workspace_root).join(path_str)
        };

        // 路径安全校验
        let canonical_path = path.canonicalize()
            .map_err(|e| crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                format!("路径解析失败: {}", e),
            ))?;

        let symbol_name = params.get("symbolName")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let symbol_type = params.get("symbolType")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "function" => Some(SymbolType::Function),
                "method" => Some(SymbolType::Method),
                "class" => Some(SymbolType::Class),
                "struct" => Some(SymbolType::Struct),
                "interface" => Some(SymbolType::Interface),
                "enum" => Some(SymbolType::Enum),
                "variable" => Some(SymbolType::Variable),
                "constant" => Some(SymbolType::Constant),
                "module" => Some(SymbolType::Module),
                _ => None,
            });

        let extensions: Vec<String> = params.get("extensions")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect())
            .unwrap_or_default();

        let recursive = params.get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let max_results = params.get("maxResults")
            .and_then(|v| v.as_u64())
            .unwrap_or(50) as usize;

        let mut searcher = self.searcher.lock().unwrap();

        match action {
            "search" => {
                let query = SearchQuery {
                    directory: canonical_path.clone(),
                    symbol_name,
                    symbol_type,
                    extensions,
                    recursive,
                    max_results,
                };

                let results = searcher.search(&query)
                    .map_err(|e| crate::errors::CommandError::tool(
                        crate::errors::TOOL_EXECUTION_ERROR,
                        e,
                    ))?;

                let result_list: Vec<Value> = results.iter().map(|r| {
                    json!({
                        "filePath": r.file_path.to_string_lossy(),
                        "symbolName": r.symbol.name,
                        "symbolType": r.symbol.symbol_type.as_str(),
                        "startLine": r.symbol.start_line,
                        "endLine": r.symbol.end_line,
                        "startCol": r.symbol.start_col,
                        "lineContent": r.line_content,
                        "docComment": r.symbol.doc_comment,
                    })
                }).collect();

                Ok(ToolResult {
                    success: true,
                    result: json!({
                        "results": result_list,
                        "total": result_list.len(),
                    }),
                    error: None,
                    metadata: None,
                })
            }
            "list_symbols" => {
                // 列出单个文件中的所有符号
                if !canonical_path.is_file() {
                    return Err(crate::errors::CommandError::tool(
                        crate::errors::TOOL_INVALID_PARAMS,
                        "list_symbols 操作需要文件路径",
                    ));
                }

                let ext = canonical_path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                let language = ProgrammingLanguage::from_extension(ext);

                let content = std::fs::read_to_string(&canonical_path)?;
                let symbols = searcher.parser.parse_symbols(&content, language)
                    .map_err(|e| crate::errors::CommandError::tool(
                        crate::errors::TOOL_EXECUTION_ERROR,
                        e,
                    ))?;

                let symbol_list: Vec<Value> = symbols.iter().map(|s| {
                    json!({
                        "name": s.name,
                        "type": s.symbol_type.as_str(),
                        "startLine": s.start_line,
                        "endLine": s.end_line,
                        "startCol": s.start_col,
                        "docComment": s.doc_comment,
                    })
                }).collect();

                Ok(ToolResult {
                    success: true,
                    result: json!({
                        "filePath": canonical_path.to_string_lossy(),
                        "language": format!("{:?}", language),
                        "symbols": symbol_list,
                        "total": symbol_list.len(),
                    }),
                    error: None,
                    metadata: None,
                })
            }
            _ => Err(crate::errors::CommandError::tool(
                crate::errors::TOOL_INVALID_PARAMS,
                format!("未知 action: {}", action),
            )),
        }
    }
}

// 引入需要的类型
use crate::services::code::parser::{ProgrammingLanguage, SymbolType};
```

**步骤 2:在 register_builtin_tools 中注册 SourceCodeTool**

> **接口对齐说明**:`register_builtin_tools` 签名已在 T3.12 中扩展为包含 `db` 参数(见 overview 4.4.1)。
> SourceCodeTool 不需要额外参数(使用 workspace_root 由 executor 注入),直接在现有签名内注册即可。

修改 `src-tauri/src/services/tool/builtin.rs` 中的 `register_builtin_tools`:

```rust
// 在 register_builtin_tools 函数中添加(T3.12 已修改签名,此处仅新增注册)
registry.register(Box::new(SourceCodeTool::new()));
```

**验证**:
- `cargo build -p workmolde_lib` 成功
- 单元测试:搜索测试代码,正确返回符号

---

### T3.22:编写集成测试

**文件**:
- 创建:`src-tauri/tests/skill_context_integration_test.rs`

**实施内容**:
```rust
//! 阶段 3 集成测试:Skill 系统、TodoWrite、SessionCompaction、SourceCode

use workmolde_lib::services::skill::{SkillLoader, SkillRegistry, SkillTool};
use workmolde_lib::services::tool::trait_def::Tool;
use workmolde_lib::services::code::parser::{LanguageParser, ProgrammingLanguage};
use workmolde_lib::services::code::search::{SourceCodeSearcher, SearchQuery};
use workmolde_lib::models::todo::{TodoList, TodoItem, TodoStatus, TodoPriority};
use serde_json::json;
use std::path::PathBuf;
use tempfile::TempDir;

/// 测试:Skill 加载器解析 SKILL.md
#[tokio::test]
async fn test_skill_loader_parses_skill_md() {
    // 创建临时 Skill 目录
    let temp_dir = TempDir::new().unwrap();
    let skill_dir = temp_dir.path().join("test-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    
    // 创建 SKILL.md 文件
    let skill_md = "---\nname: test-skill\ndescription: Test Skill for integration test\nwhen: testing\nreadOnly: true\n---\n# Test Skill\n\nThis is a test skill.\n";
    std::fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();
    
    // 加载 Skill
    let loader = SkillLoader::new(
        temp_dir.path().to_path_buf(),
        None,
        vec![],
    );
    let skills = loader.load_all().unwrap();
    
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].frontmatter.name, "test-skill");
    assert_eq!(skills[0].frontmatter.description, "Test Skill for integration test");
    assert!(skills[0].frontmatter.read_only);
}

/// 测试:Skill 注册表按模式过滤
#[tokio::test]
async fn test_skill_registry_filters_by_mode() {
    let temp_dir = TempDir::new().unwrap();
    
    // 创建两个 Skill:一个适用于所有模式,一个仅适用于 build
    let skill1_dir = temp_dir.path().join("skill1");
    std::fs::create_dir_all(&skill1_dir).unwrap();
    std::fs::write(skill1_dir.join("SKILL.md"), 
        "---\nname: skill1\ndescription: All modes\n---\nContent 1").unwrap();
    
    let skill2_dir = temp_dir.path().join("skill2");
    std::fs::create_dir_all(&skill2_dir).unwrap();
    std::fs::write(skill2_dir.join("SKILL.md"), 
        "---\nname: skill2\ndescription: Build only\nmodes:\n  - build\n---\nContent 2").unwrap();
    
    // v1.1 新增:document 模式专用 Skill
    let skill3_dir = temp_dir.path().join("skill3");
    std::fs::create_dir_all(&skill3_dir).unwrap();
    std::fs::write(skill3_dir.join("SKILL.md"), 
        "---\nname: skill3\ndescription: Document only\nmodes:\n  - document\n---\nContent 3").unwrap();
    
    let loader = SkillLoader::new(temp_dir.path().to_path_buf(), None, vec![]);
    let registry = SkillRegistry::new(loader);
    registry.reload().unwrap();
    
    // plan 模式下只能看到 skill1
    let plan_skills = registry.list_by_mode("plan");
    assert_eq!(plan_skills.len(), 1);
    assert_eq!(plan_skills[0].frontmatter.name, "skill1");
    
    // build 模式下可以看到 skill1 和 skill2
    let build_skills = registry.list_by_mode("build");
    assert_eq!(build_skills.len(), 2);
    
    // v1.1 新增:document 模式下可以看到 skill1 和 skill3
    let document_skills = registry.list_by_mode("document");
    assert_eq!(document_skills.len(), 2);
    let document_names: Vec<&str> = document_skills.iter()
        .map(|s| s.frontmatter.name.as_str())
        .collect();
    assert!(document_names.contains(&"skill1"));
    assert!(document_names.contains(&"skill3"));
    assert!(!document_names.contains(&"skill2"));
}

/// 测试:TodoList 摘要生成
#[tokio::test]
async fn test_todo_list_summary() {
    let mut todo_list = TodoList::new("test-session".to_string());
    
    todo_list.items.push(TodoItem {
        id: "1".to_string(),
        content: "完成任务 A".to_string(),
        status: TodoStatus::Completed,
        priority: TodoPriority::High,
        created_at: 0,
        updated_at: 0,
        completed_at: Some(0),
    });
    
    todo_list.items.push(TodoItem {
        id: "2".to_string(),
        content: "开始任务 B".to_string(),
        status: TodoStatus::InProgress,
        priority: TodoPriority::Medium,
        created_at: 0,
        updated_at: 0,
        completed_at: None,
    });
    
    todo_list.items.push(TodoItem {
        id: "3".to_string(),
        content: "待处理任务 C".to_string(),
        status: TodoStatus::Pending,
        priority: TodoPriority::Low,
        created_at: 0,
        updated_at: 0,
        completed_at: None,
    });
    
    let summary = todo_list.build_summary().unwrap();
    assert!(summary.contains("[x] 完成任务 A"));
    assert!(summary.contains("[>] 开始任务 B"));
    assert!(summary.contains("[ ] 待处理任务 C"));
    assert!(summary.contains("进度: 1/3 已完成"));
}

/// 测试:tree-sitter 解析 Rust 代码
#[tokio::test]
async fn test_tree_sitter_parses_rust() {
    let code = r#"
/// 测试函数
fn test_function() -> u32 {
    42
}

struct TestStruct {
    field: u32,
}

impl TestStruct {
    fn method(&self) -> u32 {
        self.field
    }
}
"#;
    
    let mut parser = LanguageParser::new();
    let symbols = parser.parse_symbols(code, ProgrammingLanguage::Rust).unwrap();
    
    // 应识别出函数、结构体、方法
    let function_count = symbols.iter().filter(|s| s.name == "test_function").count();
    assert_eq!(function_count, 1);
    
    let struct_count = symbols.iter().filter(|s| s.name == "TestStruct").count();
    assert!(struct_count >= 1);
    
    let method_count = symbols.iter().filter(|s| s.name == "method").count();
    assert_eq!(method_count, 1);
}

/// 测试:SourceCode 搜索工具
#[tokio::test]
async fn test_source_code_search() {
    let temp_dir = TempDir::new().unwrap();
    
    // 创建测试 Rust 文件
    let test_file = temp_dir.path().join("test.rs");
    std::fs::write(&test_file, r#"
fn search_target_function() -> u32 {
    42
}

fn other_function() {
    search_target_function();
}
"#).unwrap();
    
    let mut searcher = SourceCodeSearcher::new();
    let query = SearchQuery {
        directory: temp_dir.path().to_path_buf(),
        symbol_name: Some("search_*".to_string()),
        symbol_type: None,
        extensions: vec!["rs".to_string()],
        recursive: true,
        max_results: 10,
    };
    
    let results = searcher.search(&query).unwrap();
    
    // 应找到 search_target_function
    let found = results.iter().any(|r| r.symbol.name == "search_target_function");
    assert!(found, "应找到 search_target_function");
}
```

**验证**:
- `cargo test` 全部通过

---

## 四、数据库迁移

### 4.1 新增表

| 表名 | 用途 | 创建方式 |
|------|------|---------|
| `skill_overrides` | Skill 启用/禁用配置 | `CREATE TABLE IF NOT EXISTS`(幂等) |
| `todo_lists` | Todo 任务列表(按 session_id) | `CREATE TABLE IF NOT EXISTS`(幂等) |

### 4.2 迁移脚本

无独立迁移脚本,通过 `init.rs` 的 `CREATE TABLE IF NOT EXISTS` 自动处理。

---

## 五、配置变更

### 5.1 新增配置项

| 配置项 | 位置 | 默认值 | 说明 |
|--------|------|--------|------|
| `compaction.enabled` | `CompactionConfig` | `true` | 是否启用上下文压缩 |
| `compaction.triggerThreshold` | `CompactionConfig` | `0.8` | 触发压缩的 token 占比 |
| `compaction.keepRecentMessages` | `CompactionConfig` | `10` | 压缩后保留的最近消息数 |
| `compaction.keepSystemTokens` | `CompactionConfig` | `4000` | 保留的系统提示词 token 数 |
| `compaction.compactToolOutputs` | `CompactionConfig` | `true` | 是否压缩工具输出 |
| `compaction.toolOutputMaxChars` | `CompactionConfig` | `2000` | 工具输出保留的最大字符数 |

### 5.2 Skill 目录约定

| 目录 | 用途 | 优先级 |
|------|------|--------|
| `~/.agent/skills/` | 全局 Skill 目录 | 低 |
| `.agent/skills/` | 项目 Skill 目录 | 高(覆盖全局) |
| 配置的额外路径 | 自定义 Skill 目录 | 中 |

---

## 六、事件清单

### 6.1 新增事件

| 事件名 | Payload | 说明 |
|--------|---------|------|
| `agent:compaction_start` | `CompactionStartPayload` | 上下文压缩开始 |
| `agent:compaction_done` | `CompactionDonePayload` | 上下文压缩完成 |

### 6.2 事件时序

```
[Agent 执行中]
   │
   ├── 检测到 token 超过阈值
   │
   ├── emit(agent:compaction_start)
   │
   ├── 调用 LLM 生成摘要
   │
   ├── 替换消息列表
   │
   └── emit(agent:compaction_done)
```

---

## 七、参考资源

### 7.1 OpenCode 相关源码

- **Skill 系统**: `packages/opencode/src/skill/`
- **TodoWrite 工具**: `packages/opencode/src/tool/todowrite.ts`
- **SessionCompaction**: `packages/opencode/src/session/compaction.ts`
- **SourceCode 工具**: `packages/opencode/src/tool/sourcecode.ts`

### 7.2 技术文档

- **tree-sitter 官方文档**:https://tree-sitter.github.io/tree-sitter/
- **YAML frontmatter 规范**:https://jekyllrb.com/docs/front-matter/
- **Anthropic Context Engineering**:Effective Context Engineering for AI Agents

### 7.3 相关文档

- [阶段 1:核心架构与工具链](./2026-07-08-coding-agent-refactor-phase1-core.md)
- [阶段 2:权限系统与 Agent 模式](./2026-07-08-coding-agent-refactor-phase2-permission.md)
- [总体改造计划](./2026-07-08-coding-agent-refactor-overview.md)

---

## 八、任务完成状态追踪

| 任务 ID | 任务名称 | 状态 | 完成时间 | 备注 |
|---------|---------|------|---------|------|
| T3.01 | 新增 Skill 系统所需依赖 | 待实施 | - | |
| T3.02 | 定义 Skill 类型与数据模型 | 待实施 | - | |
| T3.03 | 实现 Skill 仓库与持久化 | 待实施 | - | |
| T3.04 | 实现 Skill 加载器 | 待实施 | - | |
| T3.05 | 实现 Skill 注册表 | 待实施 | - | |
| T3.06 | 实现 Skill 工具 | 待实施 | - | |
| T3.07 | Skill 清单注入系统提示词 | 待实施 | - | |
| T3.08 | Skill 权限过滤（已移除 Plan 模式 read_only 检查） | 已修改 | compaction-skill-refinement | 移除 read_only 检查，Plan 模式由 check_permission 保证只读 |
| T3.09 | Skill 热重载与文件监听 | 待实施 | - | |
| T3.10 | 定义 TodoWrite 数据模型 | 待实施 | - | |
| T3.11 | 实现 TodoWrite 仓库 | 待实施 | - | |
| T3.12 | 实现 TodoWrite 工具 | 待实施 | - | |
| T3.13 | 集成 TodoWrite 到 AgentContext | 待实施 | - | |
| T3.14 | 替代 Scratchpad 摘要功能 | 待实施 | - | |
| T3.15 | 定义 SessionCompaction 配置 | 待实施 | - | |
| T3.16 | 实现 Compaction 策略 | 已修改 | compaction-skill-refinement | generate_summary 使用英文提示词，[Context Compaction Summary] 前缀 |
| T3.17 | 集成 Compaction 到 AgentExecutor（使用 compact_messages 方法） | 已修改 | compaction-skill-refinement | compact_messages 接受 preferred_provider_id 参数 |
| T3.18 | Compaction 事件与前端通知 | 已修改 | compaction-skill-refinement | 事件前缀改为 [Context Compaction Summary] |
| T3.19 | 新增 tree-sitter 依赖 | 待实施 | - | |
| T3.20 | 实现 LanguageParser | 待实施 | - | |
| T3.21 | 实现 SourceCode 工具 | 待实施 | - | |
| T3.22 | 编写集成测试 | 待实施 | - | |

---

## 九、风险与回滚策略

### 9.1 主要风险点

1. **Skill 加载性能**:大量 Skill 文件时加载缓慢
   - 缓解:懒加载,系统提示词仅注入清单,详细内容按需加载
   - 回滚:禁用 Skill 系统,不影响其他功能

2. **tree-sitter 依赖体积**:多语言支持增加二进制体积
   - 缓解:按需编译,只包含常用语言
   - 回滚:移除 tree-sitter 依赖,降级为文本搜索

3. **Compaction 质量不稳定**:LLM 生成的摘要可能丢失关键信息
   - 缓解:保留最近 N 条消息不压缩;保留系统提示词
   - 回滚:设置 `compaction.enabled = false`

4. **TodoWrite 与 Scratchpad 冲突**:两个工具功能重叠
   - 缓解:明确分工,TodoWrite 负责结构化任务,Scratchpad 负责自由笔记
   - 回滚:移除 TodoWrite,恢复 Scratchpad 摘要注入

### 9.2 验收标准

- 所有 22 个任务(T3.01-T3.22)实施完成
- `cargo test` 全部通过(包括 5 个新增集成测试)
- `cargo clippy` 无警告
- `npx tsc -b` 无类型错误
- 手动测试:创建 `.agent/skills/test-skill/SKILL.md`,Agent 可通过 `skill` 工具加载
- 手动测试:Agent 调用 `todowrite` 工具创建任务,系统提示词中显示任务清单
- 手动测试:长对话触发上下文压缩,前端展示压缩事件
- 手动测试:`source_code` 工具搜索指定符号,返回正确结果

---

## 十、后续阶段衔接说明

本阶段完成后,后续阶段将基于 Skill 系统和上下文管理进行扩展:

- **阶段 4(子 Agent 与高级工具)**:子 Agent 可继承父 Agent 的 Skill 上下文;WebFetch/WebSearch 工具受权限系统控制
- **阶段 5(LSP 集成)**:LSP 工具与 SourceCode 工具互补,SourceCode 提供基于语法树的搜索,LSP 提供基于语义索引的跳转/引用查找

Skill 系统和上下文管理是 Agent 能力扩展的基础,必须确保本阶段完全实施并通过验收后再进入下一阶段。
