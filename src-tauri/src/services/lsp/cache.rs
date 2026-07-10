//! LSP 结果缓存:缓存高频请求结果,避免重复计算
//! 缓存 definition、hover 等请求结果,设置 TTL

use crate::models::lsp::*;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 缓存键
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    /// 请求方法
    method: String,
    /// 文件路径
    file_path: String,
    /// 行号
    line: u32,
    /// 字符位置
    character: u32,
}

/// 缓存条目
struct CacheEntry<T> {
    /// 缓存值
    value: T,
    /// 过期时间
    expires_at: Instant,
}

/// LSP 结果缓存
pub struct LspResultCache {
    /// definition 缓存
    definition_cache: RwLock<HashMap<CacheKey, CacheEntry<Vec<LspLocation>>>>,
    /// hover 缓存
    hover_cache: RwLock<HashMap<CacheKey, CacheEntry<Option<LspHover>>>>,
    /// 缓存 TTL
    ttl: Duration,
    /// 最大缓存条目数
    max_entries: usize,
}

impl Default for LspResultCache {
    fn default() -> Self {
        Self {
            definition_cache: RwLock::new(HashMap::new()),
            hover_cache: RwLock::new(HashMap::new()),
            ttl: Duration::from_secs(300), // 5 分钟
            max_entries: 500,
        }
    }
}

impl LspResultCache {
    pub fn new(ttl_seconds: u64, max_entries: usize) -> Self {
        Self {
            definition_cache: RwLock::new(HashMap::new()),
            hover_cache: RwLock::new(HashMap::new()),
            ttl: Duration::from_secs(ttl_seconds),
            max_entries,
        }
    }

    /// 获取 definition 缓存
    pub async fn get_definition(
        &self,
        file_path: &str,
        line: u32,
        character: u32,
    ) -> Option<Vec<LspLocation>> {
        let key = CacheKey {
            method: "definition".to_string(),
            file_path: file_path.to_string(),
            line,
            character,
        };
        let cache = self.definition_cache.read().await;
        cache
            .get(&key)
            .filter(|e| e.expires_at > Instant::now())
            .map(|e| e.value.clone())
    }

    /// 存储 definition 缓存
    pub async fn set_definition(
        &self,
        file_path: &str,
        line: u32,
        character: u32,
        locations: Vec<LspLocation>,
    ) {
        // max_entries=0 时不存储任何条目（缓存禁用模式）
        if self.max_entries == 0 {
            return;
        }
        let key = CacheKey {
            method: "definition".to_string(),
            file_path: file_path.to_string(),
            line,
            character,
        };
        let mut cache = self.definition_cache.write().await;

        // 检查缓存大小
        if cache.len() >= self.max_entries {
            self.evict_expired(&mut cache);
        }

        cache.insert(
            key,
            CacheEntry {
                value: locations,
                expires_at: Instant::now() + self.ttl,
            },
        );
    }

    /// 获取 hover 缓存
    pub async fn get_hover(
        &self,
        file_path: &str,
        line: u32,
        character: u32,
    ) -> Option<Option<LspHover>> {
        let key = CacheKey {
            method: "hover".to_string(),
            file_path: file_path.to_string(),
            line,
            character,
        };
        let cache = self.hover_cache.read().await;
        cache
            .get(&key)
            .filter(|e| e.expires_at > Instant::now())
            .map(|e| e.value.clone())
    }

    /// 存储 hover 缓存
    pub async fn set_hover(
        &self,
        file_path: &str,
        line: u32,
        character: u32,
        hover: Option<LspHover>,
    ) {
        // max_entries=0 时不存储任何条目（缓存禁用模式）
        if self.max_entries == 0 {
            return;
        }
        let key = CacheKey {
            method: "hover".to_string(),
            file_path: file_path.to_string(),
            line,
            character,
        };
        let mut cache = self.hover_cache.write().await;

        if cache.len() >= self.max_entries {
            self.evict_expired(&mut cache);
        }

        cache.insert(
            key,
            CacheEntry {
                value: hover,
                expires_at: Instant::now() + self.ttl,
            },
        );
    }

    /// 清除指定文件的缓存(文件修改时调用)
    pub async fn invalidate_file(&self, file_path: &str) {
        let target = file_path.to_string();

        self.definition_cache
            .write()
            .await
            .retain(|k, _| k.file_path != target);
        self.hover_cache
            .write()
            .await
            .retain(|k, _| k.file_path != target);
    }

    /// 清除所有缓存
    pub async fn clear_all(&self) {
        self.definition_cache.write().await.clear();
        self.hover_cache.write().await.clear();
    }

    /// 驱逐过期条目
    fn evict_expired<T>(&self, cache: &mut HashMap<CacheKey, CacheEntry<T>>) {
        let now = Instant::now();
        cache.retain(|_, e| e.expires_at > now);
    }
}
