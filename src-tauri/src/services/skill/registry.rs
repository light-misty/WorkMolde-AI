use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::models::skill::{SkillInfo, SkillResult};

/// Skill trait，所有技能必须实现此接口
#[async_trait]
pub trait Skill: Send + Sync {
    /// 技能名称（唯一标识）
    fn skill_name(&self) -> &str;

    /// 技能描述
    fn description(&self) -> &str;

    /// 参数 JSON Schema
    fn parameters(&self) -> Value;

    /// 技能分类
    fn category(&self) -> &str {
        "document"
    }

    /// 是否为内置技能
    fn is_builtin(&self) -> bool {
        true
    }

    /// 支持的文档类型
    fn supported_types(&self) -> Vec<String> {
        vec![]
    }

    /// 执行技能
    async fn execute(&self, params: Value) -> SkillResult;
}

/// Skill 注册表
/// 使用 Arc<dyn Skill> 存储技能，允许在锁外执行技能，避免长时间持锁阻塞其他操作
pub struct SkillRegistry {
    skills: HashMap<String, Arc<dyn Skill>>,
    /// 已禁用的 Skill ID 集合
    disabled_skills: HashSet<String>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            disabled_skills: HashSet::new(),
        }
    }

    /// 使用已禁用列表初始化
    pub fn with_disabled_skills(mut self, disabled: Vec<String>) -> Self {
        self.disabled_skills = disabled.into_iter().collect();
        self
    }

    /// 注册技能
    pub fn register(&mut self, skill: Box<dyn Skill>) {
        let name = skill.skill_name().to_string();
        log::info!("注册技能: {}", name);
        // 将 Box<dyn Skill> 转为 Arc<dyn Skill>，支持在锁外克隆引用执行
        self.skills.insert(name.clone(), Arc::from(skill));
        log::debug!("技能注册完成: {}, 当前注册总数: {}", name, self.skills.len());
    }

    /// 获取技能的 Arc 引用（可在锁外使用）
    pub fn get_arc(&self, name: &str) -> Option<Arc<dyn Skill>> {
        self.skills.get(name).cloned()
    }

    /// 获取技能
    pub fn get(&self, name: &str) -> Option<&dyn Skill> {
        self.skills.get(name).map(|s| s.as_ref())
    }

    /// 执行技能（仅执行已启用的技能）
    pub async fn execute(&self, name: &str, params: Value) -> SkillResult {
        log::info!("执行技能: {}", name);
        if self.disabled_skills.contains(name) {
            log::warn!("技能已禁用: {}", name);
            return SkillResult {
                success: false,
                output: None,
                error: Some(format!("技能 '{}' 已禁用", name)),
                duration_ms: 0,
            };
        }
        match self.skills.get(name) {
            Some(skill) => {
                log::debug!("找到技能: {}, 开始执行", name);
                let result = skill.execute(params).await;
                if result.success {
                    log::info!("技能执行成功: {}, 耗时: {}ms", name, result.duration_ms);
                } else {
                    log::error!("技能执行失败: {}, 错误: {}", name, result.error.as_deref().unwrap_or("未知错误"));
                }
                result
            }
            None => {
                log::error!("技能不存在: {}", name);
                SkillResult {
                    success: false,
                    output: None,
                    error: Some(format!("技能不存在: {}", name)),
                    duration_ms: 0,
                }
            }
        }
    }

    /// 生成 OpenAI function calling 格式的工具定义（仅包含已启用的技能）
    pub fn tool_definitions(&self) -> Vec<Value> {
        log::debug!("生成工具定义, 技能总数: {}", self.skills.len());
        let definitions: Vec<Value> = self.skills.values()
            .filter(|skill| !self.disabled_skills.contains(skill.skill_name()))
            .map(|skill| {
                json!({
                    "type": "function",
                    "function": {
                        "name": skill.skill_name(),
                        "description": skill.description(),
                        "parameters": skill.parameters(),
                    }
                })
            }).collect();
        log::debug!("工具定义生成完成, 启用数量: {}", definitions.len());
        definitions
    }

    /// 列出所有技能信息（包含启用/禁用状态）
    pub fn list_skills(&self) -> Vec<SkillInfo> {
        self.skills.values().map(|skill| {
            let skill_id = skill.skill_name();
            SkillInfo {
                id: skill_id.to_string(),
                name: skill_id.to_string(),
                description: skill.description().to_string(),
                category: skill.category().to_string(),
                is_builtin: skill.is_builtin(),
                enabled: !self.disabled_skills.contains(skill_id),
                version: "1.0.0".to_string(),
                params_schema: Some(skill.parameters()),
                supported_types: skill.supported_types(),
            }
        }).collect()
    }

    /// 切换技能启用/禁用状态，返回更新后的禁用列表
    pub fn toggle_skill(&mut self, skill_id: &str, enabled: bool) -> Vec<String> {
        if enabled {
            self.disabled_skills.remove(skill_id);
            log::info!("技能已启用: {}", skill_id);
        } else {
            self.disabled_skills.insert(skill_id.to_string());
            log::info!("技能已禁用: {}", skill_id);
        }
        self.disabled_skills.iter().cloned().collect()
    }
}
