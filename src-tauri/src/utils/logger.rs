//! 日志系统模块
//!
//! 基于 log crate + tracing_appender 实现专业日志功能：
//! - 每次启动生成带时间戳的独立日志文件（samoyed_work_YYYYMMDD_HHMMSS.log）
//! - 保留 7 天历史日志，启动时自动清理过期文件
//! - 文件输出使用 tracing_appender::non_blocking 异步写入，避免 I/O 阻塞主线程
//! - stderr 同步输出，保持双输出行为
//! - 支持 RUST_LOG 环境变量覆盖日志级别
//! - 保持现有格式：{本地时间} [{级别5s}] {模块} - {消息}

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use chrono::Local;
use tracing_appender::non_blocking::WorkerGuard;

/// 日志保留天数：超过此天数的日志文件会被自动清理
const LOG_RETENTION_DAYS: u64 = 7;

/// 全局存储当前活跃的日志文件路径（供 commands/log.rs 读取）
/// 每次启动时在 init 中设置，程序生命周期内不变
static CURRENT_LOG_FILE: OnceLock<PathBuf> = OnceLock::new();

/// 全局存储当前日志目录路径（供 SidecarManager 读取，传递给 Python sidecar）
static CURRENT_LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 全局存储 WorkerGuard，确保程序退出时 flush non_blocking 缓冲区
/// OnceLock 在程序退出时自动 drop，触发 WorkerGuard::drop 执行 flush
static WORKER_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// 获取当前活跃的日志文件路径（供 commands/log.rs 读取）
/// 返回 None 表示日志系统未初始化或降级为仅控制台输出
pub fn current_log_file() -> Option<&'static Path> {
    CURRENT_LOG_FILE.get().map(|p| p.as_path())
}

/// 获取当前日志目录路径（供 SidecarManager 读取，传递给 Python sidecar 作为 SAMOYED_WORK_LOG_DIR）
/// 返回 None 表示日志系统未初始化
pub fn current_log_dir() -> Option<&'static Path> {
    CURRENT_LOG_DIR.get().map(|p| p.as_path())
}

/// 计算日志文件目录路径
///
/// 开发模式：使用项目根目录下的 `log/` 子目录，与 Python Sidecar 保持一致
/// 生产模式：使用 Tauri 推荐的系统日志目录（Windows: %LOCALAPPDATA%\<identifier>\logs\）
pub fn resolve_log_dir(app_log_dir: Option<PathBuf>, app_data_dir: Option<PathBuf>) -> PathBuf {
    if cfg!(debug_assertions) {
        // 开发模式：基于 CARGO_MANIFEST_DIR 推导项目根目录，使用其 log/ 子目录
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| Path::new("."));
        project_root.join("log")
    } else {
        // 生产模式：使用 Tauri 推荐的系统日志目录
        app_log_dir
            .or_else(|| app_data_dir.map(|d| d.join("log")))
            .unwrap_or_else(|| PathBuf::from("log"))
    }
}

/// 初始化日志系统
///
/// - `log_dir`: 日志文件目录路径
/// - 每次启动生成带时间戳的独立日志文件（samoyed_work_YYYYMMDD_HHMMSS.log），不覆盖历史日志
/// - 开发模式默认 DEBUG，生产模式默认 INFO，均支持 RUST_LOG 环境变量覆盖
/// - 文件输出使用 non_blocking 异步写入，stderr 同步输出
/// - 如果日志文件创建失败，降级为仅 stderr 输出
/// - 启动时自动清理超过 7 天的过期日志文件
pub fn init(log_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // 确保日志目录存在
    fs::create_dir_all(log_dir)?;

    // 记录日志目录到全局状态（供 SidecarManager 读取）
    let _ = CURRENT_LOG_DIR.set(log_dir.to_path_buf());

    // 清理过期日志文件（失败不影响启动）
    cleanup_old_logs(log_dir, LOG_RETENTION_DAYS);

    // 生成带启动时间戳的日志文件名
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let file_name = format!("samoyed_work_{}.log", timestamp);
    let log_path = log_dir.join(&file_name);

    // 解析日志级别：优先 RUST_LOG 环境变量，回退到编译模式默认值
    let level_filter = resolve_log_level();

    // 尝试创建日志文件
    let file = match OpenOptions::new().create(true).append(true).open(&log_path) {
        Ok(f) => f,
        Err(e) => {
            // 文件创建失败，降级为仅 stderr 输出
            eprintln!("[日志] 无法创建日志文件，降级为仅控制台输出: {}", e);
            init_stderr_only(level_filter)?;
            log::info!("Samoyed Work 日志系统初始化完成（仅控制台输出模式）");
            return Ok(());
        }
    };

    // 用 non_blocking 包裹文件，实现异步写入（避免 I/O 阻塞主线程）
    let (non_blocking, guard) = tracing_appender::non_blocking(file);

    // 存储 WorkerGuard 到全局，确保程序退出时 flush 缓冲日志
    let _ = WORKER_GUARD.set(guard);

    // 记录当前活跃日志文件路径到全局状态
    let _ = CURRENT_LOG_FILE.set(log_path.clone());

    // 注册全局 logger（含文件输出）
    register_logger(Some(Mutex::new(non_blocking)), level_filter)?;

    log::info!(
        "Samoyed Work 日志系统初始化完成，日志文件: {}",
        log_path.display()
    );

    Ok(())
}

/// 仅初始化 stderr 输出（日志文件创建失败时的降级方案）
fn init_stderr_only(level_filter: log::LevelFilter) -> Result<(), Box<dyn std::error::Error>> {
    register_logger(None, level_filter)
}

/// 注册全局 logger（提取 init 与 init_stderr_only 的公共逻辑）
fn register_logger(
    file: Option<Mutex<tracing_appender::non_blocking::NonBlocking>>,
    level_filter: log::LevelFilter,
) -> Result<(), Box<dyn std::error::Error>> {
    log::set_boxed_logger(Box::new(DualLogger { file, level_filter }))?;
    log::set_max_level(level_filter);
    Ok(())
}

/// 解析日志级别
///
/// 优先级：
/// 1. RUST_LOG 环境变量（支持 debug/info/warn/error/trace/off）
/// 2. 编译模式默认值（开发 DEBUG / 生产 INFO）
fn resolve_log_level() -> log::LevelFilter {
    let default_level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    std::env::var("RUST_LOG")
        .ok()
        .and_then(|s| s.parse::<log::LevelFilter>().ok())
        .unwrap_or(default_level)
}

/// 清理过期日志文件
///
/// 扫描日志目录，删除修改时间超过 max_days 天的日志文件
/// 匹配前缀：samoyed_work_*.log（当前版本）、sidecar_*.log（Python Sidecar）
/// 错误静默处理（清理失败不影响应用启动）
fn cleanup_old_logs(log_dir: &Path, max_days: u64) {
    let now = SystemTime::now();
    let max_age = Duration::from_secs(max_days * 86400);

    let entries = match fs::read_dir(log_dir) {
        Ok(e) => e,
        Err(_) => return, // 目录读取失败，静默跳过
    };

    let mut cleaned_count = 0u32;
    for entry in entries.flatten() {
        let path = entry.path();

        // 匹配日志文件（samoyed_work_*.log 和 sidecar_*.log）
        let is_log_file = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|name| {
                (name.starts_with("samoyed_work_") || name.starts_with("sidecar_"))
                    && name.ends_with(".log")
            })
            .unwrap_or(false);

        if !is_log_file {
            continue;
        }

        // 获取文件修改时间，超过 max_days 则删除
        let should_delete = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|modified| now.duration_since(modified).ok())
            .map(|age| age > max_age)
            .unwrap_or(false);

        if should_delete {
            if let Err(e) = fs::remove_file(&path) {
                // 删除失败仅 stderr 提示，不影响启动
                eprintln!("[日志] 清理过期日志文件失败: {:?}: {}", path, e);
            } else {
                cleaned_count += 1;
            }
        }
    }

    if cleaned_count > 0 {
        eprintln!("[日志] 已清理 {} 个过期日志文件", cleaned_count);
    }
}

/// 双输出日志记录器
///
/// 实现 log::Log trait，将日志同时写入文件（异步）和 stderr（同步）
/// - 文件输出使用 tracing_appender::non_blocking::NonBlocking，避免 I/O 阻塞
/// - stderr 输出使用 eprintln!，同步写入
/// - 完全控制格式，保留 module_path 信息
struct DualLogger {
    /// 文件写入器（non_blocking 异步写入），None 表示仅控制台输出
    file: Option<Mutex<tracing_appender::non_blocking::NonBlocking>>,
    /// 日志级别过滤器
    level_filter: log::LevelFilter,
}

impl log::Log for DualLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= self.level_filter
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        // 格式化日志行：{本地时间} [{级别5s}] {模块} - {消息}
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let level = format_level(record.level());
        let module = record.module_path().unwrap_or("unknown");
        let line = format!("{} [{}] {} - {}\n", timestamp, level, module, record.args());

        // stderr 输出（同步）
        eprint!("{}", line);

        // 文件输出（异步，通过 non_blocking writer）
        if let Some(file) = &self.file {
            if let Ok(mut writer) = file.lock() {
                // 写入失败静默处理（non_blocking writer 在关闭后会返回错误）
                let _ = writer.write_all(line.as_bytes());
            }
        }
    }

    fn flush(&self) {
        // 刷新文件缓冲区
        if let Some(file) = &self.file {
            if let Ok(mut writer) = file.lock() {
                let _ = writer.flush();
            }
        }
        // 刷新 stderr 缓冲区
        let _ = std::io::stderr().flush();
    }
}

/// 格式化日志级别为固定宽度字符串，便于对齐
fn format_level(level: log::Level) -> &'static str {
    match level {
        log::Level::Error => "ERROR",
        log::Level::Warn => "WARN ",
        log::Level::Info => "INFO ",
        log::Level::Debug => "DEBUG",
        log::Level::Trace => "TRACE",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::FileTimes;
    use std::time::SystemTime;

    /// 创建临时测试目录（在系统临时目录下创建唯一子目录）
    fn make_test_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "samoyed_work_test_{}_{}",
            prefix,
            std::process::id()
        ));
        // 清理可能存在的旧目录
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("创建测试目录失败");
        dir
    }

    /// 创建文件并设置修改时间为指定的时间点
    fn create_file_with_mtime(path: &Path, mtime: SystemTime) {
        let file = fs::File::create(path).expect("创建文件失败");
        let times = FileTimes::new().set_modified(mtime);
        file.set_times(times).expect("设置修改时间失败");
    }

    // ========== cleanup_old_logs 测试 ==========

    #[test]
    fn test_cleanup_removes_expired_log_files() {
        // 测试清理过期的 samoyed_work_*.log 和 sidecar_*.log 文件
        let dir = make_test_dir("cleanup_expired");
        let now = SystemTime::now();
        let eight_days_ago = now - Duration::from_secs(8 * 86400);

        // 创建过期文件（8 天前修改）
        let old_samoyed_work = dir.join("samoyed_work_old.log");
        let old_sidecar = dir.join("sidecar_old.log");
        create_file_with_mtime(&old_samoyed_work, eight_days_ago);
        create_file_with_mtime(&old_sidecar, eight_days_ago);

        // 执行清理（保留 7 天）
        cleanup_old_logs(&dir, 7);

        // 验证过期文件被删除
        assert!(
            !old_samoyed_work.exists(),
            "过期的 samoyed_work 日志应被删除"
        );
        assert!(!old_sidecar.exists(), "过期的 sidecar 日志应被删除");

        // 清理测试目录
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cleanup_preserves_recent_log_files() {
        // 测试保留 7 天内的日志文件
        let dir = make_test_dir("cleanup_recent");
        let now = SystemTime::now();
        let three_days_ago = now - Duration::from_secs(3 * 86400);

        // 创建近期文件（3 天前修改）
        let recent_samoyed_work = dir.join("samoyed_work_recent.log");
        let recent_sidecar = dir.join("sidecar_recent.log");
        create_file_with_mtime(&recent_samoyed_work, three_days_ago);
        create_file_with_mtime(&recent_sidecar, three_days_ago);

        // 执行清理（保留 7 天）
        cleanup_old_logs(&dir, 7);

        // 验证近期文件被保留
        assert!(
            recent_samoyed_work.exists(),
            "7 天内的 samoyed_work 日志应保留"
        );
        assert!(recent_sidecar.exists(), "7 天内的 sidecar 日志应保留");

        // 清理测试目录
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cleanup_preserves_boundary_seven_days() {
        // 测试边界：刚好 7 天的文件应保留（> 7 天才删除，>= 不删除）
        let dir = make_test_dir("cleanup_boundary");
        let now = SystemTime::now();
        // 6 天 23 小时前（小于 7 天，应保留）
        let just_under_seven = now - Duration::from_secs(6 * 86400 + 86399);

        let boundary_file = dir.join("samoyed_work_boundary.log");
        create_file_with_mtime(&boundary_file, just_under_seven);

        cleanup_old_logs(&dir, 7);

        assert!(boundary_file.exists(), "刚好 7 天内的文件应保留");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cleanup_ignores_non_matching_files() {
        // 测试不匹配前缀/后缀的文件不被删除
        let dir = make_test_dir("cleanup_nonmatch");
        let now = SystemTime::now();
        let ten_days_ago = now - Duration::from_secs(10 * 86400);

        // 不匹配前缀的文件（旧命名 samoyed_work.log 无时间戳）
        // 注意：samoyed_work.log 不匹配 samoyed_work_*.log 模式（缺少下划线）
        let old_style = dir.join("samoyed_work.log");
        let other_log = dir.join("other.log");
        let txt_file = dir.join("samoyed_work_old.txt");
        create_file_with_mtime(&old_style, ten_days_ago);
        create_file_with_mtime(&other_log, ten_days_ago);
        create_file_with_mtime(&txt_file, ten_days_ago);

        cleanup_old_logs(&dir, 7);

        // 旧命名格式（无下划线）不应被清理，保留向后兼容
        assert!(old_style.exists(), "旧格式 samoyed_work.log 不应被清理");
        assert!(other_log.exists(), "不匹配前缀的 other.log 不应被清理");
        assert!(txt_file.exists(), "不匹配后缀的 .txt 文件不应被清理");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cleanup_nonexistent_dir_silent() {
        // 测试不存在的目录静默跳过（不 panic）
        let nonexistent = PathBuf::from("/nonexistent/path/that/does/not/exist");
        // 应该不 panic，静默返回
        cleanup_old_logs(&nonexistent, 7);
    }

    #[test]
    fn test_cleanup_empty_dir() {
        // 测试空目录不报错
        let dir = make_test_dir("cleanup_empty");
        cleanup_old_logs(&dir, 7);
        let _ = fs::remove_dir_all(&dir);
    }

    // ========== format_level 测试 ==========

    #[test]
    fn test_format_level_all_variants() {
        // 验证所有级别的格式化结果和 5 字符定宽
        assert_eq!(format_level(log::Level::Error), "ERROR");
        assert_eq!(format_level(log::Level::Warn), "WARN ");
        assert_eq!(format_level(log::Level::Info), "INFO ");
        assert_eq!(format_level(log::Level::Debug), "DEBUG");
        assert_eq!(format_level(log::Level::Trace), "TRACE");
    }

    #[test]
    fn test_format_level_width_consistency() {
        // 验证所有级别格式化结果都是 5 字符宽度（对齐）
        for level in [
            log::Level::Error,
            log::Level::Warn,
            log::Level::Info,
            log::Level::Debug,
            log::Level::Trace,
        ] {
            let formatted = format_level(level);
            assert_eq!(
                formatted.len(),
                5,
                "级别 {:?} 格式化后应为 5 字符宽度，实际: {} ({:?})",
                level,
                formatted,
                formatted
            );
        }
    }

    // ========== resolve_log_dir 测试 ==========

    #[test]
    fn test_resolve_log_dir_production_with_app_log_dir() {
        // 生产模式（release）：优先使用 app_log_dir
        let app_log_dir = PathBuf::from("/app/logs");
        let app_data_dir = PathBuf::from("/app/data");

        let result = resolve_log_dir(Some(app_log_dir.clone()), Some(app_data_dir.clone()));

        if cfg!(debug_assertions) {
            // 开发模式：使用项目根目录的 log/ 子目录
            assert!(
                result.ends_with("log"),
                "开发模式应以 log/ 结尾，实际: {:?}",
                result
            );
        } else {
            // 生产模式：使用 app_log_dir
            assert_eq!(result, app_log_dir, "生产模式应使用 app_log_dir");
        }
    }

    #[test]
    fn test_resolve_log_dir_production_fallback_to_app_data() {
        // 生产模式：app_log_dir 为 None 时，回退到 app_data_dir/log
        let app_data_dir = PathBuf::from("/app/data");
        let result = resolve_log_dir(None, Some(app_data_dir.clone()));

        if cfg!(debug_assertions) {
            assert!(result.ends_with("log"), "开发模式应以 log/ 结尾");
        } else {
            assert_eq!(
                result,
                app_data_dir.join("log"),
                "app_log_dir 为 None 时应回退到 app_data_dir/log"
            );
        }
    }

    #[test]
    fn test_resolve_log_dir_production_all_none() {
        // 生产模式：两个参数都为 None 时，回退到 "log"
        let result = resolve_log_dir(None, None);

        if cfg!(debug_assertions) {
            assert!(result.ends_with("log"), "开发模式应以 log/ 结尾");
        } else {
            assert_eq!(result, PathBuf::from("log"), "都为 None 时应回退到 'log'");
        }
    }

    // ========== resolve_log_level 测试 ==========

    #[test]
    fn test_resolve_log_level_returns_valid_level() {
        // 测试 resolve_log_level 返回一个有效的日志级别
        // 注意：不依赖环境变量的具体值，仅验证返回值有效
        let level = resolve_log_level();
        // 验证返回的是有效级别（Off 到 Trace 之间）
        assert!(level <= log::LevelFilter::Trace);
    }

    #[test]
    fn test_rust_log_env_var_parsing() {
        // 测试 RUST_LOG 环境变量的解析逻辑
        // log::LevelFilter 实现了 FromStr，支持大小写不敏感的解析
        assert_eq!(
            "debug".parse::<log::LevelFilter>().unwrap(),
            log::LevelFilter::Debug
        );
        assert_eq!(
            "INFO".parse::<log::LevelFilter>().unwrap(),
            log::LevelFilter::Info
        );
        assert_eq!(
            "warn".parse::<log::LevelFilter>().unwrap(),
            log::LevelFilter::Warn
        );
        assert_eq!(
            "error".parse::<log::LevelFilter>().unwrap(),
            log::LevelFilter::Error
        );
        assert_eq!(
            "trace".parse::<log::LevelFilter>().unwrap(),
            log::LevelFilter::Trace
        );
        assert_eq!(
            "off".parse::<log::LevelFilter>().unwrap(),
            log::LevelFilter::Off
        );
        // 无效字符串应解析失败
        assert!("invalid".parse::<log::LevelFilter>().is_err());
    }

    // ========== current_log_file / current_log_dir 全局状态测试 ==========

    #[test]
    fn test_current_log_file_no_panic() {
        // 全局状态在未调用 init 前应为 None，调用不 panic
        let _ = current_log_file();
    }

    #[test]
    fn test_current_log_dir_no_panic() {
        let _ = current_log_dir();
    }
}
