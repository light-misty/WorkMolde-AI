pub mod branch_repo;
pub mod init;
pub mod message_repo;
pub mod permission_repo;
pub mod session_repo;
pub mod session_summary_repo;
pub mod skill_repo;
pub mod snapshot_repo;
pub mod sub_agent_message_repo;
pub mod template_repo;
pub mod todo_repo;
pub mod user_preference_repo;

use crate::errors::CommandError;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

/// 数据库封装，内部持有 Mutex 保护的 SQLite 连接
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// 创建或打开数据库文件，启用 WAL 模式和外键约束，并执行初始化
    pub fn new(db_path: &Path) -> Result<Self, CommandError> {
        log::info!("创建/打开数据库，路径: {}", db_path.display());

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
            log::debug!("已创建数据库目录: {}", parent.display());
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        log::debug!("已启用 WAL 模式和外键约束");

        let db = Self {
            conn: Mutex::new(conn),
        };
        db.initialize()?;

        log::info!("数据库创建/打开成功: {}", db_path.display());
        Ok(db)
    }

    /// 执行数据库初始化（建表、索引、版本记录）
    fn initialize(&self) -> Result<(), CommandError> {
        log::info!("开始执行数据库初始化");

        let conn = self.conn.lock().map_err(|e| {
            log::error!("获取数据库连接失败: {}", e);
            CommandError::db(crate::errors::DB_CONNECTION_FAILED, e.to_string())
        })?;
        init::initialize_database(&conn)?;

        log::info!("数据库初始化完成");
        Ok(())
    }

    /// 获取 MutexGuard 保护下的数据库连接
    pub fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, CommandError> {
        self.conn.lock().map_err(|e| {
            log::error!("获取数据库连接失败: {}", e);
            CommandError::db(crate::errors::DB_CONNECTION_FAILED, e.to_string())
        })
    }
}
