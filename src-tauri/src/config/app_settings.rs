use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::errors::CommandError;

/// 确认级别
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub enum ConfirmationLevel {
    Always,
    #[default]
    EditOnly,
    Never,
}

/// 版本快照保留策略
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub enum RetentionPolicy {
    #[default]
    ByCount,
    ByDays,
    Both,
}

/// 主题模式
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

/// 外观设置
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    /// 主题模式：light / dark / system
    #[serde(default)]
    pub theme_mode: ThemeMode,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::default(),
        }
    }
}

/// 通用设置
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettings {
    #[serde(default)]
    pub author_name: String,
    /// 作者邮箱
    #[serde(default)]
    pub author_email: String,
    /// 作者公司/组织
    #[serde(default)]
    pub author_company: String,
    #[serde(default)]
    pub confirmation_level: ConfirmationLevel,
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_language() -> String {
    "zh-CN".to_string()
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            author_name: String::new(),
            author_email: String::new(),
            author_company: String::new(),
            confirmation_level: ConfirmationLevel::default(),
            language: default_language(),
        }
    }
}

/// 版本快照设置
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionSnapshot {
    #[serde(default)]
    pub retention_policy: RetentionPolicy,
    #[serde(default = "default_max_count")]
    pub max_count: u32,
    #[serde(default = "default_max_days")]
    pub max_days: u32,
}

fn default_max_count() -> u32 {
    50
}

fn default_max_days() -> u32 {
    30
}

impl Default for VersionSnapshot {
    fn default() -> Self {
        Self {
            retention_policy: RetentionPolicy::default(),
            max_count: default_max_count(),
            max_days: default_max_days(),
        }
    }
}

/// 工作区默认设置
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDefaults {
    #[serde(default)]
    pub default_workspace_id: String,
}

/// 快捷键设置
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Shortcuts {
    #[serde(default = "default_new_session")]
    pub new_session: String,
    #[serde(default = "default_close_session")]
    pub close_session: String,
    #[serde(default = "default_send_message")]
    pub send_message: String,
    #[serde(default = "default_toggle_sidebar")]
    pub toggle_sidebar: String,
    #[serde(default = "default_quick_prompt")]
    pub quick_prompt: String,
}

fn default_new_session() -> String {
    "Ctrl+N".to_string()
}
fn default_close_session() -> String {
    "Ctrl+W".to_string()
}
fn default_send_message() -> String {
    "Ctrl+Enter".to_string()
}
fn default_toggle_sidebar() -> String {
    "Ctrl+B".to_string()
}
fn default_quick_prompt() -> String {
    "Ctrl+/".to_string()
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            new_session: default_new_session(),
            close_session: default_close_session(),
            send_message: default_send_message(),
            toggle_sidebar: default_toggle_sidebar(),
            quick_prompt: default_quick_prompt(),
        }
    }
}

/// 更新通道
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Beta,
}

/// 更新设置
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettings {
    /// 更新通道：stable / beta
    #[serde(default)]
    pub channel: UpdateChannel,
    /// 是否自动检查更新
    #[serde(default = "default_auto_check")]
    pub auto_check: bool,
}

fn default_auto_check() -> bool {
    true
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            channel: UpdateChannel::default(),
            auto_check: default_auto_check(),
        }
    }
}

/// 应用设置，包含所有可配置项
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub general: GeneralSettings,
    #[serde(default)]
    pub appearance: AppearanceSettings,
    #[serde(default)]
    pub version_snapshot: VersionSnapshot,
    #[serde(default)]
    pub workspace: WorkspaceDefaults,
    #[serde(default)]
    pub shortcuts: Shortcuts,
    /// 已禁用的 Skill ID 列表
    #[serde(default)]
    pub disabled_skills: Vec<String>,
    /// 更新设置
    #[serde(default)]
    pub update: UpdateSettings,
}

/// 获取应用设置文件路径
fn config_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("config").join("app_settings.json")
}

/// 从磁盘加载应用设置，文件不存在时返回默认值
pub fn load_app_settings(data_dir: &Path) -> Result<AppSettings, CommandError> {
    let path = config_path(data_dir);
    if !path.exists() {
        log::info!("应用设置文件不存在，返回默认值: {}", path.display());
        return Ok(AppSettings::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let settings: AppSettings = serde_json::from_str(&content)?;
    log::info!("已加载应用设置: {}", path.display());
    // 合并默认值以确保新增字段有值
    Ok(merge_with_defaults(&settings, &AppSettings::default()))
}

/// 将应用设置保存到磁盘
pub fn save_app_settings(data_dir: &Path, settings: &AppSettings) -> Result<(), CommandError> {
    let path = config_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(settings)?;
    std::fs::write(&path, content)?;
    log::info!("已保存应用设置: {}", path.display());
    Ok(())
}

/// 将用户设置与默认设置合并，空值字段使用默认值填充
/// 主要用于版本升级后新增字段的补全
pub fn merge_with_defaults(
    user_settings: &AppSettings,
    default_settings: &AppSettings,
) -> AppSettings {
    AppSettings {
        general: GeneralSettings {
            author_name: if user_settings.general.author_name.is_empty() {
                default_settings.general.author_name.clone()
            } else {
                user_settings.general.author_name.clone()
            },
            author_email: if user_settings.general.author_email.is_empty() {
                default_settings.general.author_email.clone()
            } else {
                user_settings.general.author_email.clone()
            },
            author_company: if user_settings.general.author_company.is_empty() {
                default_settings.general.author_company.clone()
            } else {
                user_settings.general.author_company.clone()
            },
            confirmation_level: user_settings.general.confirmation_level.clone(),
            language: if user_settings.general.language.is_empty() {
                default_settings.general.language.clone()
            } else {
                user_settings.general.language.clone()
            },
        },
        appearance: AppearanceSettings {
            theme_mode: user_settings.appearance.theme_mode.clone(),
        },
        version_snapshot: VersionSnapshot {
            retention_policy: user_settings.version_snapshot.retention_policy.clone(),
            max_count: if user_settings.version_snapshot.max_count == 0 {
                default_settings.version_snapshot.max_count
            } else {
                user_settings.version_snapshot.max_count
            },
            max_days: if user_settings.version_snapshot.max_days == 0 {
                default_settings.version_snapshot.max_days
            } else {
                user_settings.version_snapshot.max_days
            },
        },
        workspace: WorkspaceDefaults {
            default_workspace_id: if user_settings.workspace.default_workspace_id.is_empty() {
                default_settings.workspace.default_workspace_id.clone()
            } else {
                user_settings.workspace.default_workspace_id.clone()
            },
        },
        shortcuts: Shortcuts {
            new_session: if user_settings.shortcuts.new_session.is_empty() {
                default_settings.shortcuts.new_session.clone()
            } else {
                user_settings.shortcuts.new_session.clone()
            },
            close_session: if user_settings.shortcuts.close_session.is_empty() {
                default_settings.shortcuts.close_session.clone()
            } else {
                user_settings.shortcuts.close_session.clone()
            },
            send_message: if user_settings.shortcuts.send_message.is_empty() {
                default_settings.shortcuts.send_message.clone()
            } else {
                user_settings.shortcuts.send_message.clone()
            },
            toggle_sidebar: if user_settings.shortcuts.toggle_sidebar.is_empty() {
                default_settings.shortcuts.toggle_sidebar.clone()
            } else {
                user_settings.shortcuts.toggle_sidebar.clone()
            },
            quick_prompt: if user_settings.shortcuts.quick_prompt.is_empty() {
                default_settings.shortcuts.quick_prompt.clone()
            } else {
                user_settings.shortcuts.quick_prompt.clone()
            },
        },
        disabled_skills: user_settings.disabled_skills.clone(),
        update: UpdateSettings {
            channel: user_settings.update.channel.clone(),
            auto_check: user_settings.update.auto_check,
        },
    }
}
