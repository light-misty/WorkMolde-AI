use crate::models::llm::{ChatMessage, LlmToolCall};

/// Agent 执行上下文
/// 管理对话历史和系统提示词
pub struct AgentContext {
    /// 会话 ID
    pub session_id: String,
    /// 对话消息历史
    pub messages: Vec<ChatMessage>,
    /// 系统提示词
    pub system_prompt: String,
    /// 最大迭代次数
    pub max_iterations: u32,
    /// 已持久化的消息数量，用于增量持久化
    persisted_count: usize,
}

impl AgentContext {
    pub fn new(session_id: String, system_prompt: String) -> Self {
        Self {
            session_id,
            messages: Vec::new(),
            system_prompt,
            max_iterations: 20,
            persisted_count: 0,
        }
    }

    /// 添加用户消息
    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    /// 添加助手消息
    pub fn add_assistant_message(&mut self, content: &str, tool_calls: Option<Vec<LlmToolCall>>) {
        self.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: content.to_string(),
            tool_calls,
            tool_call_id: None,
        });
    }

    /// 添加工具执行结果消息
    pub fn add_tool_result(&mut self, call_id: &str, content: &str) {
        self.messages.push(ChatMessage {
            role: "tool".to_string(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: Some(call_id.to_string()),
        });
    }

    /// 获取包含系统提示词的完整消息列表
    pub fn get_messages(&self) -> Vec<ChatMessage> {
        let mut all = vec![ChatMessage {
            role: "system".to_string(),
            content: self.system_prompt.clone(),
            tool_calls: None,
            tool_call_id: None,
        }];
        all.extend(self.messages.clone());
        all
    }

    /// 获取尚未持久化的消息列表（增量持久化用）
    /// 返回从 persisted_count 开始的新消息切片
    pub fn get_unpersisted_messages(&self) -> &[ChatMessage] {
        &self.messages[self.persisted_count..]
    }

    /// 标记当前所有消息为已持久化
    pub fn mark_persisted(&mut self) {
        self.persisted_count = self.messages.len();
    }

    /// 构建系统提示词
    pub fn build_system_prompt(workspace_path: &str) -> String {
        format!(
            "你是 DocAgent，一个专业的 AI 文档处理助手。\n\
            \n\
            你的职责是帮助用户处理各种文档操作，包括：\n\
            - 生成新文档（Word、Excel、PPT、PDF、Markdown）\n\
            - 读取和分析文档内容\n\
            - 修改已有文档\n\
            - 转换文档格式\n\
            - 搜索和管理工作区文件\n\
            \n\
            当前工作区路径: {}\n\
            \n\
            工作原则：\n\
            1. 在执行任何修改操作前，先确认用户的意图\n\
            2. 对于重要操作（如删除、覆盖），需要明确提醒用户\n\
            3. 优先使用工具完成任务，而不是仅提供建议\n\
            4. 如果操作可能造成数据丢失，先创建版本快照\n\
            5. 使用中文与用户交流",
            workspace_path
        )
    }
}
