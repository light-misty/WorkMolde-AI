# DocAgent 编程 Agent 改造 - 阶段 2:权限系统与 Agent 模式

> 文档版本:v1.1(2026-07-08 修订:新增 Document 模式,移除 plan_exit 工具,改为前端按钮切换)
> 创建日期:2026-07-08
> 所属阶段:阶段 2(在阶段 1 完成后开展)
> 上游文档:[总体改造计划](./2026-07-08-coding-agent-refactor-overview.md)
> 前置文档:[阶段 1:核心架构与工具链](./2026-07-08-coding-agent-refactor-phase1-core.md)
> 改造目标:实现 OpenCode 风格的三态权限系统(allow/deny/ask)+ Plan/Build/Document 三态模式切换 + Doom loop 检测

---

## 一、阶段目标与范围

### 1.1 阶段目标

在阶段 1 已经建立核心编程能力(文件读写、编辑、搜索、命令执行)的基础上,本阶段将 DocAgent 升级为具备精细化权限管控和模式切换的编程 Agent:

1. **三态权限系统(allow/deny/ask)**:替换现有的 `ConfirmationLevel` 三档确认级别,改为基于规则的细粒度权限管控,支持按工具类型、命令模式、文件路径匹配
2. **用户审批三选项(once/always/reject)**:替换现有的 approve/feedback 二元决策,改为"本次允许/会话内永久允许/拒绝"三选项,`always` 自动生成临时白名单规则
3. **权限规则持久化**:将权限规则存入 SQLite `permission_rules` 表,支持全局规则、项目级规则、会话级临时规则三层
4. **Plan/Build/Document 三态模式切换**:实现 Plan(只读规划)、Build(完整执行)、Document(Build 超集 + 文档 Handler)三态模式,仅通过前端按钮切换(不提供 LLM 工具切换模式)
5. **Doom loop 检测**:检测连续 3 次相同工具调用(相同参数),触发 `doom_loop` 权限规则
6. **Agent 类型特定提示词**:Plan/Build/Document 三种模式注入不同的系统提示词,引导 Agent 行为
7. **工具列表动态过滤**:基于 AgentMode 过滤工具定义,非 Document 模式下 4 个文档 Handler(docx/xlsx/pptx/pdf)不出现在 tool_definitions 中,LLM 完全感知不到它们的存在
8. **前端模式切换 UI**:在 InputArea 增加 Plan/Build/Document 三态模式切换按钮(不改变整体 UI 设计)
9. **前端权限对话框**:升级确认对话框为权限审批对话框,支持三个按钮

### 1.2 范围边界

**本阶段包含**:
- 定义权限类型(PermissionType)和动作(PermissionAction)枚举
- 实现通配符匹配工具(基于 globset/wildmatch crate)
- 创建 `permission_rules` 数据库表和仓库
- 实现 PermissionRegistry(规则加载、合并、评估)
- 改造 AgentExecutor 集成权限系统
- 改造 `confirm_operation` 命令支持 once/always/reject
- 实现临时白名单(会话内 always 规则缓存)
- 实现 Doom loop 检测器(连续 3 次相同调用)
- 实现 Agent 模式(Plan/Build/Document)枚举和切换逻辑(仅前端按钮切换,不提供 LLM 工具)
- 实现工具列表动态过滤(基于 AgentMode,非 Document 模式过滤掉 4 个文档 Handler)
- 重构系统提示词层(按 Agent 模式注入不同提示词,含 Document 模式分支)
- 改造 AppState 加入 `permission_registry` 和 `agent_mode`
- 前端:InputArea 增加模式切换按钮
- 前端:权限审批对话框升级(once/always/reject)
- 前端:设置弹窗增加权限规则管理 UI

**本阶段不包含**(留给后续阶段):
- Skill 系统 → 阶段 3
- TodoWrite 工具 → 阶段 3
- SessionCompaction 上下文压缩 → 阶段 3
- 子 Agent (task 工具) → 阶段 4
- WebFetch/WebSearch → 阶段 4
- LSP 集成 → 阶段 5

### 1.3 验收标准

- [ ] `cargo build -p docagent_lib` 编译通过,无警告
- [ ] `cargo test` 全部测试通过(含新增的权限系统测试)
- [ ] `cargo clippy` 无警告
- [ ] `cargo fmt --check` 通过
- [ ] `npm run build` 前端构建通过
- [ ] 权限规则能在数据库中持久化,重启后规则生效
- [ ] Plan 模式下 edit/bash 等修改类工具被拒绝,read/glob/grep/list 可用
- [ ] Build 模式下所有编程工具可用(受权限规则约束),文档 Handler 不出现在工具列表
- [ ] Document 模式下文档 Handler(docx/xlsx/pptx/pdf)出现在工具列表且可调用,编程工具完全可用
- [ ] 用户点击 `always` 后,会话内相同操作不再弹窗
- [ ] 连续 3 次相同工具调用触发 Doom loop 检测
- [ ] 前端 Plan/Build/Document 三态切换按钮和权限对话框交互正常
- [ ] 模式切换仅由前端按钮触发,LLM 无法自主切换模式

---

## 二、任务分解总览

本阶段共分解为 19 个任务,按依赖顺序排列:

| 任务 ID | 任务名称 | 类型 | 预估难度 | 依赖 |
|---------|---------|------|---------|------|
| T2.01 | 新增权限系统所需依赖到 Cargo.toml | 配置 | 低 | 无 |
| T2.02 | 定义权限类型与动作枚举 | 新增 | 中 | T2.01 |
| T2.03 | 实现通配符匹配工具 | 新增 | 中 | T2.01 |
| T2.04 | 创建 permission_rules 数据库表 | 新增 | 低 | T2.02 |
| T2.05 | 实现 PermissionRule 模型与仓库 | 新增 | 中 | T2.04 |
| T2.06 | 实现 PermissionRegistry 权限注册表 | 新增 | 高 | T2.03, T2.05 |
| T2.07 | 实现 PermissionEvaluator 权限评估器 | 新增 | 高 | T2.06 |
| T2.08 | 实现临时白名单(会话级 always 规则) | 新增 | 中 | T2.06 |
| T2.09 | 实现 Doom loop 检测器 | 新增 | 中 | T2.02 |
| T2.10 | 改造 AgentExecutor 集成权限系统 | 重构 | 高 | T2.07, T2.08, T2.09 |
| T2.11 | 改造 confirm_operation 命令支持 once/always/reject | 改造 | 中 | T2.10 |
| T2.12 | 定义 Agent 模式枚举(Plan/Build/Document) | 新增 | 低 | T2.02 |
| T2.13 | 实现工具列表动态过滤(按 AgentMode 过滤文档 Handler) | 新增 | 中 | T2.12 |
| T2.14 | 重构系统提示词层(按 Agent 模式注入,含 Document 分支) | 重构 | 中 | T2.12 |
| T2.15 | 改造 AppState 加入 permission_registry 和 agent_mode | 重构 | 中 | T2.06, T2.12 |
| T2.16 | 前端:InputArea 增加 Plan/Build/Document 模式切换按钮 | 前端 | 中 | T2.15 |
| T2.17 | 前端:权限审批对话框升级(once/always/reject) | 前端 | 中 | T2.11 |
| T2.18 | 前端:设置弹窗增加权限规则管理 UI | 前端 | 中 | T2.05 |
| T2.19 | 集成测试:验证权限系统与三态模式切换 | 测试 | 高 | T2.10-T2.18 |

---

## 三、详细任务实施

### T2.01:新增权限系统所需依赖到 Cargo.toml

**文件**:
- 修改: `src-tauri/Cargo.toml`

**说明**:
权限系统需要通配符匹配和日期时间处理,新增 `wildmatch` crate(轻量级通配符匹配,比 globset 更适合命令字符串匹配)。

**实施步骤**:

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 段新增:

```toml
# 权限系统通配符匹配(支持 * 和 ? 通配符,适用于命令字符串和文件路径)
wildmatch = "2.4"
```

**验证**:
```bash
cargo build -p docagent_lib
```
预期:编译通过,无警告。

---

### T2.02:定义权限类型与动作枚举

**文件**:
- 新增: `src-tauri/src/services/permission/mod.rs`
- 新增: `src-tauri/src/services/permission/types.rs`

**说明**:
定义权限系统的核心枚举类型,参照 OpenCode 的 12 类权限项和 3 种动作。

**实施步骤**:

#### 1. 创建 permission 模块入口

文件: `src-tauri/src/services/permission/mod.rs`

```rust
// 权限系统模块入口
// 实现 OpenCode 风格的三态权限系统(allow/deny/ask)

pub mod types;
pub mod wildcard;
pub mod rule;
pub mod registry;
pub mod evaluator;
pub mod session_whitelist;
pub mod doom_loop;

pub use types::*;
pub use wildcard::*;
pub use rule::*;
pub use registry::*;
pub use evaluator::*;
pub use session_whitelist::*;
pub use doom_loop::*;
```

#### 2. 定义权限类型与动作枚举

文件: `src-tauri/src/services/permission/types.rs`

```rust
use serde::{Deserialize, Serialize};
use std::fmt;

/// 权限动作(三态决策)
/// 对应 OpenCode 的 allow/deny/ask
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PermissionAction {
    /// 允许:直接执行,无需用户确认
    Allow,
    /// 拒绝:阻止执行,返回错误给 LLM
    Deny,
    /// 询问:弹出对话框,等待用户审批
    #[default]
    Ask,
}

impl fmt::Display for PermissionAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PermissionAction::Allow => write!(f, "allow"),
            PermissionAction::Deny => write!(f, "deny"),
            PermissionAction::Ask => write!(f, "ask"),
        }
    }
}

impl PermissionAction {
    /// 从字符串解析权限动作
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "allow" => Some(Self::Allow),
            "deny" => Some(Self::Deny),
            "ask" => Some(Self::Ask),
            _ => None,
        }
    }
}

/// 权限类型(对应工具类别)
/// 参照 OpenCode 的 12 类权限项
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionType {
    /// 通配:匹配所有工具
    Wildcard,
    /// 读取文件:read, read_lines
    Read,
    /// 编辑文件:edit, write, remove, rename
    Edit,
    /// 通配符搜索:glob
    Glob,
    /// 正则搜索:grep, search
    Grep,
    /// 列出目录:list
    List,
    /// 执行 Shell 命令:bash
    Bash,
    /// 写入并执行脚本:write_script
    WriteScript,
    /// 子 Agent 调用:task(阶段4实现)
    Task,
    /// Skill 加载(阶段3实现)
    Skill,
    /// LSP 调用(阶段5实现)
    Lsp,
    /// 网页抓取:webfetch(阶段4实现)
    WebFetch,
    /// 网络搜索:websearch(阶段4实现)
    WebSearch,
    /// 外部目录访问:工作区外的路径
    ExternalDirectory,
    /// Doom loop 检测:连续 3 次相同调用
    DoomLoop,
    /// 文档处理:docx, xlsx, pptx, pdf
    /// v1.1 新增:用于 Document 模式下的文档 Handler 权限控制
    Document,
}

impl fmt::Display for PermissionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PermissionType::Wildcard => write!(f, "*"),
            PermissionType::Read => write!(f, "read"),
            PermissionType::Edit => write!(f, "edit"),
            PermissionType::Glob => write!(f, "glob"),
            PermissionType::Grep => write!(f, "grep"),
            PermissionType::List => write!(f, "list"),
            PermissionType::Bash => write!(f, "bash"),
            PermissionType::WriteScript => write!(f, "write_script"),
            PermissionType::Task => write!(f, "task"),
            PermissionType::Skill => write!(f, "skill"),
            PermissionType::Lsp => write!(f, "lsp"),
            PermissionType::WebFetch => write!(f, "webfetch"),
            PermissionType::WebSearch => write!(f, "websearch"),
            PermissionType::ExternalDirectory => write!(f, "external_directory"),
            PermissionType::DoomLoop => write!(f, "doom_loop"),
            PermissionType::Document => write!(f, "document"),
        }
    }
}

impl PermissionType {
    /// 从字符串解析权限类型
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "*" => Some(Self::Wildcard),
            "read" => Some(Self::Read),
            "edit" => Some(Self::Edit),
            "glob" => Some(Self::Glob),
            "grep" => Some(Self::Grep),
            "list" => Some(Self::List),
            "bash" => Some(Self::Bash),
            "write_script" => Some(Self::WriteScript),
            "task" => Some(Self::Task),
            "skill" => Some(Self::Skill),
            "lsp" => Some(Self::Lsp),
            "webfetch" => Some(Self::WebFetch),
            "websearch" => Some(Self::WebSearch),
            "external_directory" => Some(Self::ExternalDirectory),
            "doom_loop" => Some(Self::DoomLoop),
            "document" => Some(Self::Document),
            _ => None,
        }
    }

    /// 根据工具名推断权限类型
    /// 用于将工具调用映射到权限检查
    pub fn from_tool_name(tool_name: &str) -> Self {
        match tool_name {
            "read" | "read_lines" => Self::Read,
            "edit" | "write" | "remove" | "rename" | "copy" => Self::Edit,
            "remove_dir" => Self::Edit,
            "glob" => Self::Glob,
            "grep" | "search" => Self::Grep,
            "list" => Self::List,
            "bash" => Self::Bash,
            "write_script" => Self::WriteScript,
            "task" => Self::Task,
            "webfetch" => Self::WebFetch,
            "websearch" => Self::WebSearch,
            // v1.1: 文档 Handler 映射到 Document 权限类型
            "docx" | "xlsx" | "pptx" | "pdf" => Self::Document,
            _ => Self::Wildcard,
        }
    }

    /// 判断该权限类型是否为修改类(在 Plan 模式下应被拒绝)
    /// Plan 模式只允许只读操作
    /// v1.1: 新增 Document 类型(文档 Handler 可生成/修改文档)
    pub fn is_modification(&self) -> bool {
        matches!(
            self,
            PermissionType::Edit
                | PermissionType::Bash
                | PermissionType::WriteScript
                | PermissionType::Task
                | PermissionType::WebFetch
                | PermissionType::WebSearch
                | PermissionType::Document
        )
    }
}

/// 权限规则作用域
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuleScope {
    /// 全局规则:对所有项目生效
    #[default]
    Global,
    /// 项目规则:对指定工作区生效
    Project,
    /// 会话临时规则:仅当前会话生效(always 选项生成)
    Session,
}

/// 用户审批回复
/// 对应 OpenCode 的 once/always/reject
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionResponse {
    /// 仅本次允许:下次相同操作仍需询问
    Once,
    /// 会话内永久允许:相同操作不再询问
    Always,
    /// 拒绝本次操作
    Reject,
}

impl PermissionResponse {
    /// 从用户回复解析
    /// approved=true 且 scope="once" → Once
    /// approved=true 且 scope="always" → Always
    /// approved=false → Reject
    pub fn from_user_reply(approved: bool, always: bool) -> Self {
        if !approved {
            Self::Reject
        } else if always {
            Self::Always
        } else {
            Self::Once
        }
    }
}
```

**验证**:
```bash
cargo build -p docagent_lib
```

---

### T2.03:实现通配符匹配工具

**文件**:
- 新增: `src-tauri/src/services/permission/wildcard.rs`

**说明**:
基于 `wildmatch` crate 实现通配符匹配,支持 `*` 和 `?`,并提供路径展开(`~` → 用户主目录)。

**实施步骤**:

文件: `src-tauri/src/services/permission/wildcard.rs`

```rust
use wildmatch::WildMatch;

/// 通配符匹配器
/// 支持 * (匹配任意数量字符) 和 ? (匹配单个字符)
#[derive(Debug, Clone)]
pub struct WildcardMatcher {
    pattern: String,
    matcher: WildMatch,
}

impl WildcardMatcher {
    /// 创建通配符匹配器
    pub fn new(pattern: &str) -> Self {
        Self {
            pattern: pattern.to_string(),
            matcher: WildMatch::new(pattern),
        }
    }

    /// 检查目标字符串是否匹配模式
    pub fn matches(&self, target: &str) -> bool {
        self.matcher.matches(target)
    }

    /// 获取原始模式字符串
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// 计算模式的具体性(specificity)
    /// 通配符越少,具体性越高,用于规则优先级排序
    /// 返回值越大表示越具体
    pub fn specificity(&self) -> usize {
        // 通配符 * 和 ? 的数量越少,具体性越高
        // 用 (长度 - 通配符数量) 作为具体性得分
        let wildcard_count = self.pattern.chars().filter(|c| *c == '*' || *c == '?').count();
        self.pattern.len().saturating_sub(wildcard_count)
    }
}

/// 展开路径中的 ~ 和 $HOME 为用户主目录
/// 仅用于文件路径匹配,不用于命令字符串
pub fn expand_home_path(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]).to_string_lossy().to_string();
        }
    }
    if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home.to_string_lossy().to_string();
        }
    }
    if path.starts_with("$HOME/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[6..]).to_string_lossy().to_string();
        }
    }
    path.to_string()
}

/// 规范化文件路径用于匹配
/// 1. 将反斜杠转换为正斜杠(Windows 兼容)
/// 2. 展开主目录
/// 3. 移除尾部的路径分隔符
pub fn normalize_path_for_match(path: &str) -> String {
    let expanded = expand_home_path(path);
    let normalized = expanded.replace('\\', "/");
    // 移除尾部斜杠(但保留根路径 "/")
    if normalized.len() > 1 && normalized.ends_with('/') {
        normalized.trim_end_matches('/').to_string()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wildcard_basic_match() {
        let m = WildcardMatcher::new("*.ts");
        assert!(m.matches("file.ts"));
        assert!(m.matches("src/component.ts"));
        assert!(!m.matches("file.rs"));
    }

    #[test]
    fn test_wildcard_question_mark() {
        let m = WildcardMatcher::new("file?.txt");
        assert!(m.matches("file1.txt"));
        assert!(m.matches("file2.txt"));
        assert!(!m.matches("file.txt"));
    }

    #[test]
    fn test_wildcard_command_match() {
        let m = WildcardMatcher::new("git *");
        assert!(m.matches("git status"));
        assert!(m.matches("git push origin main"));
        assert!(!m.matches("npm install"));
    }

    #[test]
    fn test_specificity() {
        let s1 = WildcardMatcher::new("*").specificity();
        let s2 = WildcardMatcher::new("src/**/*.ts").specificity();
        let s3 = WildcardMatcher::new("src/component.ts").specificity();
        // 越具体的模式 specificity 越大
        assert!(s1 < s2);
        assert!(s2 < s3);
    }

    #[test]
    fn test_normalize_path_windows() {
        let n = normalize_path_for_match("src\\components\\Button.tsx");
        assert_eq!(n, "src/components/Button.tsx");
    }
}
```

**验证**:
```bash
cargo test wildcard -- --nocapture
```

---

### T2.04:创建 permission_rules 数据库表

**文件**:
- 修改: `src-tauri/src/db/init.rs`

**说明**:
新增 `permission_rules` 表存储权限规则,支持全局、项目、会话三个作用域。

**实施步骤**:

在 `src-tauri/src/db/init.rs` 的 `create_tables` 函数末尾新增:

```rust
    // permission_rules 权限规则表(阶段2)
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
        );"
    )?;

    // 为权限规则查询创建索引
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_permission_rules_scope
         ON permission_rules(scope);
         CREATE INDEX IF NOT EXISTS idx_permission_rules_workspace
         ON permission_rules(workspace_id);
         CREATE INDEX IF NOT EXISTS idx_permission_rules_session
         ON permission_rules(session_id);
         CREATE INDEX IF NOT EXISTS idx_permission_rules_type
         ON permission_rules(permission_type);"
    )?;
```

同时,在 `src-tauri/src/db/mod.rs` 新增模块声明:

```rust
pub mod permission_repo;
```

**验证**:
```bash
cargo build -p docagent_lib
```
预期:数据库初始化时自动创建新表。

---

### T2.05:实现 PermissionRule 模型与仓库

**文件**:
- 新增: `src-tauri/src/models/permission.rs`
- 新增: `src-tauri/src/db/permission_repo.rs`
- 修改: `src-tauri/src/models/mod.rs`(新增 `pub mod permission;`)

**说明**:
定义 PermissionRule 数据模型,实现 CRUD 仓库。

**实施步骤**:

#### 1. 定义 PermissionRule 模型

文件: `src-tauri/src/models/permission.rs`

```rust
use serde::{Deserialize, Serialize};

use crate::services::permission::{PermissionAction, PermissionType, RuleScope};

/// 权限规则数据模型
/// 对应 permission_rules 表的一条记录
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRule {
    /// 规则 ID(格式:rule_<uuid>)
    pub id: String,
    /// 作用域:global / project / session
    pub scope: RuleScope,
    /// 工作区 ID(仅 scope=project 时有效)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// 会话 ID(仅 scope=session 时有效)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// 权限类型(对应工具类别)
    pub permission_type: PermissionType,
    /// 匹配模式(通配符,如 "src/**/*.ts"、"git *"、"*.env")
    pub pattern: String,
    /// 权限动作:allow / deny / ask
    pub action: PermissionAction,
    /// 规则描述(用户可读)
    #[serde(default)]
    pub description: String,
    /// 是否启用
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 创建时间(ISO 8601)
    pub created_at: String,
    /// 更新时间(ISO 8601)
    pub updated_at: String,
}

fn default_enabled() -> bool {
    true
}

impl PermissionRule {
    /// 创建新规则(自动生成 ID 和时间戳)
    pub fn new(
        scope: RuleScope,
        permission_type: PermissionType,
        pattern: String,
        action: PermissionAction,
    ) -> Self {
        let now = current_iso8601();
        Self {
            id: format!("rule_{}", uuid::Uuid::new_v4()),
            scope,
            workspace_id: None,
            session_id: None,
            permission_type,
            pattern,
            action,
            description: String::new(),
            enabled: true,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// 关联到工作区
    pub fn with_workspace(mut self, workspace_id: String) -> Self {
        self.workspace_id = Some(workspace_id);
        self
    }

    /// 关联到会话
    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// 设置描述
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }
}

/// 生成 ISO 8601 格式的当前时间字符串
fn current_iso8601() -> String {
    // 使用 SQLite 兼容的 ISO 8601 格式
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // 简化处理:实际由数据库 DEFAULT 生成,此处仅用于内存对象
    format!("{}", now)
}

/// 权限规则过滤器(查询时使用)
#[derive(Debug, Clone, Default)]
pub struct PermissionRuleFilter {
    pub scope: Option<RuleScope>,
    pub workspace_id: Option<String>,
    pub session_id: Option<String>,
    pub permission_type: Option<PermissionType>,
    pub enabled_only: bool,
}
```

#### 2. 实现 permission_repo 仓库

文件: `src-tauri/src/db/permission_repo.rs`

```rust
use rusqlite::{params, Connection};

use crate::errors::{CommandError, DB_QUERY_FAILED, DB_RECORD_NOT_FOUND};
use crate::models::permission::{PermissionRule, PermissionRuleFilter};
use crate::services::permission::{PermissionAction, PermissionType, RuleScope};

/// 插入一条权限规则
pub fn insert_rule(conn: &Connection, rule: &PermissionRule) -> Result<(), CommandError> {
    let scope_str = match rule.scope {
        RuleScope::Global => "global",
        RuleScope::Project => "project",
        RuleScope::Session => "session",
    };
    let type_str = rule.permission_type.to_string();
    let action_str = rule.action.to_string();

    conn.execute(
        "INSERT INTO permission_rules
         (id, scope, workspace_id, session_id, permission_type, pattern, action, description, enabled, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            rule.id,
            scope_str,
            rule.workspace_id,
            rule.session_id,
            type_str,
            rule.pattern,
            action_str,
            rule.description,
            rule.enabled as i32,
            rule.created_at,
            rule.updated_at,
        ],
    ).map_err(|e| CommandError::db(DB_QUERY_FAILED, format!("插入权限规则失败: {}", e)))?;

    log::info!("已插入权限规则: id={}, type={}, pattern={}, action={}",
        rule.id, type_str, rule.pattern, action_str);
    Ok(())
}

/// 根据过滤器查询权限规则列表
pub fn list_rules(conn: &Connection, filter: &PermissionRuleFilter) -> Result<Vec<PermissionRule>, CommandError> {
    let mut sql = String::from("SELECT id, scope, workspace_id, session_id, permission_type, pattern, action, description, enabled, created_at, updated_at FROM permission_rules WHERE 1=1");
    let mut param_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut param_idx = 1;

    if let Some(scope) = filter.scope {
        sql.push_str(&format!(" AND scope = ?{}", param_idx));
        let scope_str = match scope {
            RuleScope::Global => "global",
            RuleScope::Project => "project",
            RuleScope::Session => "session",
        };
        param_values.push(Box::new(scope_str.to_string()));
        param_idx += 1;
    }

    if let Some(ref workspace_id) = filter.workspace_id {
        sql.push_str(&format!(" AND (workspace_id = ?{} OR workspace_id IS NULL)", param_idx));
        param_values.push(Box::new(workspace_id.clone()));
        param_idx += 1;
    }

    if let Some(ref session_id) = filter.session_id {
        sql.push_str(&format!(" AND (session_id = ?{} OR session_id IS NULL)", param_idx));
        param_values.push(Box::new(session_id.clone()));
        param_idx += 1;
    }

    if let Some(ptype) = filter.permission_type {
        sql.push_str(&format!(" AND permission_type = ?{}", param_idx));
        param_values.push(Box::new(ptype.to_string()));
        param_idx += 1;
    }

    if filter.enabled_only {
        sql.push_str(" AND enabled = 1");
    }

    sql.push_str(" ORDER BY created_at ASC");

    let mut stmt = conn.prepare(&sql).map_err(|e| {
        CommandError::db(DB_QUERY_FAILED, format!("准备查询权限规则失败: {}", e))
    })?;

    let rows = stmt.query_map(
        param_values.iter().map(|p| p.as_ref()).collect::<Vec<_>>().as_slice(),
        |row| {
            let scope_str: String = row.get(1)?;
            let workspace_id: Option<String> = row.get(2)?;
            let session_id: Option<String> = row.get(3)?;
            let type_str: String = row.get(4)?;
            let pattern: String = row.get(5)?;
            let action_str: String = row.get(6)?;
            let description: String = row.get(7)?;
            let enabled: i32 = row.get(8)?;
            let created_at: String = row.get(9)?;
            let updated_at: String = row.get(10)?;

            let scope = match scope_str.as_str() {
                "global" => RuleScope::Global,
                "project" => RuleScope::Project,
                "session" => RuleScope::Session,
                _ => RuleScope::Global,
            };
            let permission_type = PermissionType::from_str(&type_str).unwrap_or(PermissionType::Wildcard);
            let action = PermissionAction::from_str(&action_str).unwrap_or(PermissionAction::Ask);

            Ok(PermissionRule {
                id: row.get(0)?,
                scope,
                workspace_id,
                session_id,
                permission_type,
                pattern,
                action,
                description,
                enabled: enabled != 0,
                created_at,
                updated_at,
            })
        }
    ).map_err(|e| CommandError::db(DB_QUERY_FAILED, format!("查询权限规则失败: {}", e)))?;

    let mut rules = Vec::new();
    for row in rows {
        rules.push(row.map_err(|e| CommandError::db(DB_QUERY_FAILED, format!("读取权限规则行失败: {}", e)))?);
    }
    Ok(rules)
}

/// 根据 ID 获取单条权限规则
pub fn get_rule(conn: &Connection, rule_id: &str) -> Result<PermissionRule, CommandError> {
    let mut stmt = conn.prepare(
        "SELECT id, scope, workspace_id, session_id, permission_type, pattern, action, description, enabled, created_at, updated_at FROM permission_rules WHERE id = ?1"
    ).map_err(|e| CommandError::db(DB_QUERY_FAILED, format!("准备查询权限规则失败: {}", e)))?;

    let rule = stmt.query_row(params![rule_id], |row| {
        let scope_str: String = row.get(1)?;
        let workspace_id: Option<String> = row.get(2)?;
        let session_id: Option<String> = row.get(3)?;
        let type_str: String = row.get(4)?;
        let pattern: String = row.get(5)?;
        let action_str: String = row.get(6)?;
        let description: String = row.get(7)?;
        let enabled: i32 = row.get(8)?;
        let created_at: String = row.get(9)?;
        let updated_at: String = row.get(10)?;

        let scope = match scope_str.as_str() {
            "global" => RuleScope::Global,
            "project" => RuleScope::Project,
            "session" => RuleScope::Session,
            _ => RuleScope::Global,
        };
        let permission_type = PermissionType::from_str(&type_str).unwrap_or(PermissionType::Wildcard);
        let action = PermissionAction::from_str(&action_str).unwrap_or(PermissionAction::Ask);

        Ok(PermissionRule {
            id: row.get(0)?,
            scope,
            workspace_id,
            session_id,
            permission_type,
            pattern,
            action,
            description,
            enabled: enabled != 0,
            created_at,
            updated_at,
        })
    }).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CommandError::db(DB_RECORD_NOT_FOUND, format!("权限规则不存在: {}", rule_id)),
        e => CommandError::db(DB_QUERY_FAILED, format!("查询权限规则失败: {}", e)),
    })?;

    Ok(rule)
}

/// 更新权限规则
pub fn update_rule(conn: &Connection, rule: &PermissionRule) -> Result<(), CommandError> {
    let scope_str = match rule.scope {
        RuleScope::Global => "global",
        RuleScope::Project => "project",
        RuleScope::Session => "session",
    };
    let type_str = rule.permission_type.to_string();
    let action_str = rule.action.to_string();
    let now = current_iso8601();

    conn.execute(
        "UPDATE permission_rules SET scope=?2, workspace_id=?3, session_id=?4, permission_type=?5, pattern=?6, action=?7, description=?8, enabled=?9, updated_at=?10 WHERE id=?1",
        params![
            rule.id,
            scope_str,
            rule.workspace_id,
            rule.session_id,
            type_str,
            rule.pattern,
            action_str,
            rule.description,
            rule.enabled as i32,
            now,
        ],
    ).map_err(|e| CommandError::db(DB_QUERY_FAILED, format!("更新权限规则失败: {}", e)))?;

    log::info!("已更新权限规则: id={}", rule.id);
    Ok(())
}

/// 删除权限规则
pub fn delete_rule(conn: &Connection, rule_id: &str) -> Result<(), CommandError> {
    conn.execute("DELETE FROM permission_rules WHERE id=?1", params![rule_id])
        .map_err(|e| CommandError::db(DB_QUERY_FAILED, format!("删除权限规则失败: {}", e)))?;
    log::info!("已删除权限规则: id={}", rule_id);
    Ok(())
}

/// 删除指定会话的所有临时规则(会话结束时清理)
pub fn delete_session_rules(conn: &Connection, session_id: &str) -> Result<u64, CommandError> {
    let affected = conn.execute(
        "DELETE FROM permission_rules WHERE session_id=?1 AND scope='session'",
        params![session_id],
    ).map_err(|e| CommandError::db(DB_QUERY_FAILED, format!("删除会话权限规则失败: {}", e)))?;
    log::info!("已删除会话 {} 的 {} 条临时权限规则", session_id, affected);
    Ok(affected as u64)
}

/// 生成 ISO 8601 格式的当前时间字符串
fn current_iso8601() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use std::path::Path;

    fn setup_test_db() -> Database {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        Database::new(Path::new(tmp.path())).unwrap()
    }

    #[test]
    fn test_insert_and_get_rule() {
        let db = setup_test_db();
        let conn = db.conn().unwrap();
        let rule = PermissionRule::new(
            RuleScope::Global,
            PermissionType::Bash,
            "rm *".to_string(),
            PermissionAction::Deny,
        ).with_description("禁止删除命令");

        insert_rule(&conn, &rule).unwrap();
        let fetched = get_rule(&conn, &rule.id).unwrap();
        assert_eq!(fetched.pattern, "rm *");
        assert_eq!(fetched.action, PermissionAction::Deny);
        assert_eq!(fetched.description, "禁止删除命令");
    }

    #[test]
    fn test_list_rules_by_scope() {
        let db = setup_test_db();
        let conn = db.conn().unwrap();

        let r1 = PermissionRule::new(RuleScope::Global, PermissionType::Edit, "*".into(), PermissionAction::Allow);
        let r2 = PermissionRule::new(RuleScope::Project, PermissionType::Bash, "git *".into(), PermissionAction::Allow).with_workspace("ws1".into());
        insert_rule(&conn, &r1).unwrap();
        insert_rule(&conn, &r2).unwrap();

        let filter = PermissionRuleFilter { scope: Some(RuleScope::Global), ..Default::default() };
        let rules = list_rules(&conn, &filter).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, r1.id);
    }

    #[test]
    fn test_delete_session_rules() {
        let db = setup_test_db();
        let conn = db.conn().unwrap();
        let r1 = PermissionRule::new(RuleScope::Session, PermissionType::Bash, "npm *".into(), PermissionAction::Allow).with_session("sess1".into());
        let r2 = PermissionRule::new(RuleScope::Global, PermissionType::Edit, "*".into(), PermissionAction::Allow);
        insert_rule(&conn, &r1).unwrap();
        insert_rule(&conn, &r2).unwrap();

        let deleted = delete_session_rules(&conn, "sess1").unwrap();
        assert_eq!(deleted, 1);

        let all = list_rules(&conn, &PermissionRuleFilter::default()).unwrap();
        assert_eq!(all.len(), 1);
    }
}
```

**验证**:
```bash
cargo test permission_repo -- --nocapture
```

---

### T2.06:实现 PermissionRegistry 权限注册表

**文件**:
- 新增: `src-tauri/src/services/permission/registry.rs`

**说明**:
PermissionRegistry 是权限系统的核心组件,负责:
1. 加载默认规则(内置)
2. 加载数据库中的用户规则(全局/项目/会话)
3. 合并多层规则(默认 → 全局 → 项目 → 会话)
4. 提供规则查询接口

**实施步骤**:

文件: `src-tauri/src/services/permission/registry.rs`

```rust
use std::sync::Arc;

use crate::db::Database;
use crate::db::permission_repo;
use crate::models::permission::{PermissionRule, PermissionRuleFilter};

use super::{PermissionAction, PermissionType, RuleScope, WildcardMatcher};

/// 权限注册表
/// 负责加载、合并、缓存权限规则
/// 规则优先级:会话级 > 项目级 > 全局级 > 默认级
pub struct PermissionRegistry {
    db: Arc<Database>,
    /// 默认规则(内置,不可修改)
    defaults: Vec<PermissionRule>,
}

impl PermissionRegistry {
    /// 创建权限注册表并加载默认规则
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            defaults: Self::builtin_defaults(),
        }
    }

    /// 内置默认权限规则(参照 OpenCode 默认配置)
    /// 用户未配置任何规则时使用
    fn builtin_defaults() -> Vec<PermissionRule> {
        vec![
            // 通配:默认允许
            PermissionRule::new(RuleScope::Global, PermissionType::Wildcard, "*".into(), PermissionAction::Allow),
            // 读取:默认允许,但保护 .env 文件
            PermissionRule::new(RuleScope::Global, PermissionType::Read, "*".into(), PermissionAction::Allow),
            PermissionRule::new(RuleScope::Global, PermissionType::Read, "*.env".into(), PermissionAction::Deny)
                .with_description("保护 .env 隐私配置文件"),
            PermissionRule::new(RuleScope::Global, PermissionType::Read, "*.env.*".into(), PermissionAction::Deny),
            PermissionRule::new(RuleScope::Global, PermissionType::Read, "*.env.example".into(), PermissionAction::Allow),
            // 编辑:默认允许
            PermissionRule::new(RuleScope::Global, PermissionType::Edit, "*".into(), PermissionAction::Allow),
            // 搜索类:默认允许
            PermissionRule::new(RuleScope::Global, PermissionType::Glob, "*".into(), PermissionAction::Allow),
            PermissionRule::new(RuleScope::Global, PermissionType::Grep, "*".into(), PermissionAction::Allow),
            PermissionRule::new(RuleScope::Global, PermissionType::List, "*".into(), PermissionAction::Allow),
            // 命令执行:默认允许
            PermissionRule::new(RuleScope::Global, PermissionType::Bash, "*".into(), PermissionAction::Allow),
            // 脚本执行:默认允许
            PermissionRule::new(RuleScope::Global, PermissionType::WriteScript, "*".into(), PermissionAction::Allow),
            // 安全防护类:默认询问
            PermissionRule::new(RuleScope::Global, PermissionType::ExternalDirectory, "*".into(), PermissionAction::Ask)
                .with_description("访问工作区外部目录需确认"),
            PermissionRule::new(RuleScope::Global, PermissionType::DoomLoop, "*".into(), PermissionAction::Ask)
                .with_description("连续 3 次相同调用触发死循环检测"),
            // v1.1: 文档处理 Handler:默认允许(Document 模式下可见,非 Document 模式被工具列表过滤)
            PermissionRule::new(RuleScope::Global, PermissionType::Document, "*".into(), PermissionAction::Allow)
                .with_description("文档 Handler(docx/xlsx/pptx/pdf)默认允许,仅 Document 模式下可见"),
            // 网络类:默认允许(阶段4实现)
            PermissionRule::new(RuleScope::Global, PermissionType::WebFetch, "*".into(), PermissionAction::Allow),
            PermissionRule::new(RuleScope::Global, PermissionType::WebSearch, "*".into(), PermissionAction::Allow),
            // 子 Agent:默认允许(阶段4实现)
            PermissionRule::new(RuleScope::Global, PermissionType::Task, "*".into(), PermissionAction::Allow),
            // Skill:默认允许(阶段3实现)
            PermissionRule::new(RuleScope::Global, PermissionType::Skill, "*".into(), PermissionAction::Allow),
            // LSP:默认允许(阶段5实现)
            PermissionRule::new(RuleScope::Global, PermissionType::Lsp, "*".into(), PermissionAction::Allow),
        ]
    }

    /// 获取所有生效规则(默认 + 数据库)
    /// 按 workspace_id 和 session_id 过滤
    pub fn load_effective_rules(
        &self,
        workspace_id: Option<&str>,
        session_id: Option<&str>,
    ) -> Vec<PermissionRule> {
        let mut rules = self.defaults.clone();

        // 加载数据库中的用户规则
        if let Ok(conn) = self.db.conn() {
            let filter = PermissionRuleFilter {
                workspace_id: workspace_id.map(String::from),
                session_id: session_id.map(String::from),
                enabled_only: true,
                ..Default::default()
            };
            if let Ok(db_rules) = permission_repo::list_rules(&conn, &filter) {
                rules.extend(db_rules);
            }
        }

        rules
    }

    /// 添加用户规则到数据库
    pub fn add_rule(&self, rule: PermissionRule) -> Result<(), crate::errors::CommandError> {
        let conn = self.db.conn()?;
        permission_repo::insert_rule(&conn, &rule)
    }

    /// 更新用户规则
    pub fn update_rule(&self, rule: PermissionRule) -> Result<(), crate::errors::CommandError> {
        let conn = self.db.conn()?;
        permission_repo::update_rule(&conn, &rule)
    }

    /// 删除用户规则
    pub fn delete_rule(&self, rule_id: &str) -> Result<(), crate::errors::CommandError> {
        let conn = self.db.conn()?;
        permission_repo::delete_rule(&conn, rule_id)
    }

    /// 列出用户规则(不含默认规则)
    pub fn list_user_rules(
        &self,
        filter: &PermissionRuleFilter,
    ) -> Result<Vec<PermissionRule>, crate::errors::CommandError> {
        let conn = self.db.conn()?;
        permission_repo::list_rules(&conn, filter)
    }

    /// 清理会话临时规则(会话结束时调用)
    pub fn cleanup_session_rules(&self, session_id: &str) -> Result<u64, crate::errors::CommandError> {
        let conn = self.db.conn()?;
        permission_repo::delete_session_rules(&conn, session_id)
    }

    /// 获取默认规则(只读,用于前端展示)
    pub fn default_rules(&self) -> &[PermissionRule] {
        &self.defaults
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_registry() -> PermissionRegistry {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let db = Arc::new(Database::new(std::path::Path::new(tmp.path())).unwrap());
        PermissionRegistry::new(db)
    }

    #[test]
    fn test_builtin_defaults_loaded() {
        let registry = setup_registry();
        let defaults = registry.default_rules();
        // 验证默认规则包含 .env 保护
        assert!(defaults.iter().any(|r| r.pattern == "*.env" && r.action == PermissionAction::Deny));
    }

    #[test]
    fn test_add_and_load_user_rule() {
        let registry = setup_registry();
        let rule = PermissionRule::new(
            RuleScope::Global,
            PermissionType::Bash,
            "rm *".into(),
            PermissionAction::Deny,
        );
        registry.add_rule(rule.clone()).unwrap();

        let effective = registry.load_effective_rules(None, None);
        // 默认规则 + 1 条用户规则
        assert!(effective.len() > registry.default_rules().len());
        assert!(effective.iter().any(|r| r.pattern == "rm *" && r.action == PermissionAction::Deny));
    }
}
```

**验证**:
```bash
cargo test permission_registry -- --nocapture
```

---

### T2.07:实现 PermissionEvaluator 权限评估器

**文件**:
- 新增: `src-tauri/src/services/permission/evaluator.rs`

**说明**:
PermissionEvaluator 负责评估单次工具调用的权限,返回 allow/deny/ask 决策。评估流程参照 OpenCode:
1. 加载生效规则
2. 过滤匹配规则(权限类型匹配 + 模式匹配)
3. 按特异性排序(具体路径优先于通配符)
4. 返回最具体的匹配规则的动作

**实施步骤**:

文件: `src-tauri/src/services/permission/evaluator.rs`

```rust
use crate::models::permission::PermissionRule;

use super::{PermissionAction, PermissionType, RuleScope, WildcardMatcher, normalize_path_for_match};

/// 权限评估请求
#[derive(Debug, Clone)]
pub struct PermissionRequest {
    /// 权限类型(根据工具名推断)
    pub permission_type: PermissionType,
    /// 匹配目标
    /// - 对于文件类工具:文件路径
    /// - 对于 bash 工具:命令字符串
    /// - 对于 task 工具:子 Agent 名称
    /// - 对于 webfetch 工具:URL
    pub target: String,
}

impl PermissionRequest {
    /// 从工具调用构建权限请求
    pub fn from_tool_call(tool_name: &str, params: &serde_json::Value) -> Self {
        let permission_type = PermissionType::from_tool_name(tool_name);
        let target = extract_target(tool_name, params);
        Self {
            permission_type,
            target,
        }
    }
}

/// 从工具参数中提取匹配目标
fn extract_target(tool_name: &str, params: &serde_json::Value) -> String {
    match tool_name {
        // 文件路径类工具:提取 path 或 file_path 参数
        "read" | "read_lines" | "edit" | "write" | "remove"
        | "rename" | "copy" | "file_info" | "exists" | "hash" => {
            params.get("path")
                .or_else(|| params.get("file_path"))
                .and_then(|v| v.as_str())
                .map(|s| normalize_path_for_match(s))
                .unwrap_or_else(|| "*".to_string())
        }
        // 命令执行:提取 command 参数
        "bash" | "write_script" => {
            params.get("command")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "*".to_string())
        }
        // 目录操作:提取 path 参数
        "list" | "mkdir" | "remove_dir" => {
            params.get("path")
                .and_then(|v| v.as_str())
                .map(|s| normalize_path_for_match(s))
                .unwrap_or_else(|| "*".to_string())
        }
        // 搜索类:提取 pattern 或 query 参数
        "glob" => {
            params.get("pattern")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "*".to_string())
        }
        "grep" | "search" => {
            params.get("pattern")
                .or_else(|| params.get("query"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "*".to_string())
        }
        // 网页类:提取 url 参数
        "webfetch" => {
            params.get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "*".to_string())
        }
        // 子 Agent:提取 agent 参数
        "task" => {
            params.get("agent")
                .or_else(|| params.get("agent_type"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "*".to_string())
        }
        // v1.1: 文档 Handler:提取 input_path 或 path 参数
        "docx" | "xlsx" | "pptx" | "pdf" => {
            params.get("input_path")
                .or_else(|| params.get("path"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "*".to_string())
        }
        // 默认:通配
        _ => "*".to_string(),
    }
}

/// 权限评估结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDecision {
    /// 最终动作:allow / deny / ask
    pub action: PermissionAction,
    /// 命中的规则 ID(用于日志和调试)
    pub matched_rule_id: Option<String>,
    /// 命中的规则描述
    pub matched_description: String,
    /// 评估的请求
    pub request: PermissionRequest,
}

/// 权限评估器
/// 根据生效规则评估单次工具调用的权限
pub struct PermissionEvaluator;

impl PermissionEvaluator {
    /// 评估权限请求
    /// 规则优先级:最后匹配优先(自上而下解析,最后命中的规则生效)
    /// 同时支持特异性优先:具体路径优先于通配符
    pub fn evaluate(request: &PermissionRequest, rules: &[PermissionRule]) -> PermissionDecision {
        // 收集所有匹配的规则
        let matching_rules: Vec<&PermissionRule> = rules.iter()
            .filter(|r| r.enabled)
            .filter(|r| Self::rule_matches(r, request))
            .collect();

        if matching_rules.is_empty() {
            // 无匹配规则,默认允许
            return PermissionDecision {
                action: PermissionAction::Allow,
                matched_rule_id: None,
                matched_description: "无匹配规则,默认允许".to_string(),
                request: request.clone(),
            };
        }

        // 按特异性排序:最具体的规则优先
        // OpenCode 使用"最后匹配优先",但实践中"最具体优先"更安全
        // 这里采用混合策略:先按特异性降序,特异性相同则按定义顺序(后者覆盖前者)
        let mut sorted_rules: Vec<&PermissionRule> = matching_rules.clone();
        sorted_rules.sort_by(|a, b| {
            let sa = WildcardMatcher::new(&a.pattern).specificity();
            let sb = WildcardMatcher::new(&b.pattern).specificity();
            sb.cmp(&sa) // 降序:具体的在前
        });

        // 返回最具体的规则动作
        let matched = sorted_rules[0];
        PermissionDecision {
            action: matched.action,
            matched_rule_id: Some(matched.id.clone()),
            matched_description: matched.description.clone(),
            request: request.clone(),
        }
    }

    /// 检查规则是否匹配请求
    fn rule_matches(rule: &PermissionRule, request: &PermissionRequest) -> bool {
        // 1. 权限类型匹配:Wildcard 匹配所有,否则需要精确匹配
        let type_matches = rule.permission_type == PermissionType::Wildcard
            || rule.permission_type == request.permission_type;
        if !type_matches {
            return false;
        }

        // 2. 模式匹配
        let matcher = WildcardMatcher::new(&rule.pattern);
        matcher.matches(&request.target)
    }

    /// 检查是否为外部目录访问
    /// 路径不在工作区内时触发 ExternalDirectory 权限
    pub fn is_external_directory(path: &str, workspace_root: &str) -> bool {
        let normalized_path = normalize_path_for_match(path);
        let normalized_workspace = normalize_path_for_match(workspace_root);
        !normalized_path.starts_with(&normalized_workspace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::permission::RuleScope;

    fn make_rule(scope: RuleScope, ptype: PermissionType, pattern: &str, action: PermissionAction) -> PermissionRule {
        PermissionRule::new(scope, ptype, pattern.to_string(), action)
    }

    #[test]
    fn test_evaluate_allow_default() {
        let rules = vec![make_rule(RuleScope::Global, PermissionType::Wildcard, "*", PermissionAction::Allow)];
        let req = PermissionRequest {
            permission_type: PermissionType::Read,
            target: "src/main.rs".to_string(),
        };
        let decision = PermissionEvaluator::evaluate(&req, &rules);
        assert_eq!(decision.action, PermissionAction::Allow);
    }

    #[test]
    fn test_evaluate_deny_env_file() {
        let rules = vec![
            make_rule(RuleScope::Global, PermissionType::Read, "*", PermissionAction::Allow),
            make_rule(RuleScope::Global, PermissionType::Read, "*.env", PermissionAction::Deny),
        ];
        let req = PermissionRequest {
            permission_type: PermissionType::Read,
            target: ".env".to_string(),
        };
        let decision = PermissionEvaluator::evaluate(&req, &rules);
        assert_eq!(decision.action, PermissionAction::Deny);
    }

    #[test]
    fn test_evaluate_specific_overrides_wildcard() {
        let rules = vec![
            make_rule(RuleScope::Global, PermissionType::Edit, "*", PermissionAction::Allow),
            make_rule(RuleScope::Global, PermissionType::Edit, "src/secret/*", PermissionAction::Ask),
        ];
        // 通配路径 → allow
        let req1 = PermissionRequest {
            permission_type: PermissionType::Edit,
            target: "docs/readme.md".to_string(),
        };
        assert_eq!(PermissionEvaluator::evaluate(&req1, &rules).action, PermissionAction::Allow);

        // 具体路径 → ask
        let req2 = PermissionRequest {
            permission_type: PermissionType::Edit,
            target: "src/secret/key.pem".to_string(),
        };
        assert_eq!(PermissionEvaluator::evaluate(&req2, &rules).action, PermissionAction::Ask);
    }

    #[test]
    fn test_evaluate_bash_command_pattern() {
        let rules = vec![
            make_rule(RuleScope::Global, PermissionType::Bash, "*", PermissionAction::Allow),
            make_rule(RuleScope::Global, PermissionType::Bash, "rm *", PermissionAction::Deny),
            make_rule(RuleScope::Global, PermissionType::Bash, "git push *", PermissionAction::Ask),
        ];
        // 普通命令 → allow
        let req1 = PermissionRequest {
            permission_type: PermissionType::Bash,
            target: "ls -la".to_string(),
        };
        assert_eq!(PermissionEvaluator::evaluate(&req1, &rules).action, PermissionAction::Allow);

        // 删除命令 → deny
        let req2 = PermissionRequest {
            permission_type: PermissionType::Bash,
            target: "rm -rf /tmp/test".to_string(),
        };
        assert_eq!(PermissionEvaluator::evaluate(&req2, &rules).action, PermissionAction::Deny);

        // 推送命令 → ask
        let req3 = PermissionRequest {
            permission_type: PermissionType::Bash,
            target: "git push origin main".to_string(),
        };
        assert_eq!(PermissionEvaluator::evaluate(&req3, &rules).action, PermissionAction::Ask);
    }

    #[test]
    fn test_is_external_directory() {
        assert!(PermissionEvaluator::is_external_directory("/tmp/other", "/home/user/project"));
        assert!(!PermissionEvaluator::is_external_directory("/home/user/project/src/main.rs", "/home/user/project"));
    }
}
```

**验证**:
```bash
cargo test permission_evaluator -- --nocapture
```

---

### T2.08:实现临时白名单(会话级 always 规则)

**文件**:
- 新增: `src-tauri/src/services/permission/session_whitelist.rs`

**说明**:
当用户选择 `always` 时,生成一条会话级临时规则并加入白名单。会话结束时清理。
白名单为内存缓存,避免每次查询数据库。

**实施步骤**:

文件: `src-tauri/src/services/permission/session_whitelist.rs`

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::permission::PermissionRule;

use super::{PermissionAction, PermissionType, RuleScope};

/// 会话级临时白名单
/// 当用户选择 "always" 时,生成一条临时规则并缓存
/// 会话结束时通过 cleanup_session 清理
#[derive(Debug, Clone)]
pub struct SessionWhitelist {
    /// 按 session_id 隔离的临时规则列表
    sessions: Arc<RwLock<HashMap<String, Vec<PermissionRule>>>>,
}

impl SessionWhitelist {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 添加一条会话级临时规则
    /// 当用户选择 always 时调用
    pub async fn add_rule(&self, session_id: &str, rule: PermissionRule) {
        let mut sessions = self.sessions.write().await;
        sessions.entry(session_id.to_string())
            .or_insert_with(Vec::new)
            .push(rule);
    }

    /// 获取指定会话的所有临时规则
    pub async fn get_rules(&self, session_id: &str) -> Vec<PermissionRule> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned().unwrap_or_default()
    }

    /// 清理指定会话的所有临时规则
    pub async fn cleanup_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        if let Some(rules) = sessions.remove(session_id) {
            log::info!("已清理会话 {} 的 {} 条临时权限规则", session_id, rules.len());
        }
    }

    /// 根据 always 响应生成临时规则
    /// 自动推断通配符模式:如果具体路径,使用路径;如果命令,提取命令前缀 + 通配符
    pub fn generate_always_rule(
        session_id: &str,
        permission_type: PermissionType,
        target: &str,
    ) -> PermissionRule {
        let pattern = Self::infer_pattern(permission_type, target);
        PermissionRule::new(
            RuleScope::Session,
            permission_type,
            pattern,
            PermissionAction::Allow,
        )
        .with_session(session_id.to_string())
        .with_description("用户选择 always 自动生成")
    }

    /// 根据目标和权限类型推断通配符模式
    /// - 文件路径:使用具体路径(只放行该文件)
    /// - 命令:提取命令前缀 + *(如 "git status *" 放行所有 git status 子命令)
    /// - 其他:使用具体值
    fn infer_pattern(permission_type: PermissionType, target: &str) -> String {
        match permission_type {
            PermissionType::Bash | PermissionType::WriteScript => {
                // 命令:提取前两个 token + 通配符
                // 例如 "git push origin main" → "git push *"
                let tokens: Vec<&str> = target.split_whitespace().take(2).collect();
                if tokens.is_empty() {
                    "*".to_string()
                } else if tokens.len() == 1 {
                    format!("{} *", tokens[0])
                } else {
                    format!("{} {} *", tokens[0], tokens[1])
                }
            }
            _ => {
                // 文件路径或其他:使用具体值
                target.to_string()
            }
        }
    }

    /// 检查指定会话是否有匹配的临时规则
    pub async fn check(&self, session_id: &str, permission_type: PermissionType, target: &str) -> Option<PermissionAction> {
        let sessions = self.sessions.read().await;
        let rules = sessions.get(session_id)?;
        for rule in rules {
            if !rule.enabled {
                continue;
            }
            if rule.permission_type != permission_type && rule.permission_type != PermissionType::Wildcard {
                continue;
            }
            let matcher = super::WildcardMatcher::new(&rule.pattern);
            if matcher.matches(target) {
                return Some(rule.action);
            }
        }
        None
    }
}

impl Default for SessionWhitelist {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_pattern_command() {
        let p = SessionWhitelist::infer_pattern(PermissionType::Bash, "git push origin main");
        assert_eq!(p, "git push *");

        let p = SessionWhitelist::infer_pattern(PermissionType::Bash, "ls");
        assert_eq!(p, "ls *");
    }

    #[test]
    fn test_infer_pattern_file() {
        let p = SessionWhitelist::infer_pattern(PermissionType::Edit, "src/main.rs");
        assert_eq!(p, "src/main.rs");
    }

    #[tokio::test]
    async fn test_add_and_check_rule() {
        let whitelist = SessionWhitelist::new();
        let rule = SessionWhitelist::generate_always_rule("sess1", PermissionType::Bash, "git status");
        whitelist.add_rule("sess1", rule).await;

        // 相同前缀的命令应命中白名单
        let action = whitelist.check("sess1", PermissionType::Bash, "git status --short").await;
        assert_eq!(action, Some(PermissionAction::Allow));

        // 不同会话不应命中
        let action = whitelist.check("sess2", PermissionType::Bash, "git status").await;
        assert_eq!(action, None);
    }

    #[tokio::test]
    async fn test_cleanup_session() {
        let whitelist = SessionWhitelist::new();
        let rule = SessionWhitelist::generate_always_rule("sess1", PermissionType::Bash, "npm install");
        whitelist.add_rule("sess1", rule).await;

        whitelist.cleanup_session("sess1").await;
        let action = whitelist.check("sess1", PermissionType::Bash, "npm install").await;
        assert_eq!(action, None);
    }
}
```

**验证**:
```bash
cargo test session_whitelist -- --nocapture
```

---

### T2.09:实现 Doom loop 检测器

**文件**:
- 新增: `src-tauri/src/services/permission/doom_loop.rs`

**说明**:
检测连续 3 次相同工具调用(相同参数),触发 Doom loop 权限规则。
参照 OpenCode 的实现:维护每个会话的工具调用历史,检测重复模式。

**实施步骤**:

文件: `src-tauri/src/services/permission/doom_loop.rs`

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Doom loop 检测阈值:连续相同调用的次数
const DOOM_LOOP_THRESHOLD: usize = 3;

/// 工具调用记录(用于去重比较)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ToolCallRecord {
    /// 工具名
    tool_name: String,
    /// 参数的规范化 JSON 字符串(用于比较)
    params_key: String,
}

impl ToolCallRecord {
    fn new(tool_name: &str, params: &serde_json::Value) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            params_key: Self::normalize_params(params),
        }
    }

    /// 规范化参数用于比较
    /// 移除顺序差异:对 JSON 对象的 key 排序
    fn normalize_params(params: &serde_json::Value) -> String {
        // 简单实现:序列化为字符串
        // 进阶:可以对 object 的 key 排序后序列化
        match params {
            serde_json::Value::Object(map) => {
                let mut sorted_map: Vec<(&String, &serde_json::Value)> = map.iter().collect();
                sorted_map.sort_by(|a, b| a.0.cmp(b.0));
                serde_json::to_string(&sorted_map).unwrap_or_default()
            }
            _ => params.to_string(),
        }
    }
}

/// Doom loop 检测器
/// 按 session_id 隔离,记录最近的工具调用历史
pub struct DoomLoopDetector {
    /// 按 session_id 隔离的调用历史
    sessions: Arc<RwLock<HashMap<String, Vec<ToolCallRecord>>>>,
}

impl DoomLoopDetector {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 记录一次工具调用并检测是否触发 Doom loop
    /// 返回 true 表示触发了 Doom loop(连续相同调用达到阈值)
    pub async fn record_and_check(
        &self,
        session_id: &str,
        tool_name: &str,
        params: &serde_json::Value,
    ) -> bool {
        let record = ToolCallRecord::new(tool_name, params);
        let mut sessions = self.sessions.write().await;
        let history = sessions.entry(session_id.to_string())
            .or_insert_with(Vec::new);

        // 检查最近的 N-1 次调用是否与当前调用相同
        let trigger = if history.len() >= DOOM_LOOP_THRESHOLD - 1 {
            let recent = &history[history.len() - (DOOM_LOOP_THRESHOLD - 1)..];
            recent.iter().all(|r| r == &record)
        } else {
            false
        };

        // 记录本次调用
        history.push(record);

        // 限制历史长度,避免内存无限增长
        if history.len() > 100 {
            let drain_count = history.len() - 100;
            history.drain(0..drain_count);
        }

        if trigger {
            log::warn!("检测到 Doom loop: session_id={}, tool={}, 连续 {} 次相同调用",
                session_id, tool_name, DOOM_LOOP_THRESHOLD);
        }

        trigger
    }

    /// 清理指定会话的调用历史
    pub async fn cleanup_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        if let Some(history) = sessions.remove(session_id) {
            log::debug!("已清理会话 {} 的 Doom loop 检测历史({} 条记录)", session_id, history.len());
        }
    }

    /// 获取指定会话的最近 N 次调用记录(用于调试)
    pub async fn recent_calls(&self, session_id: &str, n: usize) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id)
            .map(|h| {
                h.iter().rev().take(n)
                    .map(|r| format!("{}({})", r.tool_name, r.params_key))
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for DoomLoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_no_trigger_on_different_calls() {
        let detector = DoomLoopDetector::new();
        let params1 = json!({"path": "file1.rs"});
        let params2 = json!({"path": "file2.rs"});

        // 不同参数的调用不应触发
        assert!(!detector.record_and_check("s1", "read", &params1).await);
        assert!(!detector.record_and_check("s1", "read", &params2).await);
        assert!(!detector.record_and_check("s1", "read", &params1).await);
    }

    #[tokio::test]
    async fn test_trigger_on_same_calls() {
        let detector = DoomLoopDetector::new();
        let params = json!({"path": "file.rs", "content": "test"});

        // 第 1 次:不触发
        assert!(!detector.record_and_check("s1", "edit", &params).await);
        // 第 2 次:不触发
        assert!(!detector.record_and_check("s1", "edit", &params).await);
        // 第 3 次:触发
        assert!(detector.record_and_check("s1", "edit", &params).await);
    }

    #[tokio::test]
    async fn test_isolated_sessions() {
        let detector = DoomLoopDetector::new();
        let params = json!({"command": "ls"});

        // 会话 1:2 次
        assert!(!detector.record_and_check("s1", "bash", &params).await);
        assert!(!detector.record_and_check("s1", "bash", &params).await);

        // 会话 2:2 次(不应触发,因为是新会话)
        assert!(!detector.record_and_check("s2", "bash", &params).await);
        assert!(!detector.record_and_check("s2", "bash", &params).await);

        // 会话 1:第 3 次(触发)
        assert!(detector.record_and_check("s1", "bash", &params).await);
    }

    #[tokio::test]
    async fn test_cleanup() {
        let detector = DoomLoopDetector::new();
        let params = json!({"path": "f.rs"});
        detector.record_and_check("s1", "read", &params).await;
        detector.record_and_check("s1", "read", &params).await;

        detector.cleanup_session("s1").await;

        // 清理后重新开始计数
        assert!(!detector.record_and_check("s1", "read", &params).await);
    }
}
```

**验证**:
```bash
cargo test doom_loop -- --nocapture
```

---

### T2.10:改造 AgentExecutor 集成权限系统

**文件**:
- 修改: `src-tauri/src/services/agent/executor.rs`

**说明**:
改造 AgentExecutor,在工具执行前进行权限检查。流程:
1. 工具执行前构建 PermissionRequest
2. 优先检查会话白名单(always 规则)
3. 检查 Doom loop(触发则强制 Ask)
4. 评估权限规则(默认 → 全局 → 项目 → 会话)
5. 根据 action 决策:Allow 直接执行,Deny 返回错误,Ask 弹窗等待用户回复
6. 用户回复后:Once 执行,Always 加入白名单后执行,Reject 返回错误

**实施步骤**:

#### 1. 修改 ConfirmDecision 结构

文件: `src-tauri/src/lib.rs`

在现有 `ConfirmDecision` 旁新增 `PermissionDecision` 结构(保留旧结构以兼容):

```rust
/// 用户权限审批决策(三选项:once/always/reject)
#[derive(Debug, Clone)]
pub struct PermissionDecision {
    /// 审批响应类型
    pub response: crate::services::permission::PermissionResponse,
    /// 用户反馈(可选,Reject 时可附加说明)
    pub feedback: Option<String>,
}
```

#### 2. 改造 AgentExecutor

文件: `src-tauri/src/services/agent/executor.rs`

在 `AgentExecutor` 结构中新增权限相关字段:

```rust
use crate::services::permission::{
    PermissionRegistry, PermissionEvaluator, PermissionRequest, PermissionDecision as PermDecision,
    SessionWhitelist, DoomLoopDetector, PermissionAction, PermissionType, RuleScope,
    PermissionResponse,
};

pub struct AgentExecutor<R: Runtime> {
    router: Arc<LlmRouter>,
    tool_registry: Arc<ToolRegistry>,
    // [移除] 阶段1已移除 registry 字段
    emitter: AgentEmitter<R>,
    confirm_channels: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<crate::ConfirmDecision>>>>,
    // [新增] 权限审批通道(三选项)
    permission_channels: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<crate::PermissionDecision>>>>,
    max_iterations: u32,
    should_stop: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    persist_fn: Option<PersistFn>,
    context_usage_persist_fn: Option<ContextUsagePersistFn>,
    snapshot_fn: Option<SnapshotFn>,
    // [新增] 权限注册表
    permission_registry: Arc<PermissionRegistry>,
    // [新增] 会话白名单
    session_whitelist: Arc<SessionWhitelist>,
    // [新增] Doom loop 检测器
    doom_loop_detector: Arc<DoomLoopDetector>,
    // [新增] Agent 模式(Plan/Build)
    agent_mode: Arc<tokio::sync::RwLock<AgentMode>>,
    // [移除] confirmation_level 字段(被权限系统替代)
}
```

新增 `AgentMode` 枚举(在 `src-tauri/src/services/agent/mod.rs` 中):

```rust
/// Agent 执行模式
/// Plan:只读规划模式,禁止修改类操作
/// Build:完整执行模式,允许所有编程操作(受权限规则约束)
/// Document:Build 超集 + 4 个文档 Handler 动态加入工具列表
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    /// Build 模式(默认):完整编程执行能力,文档 Handler 不出现在工具列表
    #[default]
    Build,
    /// Plan 模式:只读规划,禁止 edit/bash 等修改类操作
    Plan,
    /// Document 模式:Build 超集,4 个文档 Handler(docx/xlsx/pptx/pdf)动态加入工具列表
    Document,
}

impl AgentMode {
    pub fn is_plan(&self) -> bool {
        matches!(self, AgentMode::Plan)
    }

    pub fn is_build(&self) -> bool {
        matches!(self, AgentMode::Build)
    }

    pub fn is_document(&self) -> bool {
        matches!(self, AgentMode::Document)
    }

    /// 判断当前模式是否应包含文档 Handler(仅 Document 模式返回 true)
    pub fn includes_document_handlers(&self) -> bool {
        matches!(self, AgentMode::Document)
    }
}
```

#### 3. 新增权限检查方法

在 `AgentExecutor` 的 `impl` 块中新增:

```rust
    /// 工具执行前的权限检查
    /// 返回 Ok(()) 表示允许执行,Err 表示拒绝
    async fn check_permission(
        &self,
        session_id: &str,
        tool_name: &str,
        params: &serde_json::Value,
        workspace_root: &str,
    ) -> Result<(), CommandError> {
        // 0. Plan 模式下,修改类工具直接拒绝
        {
            let mode = self.agent_mode.read().await;
            if mode.is_plan() {
                let ptype = PermissionType::from_tool_name(tool_name);
                if ptype.is_modification() {
                    return Err(CommandError::agent(
                        crate::errors::AGENT_OPERATION_REJECTED,
                        format!("Plan 模式下禁止执行修改类操作: {}。请通过前端按钮切换到 Build 或 Document 模式", tool_name),
                    ));
                }
            }
        }

        // 1. 检查外部目录访问
        let ptype = PermissionType::from_tool_name(tool_name);
        if matches!(ptype, PermissionType::Read | PermissionType::Edit | PermissionType::List) {
            if let Some(path) = params.get("path").or_else(|| params.get("file_path")).and_then(|v| v.as_str()) {
                let abs_path = if std::path::Path::new(path).is_absolute() {
                    path.to_string()
                } else {
                    format!("{}/{}", workspace_root, path)
                };
                if PermissionEvaluator::is_external_directory(&abs_path, workspace_root) {
                    // 外部目录访问:检查权限
                    let ext_req = PermissionRequest {
                        permission_type: PermissionType::ExternalDirectory,
                        target: abs_path.clone(),
                    };
                    let rules = self.permission_registry.load_effective_rules(None, Some(session_id));
                    let decision = PermissionEvaluator::evaluate(&ext_req, &rules);
                    match decision.action {
                        PermissionAction::Allow => {}
                        PermissionAction::Deny => {
                            return Err(CommandError::agent(
                                crate::errors::AGENT_OPERATION_REJECTED,
                                format!("访问外部目录被拒绝: {}", abs_path),
                            ));
                        }
                        PermissionAction::Ask => {
                            self.request_permission(
                                session_id,
                                "external_directory",
                                &serde_json::json!({"path": abs_path}),
                                "访问工作区外部目录",
                            ).await?;
                        }
                    }
                }
            }
        }

        // 2. 检查 Doom loop
        let doom_loop_triggered = self.doom_loop_detector
            .record_and_check(session_id, tool_name, params)
            .await;
        if doom_loop_triggered {
            let doom_req = PermissionRequest {
                permission_type: PermissionType::DoomLoop,
                target: format!("{}({})", tool_name, params),
            };
            // Doom loop 默认为 Ask,弹窗询问用户
            self.request_permission(
                session_id,
                "doom_loop",
                &serde_json::json!({
                    "tool": tool_name,
                    "params": params,
                    "message": "检测到连续 3 次相同工具调用,可能存在死循环"
                }),
                "Doom loop 检测:连续相同调用",
            ).await?;
        }

        // 3. 检查会话白名单(always 规则)
        let request = PermissionRequest::from_tool_call(tool_name, params);
        if let Some(action) = self.session_whitelist
            .check(session_id, request.permission_type, &request.target)
            .await
        {
            match action {
                PermissionAction::Allow => return Ok(()),
                PermissionAction::Deny => {
                    return Err(CommandError::agent(
                        crate::errors::AGENT_OPERATION_REJECTED,
                        format!("操作被会话白名单拒绝: {}", tool_name),
                    ));
                }
                PermissionAction::Ask => {
                    // 继续到下一步弹窗
                }
            }
        }

        // 4. 评估权限规则
        let rules = self.permission_registry.load_effective_rules(None, Some(session_id));
        let decision = PermissionEvaluator::evaluate(&request, &rules);

        match decision.action {
            PermissionAction::Allow => Ok(()),
            PermissionAction::Deny => {
                Err(CommandError::agent(
                    crate::errors::AGENT_OPERATION_REJECTED,
                    format!("操作被权限规则拒绝: {}。规则: {}",
                        tool_name, decision.matched_description),
                ))
            }
            PermissionAction::Ask => {
                // 弹窗请求用户审批
                let response = self.request_permission_with_response(
                    session_id,
                    tool_name,
                    params,
                    &format!("权限审批:{}", tool_name),
                ).await?;

                match response {
                    PermissionResponse::Once => Ok(()),
                    PermissionResponse::Always => {
                        // 生成临时规则加入白名单
                        let rule = SessionWhitelist::generate_always_rule(
                            session_id,
                            request.permission_type,
                            &request.target,
                        );
                        self.session_whitelist.add_rule(session_id, rule).await;
                        Ok(())
                    }
                    PermissionResponse::Reject => {
                        Err(CommandError::agent(
                            crate::errors::AGENT_OPERATION_REJECTED,
                            "用户拒绝操作".to_string(),
                        ))
                    }
                }
            }
        }
    }

    /// 请求用户权限审批(简单版,仅 approve/reject)
    async fn request_permission(
        &self,
        session_id: &str,
        tool_name: &str,
        params: &serde_json::Value,
        description: &str,
    ) -> Result<(), CommandError> {
        let response = self.request_permission_with_response(
            session_id, tool_name, params, description,
        ).await?;
        match response {
            PermissionResponse::Once | PermissionResponse::Always => Ok(()),
            PermissionResponse::Reject => Err(CommandError::agent(
                crate::errors::AGENT_OPERATION_REJECTED,
                "用户拒绝操作".to_string(),
            )),
        }
    }

    /// 请求用户权限审批(完整版,返回三选项响应)
    async fn request_permission_with_response(
        &self,
        session_id: &str,
        tool_name: &str,
        params: &serde_json::Value,
        description: &str,
    ) -> Result<PermissionResponse, CommandError> {
        let operation_id = format!("perm_{}", uuid::Uuid::new_v4());

        let risk_level = Self::assess_risk_level(tool_name, params);

        let desc = Self::format_permission_description(tool_name, params, description);

        // 创建审批通道
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut channels = self.permission_channels.lock().await;
            channels.insert(operation_id.clone(), tx);
        }

        // 发射权限审批事件
        if self.emitter.emit_confirm(ConfirmPayload {
            session_id: session_id.to_string(),
            operation_id: operation_id.clone(),
            operation_type: tool_name.to_string(),
            description: desc,
            details: params.clone(),
            risk_level: risk_level.to_string(),
        }).is_err() {
            let mut channels = self.permission_channels.lock().await;
            channels.remove(&operation_id);
            return Err(CommandError::new(
                crate::errors::RUNTIME_EVENT_EMIT_ERROR,
                "发射权限审批事件失败",
            ));
        }

        // 等待用户响应(5 分钟超时)
        match tokio::time::timeout(Duration::from_secs(CONFIRM_TIMEOUT_SECS), rx).await {
            Ok(Ok(decision)) => {
                let mut channels = self.permission_channels.lock().await;
                channels.remove(&operation_id);
                log::info!("权限审批结果: operation_id={}, response={:?}", operation_id, decision.response);
                Ok(decision.response)
            }
            Ok(Err(_)) => {
                let mut channels = self.permission_channels.lock().await;
                channels.remove(&operation_id);
                log::warn!("权限审批通道关闭: operation_id={}", operation_id);
                Err(CommandError::agent(
                    crate::errors::AGENT_CONFIRMATION_TIMEOUT,
                    "权限审批通道关闭".to_string(),
                ))
            }
            Err(_) => {
                let mut channels = self.permission_channels.lock().await;
                channels.remove(&operation_id);
                log::warn!("权限审批超时: operation_id={}", operation_id);
                Err(CommandError::agent(
                    crate::errors::AGENT_CONFIRMATION_TIMEOUT,
                    "权限审批超时(5 分钟未响应)".to_string(),
                ))
            }
        }
    }

    /// 评估操作风险等级
    fn assess_risk_level(tool_name: &str, params: &serde_json::Value) -> &'static str {
        match tool_name {
            "remove" | "remove_dir" => "critical",
            "bash" => {
                if let Some(cmd) = params.get("command").and_then(|v| v.as_str()) {
                    if is_high_risk_command(cmd) {
                        "critical"
                    } else {
                        "high"
                    }
                } else {
                    "high"
                }
            }
            "edit" | "write" | "rename" => "medium",
            "write_script" => "medium",
            "read" | "read_lines" | "list" | "file_info" | "exists" => "low",
            "glob" | "grep" | "search" | "hash" => "low",
            _ => "normal",
        }
    }

    /// 格式化权限审批描述
    fn format_permission_description(tool_name: &str, params: &serde_json::Value, base_desc: &str) -> String {
        match tool_name {
            "edit" => {
                let path = params.get("path").or_else(|| params.get("file_path"))
                    .and_then(|v| v.as_str()).unwrap_or("未知");
                format!("编辑文件: {}", path)
            }
            "write" => {
                let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("未知");
                format!("写入文件: {}", path)
            }
            "remove" => {
                let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("未知");
                format!("删除文件: {}", path)
            }
            "bash" => {
                let cmd = params.get("command").and_then(|v| v.as_str()).unwrap_or("未知");
                format!("执行命令: {}", cmd)
            }
            "write_script" => {
                let lang = params.get("language").and_then(|v| v.as_str()).unwrap_or("脚本");
                format!("写入并执行 {} 脚本", lang)
            }
            _ => base_desc.to_string(),
        }
    }
```

#### 4. 在工具执行前调用权限检查

在 `execute` 方法的工具执行分支中,在 `tool_arc.execute()` 之前插入权限检查:

```rust
                    // [新增] 权限检查:在工具执行前进行
                    if let Err(e) = self.check_permission(
                        &ctx.session_id,
                        &tool_call.name,
                        &arguments,
                        &ctx.workspace_root,
                    ).await {
                        // 权限拒绝:返回错误结果给 LLM
                        let error_msg = format!("权限拒绝: {}", e.message);
                        log::warn!("工具 {} 权限拒绝: {}", tool_call.name, e.message);
                        tool_results.push(serde_json::json!({
                            "tool_call_id": tool_call.id,
                            "role": "tool",
                            "content": error_msg,
                            "is_error": true,
                        }));
                        // 发射 tool_result 事件(标记失败)
                        self.emitter.emit_tool_result(ToolResultPayload {
                            session_id: ctx.session_id.clone(),
                            call_id: tool_call.id.clone(),
                            success: false,
                            result: serde_json::json!({"error": error_msg}),
                            error: Some(e.message.clone()),
                            duration_ms: 0,
                        }).ok();
                        continue;
                    }

                    // 原有工具执行逻辑
                    let tool_arc = ...;
                    let result = tool_arc.execute(arguments_clone.clone(), workspace_root).await;
```

**验证**:
```bash
cargo build -p docagent_lib
```

---

### T2.11:改造 confirm_operation 命令支持 once/always/reject

**文件**:
- 修改: `src-tauri/src/commands/agent.rs`

**说明**:
升级 `confirm_operation` 命令为权限审批命令,新增 `permission_respond` 命令处理三选项响应。
保留旧 `confirm_operation` 命令以兼容前端逐步迁移。

**实施步骤**:

在 `src-tauri/src/commands/agent.rs` 新增权限审批命令:

```rust
/// 权限审批响应
/// 用户在权限对话框中选择 once/always/reject
#[tauri::command]
pub async fn permission_respond(
    session_id: String,
    operation_id: String,
    response: String,
    feedback: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("permission_respond 请求: session_id={}, operation_id={}, response={}",
        session_id, operation_id, response);

    let perm_response = crate::services::permission::PermissionResponse::from_str(&response)
        .ok_or_else(|| CommandError::new(
            crate::errors::CONFIG_INVALID_VALUE,
            format!("无效的权限响应: {}", response),
        ))?;

    let decision = crate::PermissionDecision {
        response: perm_response,
        feedback,
    };

    // 优先查找权限审批通道
    let sender = {
        let mut channels = state.permission_channels.lock().await;
        channels.remove(&operation_id)
    };

    match sender {
        Some(tx) => {
            if tx.send(decision).is_err() {
                log::warn!("permission_respond: 接收端已关闭, operation_id={}", operation_id);
                return Err(CommandError::agent(
                    AGENT_SESSION_NOT_FOUND,
                    "Agent 执行已结束,无法响应权限审批".to_string(),
                ));
            }
            log::info!("permission_respond: 响应已发送, operation_id={}, response={}",
                operation_id, response);
            Ok(())
        }
        None => {
            // 回退到旧的 confirm_channels(兼容阶段)
            let old_sender = {
                let mut channels = state.confirm_channels.lock().await;
                channels.remove(&operation_id)
            };
            if let Some(tx) = old_sender {
                let approved = !matches!(perm_response, crate::services::permission::PermissionResponse::Reject);
                let old_decision = crate::ConfirmDecision {
                    approved,
                    feedback: decision.feedback,
                };
                if tx.send(old_decision).is_err() {
                    return Err(CommandError::agent(
                        AGENT_SESSION_NOT_FOUND,
                        "Agent 执行已结束,无法确认操作".to_string(),
                    ));
                }
                Ok(())
            } else {
                Err(CommandError::agent(
                    AGENT_SESSION_NOT_FOUND,
                    format!("未找到权限审批通道: {}", operation_id),
                ))
            }
        }
    }
}
```

在 `src-tauri/src/services/permission/types.rs` 中为 `PermissionResponse` 添加 `from_str`:

```rust
impl PermissionResponse {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "once" => Some(Self::Once),
            "always" => Some(Self::Always),
            "reject" => Some(Self::Reject),
            _ => None,
        }
    }
}
```

在 `src-tauri/src/lib.rs` 的 `invoke_handler` 中注册新命令:

```rust
// 在 invoke_handler 中新增
crate::commands::agent::permission_respond,
```

**验证**:
```bash
cargo build -p docagent_lib
```

---

### T2.12:定义 Agent 模式枚举(Plan/Build/Document)

**文件**:
- 新增: `src-tauri/src/services/agent/mod.rs` 中添加(若已存在则修改)

**说明**:
已在 T2.10 中定义 `AgentMode` 枚举(含 Plan/Build/Document 三态),此处补充模式切换方法。

> v1.1 修订:模式切换仅由前端按钮触发,不提供 plan_exit 等切换工具。AgentModeManager 的 `switch_to_*` 方法仅供前端命令调用。

**实施步骤**:

在 `src-tauri/src/services/agent/mod.rs` 中新增模式切换逻辑:

```rust
use tokio::sync::RwLock;

/// Agent 模式管理器
/// 负责跟踪每个会话的当前模式(Plan/Build/Document)
/// 模式切换仅由前端按钮触发,不提供 LLM 工具切换模式
pub struct AgentModeManager {
    /// 按 session_id 隔离的模式状态
    modes: Arc<RwLock<HashMap<String, AgentMode>>>,
}

impl AgentModeManager {
    pub fn new() -> Self {
        Self {
            modes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 获取指定会话的当前模式(默认 Build)
    pub async fn get_mode(&self, session_id: &str) -> AgentMode {
        let modes = self.modes.read().await;
        modes.get(session_id).copied().unwrap_or_default()
    }

    /// 设置指定会话的模式(由前端命令调用)
    pub async fn set_mode(&self, session_id: &str, mode: AgentMode) {
        let mut modes = self.modes.write().await;
        let old = modes.insert(session_id.to_string(), mode);
        log::info!("会话 {} 模式切换: {:?} → {:?}", session_id, old, mode);
    }

    /// 切换到 Plan 模式(前端按钮触发)
    pub async fn switch_to_plan(&self, session_id: &str) {
        self.set_mode(session_id, AgentMode::Plan).await;
    }

    /// 切换到 Build 模式(前端按钮触发)
    pub async fn switch_to_build(&self, session_id: &str) {
        self.set_mode(session_id, AgentMode::Build).await;
    }

    /// 切换到 Document 模式(前端按钮触发)
    /// Document 模式下,4 个文档 Handler 会动态加入工具列表
    pub async fn switch_to_document(&self, session_id: &str) {
        self.set_mode(session_id, AgentMode::Document).await;
    }

    /// 清理会话模式状态
    pub async fn cleanup(&self, session_id: &str) {
        let mut modes = self.modes.write().await;
        modes.remove(session_id);
    }
}

impl Default for AgentModeManager {
    fn default() -> Self {
        Self::new()
    }
}
```

---

### T2.13:实现工具列表动态过滤(按 AgentMode 过滤文档 Handler)

**文件**:
- 修改: `src-tauri/src/services/agent/executor.rs`(在构建 tool_definitions 时按模式过滤)

**说明**:
> v1.1 修订:原 T2.13 为"实现 plan_exit 工具",现已移除。模式切换改为仅前端按钮触发,因此 T2.13 重新定义为"工具列表动态过滤"。

本任务实现基于 `AgentMode` 的工具列表动态过滤机制:
1. executor 在每轮迭代构建 `tool_definitions`(发送给 LLM 的工具清单)时,根据当前会话的 `AgentMode` 过滤工具
2. **非 Document 模式**(Plan/Build):过滤掉 4 个文档 Handler(`docx`/`xlsx`/`pptx`/`pdf`),LLM 完全感知不到它们的存在
3. **Document 模式**:4 个文档 Handler 出现在工具列表中,LLM 可以调用它们处理文档

**设计要点**:
- 过滤发生在 `tool_definitions` 构建阶段,不影响 `handler_registry` 的注册内容(Handler 始终注册在 AppState 中)
- 过滤是"按需可见性控制",而非"启用/禁用",Handler 代码本身不做任何改动
- 这保证了阶段 1 的"保留 Handler"设计与阶段 2 的"按模式过滤"设计的无缝衔接

**实施步骤**:

#### 1. 定义文档 Handler 名称常量

在 `src-tauri/src/services/agent/executor.rs` 中新增:

```rust
/// 文档 Handler 名称列表(仅在 Document 模式下对 LLM 可见)
/// 这些 Handler 始终注册在 handler_registry 中,但非 Document 模式下不出现在 tool_definitions
const DOCUMENT_HANDLER_NAMES: &[&str] = &[
    "docx",
    "xlsx",
    "pptx",
    "pdf",
];

/// 判断工具名称是否为文档 Handler
fn is_document_handler(tool_name: &str) -> bool {
    DOCUMENT_HANDLER_NAMES.contains(&tool_name)
}
```

#### 2. 在 tool_definitions 构建逻辑中加入过滤

在 `AgentExecutor` 构建发送给 LLM 的 `tool_definitions` 时,根据当前会话的 `AgentMode` 过滤:

```rust
impl AgentExecutor {
    /// 构建发送给 LLM 的工具定义列表(按 AgentMode 过滤)
    /// 非 Document 模式下,4 个文档 Handler 不出现在列表中
    async fn build_tool_definitions(
        &self,
        session_id: &str,
    ) -> Vec<serde_json::Value> {
        // 1. 获取当前会话的 AgentMode
        let mode = self.agent_mode.read().await;
        let include_document_handlers = mode.includes_document_handlers();
        drop(mode); // 立即释放锁

        // 2. 收集所有 Tool 的定义
        let mut definitions: Vec<serde_json::Value> = self.tool_registry
            .list()
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.tool_name(),
                    "description": tool.description(),
                    "parameters": tool.parameters(),
                })
            })
            .collect();

        // 3. 收集 Handler 的定义(按模式过滤)
        {
            let handler_registry = self.registry.lock().await;
            for handler in handler_registry.list() {
                let name = handler.handler_name();
                // [关键] 非 Document 模式下,跳过文档 Handler
                if !include_document_handlers && is_document_handler(name) {
                    log::debug!("模式过滤:会话 {} 当前模式不包含文档 Handler,跳过 {}", session_id, name);
                    continue;
                }
                definitions.push(serde_json::json!({
                    "name": name,
                    "description": handler.description(),
                    "parameters": handler.parameters(),
                }));
            }
        }

        // 4. 按字母序排序(缓存优化,见阶段 1 设计)
        definitions.sort_by(|a, b| {
            a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or(""))
        });

        definitions
    }
}
```

#### 3. 在主循环中调用过滤后的构建方法

在 `run_agent` 的主循环中,每轮迭代调用 `build_tool_definitions` 替代原有的直接收集逻辑:

```rust
// 在主循环开始处(每轮迭代)
let tool_definitions = self.build_tool_definitions(session_id).await;
log::info!(
    "会话 {} 当前模式工具数量: {}(含文档 Handler: {})",
    session_id,
    tool_definitions.len(),
    // 仅 Document 模式才含文档 Handler
    self.agent_mode.read().await.includes_document_handlers()
);
```

#### 4. 工具执行时的防御性校验

虽然 `tool_definitions` 已经过滤,但为防止 LLM 幻觉调用不可见的 Handler,在工具执行分支加入防御性校验:

```rust
// 在工具执行逻辑中(handler_arc 查找前)
let mode = self.agent_mode.read().await;
if !mode.includes_document_handlers() && is_document_handler(&tool_name) {
    log::warn!(
        "会话 {} 当前模式不包含文档 Handler,但 LLM 尝试调用 {}(可能是幻觉)",
        session_id, tool_name
    );
    return Err(CommandError::agent(
        crate::errors::AGENT_OPERATION_REJECTED,
        format!("工具 {} 在当前模式下不可用", tool_name),
    ));
}
drop(mode);
```

**验证**:
```bash
# 编译验证
cargo build -p docagent_lib

# 单元测试:验证不同模式下的工具列表
cargo test tool_definitions_filtering
```

**预期结果**:
- Build 模式:`tool_definitions` 中不包含 `docx`/`xlsx`/`pptx`/`pdf`
- Plan 模式:同 Build 模式,不包含文档 Handler
- Document 模式:`tool_definitions` 中包含 4 个文档 Handler

---

### T2.14:重构系统提示词层(按 Agent 模式注入,含 Document 分支)

**文件**:
- 修改: `src-tauri/src/services/agent/context.rs`

**说明**:
根据 Agent 模式注入不同的提示词:
- Plan 模式:强调"只读规划,禁止修改",引导 Agent 制定详细计划
- Build 模式:强调"完整执行",引导 Agent 按计划实施
- Document 模式:Build 超集,强调"文档处理能力可用",引导 Agent 使用文档 Handler

> v1.1 修订:新增 Document 模式分支。Plan 模式不再提及 plan_exit 工具(模式切换改为前端按钮)。

**实施步骤**:

在 `src-tauri/src/services/agent/context.rs` 中新增模式特定提示词层:

```rust
/// Agent 模式特定提示词层
/// 根据 Plan/Build/Document 模式注入不同的行为指导
pub fn layer_agent_mode(mode: &AgentMode) -> String {
    match mode {
        AgentMode::Plan => {
            // Plan 模式:只读规划
            r#"
## 当前模式:Plan(规划模式)

你正处于 Plan 模式,在此模式下你只能进行只读操作:

### 允许的操作
- 读取文件内容(read, read_lines)
- 搜索文件(glob, grep, search)
- 列出目录(list)
- 获取文件信息(file_info, exists)
- 计算文件哈希(hash)

### 禁止的操作
- 编辑文件(edit, write)
- 删除文件(remove, remove_dir)
- 执行命令(bash)
- 写入脚本(write_script)
- 重命名/复制文件(rename, copy)

### 你的任务
1. 充分理解用户需求
2. 探索现有代码结构,理解架构
3. 制定详细的实施计划,包括:
   - 需要修改的文件列表
   - 每个文件的具体修改内容
   - 修改顺序和依赖关系
   - 验证方法(测试命令)
4. 完成规划后,请告知用户切换到 Build 或 Document 模式以开始实施

### 计划格式
请以结构化格式输出计划:
```
## 实施计划

### 目标
[简要描述目标]

### 文件修改
1. `path/to/file1` - [修改说明]
2. `path/to/file2` - [修改说明]

### 执行顺序
1. [步骤1]
2. [步骤2]

### 验证方法
- [测试命令1]
- [测试命令2]
```
"#.to_string()
        }
        AgentMode::Build => {
            // Build 模式:完整执行
            r#"
## 当前模式:Build(执行模式)

你正处于 Build 模式,拥有完整的代码编辑和命令执行能力。

### 你的任务
1. 如果存在之前的计划,按照计划逐步实施
2. 每次修改后验证结果(运行测试、编译检查)
3. 遇到问题时调整策略,必要时请用户切换到 Plan 模式重新规划
4. 完成所有修改后,运行完整测试套件验证

### 最佳实践
- 一次只修改一个文件,验证后再继续
- 使用 edit 工具进行精确修改,避免覆盖整个文件
- 修改后立即运行相关测试
- 遇到编译错误时优先修复,不要继续添加新代码

### 注意
- 当前模式下文档 Handler(docx/xlsx/pptx/pdf)不可用
- 如需处理 Word/Excel/PPT/PDF 文档,请告知用户切换到 Document 模式
"#.to_string()
        }
        AgentMode::Document => {
            // Document 模式:Build 超集 + 文档处理能力
            r#"
## 当前模式:Document(文档处理模式)

你正处于 Document 模式,拥有完整的代码编辑和命令执行能力,并且可以使用文档处理 Handler。

### 可用的文档处理工具
- `docx`:Word 文档处理(读取/转换/分析/生成)
- `xlsx`:Excel 文档处理(读取/转换/分析/生成)
- `pptx`:PPT 文档处理(读取/转换/分析/生成)
- `pdf`:PDF 文档处理(读取/转换/分析/生成)

### 你的任务
1. 使用文档 Handler 读取、分析、生成或修改 Word/Excel/PPT/PDF 文档
2. 结合编程工具(edit/glob/grep/bash)完成复杂文档处理任务
3. 文档生成后,使用文档验证机制检查质量
4. 必要时编写脚本处理批量文档操作

### 最佳实践
- 文档生成优先使用 Handler,而非手动编写脚本
- 修改文档前先读取原文档结构,理解格式
- 文档生成后执行验证(validator),检查常见问题
- 大型文档处理时,分步骤完成并验证中间结果

### 文档处理与编程的结合
- 可以用 grep 搜索文档内容(对 Markdown/CSV 等文本格式)
- 可以用 edit 修改 Markdown 文档
- 可以用 bash 执行文档转换脚本
- 可以用 write_script 编写复杂的文档处理流程
"#.to_string()
        }
    }
}
```

在 `build_system_prompt_with_task` 方法中注入此层:

```rust
pub fn build_system_prompt_with_task(
    workspace_path: &str,
    task_type: &TaskType,
    tool_count: usize,
    handler_count: usize,
    budget: &TokenBudgetManager,
    agents_md: Option<&str>,
    env_info: &EnvironmentInfo,
    agent_mode: &AgentMode,  // [新增] 参数
) -> String {
    let mut prompt = String::new();

    // Layer 0: 身份层
    prompt.push_str(&layer_identity());
    prompt.push_str("\n\n");

    // Layer 1: 规则层
    prompt.push_str(&layer_rules());
    prompt.push_str("\n\n");

    // Layer 2: 上下文层
    prompt.push_str(&layer_context(workspace_path, tool_count, handler_count, env_info));
    prompt.push_str("\n\n");

    // Layer 2.5: AGENTS.md 规则(阶段1实现)
    if let Some(md) = agents_md {
        prompt.push_str(md);
        prompt.push_str("\n\n");
    }

    // Layer 3: Agent 模式特定提示词
    prompt.push_str(&layer_agent_mode(agent_mode));
    prompt.push_str("\n\n");

    // Layer 4: 工具策略层
    prompt.push_str(&layer_tool_strategy());
    prompt.push_str("\n\n");

    // ... 其他层 ...

    prompt
}
```

**验证**:
```bash
cargo build -p docagent_lib
```

---

### T2.15:改造 AppState 加入 permission_registry 和 agent_mode

**文件**:
- 修改: `src-tauri/src/lib.rs`

**说明**:
在 AppState 中加入权限系统相关字段,并调整初始化流程。

**实施步骤**:

#### 1. 修改 AppState 结构

文件: `src-tauri/src/lib.rs`

```rust
pub struct AppState {
    pub db: Arc<crate::db::Database>,
    pub config: Arc<tokio::sync::Mutex<crate::config::ConfigManager>>,
    pub active_agents: Arc<tokio::sync::Mutex<HashMap<String, bool>>>,
    // [保留] 旧确认通道(兼容)
    pub confirm_channels: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<ConfirmDecision>>>>,
    // [新增] 权限审批通道(三选项)
    pub permission_channels: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<PermissionDecision>>>>,
    // [移除] 阶段1已移除 doc_service 和 handler_registry
    pub llm_router: Arc<tokio::sync::RwLock<Arc<crate::services::llm::router::LlmRouter>>>,
    pub tool_registry: Arc<crate::services::tool::registry::ToolRegistry>,
    pub fs_watcher: Arc<crate::services::fs_watcher::FsWatcherService<tauri::Wry>>,
    pub network_monitor: Arc<crate::services::network_monitor::NetworkMonitor<tauri::Wry>>,
    pub scratchpad_states: crate::services::tool::builtin::SharedScratchpadStates,
    // [新增] 权限注册表
    pub permission_registry: Arc<crate::services::permission::PermissionRegistry>,
    // [新增] 会话白名单
    pub session_whitelist: Arc<crate::services::permission::SessionWhitelist>,
    // [新增] Doom loop 检测器
    pub doom_loop_detector: Arc<crate::services::permission::DoomLoopDetector>,
    // [新增] Agent 模式管理器
    pub agent_mode_manager: Arc<crate::services::agent::AgentModeManager>,
}
```

#### 2. 修改初始化流程

在 `setup` 闭包中初始化权限系统:

```rust
// [新增] 初始化权限系统
let permission_registry = Arc::new(
    crate::services::permission::PermissionRegistry::new(Arc::clone(&database))
);
let session_whitelist = Arc::new(crate::services::permission::SessionWhitelist::new());
let doom_loop_detector = Arc::new(crate::services::permission::DoomLoopDetector::new());
let agent_mode_manager = Arc::new(crate::services::agent::AgentModeManager::new());

log::info!("权限系统已初始化");

// 注册内置工具时传入 mode_manager
crate::services::tool::builtin::register_builtin_tools(
    &tool_registry,
    workspace_root.clone(),
    git_bash_path.clone(),
    Arc::clone(&agent_mode_manager),
);
```

#### 3. 修改 start_agent 命令

在 `start_agent` 中将权限系统组件传入 `run_agent`:

```rust
let permission_registry = Arc::clone(&state.permission_registry);
let session_whitelist = Arc::clone(&state.session_whitelist);
let doom_loop_detector = Arc::clone(&state.doom_loop_detector);
let agent_mode_manager = Arc::clone(&state.agent_mode_manager);
let permission_channels = Arc::clone(&state.permission_channels);

// ... 传入 run_agent ...
```

**验证**:
```bash
cargo build -p docagent_lib
```

---

### T2.16:前端:InputArea 增加 Plan/Build/Document 模式切换按钮

**文件**:
- 修改: `src/components/layout/InputArea.tsx`
- 新增: `src/stores/useAgentModeStore.ts`

**说明**:
在 InputArea 组件中增加三态模式切换按钮,不改变现有 UI 布局。
按钮位置:输入框左下角,与现有 Provider 选择器对称。

> v1.1 修订:从 Plan/Build 双态切换改为 Plan/Build/Document 三态切换。由于三态无法用单一 toggle 按钮表达,改为三按钮组(或下拉菜单)。

**实施步骤**:

#### 1. 创建 Agent 模式 Store

文件: `src/stores/useAgentModeStore.ts`

```typescript
import { create } from 'zustand';

// v1.1: 新增 'document' 模式
export type AgentMode = 'plan' | 'build' | 'document';

interface AgentModeState {
  /** 当前会话的模式 */
  mode: AgentMode;
  /** 设置模式 */
  setMode: (mode: AgentMode) => void;
  /** 切换到 Plan 模式(只读规划) */
  switchToPlan: () => void;
  /** 切换到 Build 模式(完整执行) */
  switchToBuild: () => void;
  /** 切换到 Document 模式(Build 超集 + 文档 Handler) */
  switchToDocument: () => void;
}

export const useAgentModeStore = create<AgentModeState>((set) => ({
  mode: 'build', // 默认 Build 模式
  setMode: (mode) => set({ mode }),
  switchToPlan: () => set({ mode: 'plan' }),
  switchToBuild: () => set({ mode: 'build' }),
  switchToDocument: () => set({ mode: 'document' }),
}));
```

#### 2. 修改 InputArea 组件

在 `src/components/layout/InputArea.tsx` 中增加三态模式切换按钮组:

```tsx
import { useAgentModeStore, type AgentMode } from '../../stores/useAgentModeStore';

// 三态模式切换按钮组(与 ProviderSelector 对称)
function ModeSwitchButton() {
  const { t } = useTranslation();
  const mode = useAgentModeStore((s) => s.mode);
  const setMode = useAgentModeStore((s) => s.setMode);

  // 模式配置:plan/build/document 三态
  const modes: Array<{ key: AgentMode; icon: string; labelKey: string; titleKey: string }> = [
    { key: 'plan', icon: 'eye', labelKey: 'agentMode.plan', titleKey: 'agentMode.planMode' },
    { key: 'build', icon: 'hammer', labelKey: 'agentMode.build', titleKey: 'agentMode.buildMode' },
    { key: 'document', icon: 'file-text', labelKey: 'agentMode.document', titleKey: 'agentMode.documentMode' },
  ];

  return (
    <div className="mode-switch-group" role="group" aria-label={t('agentMode.switchGroup')}>
      {modes.map(({ key, icon, labelKey, titleKey }) => (
        <button
          key={key}
          type="button"
          onClick={() => setMode(key)}
          className={`mode-switch-btn mode-${key} ${mode === key ? 'active' : ''}`}
          title={t(titleKey)}
          aria-pressed={mode === key}
        >
          <Icon name={icon} size={14} />
          <span>{t(labelKey)}</span>
        </button>
      ))}
    </div>
  );
}

// 在 InputArea 的 return JSX 中,在 ProviderSelector 旁添加:
// <div className="input-toolbar">
//   <ProviderSelector ... />
//   <ModeSwitchButton />
//   <WorkspaceSelector ... />
// </div>
```

#### 3. 添加样式

在 `src/styles/globals.css` 中添加:

```css
/* 三态模式切换按钮组 */
.mode-switch-group {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  border-radius: 6px;
  border: 1px solid var(--border-color);
  overflow: hidden;
}

.mode-switch-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  font-size: 12px;
  background: var(--bg-secondary);
  cursor: pointer;
  transition: all 0.15s ease;
  border: none;
  color: var(--text-secondary);
}

.mode-switch-btn:hover {
  background: var(--bg-hover);
}

/* 激活状态 */
.mode-switch-btn.active {
  font-weight: 600;
}

.mode-switch-btn.mode-plan.active {
  color: var(--color-info);
  background: color-mix(in srgb, var(--color-info) 15%, transparent);
}

.mode-switch-btn.mode-build.active {
  color: var(--color-success);
  background: color-mix(in srgb, var(--color-success) 15%, transparent);
}

.mode-switch-btn.mode-document.active {
  color: var(--color-warning);
  background: color-mix(in srgb, var(--color-warning) 15%, transparent);
}
```

#### 4. 添加国际化

在 `src/i18n/locales/zh-CN.json` 中添加:

```json
{
  "agentMode": {
    "plan": "规划",
    "build": "构建",
    "document": "文档",
    "planMode": "规划模式:只读,禁止修改",
    "buildMode": "构建模式:完整编程执行能力",
    "documentMode": "文档模式:构建能力 + 文档 Handler(docx/xlsx/pptx/pdf)",
    "switchGroup": "Agent 模式切换",
    "switchToPlan": "切换到规划模式",
    "switchToBuild": "切换到构建模式",
    "switchToDocument": "切换到文档模式"
  }
}
```

在 `src/i18n/locales/en-US.json` 中添加对应英文:

```json
{
  "agentMode": {
    "plan": "Plan",
    "build": "Build",
    "document": "Document",
    "planMode": "Plan mode: read-only, no modifications",
    "buildMode": "Build mode: full programming execution",
    "documentMode": "Document mode: Build + document Handlers (docx/xlsx/pptx/pdf)",
    "switchGroup": "Agent mode switch",
    "switchToPlan": "Switch to Plan mode",
    "switchToBuild": "Switch to Build mode",
    "switchToDocument": "Switch to Document mode"
  }
}
```

#### 5. 在 sendMessage 时传递 mode

在 `useAgent.ts` 的 `sendMessage` 中,将当前模式传入 `start_agent` 的 options:

```typescript
const mode = useAgentModeStore.getState().mode;
await tauriCmd.startAgent(sessionId, prompt, {
  // ... 既有选项 ...
  mode,  // [新增] Agent 模式:plan / build / document
});
```

#### 6. 新增切换模式的 Tauri 命令

> v1.1 新增:由于模式切换由前端按钮触发,需要新增 Tauri 命令让前端调用后端的 `AgentModeManager`。

在 `src-tauri/src/commands/agent.rs` 中新增:

```rust
/// 切换会话的 Agent 模式(前端按钮触发)
#[tauri::command]
pub async fn switch_agent_mode(
    state: tauri::State<'_, AppState>,
    session_id: String,
    mode: String, // "plan" / "build" / "document"
) -> Result<(), CommandError> {
    let agent_mode = match mode.as_str() {
        "plan" => AgentMode::Plan,
        "build" => AgentMode::Build,
        "document" => AgentMode::Document,
        _ => return Err(CommandError::validation(
            format!("无效的 Agent 模式: {}", mode),
        )),
    };
    state.agent_mode_manager.switch_to_mode(&session_id, agent_mode).await;
    log::info!("会话 {} 模式已切换为: {:?}", session_id, agent_mode);
    Ok(())
}
```

在 `src/services/tauri.ts` 中封装:

```typescript
/** 切换 Agent 模式(前端按钮触发) */
export async function switchAgentMode(sessionId: string, mode: 'plan' | 'build' | 'document'): Promise<void> {
  await invoke('switch_agent_mode', { sessionId, mode });
}
```

在 `useAgentModeStore.ts` 的 `setMode` 中调用后端命令:

```typescript
setMode: (mode) => {
  set({ mode });
  // 同步到后端 AgentModeManager
  // 注意:需要在外部调用时传入 sessionId,或在组件中处理
},
```

**验证**:
```bash
npm run build
cargo build -p docagent_lib
```

---

### T2.17:前端:权限审批对话框升级(once/always/reject)

**文件**:
- 修改: `src/components/workflow/ConfirmNode.tsx`(或等效的确认对话框组件)
- 修改: `src/hooks/useAgent.ts`

**说明**:
升级确认对话框,从 approve/reject 二选项改为 once/always/reject 三选项。

**实施步骤**:

#### 1. 添加权限审批命令封装

文件: `src/services/tauri.ts`

```typescript
/**
 * 响应权限审批
 * @param sessionId 会话 ID
 * @param operationId 操作 ID
 * @param response 响应类型: once / always / reject
 * @param feedback 反馈信息(可选)
 */
export async function permissionRespond(
  sessionId: string,
  operationId: string,
  response: 'once' | 'always' | 'reject',
  feedback?: string,
): Promise<void> {
  await invoke('permission_respond', {
    sessionId,
    operationId,
    response,
    feedback,
  });
}
```

#### 2. 修改 useAgent hook

文件: `src/hooks/useAgent.ts`

```typescript
// 修改 confirmOperation 为 permissionRespond
const respondPermission = useCallback(
  async (
    operationId: string,
    response: 'once' | 'always' | 'reject',
    feedback?: string,
  ) => {
    if (!sessionId) return;
    try {
      await tauriCmd.permissionRespond(sessionId, operationId, response, feedback);
      setPendingConfirmation(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  },
  [sessionId],
);

// 在返回对象中导出
return {
  // ... 既有字段 ...
  respondPermission,  // [新增] 替代 confirmOperation
};
```

#### 3. 修改确认对话框 UI

在显示确认对话框的组件中(如 `ConfirmNode` 或 `WorkflowTimeline` 中的确认节点),将按钮改为三个:

```tsx
function PermissionDialog({ payload, onRespond }: {
  payload: ConfirmPayload;
  onRespond: (response: 'once' | 'always' | 'reject') => void;
}) {
  const { t } = useTranslation();
  const [feedback, setFeedback] = useState('');

  const riskColor = {
    low: 'var(--color-success)',
    normal: 'var(--color-info)',
    high: 'var(--color-warning)',
    critical: 'var(--color-danger)',
  }[payload.riskLevel] || 'var(--color-info)';

  return (
    <div className="permission-dialog">
      <div className="permission-header">
        <Icon name="shield" size={16} style={{ color: riskColor }} />
        <span className="permission-title">{payload.description}</span>
      </div>

      {payload.details && Object.keys(payload.details).length > 0 && (
        <pre className="permission-details">
          {JSON.stringify(payload.details, null, 2)}
        </pre>
      )}

      <div className="permission-feedback">
        <input
          type="text"
          value={feedback}
          onChange={(e) => setFeedback(e.target.value)}
          placeholder={t('permission.feedbackPlaceholder')}
          className="permission-input"
        />
      </div>

      <div className="permission-buttons">
        <button
          className="btn btn-once"
          onClick={() => onRespond('once')}
        >
          {t('permission.once')}
        </button>
        <button
          className="btn btn-always"
          onClick={() => onRespond('always')}
        >
          {t('permission.always')}
        </button>
        <button
          className="btn btn-reject"
          onClick={() => onRespond('reject')}
        >
          {t('permission.reject')}
        </button>
      </div>
    </div>
  );
}
```

#### 4. 添加样式和国际化

在 `src/styles/globals.css` 添加:

```css
.permission-dialog {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 12px;
  background: var(--bg-secondary);
  margin: 8px 0;
}

.permission-buttons {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.btn-once { background: var(--color-info); color: white; }
.btn-always { background: var(--color-success); color: white; }
.btn-reject { background: var(--color-danger); color: white; }
```

在 `src/i18n/locales/zh-CN.json` 添加:

```json
{
  "permission": {
    "once": "本次允许",
    "always": "永久允许",
    "reject": "拒绝",
    "feedbackPlaceholder": "反馈信息(可选)"
  }
}
```

**验证**:
```bash
npm run build
```

---

### T2.18:前端:设置弹窗增加权限规则管理 UI

**文件**:
- 新增: `src/components/settings/PermissionTab.tsx`
- 修改: `src/components/settings/SettingsDialog.tsx`(注册新标签页)

**说明**:
在设置弹窗中增加"权限规则"标签页,允许用户查看、添加、编辑、删除权限规则。

**实施步骤**:

#### 1. 新增 PermissionTab 组件

文件: `src/components/settings/PermissionTab.tsx`

```tsx
import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../common/Button';
import { Icon } from '../common/Icon';
import * as tauriCmd from '../../services/tauri';
import type { PermissionRule } from '../../types/permission';

export function PermissionTab() {
  const { t } = useTranslation();
  const [rules, setRules] = useState<PermissionRule[]>([]);
  const [loading, setLoading] = useState(true);
  const [editingRule, setEditingRule] = useState<PermissionRule | null>(null);

  useEffect(() => {
    loadRules();
  }, []);

  const loadRules = async () => {
    setLoading(true);
    try {
      const result = await tauriCmd.listPermissionRules();
      setRules(result);
    } catch (err) {
      console.error('加载权限规则失败:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleDelete = async (ruleId: string) => {
    if (!confirm(t('permission.confirmDelete'))) return;
    try {
      await tauriCmd.deletePermissionRule(ruleId);
      await loadRules();
    } catch (err) {
      console.error('删除权限规则失败:', err);
    }
  };

  if (loading) {
    return <div className="permission-tab loading">{t('common.loading')}</div>;
  }

  return (
    <div className="permission-tab">
      <div className="tab-header">
        <h3>{t('permission.title')}</h3>
        <Button onClick={() => setEditingRule({} as PermissionRule)}>
          <Icon name="plus" size={14} />
          {t('permission.addRule')}
        </Button>
      </div>

      <div className="rules-list">
        {rules.map((rule) => (
          <div key={rule.id} className="rule-item">
            <div className="rule-info">
              <span className="rule-type">{rule.permissionType}</span>
              <code className="rule-pattern">{rule.pattern}</code>
              <span className={`rule-action action-${rule.action}`}>{rule.action}</span>
              {rule.description && (
                <span className="rule-desc">{rule.description}</span>
              )}
            </div>
            <div className="rule-actions">
              <button onClick={() => setEditingRule(rule)}>
                <Icon name="edit" size={14} />
              </button>
              <button onClick={() => handleDelete(rule.id)}>
                <Icon name="trash" size={14} />
              </button>
            </div>
          </div>
        ))}
        {rules.length === 0 && (
          <div className="empty-state">{t('permission.noRules')}</div>
        )}
      </div>

      {editingRule && (
        <PermissionRuleEditor
          rule={editingRule}
          onClose={() => setEditingRule(null)}
          onSaved={() => {
            setEditingRule(null);
            loadRules();
          }}
        />
      )}
    </div>
  );
}

function PermissionRuleEditor({ rule, onClose, onSaved }: {
  rule: PermissionRule;
  onClose: () => void;
  onSaved: () => void;
}) {
  const { t } = useTranslation();
  const [form, setForm] = useState({
    scope: rule.scope || 'global',
    permissionType: rule.permissionType || 'bash',
    pattern: rule.pattern || '*',
    action: rule.action || 'ask',
    description: rule.description || '',
  });

  const handleSave = async () => {
    try {
      if (rule.id) {
        await tauriCmd.updatePermissionRule({ ...rule, ...form });
      } else {
        await tauriCmd.addPermissionRule(form);
      }
      onSaved();
    } catch (err) {
      console.error('保存权限规则失败:', err);
    }
  };

  return (
    <div className="rule-editor-modal">
      <h4>{rule.id ? t('permission.editRule') : t('permission.addRule')}</h4>

      <div className="form-field">
        <label>{t('permission.scope')}</label>
        <select
          value={form.scope}
          onChange={(e) => setForm({ ...form, scope: e.target.value })}
        >
          <option value="global">{t('permission.scopeGlobal')}</option>
          <option value="project">{t('permission.scopeProject')}</option>
        </select>
      </div>

      <div className="form-field">
        <label>{t('permission.type')}</label>
        <select
          value={form.permissionType}
          onChange={(e) => setForm({ ...form, permissionType: e.target.value })}
        >
          <option value="*">{t('permission.typeWildcard')}</option>
          <option value="read">read</option>
          <option value="edit">edit</option>
          <option value="bash">bash</option>
          <option value="glob">glob</option>
          <option value="grep">grep</option>
          <option value="external_directory">external_directory</option>
          <option value="doom_loop">doom_loop</option>
        </select>
      </div>

      <div className="form-field">
        <label>{t('permission.pattern')}</label>
        <input
          type="text"
          value={form.pattern}
          onChange={(e) => setForm({ ...form, pattern: e.target.value })}
          placeholder="如: *.env 或 git * 或 src/**/*.ts"
        />
      </div>

      <div className="form-field">
        <label>{t('permission.action')}</label>
        <select
          value={form.action}
          onChange={(e) => setForm({ ...form, action: e.target.value })}
        >
          <option value="allow">{t('permission.actionAllow')}</option>
          <option value="deny">{t('permission.actionDeny')}</option>
          <option value="ask">{t('permission.actionAsk')}</option>
        </select>
      </div>

      <div className="form-field">
        <label>{t('permission.description')}</label>
        <input
          type="text"
          value={form.description}
          onChange={(e) => setForm({ ...form, description: e.target.value })}
        />
      </div>

      <div className="editor-actions">
        <Button onClick={onClose}>{t('common.cancel')}</Button>
        <Button variant="primary" onClick={handleSave}>{t('common.save')}</Button>
      </div>
    </div>
  );
}
```

#### 2. 新增 Tauri 命令封装

文件: `src/services/tauri.ts`

```typescript
export interface PermissionRule {
  id?: string;
  scope: 'global' | 'project' | 'session';
  workspaceId?: string;
  permissionType: string;
  pattern: string;
  action: 'allow' | 'deny' | 'ask';
  description: string;
  enabled: boolean;
}

export async function listPermissionRules(): Promise<PermissionRule[]> {
  return invoke('list_permission_rules');
}

export async function addPermissionRule(rule: Partial<PermissionRule>): Promise<void> {
  await invoke('add_permission_rule', { rule });
}

export async function updatePermissionRule(rule: PermissionRule): Promise<void> {
  await invoke('update_permission_rule', { rule });
}

export async function deletePermissionRule(ruleId: string): Promise<void> {
  await invoke('delete_permission_rule', { ruleId });
}
```

#### 3. 新增 Rust 端命令

文件: `src-tauri/src/commands/permission.rs`

```rust
use tauri::State;
use crate::errors::CommandError;
use crate::models::permission::{PermissionRule, PermissionRuleFilter};
use crate::services::permission::{PermissionAction, PermissionType, RuleScope};
use crate::AppState;

#[tauri::command]
pub async fn list_permission_rules(
    state: State<'_, AppState>,
) -> Result<Vec<PermissionRule>, CommandError> {
    state.permission_registry.list_user_rules(&PermissionRuleFilter::default())
}

#[tauri::command]
pub async fn add_permission_rule(
    rule: PermissionRule,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    state.permission_registry.add_rule(rule)
}

#[tauri::command]
pub async fn update_permission_rule(
    rule: PermissionRule,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    state.permission_registry.update_rule(rule)
}

#[tauri::command]
pub async fn delete_permission_rule(
    rule_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    state.permission_registry.delete_rule(&rule_id)
}
```

在 `src-tauri/src/lib.rs` 注册命令:

```rust
crate::commands::permission::list_permission_rules,
crate::commands::permission::add_permission_rule,
crate::commands::permission::update_permission_rule,
crate::commands::permission::delete_permission_rule,
```

#### 4. 在 SettingsDialog 中注册新标签页

文件: `src/components/settings/SettingsDialog.tsx`

```tsx
import { PermissionTab } from './PermissionTab';

// 在 tabs 数组中添加
const tabs = [
  // ... 既有标签页 ...
  { id: 'permission', label: t('permission.tabTitle'), component: PermissionTab },
];
```

#### 5. 添加国际化

在 `src/i18n/locales/zh-CN.json` 添加:

```json
{
  "permission": {
    "tabTitle": "权限规则",
    "title": "权限规则管理",
    "addRule": "添加规则",
    "editRule": "编辑规则",
    "confirmDelete": "确定删除此权限规则?",
    "noRules": "暂无权限规则,所有操作将使用默认权限",
    "scope": "作用域",
    "scopeGlobal": "全局",
    "scopeProject": "项目",
    "type": "权限类型",
    "typeWildcard": "通配(所有)",
    "pattern": "匹配模式",
    "action": "动作",
    "actionAllow": "允许",
    "actionDeny": "拒绝",
    "actionAsk": "询问",
    "description": "描述"
  }
}
```

**验证**:
```bash
npm run build
cargo build -p docagent_lib
```

---

### T2.19:集成测试:验证权限系统与模式切换

**文件**:
- 新增: `src-tauri/tests/permission_integration_test.rs`

**说明**:
端到端测试权限系统和模式切换的完整流程。

**实施步骤**:

文件: `src-tauri/tests/permission_integration_test.rs`

```rust
use docagent_lib::services::permission::*;
use docagent_lib::models::permission::{PermissionRule, PermissionRuleFilter};

#[tokio::test]
async fn test_full_permission_flow() {
    // 1. 初始化权限系统
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let db = std::sync::Arc::new(docagent_lib::db::Database::new(std::path::Path::new(tmp.path())).unwrap());
    let registry = PermissionRegistry::new(db);
    let whitelist = SessionWhitelist::new();
    let doom_loop = DoomLoopDetector::new();

    // 2. 添加用户规则:禁止读取 .env 文件
    let rule = PermissionRule::new(
        RuleScope::Global,
        PermissionType::Read,
        "*.env".to_string(),
        PermissionAction::Deny,
    );
    registry.add_rule(rule).unwrap();

    // 3. 测试 .env 文件被拒绝
    let rules = registry.load_effective_rules(None, None);
    let req = PermissionRequest {
        permission_type: PermissionType::Read,
        target: "config.env".to_string(),
    };
    let decision = PermissionEvaluator::evaluate(&req, &rules);
    assert_eq!(decision.action, PermissionAction::Deny);

    // 4. 测试普通文件允许
    let req2 = PermissionRequest {
        permission_type: PermissionType::Read,
        target: "src/main.rs".to_string(),
    };
    let decision2 = PermissionEvaluator::evaluate(&req2, &rules);
    assert_eq!(decision2.action, PermissionAction::Allow);

    // 5. 测试白名单
    let rule = SessionWhitelist::generate_always_rule("sess1", PermissionType::Bash, "git status");
    whitelist.add_rule("sess1", rule).await;
    let action = whitelist.check("sess1", PermissionType::Bash, "git status --short").await;
    assert_eq!(action, Some(PermissionAction::Allow));

    // 6. 测试 Doom loop 检测
    let params = serde_json::json!({"path": "file.rs"});
    assert!(!doom_loop.record_and_check("s1", "read", &params).await);
    assert!(!doom_loop.record_and_check("s1", "read", &params).await);
    assert!(doom_loop.record_and_check("s1", "read", &params).await);
}

#[tokio::test]
async fn test_plan_mode_blocks_modification() {
    use docagent_lib::services::agent::AgentMode;

    // Plan 模式下,修改类工具应被拒绝
    let mode = AgentMode::Plan;
    assert!(mode.is_plan());

    let ptype = PermissionType::from_tool_name("edit");
    assert!(ptype.is_modification());

    let ptype = PermissionType::from_tool_name("bash");
    assert!(ptype.is_modification());

    // 只读工具不应被拒绝
    let ptype = PermissionType::from_tool_name("read");
    assert!(!ptype.is_modification());

    let ptype = PermissionType::from_tool_name("glob");
    assert!(!ptype.is_modification());
}

#[tokio::test]
async fn test_agent_mode_manager() {
    use docagent_lib::services::agent::AgentModeManager;

    let manager = AgentModeManager::new();

    // 默认 Build 模式
    assert_eq!(manager.get_mode("s1").await, docagent_lib::services::agent::AgentMode::Build);

    // 切换到 Plan
    manager.switch_to_plan("s1").await;
    assert!(manager.get_mode("s1").await.is_plan());

    // 切换回 Build
    manager.switch_to_build("s1").await;
    assert!(manager.get_mode("s1").await.is_build());

    // v1.1 新增:切换到 Document 模式
    manager.switch_to_document("s1").await;
    assert!(manager.get_mode("s1").await.is_document());
    assert!(manager.get_mode("s1").await.includes_document_handlers());

    // Document 模式切换回 Build
    manager.switch_to_build("s1").await;
    assert!(!manager.get_mode("s1").await.includes_document_handlers());
}

#[tokio::test]
async fn test_document_mode_includes_handlers() {
    // v1.1 新增:验证 Document 模式下文档 Handler 可见性
    use docagent_lib::services::agent::AgentMode;

    // Document 模式应包含文档 Handler
    let mode = AgentMode::Document;
    assert!(mode.includes_document_handlers());
    assert!(mode.is_document());

    // Build 模式不应包含文档 Handler
    let mode = AgentMode::Build;
    assert!(!mode.includes_document_handlers());

    // Plan 模式不应包含文档 Handler
    let mode = AgentMode::Plan;
    assert!(!mode.includes_document_handlers());
}

#[tokio::test]
async fn test_tool_definitions_filtering_by_mode() {
    // v1.1 新增:验证工具列表按 AgentMode 过滤
    use docagent_lib::services::agent::AgentMode;

    // 文档 Handler 名称列表
    let document_handlers = ["docx", "xlsx", "pptx", "pdf"];

    // 模拟工具列表过滤逻辑
    let all_tools: Vec<&str> = vec![
        "edit", "glob", "grep", "read", "bash", "write",
        "docx", "xlsx", "pptx", "pdf",
    ];

    // Build 模式:过滤掉文档 Handler
    let mode = AgentMode::Build;
    let filtered: Vec<&str> = all_tools.iter()
        .filter(|&name| {
            mode.includes_document_handlers() || !document_handlers.contains(name)
        })
        .copied()
        .collect();
    assert!(!filtered.contains(&"docx"));
    assert!(!filtered.contains(&"xlsx"));
    assert!(!filtered.contains(&"pptx"));
    assert!(!filtered.contains(&"pdf"));
    assert!(filtered.contains(&"edit"));
    assert!(filtered.contains(&"glob"));

    // Document 模式:包含文档 Handler
    let mode = AgentMode::Document;
    let filtered: Vec<&str> = all_tools.iter()
        .filter(|&name| {
            mode.includes_document_handlers() || !document_handlers.contains(name)
        })
        .copied()
        .collect();
    assert!(filtered.contains(&"docx"));
    assert!(filtered.contains(&"xlsx"));
    assert!(filtered.contains(&"pptx"));
    assert!(filtered.contains(&"pdf"));
}

#[tokio::test]
async fn test_permission_rule_persistence() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let db = std::sync::Arc::new(docagent_lib::db::Database::new(std::path::Path::new(tmp.path())).unwrap());
    let registry = PermissionRegistry::new(db.clone());

    // 添加规则
    let rule = PermissionRule::new(
        RuleScope::Global,
        PermissionType::Bash,
        "rm *".to_string(),
        PermissionAction::Deny,
    ).with_description("禁止删除");
    registry.add_rule(rule.clone()).unwrap();

    // 重新加载(模拟重启)
    let registry2 = PermissionRegistry::new(db);
    let rules = registry2.load_effective_rules(None, None);
    assert!(rules.iter().any(|r| r.pattern == "rm *" && r.action == PermissionAction::Deny));
}

#[test]
fn test_default_rules_include_env_protection() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let db = std::sync::Arc::new(docagent_lib::db::Database::new(std::path::Path::new(tmp.path())).unwrap());
    let registry = PermissionRegistry::new(db);

    let defaults = registry.default_rules();
    // 验证 .env 文件被默认保护
    assert!(defaults.iter().any(|r| {
        r.pattern == "*.env" && r.action == PermissionAction::Deny
    }));
    // 验证 .env.example 允许
    assert!(defaults.iter().any(|r| {
        r.pattern == "*.env.example" && r.action == PermissionAction::Allow
    }));
}
```

**验证**:
```bash
cargo test --test permission_integration_test -- --nocapture
```

---

## 四、实施检查清单

### 4.1 编译与测试

```bash
# 1. Rust 编译
cargo build -p docagent_lib

# 2. Rust 测试
cargo test

# 3. Clippy 检查
cargo clippy

# 4. 格式检查
cargo fmt --check

# 5. 前端构建
npm run build
```

### 4.2 功能验证

| 验证项 | 操作 | 预期结果 |
|--------|------|----------|
| 权限规则持久化 | 在设置中添加规则,重启应用 | 规则仍然生效 |
| Plan 模式阻止修改 | 切换到 Plan 模式,让 Agent 编辑文件 | Agent 被拒绝,提示切换到 Build/Document 模式 |
| Build 模式完整执行 | 切换到 Build 模式,让 Agent 编辑文件 | Agent 正常执行 |
| Build 模式过滤文档 Handler | 切换到 Build 模式,检查工具列表 | docx/xlsx/pptx/pdf Handler 不在工具列表中 |
| Document 模式包含文档 Handler | 切换到 Document 模式,检查工具列表 | docx/xlsx/pptx/pdf Handler 出现在工具列表中 |
| Document 模式编程工具可用 | 切换到 Document 模式,让 Agent 编辑代码 | Agent 正常执行编程操作 |
| Document 模式调用文档 Handler | 切换到 Document 模式,让 Agent 读取 docx 文件 | Handler 正常执行,返回文档内容 |
| 模式切换仅前端触发 | Agent 尝试调用模式切换工具 | 无此工具可用,LLM 无法自主切换模式 |
| once 选项 | 选择"本次允许",再次触发相同操作 | 再次弹窗 |
| always 选项 | 选择"永久允许",再次触发相同操作 | 不再弹窗,直接执行 |
| reject 选项 | 选择"拒绝" | 操作被拒绝,Agent 收到错误反馈 |
| Doom loop 检测 | 让 Agent 连续 3 次调用相同工具 | 第 3 次触发权限弹窗 |
| .env 文件保护 | 让 Agent 读取 .env 文件 | 被默认规则拒绝 |
| 外部目录访问 | 让 Agent 访问工作区外路径 | 触发权限弹窗 |

### 4.3 日志验证

检查 `log/docagent.log` 中包含以下关键日志:
- `已插入权限规则`
- `已更新权限规则`
- `检测到 Doom loop`
- `会话 X 模式切换`
- `权限审批结果`
- `已清理会话 X 的临时权限规则`

---

## 五、风险与回滚

### 5.1 技术风险

| 风险 | 影响 | 应对措施 |
|------|------|----------|
| **权限规则过多影响性能** | 工具调用前评估耗时增加 | 限制规则数量(建议<100 条);热规则内存缓存;按 permission_type 分组索引 |
| **always 白名单误放行危险操作** | 安全风险 | always 仅生成会话级临时规则,会话结束自动清理;UI 明确提示"永久允许"含义 |
| **Doom loop 误报** | 正常的批量操作被拦截 | 阈值可配置(默认 3 次);检测相同参数而非相似参数;允许用户选择"本次允许"继续 |
| **Plan 模式过严影响体验** | Agent 无法完成简单任务 | Plan 模式允许只读工具;前端按钮一键切换到 Build/Document 模式 |
| **Document 模式工具列表过滤失效** | 非 Document 模式下文档 Handler 可见 | 单元测试覆盖各模式下的工具列表;executor 构建工具定义后断言 Handler 存在/不存在 |
| **LLM 幻觉调用不可见 Handler** | 非 Document 模式下 LLM 尝试调用文档 Handler | 工具执行时加入防御性校验,拒绝并返回错误提示 |
| **权限审批超时阻塞** | Agent 长时间等待 | 5 分钟超时自动拒绝;UI 提示有待审批操作 |
| **旧 confirm_operation 不兼容** | 前端未升级时功能异常 | 保留旧命令兼容;permission_respond 优先查找新通道,回退到旧通道 |

### 5.2 回滚策略

若本阶段改造出现严重问题,可按以下步骤回滚:

1. **回退代码**:恢复到阶段 1 完成时的提交
2. **数据库兼容**:`permission_rules` 表为新增表,不影响既有功能,可保留不动
3. **前端兼容**:`PermissionTab` 为新增标签页,可隐藏不显示
4. **配置兼容**:`AgentMode` 默认为 Build,不影响既有行为

---

## 六、后续阶段衔接

### 6.1 与阶段 3 的衔接

本阶段实现的权限系统将用于阶段 3 的 Skill 加载:
- Skill 加载需要检查 `PermissionType::Skill` 权限
- Skill 中定义的工具调用仍受权限系统管控
- TodoWrite 工具的执行也需要权限检查
- **v1.1 新增**:Skill 的 `is_applicable_to_mode(mode)` 需支持 "document" 字符串,文档相关 Skill 仅在 Document 模式下可见

### 6.2 与阶段 4 的衔接

本阶段实现的权限系统将用于阶段 4 的子 Agent 和网络工具:
- task 工具调用子 Agent 需要检查 `PermissionType::Task` 权限
- WebFetch/WebSearch 工具需要检查对应权限
- 子 Agent 的工具调用继承主 Agent 的权限规则
- 子 Agent 独立的 Doom loop 检测
- **v1.1 新增**:子 Agent 继承父 Agent 的 AgentMode(若主 Agent 在 Document 模式,子 Agent 也能访问文档 Handler)

### 6.3 与阶段 5 的衔接

本阶段实现的权限系统将用于阶段 5 的 LSP 集成:
- LSP 工具调用需要检查 `PermissionType::Lsp` 权限
- LSP 默认全局允许,不支持细粒度规则

### 6.4 待优化项(后续迭代)

- **规则导入导出**:支持 JSON 格式导入导出权限规则
- **规则模板**:提供常见场景的规则模板(个人开发/团队协作/代码审计)
- **规则分享**:支持工作区级规则共享
- **权限审计日志**:记录每次权限决策的详细信息,用于安全审计
- **规则热重载**:修改规则后无需重启即可生效

---

## 七、参考资源

### 7.1 OpenCode 权限系统源码

- **权限模块**: `packages/opencode/src/permission/`
  - `index.ts`:权限规则定义和合并
  - `evaluate.ts`:权限评估逻辑
  - `wildcard.ts`:通配符匹配
- **Agent 模块**: `packages/opencode/src/agent/agent.ts`
  - build/plan/general/explore Agent 定义
  - 权限合并示例
- **Plan/Build 模式切换**: `packages/opencode/src/session/prompt.ts`
  - `BUILD_SWITCH` 提示词
  - 模式切换检测逻辑

### 7.2 OpenCode 默认权限规则

```json
{
  "permission": {
    "read": { "*": "allow", "*.env": "deny", "*.env.*": "deny", "*.env.example": "allow" },
    "edit": "allow",
    "bash": "allow",
    "glob": "allow",
    "grep": "allow",
    "external_directory": "ask",
    "doom_loop": "ask",
    "task": "allow",
    "skill": "allow",
    "webfetch": "allow",
    "websearch": "allow"
  }
}
```

### 7.3 DocAgent 相关文档

- [阶段 1:核心架构与工具链](./2026-07-08-coding-agent-refactor-phase1-core.md)
- [总体改造计划](./2026-07-08-coding-agent-refactor-overview.md)
- [Tauri 命令规范](../tauri_commands.md)
- [数据库设计](../database_design.md)

### 7.4 技术规范参考

- **OpenCode 权限配置教程**:https://www.51cto.com/article/848369.html
- **OpenCode Build 智能体分析**:https://blog.csdn.net/liwei9006/article/details/160032651
- **wildmatch crate 文档**:https://docs.rs/wildmatch
- **Anthropic Effective Context Engineering**:Structured Note-taking 模式

---

## 八、任务完成状态追踪

| 任务 ID | 任务名称 | 状态 | 完成时间 | 备注 |
|---------|---------|------|---------|------|
| T2.01 | 新增权限系统所需依赖到 Cargo.toml | 待实施 | - | |
| T2.02 | 定义权限类型与动作枚举 | 待实施 | - | |
| T2.03 | 实现通配符匹配工具 | 待实施 | - | |
| T2.04 | 创建 permission_rules 数据库表 | 待实施 | - | |
| T2.05 | 实现 PermissionRule 模型与仓库 | 待实施 | - | |
| T2.06 | 实现 PermissionRegistry 权限注册表 | 待实施 | - | |
| T2.07 | 实现 PermissionEvaluator 权限评估器 | 待实施 | - | |
| T2.08 | 实现临时白名单(会话级 always 规则) | 待实施 | - | |
| T2.09 | 实现 Doom loop 检测器 | 待实施 | - | |
| T2.10 | 改造 AgentExecutor 集成权限系统 | 待实施 | - | |
| T2.11 | 改造 confirm_operation 命令支持 once/always/reject | 待实施 | - | |
| T2.12 | 定义 Agent 模式枚举(Plan/Build/Document) | 待实施 | - | |
| T2.13 | 实现工具列表动态过滤(按 AgentMode 过滤文档 Handler) | 待实施 | - | v1.1:替代原 plan_exit 工具 |
| T2.14 | 重构系统提示词层(按 Agent 模式注入,含 Document 分支) | 待实施 | - | |
| T2.15 | 改造 AppState 加入 permission_registry 和 agent_mode | 待实施 | - | |
| T2.16 | 前端:InputArea 增加 Plan/Build/Document 模式切换按钮 | 待实施 | - | |
| T2.17 | 前端:权限审批对话框升级(once/always/reject) | 待实施 | - | |
| T2.18 | 前端:设置弹窗增加权限规则管理 UI | 待实施 | - | |
| T2.19 | 集成测试:验证权限系统与三态模式切换 | 待实施 | - | |

---

## 九、风险与回滚策略

### 9.1 主要风险点

1. **数据库迁移风险**:新增 `permission_rules` 表可能与现有数据冲突
   - 缓解:使用 `CREATE TABLE IF NOT EXISTS`,保证幂等
   - 回滚:删除 `permission_rules` 表即可,不影响其他表

2. **权限系统阻塞 Agent 执行**:权限检查可能因通道等待导致超时
   - 缓解:保留 5 分钟超时机制;前端弹窗及时响应
   - 回滚:将所有规则默认设为 `allow`,等同关闭权限系统

3. **Plan/Build/Document 模式切换引入死锁**:Plan 模式下误调用编辑工具导致循环
   - 缓解:Doom loop 检测器会在 3 次后触发 `ask`
   - 回滚:在 AgentExecutor 中移除 `layer_agent_mode` 调用

4. **前端 UI 兼容性**:新弹窗可能影响既有确认流程
   - 缓解:保留 `ConfirmPayload` 结构,仅扩展字段
   - 回滚:恢复 `confirmHandler` 签名为 `(approved, feedback?)`

5. **Document 模式工具列表过滤失效**:非 Document 模式下文档 Handler 意外可见
   - 缓解:单元测试覆盖各模式下的工具列表;executor 构建工具定义后断言 Handler 存在/不存在
   - 回滚:在 build_tool_definitions 中移除模式过滤逻辑(所有 Handler 始终可见)

### 9.2 验收标准

- 所有 19 个任务(T2.01-T2.19)实施完成
- `cargo test` 全部通过(包括 7 个新增集成测试,含 Document 模式测试)
- `cargo clippy` 无警告
- `npx tsc -b` 无类型错误
- 手动测试:Plan 模式下编辑工具被拒绝并提示用户切换模式
- 手动测试:Build 模式下文档 Handler 不在工具列表
- 手动测试:Document 模式下文档 Handler 可用且编程工具正常
- 手动测试:连续 3 次相同工具调用触发 Doom loop 警告
- 手动测试:用户选择 "始终允许" 后相同操作不再弹窗

---

## 十、后续阶段衔接说明

本阶段完成后,后续阶段将基于权限系统进行扩展:

- **阶段 3(Skill 系统与上下文管理)**:Skill 工具将复用 `PermissionType::Skill` 进行权限控制
- **阶段 4(子 Agent 与高级工具)**:Task 工具将复用 `PermissionType::Task` 进行权限控制,子 Agent 继承父 Agent 的权限上下文
- **阶段 5(LSP 集成)**:LSP 工具默认 `allow`,但可通过规则配置为 `ask`

权限系统是后续所有阶段的基础设施,必须确保本阶段完全实施并通过验收后再进入下一阶段。