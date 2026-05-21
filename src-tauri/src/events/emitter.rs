use tauri::{AppHandle, Emitter, Runtime};

use crate::errors::CommandError;
use super::types;

/// Agent 事件发射器，封装 Tauri 事件发送逻辑
pub struct AgentEmitter<R: Runtime> {
    app_handle: AppHandle<R>,
}

impl<R: Runtime> Clone for AgentEmitter<R> {
    fn clone(&self) -> Self {
        Self {
            app_handle: self.app_handle.clone(),
        }
    }
}

impl<R: Runtime> AgentEmitter<R> {
    /// 创建事件发射器实例
    pub fn new(app_handle: AppHandle<R>) -> Self {
        Self { app_handle }
    }

    /// 发射 Agent 思考链增量事件
    pub fn emit_thinking(&self, payload: types::ThinkingPayload) -> Result<(), CommandError> {
        log::debug!("发射事件: {} (session_id={})", types::AGENT_THINKING, payload.session_id);
        self.app_handle
            .emit(types::AGENT_THINKING, payload)
            .map_err(|e| {
                log::warn!("发射事件 {} 失败: {}", types::AGENT_THINKING, e);
                CommandError::from(e)
            })
    }

    /// 发射 Agent 回复内容增量事件
    pub fn emit_content(&self, payload: types::ContentPayload) -> Result<(), CommandError> {
        log::debug!("发射事件: {} (session_id={})", types::AGENT_CONTENT, payload.session_id);
        self.app_handle
            .emit(types::AGENT_CONTENT, payload)
            .map_err(|e| {
                log::warn!("发射事件 {} 失败: {}", types::AGENT_CONTENT, e);
                CommandError::from(e)
            })
    }

    /// 发射 Tool 调用开始事件
    pub fn emit_tool_call(&self, payload: types::ToolCallPayload) -> Result<(), CommandError> {
        log::debug!("发射事件: {} (session_id={})", types::AGENT_TOOL_CALL, payload.session_id);
        self.app_handle
            .emit(types::AGENT_TOOL_CALL, payload)
            .map_err(|e| {
                log::warn!("发射事件 {} 失败: {}", types::AGENT_TOOL_CALL, e);
                CommandError::from(e)
            })
    }

    /// 发射 Tool 执行结果事件
    pub fn emit_tool_result(&self, payload: types::ToolResultPayload) -> Result<(), CommandError> {
        log::debug!("发射事件: {} (session_id={})", types::AGENT_TOOL_RESULT, payload.session_id);
        self.app_handle
            .emit(types::AGENT_TOOL_RESULT, payload)
            .map_err(|e| {
                log::warn!("发射事件 {} 失败: {}", types::AGENT_TOOL_RESULT, e);
                CommandError::from(e)
            })
    }

    /// 发射需要用户确认的事件
    pub fn emit_confirm(&self, payload: types::ConfirmPayload) -> Result<(), CommandError> {
        log::debug!("发射事件: {} (session_id={})", types::AGENT_CONFIRM, payload.session_id);
        self.app_handle
            .emit(types::AGENT_CONFIRM, payload)
            .map_err(|e| {
                log::warn!("发射事件 {} 失败: {}", types::AGENT_CONFIRM, e);
                CommandError::from(e)
            })
    }

    /// 发射 Todo 列表更新事件
    pub fn emit_todo_update(&self, payload: types::TodoUpdatePayload) -> Result<(), CommandError> {
        log::debug!("发射事件: {} (session_id={})", types::AGENT_TODO_UPDATE, payload.session_id);
        self.app_handle
            .emit(types::AGENT_TODO_UPDATE, payload)
            .map_err(|e| {
                log::warn!("发射事件 {} 失败: {}", types::AGENT_TODO_UPDATE, e);
                CommandError::from(e)
            })
    }

    /// 发射 Agent 执行完成事件
    pub fn emit_done(&self, payload: types::DonePayload) -> Result<(), CommandError> {
        log::debug!("发射事件: {} (session_id={})", types::AGENT_DONE, payload.session_id);
        self.app_handle
            .emit(types::AGENT_DONE, payload)
            .map_err(|e| {
                log::warn!("发射事件 {} 失败: {}", types::AGENT_DONE, e);
                CommandError::from(e)
            })
    }

    /// 发射 Agent 执行错误事件
    pub fn emit_error(&self, payload: types::ErrorPayload) -> Result<(), CommandError> {
        log::debug!("发射事件: {} (session_id={})", types::AGENT_ERROR, payload.session_id);
        self.app_handle
            .emit(types::AGENT_ERROR, payload)
            .map_err(|e| {
                log::warn!("发射事件 {} 失败: {}", types::AGENT_ERROR, e);
                CommandError::from(e)
            })
    }

    /// 发射 Agent 执行中断事件
    pub fn emit_stopped(&self, payload: types::StoppedPayload) -> Result<(), CommandError> {
        log::debug!("发射事件: {} (session_id={})", types::AGENT_STOPPED, payload.session_id);
        self.app_handle
            .emit(types::AGENT_STOPPED, payload)
            .map_err(|e| {
                log::warn!("发射事件 {} 失败: {}", types::AGENT_STOPPED, e);
                CommandError::from(e)
            })
    }

    /// 发射 Token 用量更新事件
    pub fn emit_token_update(&self, payload: types::TokenUpdatePayload) -> Result<(), CommandError> {
        log::debug!("发射事件: {} (session_id={})", types::TOKEN_UPDATE, payload.session_id);
        self.app_handle
            .emit(types::TOKEN_UPDATE, payload)
            .map_err(|e| {
                log::warn!("发射事件 {} 失败: {}", types::TOKEN_UPDATE, e);
                CommandError::from(e)
            })
    }

    /// 发射会话更新事件
    pub fn emit_session_updated(&self, payload: types::SessionUpdatePayload) -> Result<(), CommandError> {
        log::debug!("发射事件: {} (session_id={}, change_type={})", types::SESSION_UPDATED, payload.session_id, payload.change_type);
        self.app_handle
            .emit(types::SESSION_UPDATED, payload)
            .map_err(|e| {
                log::warn!("发射事件 {} 失败: {}", types::SESSION_UPDATED, e);
                CommandError::from(e)
            })
    }

    /// 发射工作区变更事件
    pub fn emit_workspace_change(&self, payload: types::WorkspaceChangePayload) -> Result<(), CommandError> {
        log::debug!("发射事件: {} (workspace_id={})", types::WORKSPACE_CHANGE, payload.workspace_id);
        self.app_handle
            .emit(types::WORKSPACE_CHANGE, payload)
            .map_err(|e| {
                log::warn!("发射事件 {} 失败: {}", types::WORKSPACE_CHANGE, e);
                CommandError::from(e)
            })
    }

    /// 发射 LLM Provider 切换通知事件
    pub fn emit_provider_switch(&self, payload: types::ProviderSwitchPayload) -> Result<(), CommandError> {
        log::debug!(
            "发射事件: {} (from={}, to={})",
            types::LLM_PROVIDER_SWITCH,
            payload.from_provider_id,
            payload.to_provider_id
        );
        self.app_handle
            .emit(types::LLM_PROVIDER_SWITCH, payload)
            .map_err(|e| {
                log::warn!("发射事件 {} 失败: {}", types::LLM_PROVIDER_SWITCH, e);
                CommandError::from(e)
            })
    }
}
