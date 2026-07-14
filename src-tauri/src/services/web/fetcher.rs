//! Web 内容获取器：获取 URL 内容并转换为 Markdown
//! 支持 HTML（转 Markdown）、JSON（格式化）、纯文本/Markdown（直接使用）

use crate::services::web::url_validator::{UrlValidator, ValidationResult};
use reqwest::Client;
use std::time::{Duration, Instant};

/// Web 内容获取结果
#[derive(Debug, Clone)]
pub struct FetchResult {
    /// 原始 URL
    pub url: String,
    /// 最终 URL（可能经过重定向）
    pub final_url: String,
    /// 内容类型（text/html, application/json 等）
    pub content_type: String,
    /// 转换后的 Markdown 内容
    pub markdown: String,
    /// 内容长度（字符数）
    pub content_length: usize,
    /// 获取耗时（毫秒）
    pub fetch_duration_ms: u64,
}

/// Web 内容获取器
pub struct WebFetcher {
    /// HTTP 客户端
    client: Client,
    /// URL 验证器
    validator: UrlValidator,
    /// 请求超时时间
    timeout: Duration,
    /// 最大内容长度（字符数）
    max_content_length: usize,
}

impl WebFetcher {
    /// 创建 WebFetcher 实例
    /// - 请求超时 30 秒
    /// - 最多 5 次重定向
    /// - User-Agent: Mozilla/5.0 (compatible; WorkMolde-AI/1.0)
    /// - max_content_length 默认 100000
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .user_agent("Mozilla/5.0 (compatible; WorkMolde-AI/1.0)")
            .build()
            .expect("构建 HTTP 客户端失败");
        Self {
            client,
            validator: UrlValidator::new(),
            timeout: Duration::from_secs(30),
            max_content_length: 100_000,
        }
    }

    /// 获取 URL 内容并转换为 Markdown
    ///
    /// 流程：
    /// 1. 验证 URL（防止访问内网/恶意地址）
    /// 2. 发送 GET 请求（带超时）
    /// 3. 检查状态码（非 2xx 返回错误）
    /// 4. 根据 content_type 转换内容
    /// 5. 截断超长内容
    pub async fn fetch(&self, url: &str) -> Result<FetchResult, String> {
        // 1. 验证 URL
        match self.validator.validate(url) {
            ValidationResult::Valid => {}
            ValidationResult::Invalid(reason) => return Err(reason),
        }

        // 2. 记录开始时间并发送请求
        let start = Instant::now();
        let response = self
            .client
            .get(url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| format!("HTTP 请求失败: {}", e))?;

        // 3. 获取最终 URL 和内容类型
        let final_url = response.url().to_string();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/plain")
            .to_string();

        // 4. 检查状态码
        let status = response.status();
        if !status.is_success() {
            return Err(format!("HTTP 状态码错误: {}", status));
        }

        // 5. 读取响应体
        let body = response
            .text()
            .await
            .map_err(|e| format!("读取响应体失败: {}", e))?;

        let fetch_duration_ms = start.elapsed().as_millis() as u64;

        // 6. 根据 content_type 转换内容
        let mut markdown = self.convert_content(&content_type, &body);

        // 7. 截断超长内容
        if markdown.len() > self.max_content_length {
            let original_len = markdown.len();
            markdown.truncate(self.max_content_length);
            markdown.push_str(&format!("\n\n[内容已截断,原始长度 {} 字符]", original_len));
        }

        let content_length = markdown.chars().count();

        Ok(FetchResult {
            url: url.to_string(),
            final_url,
            content_type,
            markdown,
            content_length,
            fetch_duration_ms,
        })
    }

    /// 根据内容类型转换响应体为 Markdown
    fn convert_content(&self, content_type: &str, body: &str) -> String {
        // 内容类型匹配（不区分大小写，支持带 charset 等参数的形式）
        let ct = content_type.to_lowercase();
        if ct.contains("text/html") {
            // HTML 转 Markdown
            html2md::parse_html(body)
        } else if ct.contains("application/json") {
            // JSON 格式化（解析失败则直接使用原文）
            match serde_json::from_str::<serde_json::Value>(body) {
                Ok(value) => {
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| body.to_string())
                }
                Err(_) => body.to_string(),
            }
        } else if ct.contains("text/plain") || ct.contains("text/markdown") {
            // 纯文本或 Markdown 直接使用
            body.to_string()
        } else {
            // 其他内容类型不支持
            format!("[不支持的内容类型: {}]", content_type)
        }
    }

    /// 设置最大内容长度（builder 模式）
    pub fn with_max_content_length(mut self, max: usize) -> Self {
        self.max_content_length = max;
        self
    }
}

impl Default for WebFetcher {
    fn default() -> Self {
        Self::new()
    }
}
