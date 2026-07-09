pub mod context;
pub mod executor;
pub mod prompts;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

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
