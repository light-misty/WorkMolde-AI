use serde::{Deserialize, Serialize};

// ============================================================
// LLM 相关错误码 (1000-1999)
// ============================================================
pub const LLM_CONNECTION_FAILED: u32 = 1001;
pub const LLM_AUTH_FAILED: u32 = 1002;
pub const LLM_RATE_LIMITED: u32 = 1003;
pub const LLM_QUOTA_EXCEEDED: u32 = 1004;
pub const LLM_MODEL_NOT_FOUND: u32 = 1005;
pub const LLM_TIMEOUT: u32 = 1006;
pub const LLM_INVALID_REQUEST: u32 = 1007;
pub const LLM_STREAM_ERROR: u32 = 1008;
pub const LLM_PROVIDER_UNAVAILABLE: u32 = 1009;
pub const LLM_RESPONSE_PARSE_ERROR: u32 = 1010;
/// DNS解析失败
pub const LLM_DNS_RESOLVE_FAILED: u32 = 1011;
/// 连接被拒绝
pub const LLM_CONNECTION_REFUSED: u32 = 1012;
/// SSL/TLS握手失败
pub const LLM_SSL_ERROR: u32 = 1013;
/// 网络不可达
pub const LLM_NETWORK_UNREACHABLE: u32 = 1014;

// ============================================================
// Agent 相关错误码 (2000-2999)
// ============================================================
pub const AGENT_ALREADY_RUNNING: u32 = 2001;
pub const AGENT_NOT_RUNNING: u32 = 2002;
pub const AGENT_MAX_ITERATIONS: u32 = 2003;
pub const AGENT_CONFIRMATION_TIMEOUT: u32 = 2004;
pub const AGENT_OPERATION_REJECTED: u32 = 2005;
pub const AGENT_HANDLER_NOT_FOUND: u32 = 2006;
pub const AGENT_EXECUTION_ERROR: u32 = 2008;
pub const AGENT_SESSION_NOT_FOUND: u32 = 2010;

// ============================================================
// 文档处理错误码 (3000-3999)
// ============================================================
pub const DOC_FILE_NOT_FOUND: u32 = 3001;
pub const DOC_FORMAT_UNSUPPORTED: u32 = 3002;
pub const DOC_PARSE_ERROR: u32 = 3003;
pub const DOC_WRITE_ERROR: u32 = 3004;
pub const DOC_CONVERT_ERROR: u32 = 3005;
pub const DOC_TEMPLATE_NOT_FOUND: u32 = 3006;
pub const DOC_TEMPLATE_ERROR: u32 = 3007;
pub const DOC_VERSION_NOT_FOUND: u32 = 3008;
pub const DOC_ROLLBACK_FAILED: u32 = 3009;
pub const DOC_SIDECAR_ERROR: u32 = 3010;
pub const DOC_PERMISSION_DENIED: u32 = 3011;
pub const DOC_FILE_TOO_LARGE: u32 = 3012;

// ============================================================
// 数据库错误码 (4000-4999)
// ============================================================
pub const DB_CONNECTION_FAILED: u32 = 4001;
pub const DB_QUERY_FAILED: u32 = 4002;
pub const DB_RECORD_NOT_FOUND: u32 = 4003;
pub const DB_RECORD_EXISTS: u32 = 4004;
pub const DB_CONSTRAINT_VIOLATION: u32 = 4005;
pub const DB_MIGRATION_FAILED: u32 = 4006;
pub const DB_CORRUPTED: u32 = 4007;

// ============================================================
// 配置错误码 (5000-5999)
// ============================================================
pub const CONFIG_INVALID_FORMAT: u32 = 5001;
pub const CONFIG_MISSING_FIELD: u32 = 5002;
pub const CONFIG_INVALID_VALUE: u32 = 5003;
pub const CONFIG_IMPORT_FAILED: u32 = 5004;
pub const CONFIG_EXPORT_FAILED: u32 = 5005;
pub const CONFIG_PROVIDER_NOT_FOUND: u32 = 5006;
pub const CONFIG_WORKSPACE_PATH_EXISTS: u32 = 5008;

// ============================================================
// 文件系统错误码 (6000-6999)
// ============================================================
pub const FS_PATH_NOT_FOUND: u32 = 6001;
pub const FS_PERMISSION_DENIED: u32 = 6002;
pub const FS_ALREADY_EXISTS: u32 = 6003;
pub const FS_NOT_A_DIRECTORY: u32 = 6004;
pub const FS_DISK_FULL: u32 = 6005;
pub const FS_IO_ERROR: u32 = 6006;
pub const FS_WATCH_ERROR: u32 = 6007;
pub const FS_ENCODING_ERROR: u32 = 6008;

// ============================================================
// 运行时错误码 (7000-7999)
// ============================================================
pub const RUNTIME_EVENT_EMIT_ERROR: u32 = 7001;

// ============================================================
// 更新相关错误码 (8000-8999)
// ============================================================
/// 更新检查失败
pub const UPDATE_CHECK_FAILED: u32 = 8001;
/// 更新下载失败
pub const UPDATE_DOWNLOAD_FAILED: u32 = 8002;
/// 更新安装失败
pub const UPDATE_INSTALL_FAILED: u32 = 8003;
/// 没有可用更新
pub const UPDATE_NO_UPDATE_AVAILABLE: u32 = 8004;
/// 更新网络错误
pub const UPDATE_NETWORK_ERROR: u32 = 8005;

// ============================================================
// Tool 相关错误码 (9000-9999)
// ============================================================
/// 工具不存在
pub const TOOL_NOT_FOUND: u32 = 9001;
/// 工具参数无效
pub const TOOL_INVALID_PARAMS: u32 = 9002;
/// 工具执行失败
pub const TOOL_EXECUTION_ERROR: u32 = 9003;
/// 工具路径越界
pub const TOOL_PATH_OUT_OF_BOUNDS: u32 = 9004;

/// 统一命令错误类型，所有 Tauri 命令的错误均通过此结构体返回
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommandError {
    /// 错误码，参见错误码常量定义
    pub code: u32,
    /// 人类可读的错误描述
    pub message: String,
}

impl CommandError {
    /// 创建新的错误实例
    pub fn new(code: u32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// 快捷创建 LLM 错误
    pub fn llm(code: u32, message: impl Into<String>) -> Self {
        Self::new(code, message)
    }

    /// 快捷创建 Agent 错误
    pub fn agent(code: u32, message: impl Into<String>) -> Self {
        Self::new(code, message)
    }

    /// 快捷创建文档处理错误
    pub fn doc(code: u32, message: impl Into<String>) -> Self {
        Self::new(code, message)
    }

    /// 快捷创建数据库错误
    pub fn db(code: u32, message: impl Into<String>) -> Self {
        Self::new(code, message)
    }

    /// 快捷创建配置错误
    pub fn config(code: u32, message: impl Into<String>) -> Self {
        Self::new(code, message)
    }

    /// 快捷创建文件系统错误
    pub fn fs(code: u32, message: impl Into<String>) -> Self {
        Self::new(code, message)
    }

    /// 快捷创建更新错误
    pub fn update(code: u32, message: impl Into<String>) -> Self {
        Self::new(code, message)
    }

    /// 快捷创建工具错误
    pub fn tool(code: u32, message: impl Into<String>) -> Self {
        Self::new(code, message)
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[E{}] {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

impl From<rusqlite::Error> for CommandError {
    fn from(err: rusqlite::Error) -> Self {
        let code = match &err {
            rusqlite::Error::QueryReturnedNoRows => DB_RECORD_NOT_FOUND,
            _ => DB_QUERY_FAILED,
        };
        Self::new(code, err.to_string())
    }
}

impl From<reqwest::Error> for CommandError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::new(LLM_TIMEOUT, err.to_string())
        } else if err.is_connect() {
            // 连接错误：使用细化分类（DNS/连接被拒绝/SSL/网络不可达）
            let (code, msg) = crate::services::llm::provider::classify_connection_error(&err);
            Self::new(code, msg)
        } else if err.is_status() {
            let code = match err.status() {
                Some(status) if status.as_u16() == 401 => LLM_AUTH_FAILED,
                Some(status) if status.as_u16() == 429 => LLM_RATE_LIMITED,
                Some(status) if status.as_u16() == 404 => LLM_MODEL_NOT_FOUND,
                _ => LLM_INVALID_REQUEST,
            };
            Self::new(code, err.to_string())
        } else {
            Self::new(LLM_CONNECTION_FAILED, err.to_string())
        }
    }
}

impl From<serde_json::Error> for CommandError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(CONFIG_INVALID_FORMAT, err.to_string())
    }
}

impl From<std::io::Error> for CommandError {
    fn from(err: std::io::Error) -> Self {
        let code = match err.kind() {
            std::io::ErrorKind::NotFound => FS_PATH_NOT_FOUND,
            std::io::ErrorKind::PermissionDenied => FS_PERMISSION_DENIED,
            std::io::ErrorKind::AlreadyExists => FS_ALREADY_EXISTS,
            _ => FS_IO_ERROR,
        };
        Self::new(code, err.to_string())
    }
}

impl From<tauri::Error> for CommandError {
    fn from(err: tauri::Error) -> Self {
        Self::new(RUNTIME_EVENT_EMIT_ERROR, err.to_string())
    }
}
