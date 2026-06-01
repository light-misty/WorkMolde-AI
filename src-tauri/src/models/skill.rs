use serde::{Deserialize, Serialize};

/// Skill 信息
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    /// "document" | "data" | "format"
    pub category: String,
    /// 是否为内置 Skill
    pub is_builtin: bool,
    /// 是否已启用
    pub enabled: bool,
    pub version: String,
    /// 参数 JSON Schema
    pub params_schema: Option<serde_json::Value>,
    /// 支持的文档类型
    pub supported_types: Vec<String>,
}

/// Skill 执行结果
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillResult {
    pub success: bool,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// Skill 展示信息
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    pub title: String,
    pub description: String,
    pub icon: Option<String>,
}
