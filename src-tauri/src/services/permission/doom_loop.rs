use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Doom loop 检测阈值:连续相同调用的次数
const DOOM_LOOP_THRESHOLD: usize = 3;

/// 工具调用记录(用于去重比较)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ToolCallRecord {
    /// 工具名
    tool_name: String,
    /// 参数的规范化 JSON 字符串(用于比较)
    params_key: String,
}

impl ToolCallRecord {
    fn new(tool_name: &str, params: &serde_json::Value) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            params_key: Self::normalize_params(params),
        }
    }

    /// 规范化参数用于比较
    /// 移除顺序差异:对 JSON 对象的 key 排序
    fn normalize_params(params: &serde_json::Value) -> String {
        match params {
            serde_json::Value::Object(map) => {
                let mut sorted_map: Vec<(&String, &serde_json::Value)> = map.iter().collect();
                sorted_map.sort_by(|a, b| a.0.cmp(b.0));
                serde_json::to_string(&sorted_map).unwrap_or_default()
            }
            _ => params.to_string(),
        }
    }
}

/// Doom loop 检测器
/// 按 session_id 隔离,记录最近的工具调用历史
pub struct DoomLoopDetector {
    /// 按 session_id 隔离的调用历史
    sessions: Arc<RwLock<HashMap<String, Vec<ToolCallRecord>>>>,
}

impl DoomLoopDetector {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 记录一次工具调用并检测是否触发 Doom loop
    /// 返回 true 表示触发了 Doom loop(连续相同调用达到阈值)
    pub async fn record_and_check(
        &self,
        session_id: &str,
        tool_name: &str,
        params: &serde_json::Value,
    ) -> bool {
        let record = ToolCallRecord::new(tool_name, params);
        let mut sessions = self.sessions.write().await;
        let history = sessions
            .entry(session_id.to_string())
            .or_insert_with(Vec::new);

        // 检查最近的 N-1 次调用是否与当前调用相同
        let trigger = if history.len() >= DOOM_LOOP_THRESHOLD - 1 {
            let recent = &history[history.len() - (DOOM_LOOP_THRESHOLD - 1)..];
            recent.iter().all(|r| r == &record)
        } else {
            false
        };

        // 记录本次调用
        history.push(record);

        // 限制历史长度,避免内存无限增长
        if history.len() > 100 {
            let drain_count = history.len() - 100;
            history.drain(0..drain_count);
        }

        if trigger {
            log::warn!(
                "检测到 Doom loop: session_id={}, tool={}, 连续 {} 次相同调用",
                session_id,
                tool_name,
                DOOM_LOOP_THRESHOLD
            );
        }

        trigger
    }

    /// 清理指定会话的调用历史
    pub async fn cleanup_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        if let Some(history) = sessions.remove(session_id) {
            log::debug!(
                "已清理会话 {} 的 Doom loop 检测历史({} 条记录)",
                session_id,
                history.len()
            );
        }
    }

    /// 获取指定会话的最近 N 次调用记录(用于调试)
    pub async fn recent_calls(&self, session_id: &str, n: usize) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions
            .get(session_id)
            .map(|h| {
                h.iter()
                    .rev()
                    .take(n)
                    .map(|r| format!("{}({})", r.tool_name, r.params_key))
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for DoomLoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_no_trigger_on_different_calls() {
        let detector = DoomLoopDetector::new();
        let params1 = json!({"path": "file1.rs"});
        let params2 = json!({"path": "file2.rs"});

        // 不同参数的调用不应触发
        assert!(!detector.record_and_check("s1", "read", &params1).await);
        assert!(!detector.record_and_check("s1", "read", &params2).await);
        assert!(!detector.record_and_check("s1", "read", &params1).await);
    }

    #[tokio::test]
    async fn test_trigger_on_same_calls() {
        let detector = DoomLoopDetector::new();
        let params = json!({"path": "file.rs", "content": "test"});

        // 第 1 次:不触发
        assert!(!detector.record_and_check("s1", "edit", &params).await);
        // 第 2 次:不触发
        assert!(!detector.record_and_check("s1", "edit", &params).await);
        // 第 3 次:触发
        assert!(detector.record_and_check("s1", "edit", &params).await);
    }

    #[tokio::test]
    async fn test_isolated_sessions() {
        let detector = DoomLoopDetector::new();
        let params = json!({"command": "ls"});

        // 会话 1:2 次
        assert!(!detector.record_and_check("s1", "bash", &params).await);
        assert!(!detector.record_and_check("s1", "bash", &params).await);

        // 会话 2:2 次(不应触发,因为是新会话)
        assert!(!detector.record_and_check("s2", "bash", &params).await);
        assert!(!detector.record_and_check("s2", "bash", &params).await);

        // 会话 1:第 3 次(触发)
        assert!(detector.record_and_check("s1", "bash", &params).await);
    }

    #[tokio::test]
    async fn test_cleanup() {
        let detector = DoomLoopDetector::new();
        let params = json!({"path": "f.rs"});
        detector.record_and_check("s1", "read", &params).await;
        detector.record_and_check("s1", "read", &params).await;

        detector.cleanup_session("s1").await;

        // 清理后重新开始计数
        assert!(!detector.record_and_check("s1", "read", &params).await);
    }
}
