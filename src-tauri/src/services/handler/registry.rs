use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::models::handler::{HandlerInfo, HandlerResult};

/// Handler trait，所有处理器必须实现此接口
#[async_trait]
pub trait Handler: Send + Sync {
    /// 处理器名称（唯一标识）
    fn handler_name(&self) -> &str;

    /// 处理器描述
    fn description(&self) -> &str;

    /// 参数 JSON Schema
    fn parameters(&self) -> Value;

    /// 处理器分类
    fn category(&self) -> &str {
        "document"
    }

    /// 是否为内置处理器
    fn is_builtin(&self) -> bool {
        true
    }

    /// 支持的文档类型
    fn supported_types(&self) -> Vec<String> {
        vec![]
    }

    /// 执行处理器
    async fn execute(&self, params: Value) -> HandlerResult;
}

/// Handler 注册表
/// 使用 Arc<dyn Handler> 存储处理器，允许在锁外执行处理器，避免长时间持锁阻塞其他操作
/// 内置处理器始终启用，不可禁用
pub struct HandlerRegistry {
    handlers: HashMap<String, Arc<dyn Handler>>,
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// 注册内置处理器
    pub fn register(&mut self, handler: Box<dyn Handler>) {
        let name = handler.handler_name().to_string();
        log::info!("注册处理器: {}", name);
        self.handlers.insert(name.clone(), Arc::from(handler));
        log::debug!("处理器注册完成: {}, 当前注册总数: {}", name, self.handlers.len());
    }

    /// 获取处理器的 Arc 引用（可在锁外使用）
    pub fn get_arc(&self, name: &str) -> Option<Arc<dyn Handler>> {
        self.handlers.get(name).cloned()
    }

    /// 获取处理器
    pub fn get(&self, name: &str) -> Option<&dyn Handler> {
        self.handlers.get(name).map(|s| s.as_ref())
    }

    /// 执行处理器
    pub async fn execute(&self, name: &str, params: Value) -> HandlerResult {
        log::info!("执行处理器: {}", name);
        match self.handlers.get(name) {
            Some(handler) => {
                log::debug!("找到处理器: {}, 开始执行", name);
                let result = handler.execute(params).await;
                if result.success {
                    log::info!("处理器执行成功: {}, 耗时: {}ms", name, result.duration_ms);
                } else {
                    log::error!("处理器执行失败: {}, 错误: {}", name, result.error.as_deref().unwrap_or("未知错误"));
                }
                result
            }
            None => {
                log::error!("处理器不存在: {}", name);
                HandlerResult {
                    success: false,
                    output: None,
                    error: Some(format!("处理器不存在: {}", name)),
                    duration_ms: 0,
                }
            }
        }
    }

    /// 生成 OpenAI function calling 格式的工具定义（包含所有已注册处理器）
    pub fn tool_definitions(&self) -> Vec<Value> {
        log::debug!("生成工具定义, 处理器总数: {}", self.handlers.len());
        let definitions: Vec<Value> = self.handlers.values()
            .map(|handler| {
                json!({
                    "type": "function",
                    "function": {
                        "name": handler.handler_name(),
                        "description": handler.description(),
                        "parameters": handler.parameters(),
                    }
                })
            }).collect();
        log::debug!("工具定义生成完成, 数量: {}", definitions.len());
        definitions
    }

    /// 列出所有处理器信息（内置处理器始终启用）
    pub fn list_handlers(&self) -> Vec<HandlerInfo> {
        self.handlers.values().map(|handler| {
            let handler_id = handler.handler_name();
            HandlerInfo {
                id: handler_id.to_string(),
                name: handler_id.to_string(),
                description: handler.description().to_string(),
                category: handler.category().to_string(),
                is_builtin: handler.is_builtin(),
                enabled: true,
                version: "1.0.0".to_string(),
                params_schema: Some(handler.parameters()),
                supported_types: handler.supported_types(),
            }
        }).collect()
    }

    /// 检查处理器是否存在
    pub fn contains_handler(&self, handler_id: &str) -> bool {
        self.handlers.contains_key(handler_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHandler { name: String }

    impl MockHandler {
        fn new(name: &str) -> Self { Self { name: name.to_string() } }
    }

    #[async_trait]
    impl Handler for MockHandler {
        fn handler_name(&self) -> &str { &self.name }
        fn description(&self) -> &str { "mock handler" }
        fn parameters(&self) -> Value { json!({"type": "object"}) }
        fn is_builtin(&self) -> bool { false }
        async fn execute(&self, _params: Value) -> crate::models::handler::HandlerResult {
            crate::models::handler::HandlerResult {
                success: true, output: None, error: None, duration_ms: 0,
            }
        }
    }

    #[test]
    fn test_register_and_list() {
        let mut registry = HandlerRegistry::new();
        registry.register(Box::new(MockHandler::new("test_handler")));
        let handlers = registry.list_handlers();
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].id, "test_handler");
        assert!(!handlers[0].is_builtin);
        // 内置处理器始终启用
        assert!(handlers[0].enabled);
    }

    #[test]
    fn test_all_handlers_in_tool_definitions() {
        let mut registry = HandlerRegistry::new();
        registry.register(Box::new(MockHandler::new("handler_a")));
        registry.register(Box::new(MockHandler::new("handler_b")));
        let defs = registry.tool_definitions();
        // 所有注册的处理器都应出现在工具定义中
        assert_eq!(defs.len(), 2);
    }
}
