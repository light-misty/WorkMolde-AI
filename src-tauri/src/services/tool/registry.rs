use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::models::tool::ToolInfo;
use super::trait_def::Tool;

/// Tool 注册表
/// 工具在运行时不会增删，不需要 Mutex 保护
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// 注册工具
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.tool_name().to_string();
        log::info!("注册工具: {}", name);
        self.tools.insert(name.clone(), Arc::from(tool));
        log::debug!("工具注册完成: {}, 当前注册总数: {}", name, self.tools.len());
    }

    /// 获取工具的 Arc 引用
    pub fn get_arc(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// 生成 OpenAI function calling 格式的工具定义
    pub fn tool_definitions(&self) -> Vec<Value> {
        log::debug!("生成工具定义, 工具总数: {}", self.tools.len());
        let definitions: Vec<Value> = self.tools.values()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.tool_name(),
                        "description": tool.description(),
                        "parameters": tool.parameters(),
                    }
                })
            }).collect();
        log::debug!("工具定义生成完成, 数量: {}", definitions.len());
        definitions
    }

    /// 列出所有工具信息
    pub fn list_tools(&self) -> Vec<ToolInfo> {
        self.tools.values().map(|tool| {
            ToolInfo {
                id: tool.tool_name().to_string(),
                name: tool.tool_name().to_string(),
                description: tool.description().to_string(),
                category: tool.category().to_string(),
                is_builtin: true,
                enabled: true,
                version: "1.0.0".to_string(),
                params_schema: Some(tool.parameters()),
            }
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct MockTool {
        name: String,
        desc: String,
    }

    impl MockTool {
        fn new(name: &str, desc: &str) -> Self {
            Self { name: name.to_string(), desc: desc.to_string() }
        }
    }

    #[async_trait]
    impl Tool for MockTool {
        fn tool_name(&self) -> &str { &self.name }
        fn description(&self) -> &str { &self.desc }
        fn parameters(&self) -> Value { json!({"type": "object"}) }
        async fn execute(&self, _params: Value) -> crate::models::tool::ToolResult {
            crate::models::tool::ToolResult {
                success: true,
                output: None,
                error: None,
                duration_ms: 0, error_code: None,
            }
        }
    }

    #[test]
    fn test_empty_registry() {
        let registry = ToolRegistry::new();
        assert_eq!(registry.list_tools().len(), 0);
        assert_eq!(registry.tool_definitions().len(), 0);
        assert!(registry.get_arc("nonexistent").is_none());
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("test_tool", "测试工具")));

        assert!(registry.get_arc("test_tool").is_some());
        assert!(registry.get_arc("nonexistent").is_none());
        assert_eq!(registry.list_tools().len(), 1);
    }

    #[test]
    fn test_tool_definitions_format() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("my_tool", "我的工具")));

        let defs = registry.tool_definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0]["type"], "function");
        assert_eq!(defs[0]["function"]["name"], "my_tool");
        assert_eq!(defs[0]["function"]["description"], "我的工具");
    }

    #[tokio::test]
    async fn test_mock_tool_execute() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(MockTool::new("mock", "模拟工具")));

        let tool = registry.get_arc("mock").unwrap();
        let result = tool.execute(json!({})).await;
        assert!(result.success);
    }
}
