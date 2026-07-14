//! Web 搜索器：支持多种搜索后端
//! 默认使用 MCP 协议（Exa AI 托管服务），也支持 Tavily/SerpAPI（需 API Key）

use crate::config::app_settings::WebSearchConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// 搜索结果项
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultItem {
    /// 标题
    pub title: String,
    /// URL
    pub url: String,
    /// 摘要
    pub snippet: String,
    /// 显示 URL（部分搜索引擎提供）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_url: Option<String>,
}

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    /// 搜索查询
    pub query: String,
    /// 搜索引擎
    pub engine: String,
    /// 结果列表
    pub results: Vec<SearchResultItem>,
    /// 总结果数（估计）
    pub total_results: Option<u64>,
    /// 搜索耗时（毫秒）
    pub search_duration_ms: u64,
}

/// Web 搜索器
pub struct WebSearcher {
    /// 配置
    config: WebSearchConfig,
    /// HTTP 客户端
    client: Client,
}

impl WebSearcher {
    /// 创建 Web 搜索器
    /// - HTTP 客户端超时为 config.timeout_seconds 秒
    /// - User-Agent: "Mozilla/5.0 (compatible; WorkMolde-AI/1.0)"
    pub fn new(config: WebSearchConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .user_agent("Mozilla/5.0 (compatible; WorkMolde-AI/1.0)")
            .build()
            .unwrap_or_else(|e| {
                log::warn!("HTTP 客户端构建失败，使用默认配置: {}", e);
                Client::default()
            });
        Self { config, client }
    }

    /// 执行搜索
    ///
    /// 根据 config.backend 分发到对应的搜索后端：
    /// - "mcp" → MCP 协议（Exa AI 托管服务）
    /// - "tavily" → Tavily API
    /// - "serpapi" → SerpAPI
    /// - 其他 → 默认使用 mcp
    pub async fn search(&self, query: &str) -> Result<SearchResponse, String> {
        // 检查是否启用
        if !self.config.enabled {
            return Err("网络搜索已被禁用".to_string());
        }

        let start = Instant::now();

        // 根据后端类型分发
        let results = match self.config.backend.as_str() {
            "mcp" => self.search_via_mcp(query).await?,
            "tavily" => self.search_tavily(query).await?,
            "serpapi" => self.search_serpapi(query).await?,
            other => {
                // 未知后端，回退到 mcp
                log::warn!("未知的搜索后端: {}，回退到 mcp", other);
                self.search_via_mcp(query).await?
            }
        };

        let search_duration_ms = start.elapsed().as_millis() as u64;

        Ok(SearchResponse {
            query: query.to_string(),
            engine: self.config.backend.clone(),
            results,
            total_results: None,
            search_duration_ms,
        })
    }

    /// 通过 MCP 协议搜索（默认 Exa AI 托管服务）
    ///
    /// 构建 JSON-RPC 2.0 请求，调用 tools/call 方法，工具名为 web_search。
    /// 响应解析：优先尝试 result.content 数组，若不是数组则尝试 result 直接作为数组。
    async fn search_via_mcp(&self, query: &str) -> Result<Vec<SearchResultItem>, String> {
        // 构建 JSON-RPC 2.0 请求
        let request_body = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "web_search",
                "arguments": {
                    "query": query,
                    "max_results": self.config.max_results
                }
            },
            "id": 1
        });

        // 发送 POST 请求
        let response = self
            .client
            .post(&self.config.mcp_endpoint)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("MCP 搜索请求失败: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(format!("MCP 搜索返回错误状态: {} - {}", status, text));
        }

        let body: Value = response
            .json()
            .await
            .map_err(|e| format!("MCP 响应解析失败: {}", e))?;

        // 检查 JSON-RPC 错误
        if let Some(error) = body.get("error") {
            return Err(format!("MCP 搜索返回错误: {}", error));
        }

        // 解析结果：优先尝试 result.content 数组，若不是数组则尝试 result 直接作为数组
        let items: Vec<Value> = if let Some(content) = body
            .get("result")
            .and_then(|r| r.get("content"))
            .and_then(|c| c.as_array())
        {
            content.clone()
        } else if let Some(result_array) = body.get("result").and_then(|r| r.as_array()) {
            result_array.clone()
        } else {
            return Err("MCP 响应格式不符合预期: 未找到 result.content 或 result 数组".to_string());
        };

        // 转换为 SearchResultItem
        let mut results = Vec::new();
        for item in items.iter().take(self.config.max_results) {
            // MCP 响应中的 content 项可能是 {type: "text", text: "..."} 格式
            // 也可能直接包含 title/url/snippet 字段
            let (title, url, snippet) =
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    // 尝试将 text 字段解析为 JSON（可能是序列化的搜索结果）
                    if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                        extract_fields(&parsed)
                    } else {
                        // text 不是 JSON，截取前 100 字符作为标题，完整文本作为摘要
                        let title_text: String = text.chars().take(100).collect();
                        (title_text, String::new(), text.to_string())
                    }
                } else {
                    extract_fields(item)
                };

            // 仅保留有标题或 URL 的结果
            if !url.is_empty() || !title.is_empty() {
                results.push(SearchResultItem {
                    title,
                    url,
                    snippet,
                    display_url: None,
                });
            }
        }

        Ok(results)
    }

    /// 通过 Tavily API 搜索
    ///
    /// POST 到 https://api.tavily.com/search
    /// 请求体包含 api_key、query、max_results、include_answer。
    /// 响应中 results 数组的 content 字段映射到 snippet。
    async fn search_tavily(&self, query: &str) -> Result<Vec<SearchResultItem>, String> {
        // 检查 API Key
        if self.config.api_key.is_empty() {
            return Err("Tavily 搜索需要 API Key".to_string());
        }

        // 构建请求体
        let request_body = json!({
            "api_key": self.config.api_key,
            "query": query,
            "max_results": self.config.max_results,
            "include_answer": false
        });

        // 发送 POST 请求
        let response = self
            .client
            .post("https://api.tavily.com/search")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Tavily 搜索请求失败: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(format!("Tavily 搜索返回错误状态: {} - {}", status, text));
        }

        let body: Value = response
            .json()
            .await
            .map_err(|e| format!("Tavily 响应解析失败: {}", e))?;

        // 解析 results 数组
        let results_array = body
            .get("results")
            .and_then(|r| r.as_array())
            .ok_or_else(|| "Tavily 响应中未找到 results 数组".to_string())?;

        // 转换为 SearchResultItem（content 映射到 snippet）
        let mut results = Vec::new();
        for item in results_array.iter().take(self.config.max_results) {
            let title = item
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            let url = item
                .get("url")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .to_string();
            let snippet = item
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();

            results.push(SearchResultItem {
                title,
                url,
                snippet,
                display_url: None,
            });
        }

        Ok(results)
    }

    /// 通过 SerpAPI 搜索
    ///
    /// GET https://serpapi.com/search?q={encoded_query}&api_key={api_key}&num={max_results}
    /// 响应中 organic_results 数组的 link 字段映射到 url。
    async fn search_serpapi(&self, query: &str) -> Result<Vec<SearchResultItem>, String> {
        // 检查 API Key
        if self.config.api_key.is_empty() {
            return Err("SerpAPI 搜索需要 API Key".to_string());
        }

        // 使用 urlencoding::encode 编码 query
        let encoded_query = urlencoding::encode(query);
        let url = format!(
            "https://serpapi.com/search?q={}&api_key={}&num={}",
            encoded_query, self.config.api_key, self.config.max_results
        );

        // 发送 GET 请求
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("SerpAPI 搜索请求失败: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(format!("SerpAPI 搜索返回错误状态: {} - {}", status, text));
        }

        let body: Value = response
            .json()
            .await
            .map_err(|e| format!("SerpAPI 响应解析失败: {}", e))?;

        // 解析 organic_results 数组
        let results_array = body
            .get("organic_results")
            .and_then(|r| r.as_array())
            .ok_or_else(|| "SerpAPI 响应中未找到 organic_results 数组".to_string())?;

        // 转换为 SearchResultItem（link 映射到 url）
        let mut results = Vec::new();
        for item in results_array.iter().take(self.config.max_results) {
            let title = item
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            let url = item
                .get("link")
                .and_then(|l| l.as_str())
                .unwrap_or("")
                .to_string();
            let snippet = item
                .get("snippet")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();

            results.push(SearchResultItem {
                title,
                url,
                snippet,
                display_url: None,
            });
        }

        Ok(results)
    }
}

/// 从 JSON 对象中提取 title/url/snippet 字段
fn extract_fields(value: &Value) -> (String, String, String) {
    let title = value
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    let url = value
        .get("url")
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();
    let snippet = value
        .get("snippet")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    (title, url, snippet)
}
