use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::mpsc;

use super::provider::LlmProvider;
use crate::config::llm_config::AdvancedConfig;
use crate::errors::CommandError;
use crate::models::llm::*;

/// Anthropic Claude Messages API 适配器
/// 实现 Anthropic 原生 Messages API 协议，与 OpenAI 格式存在以下关键差异：
/// 1. 认证使用 x-api-key 头而非 Authorization Bearer
/// 2. 必须包含 anthropic-version 头
/// 3. system 消息放在顶层 system 字段，不在 messages 数组中
/// 4. max_tokens 为必需字段
/// 5. Tool Calling 使用 input_schema 而非 parameters
/// 6. 响应 content 为数组格式（text / tool_use 块）
/// 7. 流式 SSE 事件格式不同（message_start / content_block_start / content_block_delta 等）
pub struct AnthropicAdapter {
    api_base_url: String,
    api_key: String,
    model: String,
    advanced: AdvancedConfig,
    /// 用于非流式请求的客户端（支持压缩）
    client: Client,
    /// 用于流式请求的客户端（禁用压缩，避免 bytes_stream 解码错误）
    streaming_client: Client,
}

impl AnthropicAdapter {
    pub fn new(
        api_base_url: String,
        api_key: String,
        model: String,
        advanced: AdvancedConfig,
    ) -> Self {
        let timeout = Duration::from_secs(advanced.timeout_seconds as u64);
        // 创建两个客户端：
        // 1. 非流式请求客户端：默认启用 gzip 压缩，减少传输数据量
        // 2. 流式请求客户端：禁用压缩，避免 bytes_stream() 解码错误
        //    原因：reqwest 的 bytes_stream() 不会自动解压缩 gzip 响应，
        //    导致 "error decoding response body" 错误
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        let streaming_client = Client::builder()
            .timeout(Duration::from_secs(300))
            .no_gzip()
            .no_deflate()
            .no_brotli()
            .tcp_keepalive(Some(Duration::from_secs(60)))
            .build()
            .unwrap_or_default();

        Self {
            api_base_url,
            api_key,
            model,
            advanced,
            client,
            streaming_client,
        }
    }

    /// 将内部 ChatMessage 列表转换为 Anthropic Messages API 格式
    /// 关键转换：
    /// 1. system role 消息提取到顶层 system 字段（多个系统消息用换行拼接）
    /// 2. assistant 的 tool_calls 转换为 content blocks（tool_use 类型）
    /// 3. tool role 消息转换为 user role 的 tool_result content blocks
    /// 4. 连续的 tool 消息合并为同一个 user 消息（Anthropic 要求）
    fn convert_messages(&self, messages: &[ChatMessage]) -> (Option<Value>, Vec<Value>) {
        let mut system_parts: Vec<String> = Vec::new();
        let mut anthropic_messages: Vec<Value> = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    // 系统消息提取到顶层 system 字段
                    system_parts.push(msg.content.clone());
                }
                "assistant" => {
                    let mut content_blocks: Vec<Value> = Vec::new();

                    if let Some(rc) = &msg.reasoning_content {
                        content_blocks.push(json!({
                            "type": "thinking",
                            "thinking": rc,
                        }));
                    }

                    if !msg.content.is_empty() {
                        content_blocks.push(json!({
                            "type": "text",
                            "text": msg.content,
                        }));
                    }

                    // 如果有 tool_calls，转换为 tool_use content blocks
                    if let Some(tool_calls) = &msg.tool_calls {
                        for tc in tool_calls {
                            // Anthropic 的 input 是 JSON 对象，需要解析 arguments 字符串
                            let input: Value =
                                serde_json::from_str(&tc.arguments).unwrap_or(json!({}));
                            content_blocks.push(json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.name,
                                "input": input,
                            }));
                        }
                    }

                    // 空的 assistant 消息，添加空文本块（Anthropic 要求 content 非空）
                    if content_blocks.is_empty() {
                        content_blocks.push(json!({
                            "type": "text",
                            "text": "",
                        }));
                    }

                    anthropic_messages.push(json!({
                        "role": "assistant",
                        "content": content_blocks,
                    }));
                }
                "tool" => {
                    // tool 消息转换为 user role 的 tool_result content block
                    // Anthropic 要求 tool_result 在 user 消息中
                    let tool_result_block = json!({
                        "type": "tool_result",
                        "tool_use_id": msg.tool_call_id.as_deref().unwrap_or(""),
                        "content": msg.content,
                    });

                    // 检查前一条消息是否也是 tool role 转换的 user 消息，如果是则合并
                    // Anthropic 要求同一轮的所有 tool_result 在同一个 user 消息中
                    let should_merge = anthropic_messages
                        .last()
                        .map(|last| {
                            last["role"] == "user" && last.get("_merged_tool_results").is_some()
                        })
                        .unwrap_or(false);

                    if should_merge {
                        // 合并到已有的 user 消息中
                        if let Some(last) = anthropic_messages.last_mut() {
                            if let Some(blocks) = last["content"].as_array_mut() {
                                blocks.push(tool_result_block);
                            }
                        }
                    } else {
                        // 创建新的 user 消息
                        let mut user_msg = json!({
                            "role": "user",
                            "content": [tool_result_block],
                        });
                        // 内部标记，标识此消息由 tool_result 合并而来
                        // 序列化前会被移除
                        user_msg["_merged_tool_results"] = json!(true);
                        anthropic_messages.push(user_msg);
                    }
                }
                "user" => {
                    // 支持多模态消息：当 content_parts 存在且非空时，构建 Anthropic Vision API 格式的 content 数组
                    let content_value = if let Some(parts) = &msg.content_parts {
                        if !parts.is_empty() {
                            let blocks: Vec<Value> = parts
                                .iter()
                                .map(|part| match part {
                                    ContentPart::Text { text } => json!({
                                        "type": "text",
                                        "text": text,
                                    }),
                                    ContentPart::Image { mime_type, data } => json!({
                                        "type": "image",
                                        "source": {
                                            "type": "base64",
                                            "media_type": mime_type,
                                            "data": data,
                                        },
                                    }),
                                })
                                .collect();
                            json!(blocks)
                        } else {
                            json!(msg.content)
                        }
                    } else {
                        json!(msg.content)
                    };
                    anthropic_messages.push(json!({
                        "role": "user",
                        "content": content_value,
                    }));
                }
                _ => {
                    log::warn!("未知消息角色: {}, 跳过", msg.role);
                }
            }
        }

        // 清理内部标记字段，避免发送到 API
        for msg in &mut anthropic_messages {
            if let Some(obj) = msg.as_object_mut() {
                obj.remove("_merged_tool_results");
            }
        }

        // 构建顶层 system 字段
        // Anthropic 支持字符串或 content blocks 数组格式，这里使用字符串格式
        let system = if system_parts.is_empty() {
            None
        } else {
            Some(json!(system_parts.join("\n")))
        };

        (system, anthropic_messages)
    }

    /// 构建 Anthropic Messages API 请求体
    /// max_tokens_override: 可选的 max_tokens 覆盖值，用于截断重试时增大输出限制
    fn build_request_body(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        stream: bool,
        max_tokens_override: Option<u32>,
    ) -> Value {
        let (system, anthropic_messages) = self.convert_messages(messages);

        let mut body = json!({
            "model": self.model,
            "messages": anthropic_messages,
            "max_tokens": max_tokens_override.unwrap_or(self.advanced.max_tokens),
            "stream": stream,
        });

        // 添加顶层 system 字段（Anthropic 不支持在 messages 中使用 system role）
        if let Some(sys) = system {
            body["system"] = sys;
        }

        // 添加工具定义
        // Anthropic 使用 input_schema 而非 parameters，且不需要外层 function 包装
        if !tools.is_empty() {
            body["tools"] = json!(tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters,
                    })
                })
                .collect::<Vec<_>>());
        }

        body["temperature"] = json!(self.advanced.temperature);
        body["top_p"] = json!(self.advanced.top_p);

        body
    }

    /// 构建 Anthropic API 端点 URL
    /// 自动处理 base URL 是否包含 /v1 的情况：
    /// - https://api.anthropic.com -> https://api.anthropic.com/v1/messages
    /// - https://api.anthropic.com/v1 -> https://api.anthropic.com/v1/messages
    fn build_api_url(&self) -> String {
        let base = self.api_base_url.trim_end_matches('/');
        if base.ends_with("/v1") {
            format!("{}/messages", base)
        } else {
            format!("{}/v1/messages", base)
        }
    }

    /// 发送请求，带重试逻辑（使用普通客户端，支持压缩）
    async fn send_with_retry(
        &self,
        url: &str,
        body: &Value,
    ) -> Result<reqwest::Response, CommandError> {
        self.send_with_retry_internal(url, body, &self.client).await
    }

    /// 发送流式请求，带重试逻辑（使用流式客户端，禁用压缩）
    async fn send_streaming_with_retry(
        &self,
        url: &str,
        body: &Value,
    ) -> Result<reqwest::Response, CommandError> {
        self.send_with_retry_internal(url, body, &self.streaming_client)
            .await
    }

    /// 内部发送请求实现，带重试逻辑
    /// 与 OpenAI 适配器的主要差异：
    /// 1. 使用 x-api-key 头认证（而非 Authorization Bearer）
    /// 2. 必须包含 anthropic-version 头
    /// 3. 额外处理 529 过载错误码（Anthropic 特有）
    /// 4. 额外处理 400 请求参数错误
    /// 5. DNS 解析失败时使用更短的重试间隔（200ms）并额外增加1次重试机会
    async fn send_with_retry_internal(
        &self,
        url: &str,
        body: &Value,
        client: &Client,
    ) -> Result<reqwest::Response, CommandError> {
        let max_retries = self.advanced.max_retries;
        let mut _last_error = None;
        let mut dns_extra_retry = true;
        let mut is_dns_failure = false;
        let mut total_attempt: u32 = 0;

        loop {
            total_attempt += 1;

            if total_attempt > 1 {
                let delay = if is_dns_failure {
                    Duration::from_millis(200)
                } else {
                    Duration::from_millis(500 * 2u64.pow(total_attempt.saturating_sub(2)))
                };
                log::warn!(
                    "请求重试, model={}, 第{}次重试, 延迟{}ms",
                    self.model,
                    total_attempt - 1,
                    delay.as_millis()
                );
                tokio::time::sleep(delay).await;
            }

            let mut request = client.post(url);
            request = request
                .header("x-api-key", &self.api_key)
                .header("Content-Type", "application/json")
                .header("anthropic-version", "2023-06-01");

            for (key, value) in &self.advanced.extra_headers {
                request = request.header(key.as_str(), value.as_str());
            }

            match request.json(body).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        return Ok(response);
                    }

                    let error_body = response.text().await.unwrap_or_default();

                    if status.as_u16() == 401 {
                        log::error!("认证失败(401), model={}", self.model);
                        return Err(CommandError::llm(1002, format!("认证失败: {}", error_body)));
                    }
                    if status.as_u16() == 429 {
                        if total_attempt <= max_retries {
                            log::warn!("请求频率受限(429), model={}, 准备重试", self.model);
                            _last_error = Some(CommandError::llm(
                                1003,
                                "请求频率受限，正在重试".to_string(),
                            ));
                            is_dns_failure = false;
                            continue;
                        }
                        return Err(CommandError::llm(
                            1003,
                            format!("请求频率受限: {}", error_body),
                        ));
                    }
                    if status.as_u16() == 404 {
                        log::error!("模型不存在(404), model={}", self.model);
                        return Err(CommandError::llm(
                            1005,
                            format!("模型不存在: {}", error_body),
                        ));
                    }
                    if status.as_u16() == 400 {
                        log::error!("请求参数错误(400), model={}", self.model);
                        return Err(CommandError::llm(
                            1007,
                            format!("请求参数错误: {}", error_body),
                        ));
                    }
                    // Anthropic 特有的过载错误码 529
                    if status.as_u16() == 529 {
                        if total_attempt <= max_retries {
                            log::warn!("API 过载(529), model={}, 准备重试", self.model);
                            _last_error =
                                Some(CommandError::llm(1003, "API 过载，正在重试".to_string()));
                            is_dns_failure = false;
                            continue;
                        }
                        return Err(CommandError::llm(1003, format!("API 过载: {}", error_body)));
                    }
                    // 5xx 服务端错误，可重试
                    if status.as_u16() >= 500
                        && status.as_u16() != 529
                        && total_attempt <= max_retries
                    {
                        log::warn!("服务端错误({}), model={}, 准备重试", status, self.model);
                        _last_error = Some(CommandError::llm(
                            1001,
                            format!("服务端错误 ({}), 正在重试", status),
                        ));
                        is_dns_failure = false;
                        continue;
                    }

                    _last_error = Some(CommandError::llm(
                        1000,
                        format!("API 请求失败 ({}): {}", status, error_body),
                    ));
                }
                Err(e) => {
                    if e.is_timeout() {
                        if total_attempt <= max_retries {
                            log::warn!("请求超时, model={}, 准备重试", self.model);
                            _last_error =
                                Some(CommandError::llm(1006, "请求超时，正在重试".to_string()));
                            is_dns_failure = false;
                            continue;
                        }
                        return Err(CommandError::llm(1006, "请求超时".to_string()));
                    }
                    // DNS 解析失败特殊处理
                    if super::provider::is_dns_error(&e) {
                        if dns_extra_retry {
                            dns_extra_retry = false;
                            is_dns_failure = true;
                            log::warn!("DNS解析失败, model={}, 额外重试1次", self.model);
                            _last_error = Some(CommandError::llm(
                                crate::errors::LLM_DNS_RESOLVE_FAILED,
                                "DNS解析失败，正在重试".to_string(),
                            ));
                            continue;
                        }
                        return Err(CommandError::llm(
                            crate::errors::LLM_DNS_RESOLVE_FAILED,
                            format!("DNS解析失败: {}", e),
                        ));
                    }
                    // 细化连接错误分类：连接被拒绝/SSL/网络不可达
                    let (code, msg) = super::provider::classify_connection_error(&e);
                    _last_error = Some(CommandError::llm(code, msg));
                }
            }

            break;
        }

        let err = _last_error.unwrap_or_else(|| CommandError::llm(1000, "未知错误".to_string()));
        log::error!(
            "请求最终失败, model={}, 重试耗尽, 错误: {}",
            self.model,
            err.message
        );
        Err(err)
    }

    /// 解析 Anthropic 非流式响应，转换为内部 ChatResponse 格式
    /// Anthropic 响应格式：
    /// - content 为数组，包含 text 和 tool_use 类型的 content block
    /// - stop_reason 对应 OpenAI 的 finish_reason
    /// - usage 中使用 input_tokens / output_tokens 而非 prompt_tokens / completion_tokens
    fn parse_response(&self, value: Value) -> Result<ChatResponse, CommandError> {
        let id = value["id"].as_str().unwrap_or("").to_string();

        // 解析 content 数组，提取文本和 tool_use
        let mut text_content = String::new();
        let mut tool_calls: Vec<LlmToolCall> = Vec::new();
        let mut reasoning_content: Option<String> = None;

        if let Some(content_blocks) = value["content"].as_array() {
            for (i, block) in content_blocks.iter().enumerate() {
                let block_type = block["type"].as_str().unwrap_or("");
                match block_type {
                    "thinking" => {
                        let thinking_text = block["thinking"].as_str().unwrap_or("");
                        reasoning_content = Some(match reasoning_content {
                            Some(existing) => existing + thinking_text,
                            None => thinking_text.to_string(),
                        });
                    }
                    "text" => {
                        text_content.push_str(block["text"].as_str().unwrap_or(""));
                    }
                    "tool_use" => {
                        let tool_id = block["id"].as_str().unwrap_or("").to_string();
                        let tool_name = block["name"].as_str().unwrap_or("").to_string();
                        // Anthropic 的 input 是 JSON 对象，需要序列化为字符串以匹配内部格式
                        let arguments = serde_json::to_string(&block["input"])
                            .unwrap_or_else(|_| "{}".to_string());
                        tool_calls.push(LlmToolCall {
                            index: i as u32,
                            id: tool_id,
                            name: tool_name,
                            arguments,
                        });
                    }
                    _ => {
                        log::warn!("未知 content block 类型: {}", block_type);
                    }
                }
            }
        }

        // 映射 stop_reason 到 finish_reason（对齐 OpenAI 的值）
        let finish_reason = value["stop_reason"].as_str().map(|r| match r {
            "end_turn" => "stop".to_string(),
            "tool_use" => "tool_calls".to_string(),
            "max_tokens" => "length".to_string(),
            "stop_sequence" => "stop".to_string(),
            other => other.to_string(),
        });

        // 映射 usage（Anthropic 使用 input_tokens / output_tokens + 缓存字段）
        // 缓存字段兼容 DeepSeek 原生命名 (prompt_cache_hit_tokens) 和 Anthropic 命名 (cache_read_input_tokens)
        let usage = value["usage"].as_object().map(|u| {
            let (cache_hit, cache_miss, cache_creation) = extract_cache_fields(u);
            ChatUsage {
                prompt_tokens: u["input_tokens"].as_u64().unwrap_or(0),
                completion_tokens: u["output_tokens"].as_u64().unwrap_or(0),
                total_tokens: u["input_tokens"].as_u64().unwrap_or(0)
                    + u["output_tokens"].as_u64().unwrap_or(0),
                prompt_cache_hit_tokens: cache_hit,
                prompt_cache_miss_tokens: cache_miss,
                cache_creation_input_tokens: cache_creation,
                cache_read_input_tokens: u["cache_read_input_tokens"].as_u64().unwrap_or(0),
                cached_content_token_count: 0,
            }
        });

        Ok(ChatResponse {
            id,
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".to_string(),
                    content: text_content,
                    content_parts: None,
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                    tool_call_id: None,
                    reasoning_content,
                    attachments: None,
                    metadata: None,
                },
                finish_reason,
            }],
            usage,
        })
    }
}

/// 从 Anthropic/DeepSeek 响应中提取缓存字段
/// 兼容两种命名规范：
/// - Anthropic: cache_read_input_tokens
/// - DeepSeek:  prompt_cache_hit_tokens / prompt_cache_miss_tokens
fn extract_cache_fields(u: &serde_json::Map<String, serde_json::Value>) -> (u64, u64, u64) {
    let cache_hit = u
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        .max(
            u.get("prompt_cache_hit_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        );

    let cache_miss = u
        .get("prompt_cache_miss_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| {
            let input = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
            let read = u
                .get("cache_read_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            input.saturating_sub(read)
        });

    let creation = u
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    (cache_hit, cache_miss, creation)
}

#[async_trait]
impl LlmProvider for AnthropicAdapter {
    fn provider_name(&self) -> &str {
        &self.model
    }

    async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ChatResponse, CommandError> {
        log::info!("发送 Anthropic 非流式请求, model={}", self.model);
        let url = self.build_api_url();
        let body = self.build_request_body(messages, tools, false, None);
        let response = self.send_with_retry(&url, &body).await?;
        let value: Value = response.json().await.map_err(|e| {
            log::error!(
                "解析 Anthropic 非流式响应失败, model={}, 错误: {}",
                self.model,
                e
            );
            CommandError::llm(1000, format!("解析响应失败: {}", e))
        })?;
        log::info!("Anthropic 非流式请求完成, model={}", self.model);
        self.parse_response(value)
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<mpsc::Receiver<Result<StreamChunk, CommandError>>, CommandError> {
        log::info!("发送 Anthropic 流式请求, model={}", self.model);
        let url = self.build_api_url();
        let body = self.build_request_body(messages, tools, true, None);
        // 使用流式专用客户端（禁用压缩），避免 bytes_stream 解码错误
        let response = self.send_streaming_with_retry(&url, &body).await?;

        let (tx, rx) = mpsc::channel(100);
        let model_name = self.model.clone();

        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();
            // tool_use 块的计数器，用于生成 OpenAI 兼容的 tool_call index
            let mut tool_call_counter: u32 = 0;
            // 收集 message_start 中的输入 usage（含缓存字段）
            let mut input_usage: Option<ChatUsage> = None;

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&text);

                        // 解析 SSE 事件（以空行分隔）
                        while let Some(pos) = buffer.find("\n\n") {
                            let event_text = buffer[..pos].to_string();
                            buffer = buffer[pos + 2..].to_string();

                            // 提取 data 行内容
                            let mut data_line = String::new();
                            for line in event_text.lines() {
                                if let Some(d) = line.strip_prefix("data:") {
                                    data_line = d.trim().to_string();
                                }
                            }

                            if data_line.is_empty() {
                                continue;
                            }

                            let data: Value = match serde_json::from_str(&data_line) {
                                Ok(v) => v,
                                Err(e) => {
                                    log::error!(
                                        "解析 Anthropic SSE 数据失败, model={}, 错误: {}",
                                        model_name,
                                        e
                                    );
                                    let _ = tx
                                        .send(Err(CommandError::llm(
                                            1000,
                                            format!("解析 SSE 数据失败: {}", e),
                                        )))
                                        .await;
                                    continue;
                                }
                            };

                            // 根据 data 中的 type 字段判断事件类型
                            let msg_type = data["type"].as_str().unwrap_or("");

                            match msg_type {
                                "message_start" => {
                                    // 消息开始，提取 message id 和 role，以及 usage（含缓存字段）
                                    let msg_id =
                                        data["message"]["id"].as_str().unwrap_or("").to_string();

                                    // 捕获输入 usage（含 cache 字段，兼容 Anthropic 和 DeepSeek 命名）
                                    if let Some(u) = data["message"]["usage"].as_object() {
                                        let (cache_hit, cache_miss, cache_creation) =
                                            extract_cache_fields(u);
                                        input_usage = Some(ChatUsage {
                                            prompt_tokens: u["input_tokens"].as_u64().unwrap_or(0),
                                            completion_tokens: 0,
                                            total_tokens: u["input_tokens"].as_u64().unwrap_or(0),
                                            prompt_cache_hit_tokens: cache_hit,
                                            prompt_cache_miss_tokens: cache_miss,
                                            cache_creation_input_tokens: cache_creation,
                                            cache_read_input_tokens: u["cache_read_input_tokens"]
                                                .as_u64()
                                                .unwrap_or(0),
                                            cached_content_token_count: 0,
                                        });
                                    }

                                    let chunk = StreamChunk {
                                        id: msg_id,
                                        choices: vec![StreamChoice {
                                            index: 0,
                                            delta: StreamDelta {
                                                role: Some("assistant".to_string()),
                                                content: None,
                                                reasoning_content: None,
                                                tool_calls: None,
                                            },
                                            finish_reason: None,
                                        }],
                                        usage: None,
                                    };
                                    if tx.send(Ok(chunk)).await.is_err() {
                                        return;
                                    }
                                }
                                "content_block_start" => {
                                    let content_block = &data["content_block"];
                                    let block_type = content_block["type"].as_str().unwrap_or("");

                                    match block_type {
                                        "tool_use" => {
                                            // 工具调用开始，发送初始 tool_call 信息
                                            let tool_id = content_block["id"]
                                                .as_str()
                                                .unwrap_or("")
                                                .to_string();
                                            let tool_name = content_block["name"]
                                                .as_str()
                                                .unwrap_or("")
                                                .to_string();
                                            let current_tool_index = tool_call_counter;
                                            tool_call_counter += 1;
                                            let chunk = StreamChunk {
                                                id: String::new(),
                                                choices: vec![StreamChoice {
                                                    index: 0,
                                                    delta: StreamDelta {
                                                        role: None,
                                                        content: None,
                                                        reasoning_content: None,
                                                        tool_calls: Some(vec![LlmToolCall {
                                                            index: current_tool_index,
                                                            id: tool_id,
                                                            name: tool_name,
                                                            arguments: String::new(),
                                                        }]),
                                                    },
                                                    finish_reason: None,
                                                }],
                                                usage: None,
                                            };
                                            if tx.send(Ok(chunk)).await.is_err() {
                                                return;
                                            }
                                        }
                                        "text" => {}
                                        "thinking" => {}
                                        _ => {}
                                    }
                                }
                                "content_block_delta" => {
                                    let delta = &data["delta"];
                                    let delta_type = delta["type"].as_str().unwrap_or("");

                                    match delta_type {
                                        "text_delta" => {
                                            let text = delta["text"].as_str().unwrap_or("");
                                            let chunk = StreamChunk {
                                                id: String::new(),
                                                choices: vec![StreamChoice {
                                                    index: 0,
                                                    delta: StreamDelta {
                                                        role: None,
                                                        content: Some(text.to_string()),
                                                        reasoning_content: None,
                                                        tool_calls: None,
                                                    },
                                                    finish_reason: None,
                                                }],
                                                usage: None,
                                            };
                                            if tx.send(Ok(chunk)).await.is_err() {
                                                return;
                                            }
                                        }
                                        "input_json_delta" => {
                                            let partial_json =
                                                delta["partial_json"].as_str().unwrap_or("");
                                            let current_tool_index = if tool_call_counter > 0 {
                                                tool_call_counter - 1
                                            } else {
                                                0
                                            };
                                            let chunk = StreamChunk {
                                                id: String::new(),
                                                choices: vec![StreamChoice {
                                                    index: 0,
                                                    delta: StreamDelta {
                                                        role: None,
                                                        content: None,
                                                        reasoning_content: None,
                                                        tool_calls: Some(vec![LlmToolCall {
                                                            index: current_tool_index,
                                                            id: String::new(),
                                                            name: String::new(),
                                                            arguments: partial_json.to_string(),
                                                        }]),
                                                    },
                                                    finish_reason: None,
                                                }],
                                                usage: None,
                                            };
                                            if tx.send(Ok(chunk)).await.is_err() {
                                                return;
                                            }
                                        }
                                        "thinking_delta" => {
                                            let thinking_text =
                                                delta["thinking"].as_str().unwrap_or("");
                                            let chunk = StreamChunk {
                                                id: String::new(),
                                                choices: vec![StreamChoice {
                                                    index: 0,
                                                    delta: StreamDelta {
                                                        role: None,
                                                        content: None,
                                                        reasoning_content: Some(
                                                            thinking_text.to_string(),
                                                        ),
                                                        tool_calls: None,
                                                    },
                                                    finish_reason: None,
                                                }],
                                                usage: None,
                                            };
                                            if tx.send(Ok(chunk)).await.is_err() {
                                                return;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                "content_block_stop" => {}
                                "message_delta" => {
                                    // 消息级别更新，包含 stop_reason 和 usage（output_tokens）
                                    let stop_reason =
                                        data["delta"]["stop_reason"].as_str().map(|r| match r {
                                            "end_turn" => "stop".to_string(),
                                            "tool_use" => "tool_calls".to_string(),
                                            "max_tokens" => "length".to_string(),
                                            "stop_sequence" => "stop".to_string(),
                                            other => other.to_string(),
                                        });

                                    // 从 message_delta 提取 output 用量，与 input_usage 合并
                                    let usage =
                                        data.get("usage").and_then(|u| u.as_object()).map(|u| {
                                            let output_tokens =
                                                u["output_tokens"].as_u64().unwrap_or(0);
                                            let total_input = input_usage
                                                .as_ref()
                                                .map(|i| i.prompt_tokens)
                                                .unwrap_or(0);
                                            ChatUsage {
                                                prompt_tokens: total_input,
                                                completion_tokens: output_tokens,
                                                total_tokens: total_input + output_tokens,
                                                prompt_cache_hit_tokens: input_usage
                                                    .as_ref()
                                                    .map(|i| i.prompt_cache_hit_tokens)
                                                    .unwrap_or(0),
                                                prompt_cache_miss_tokens: input_usage
                                                    .as_ref()
                                                    .map(|i| i.prompt_cache_miss_tokens)
                                                    .unwrap_or(0),
                                                cache_creation_input_tokens: input_usage
                                                    .as_ref()
                                                    .map(|i| i.cache_creation_input_tokens)
                                                    .unwrap_or(0),
                                                cache_read_input_tokens: input_usage
                                                    .as_ref()
                                                    .map(|i| i.cache_read_input_tokens)
                                                    .unwrap_or(0),
                                                cached_content_token_count: 0,
                                            }
                                        });

                                    let chunk = StreamChunk {
                                        id: String::new(),
                                        choices: vec![StreamChoice {
                                            index: 0,
                                            delta: StreamDelta {
                                                role: None,
                                                content: None,
                                                reasoning_content: None,
                                                tool_calls: None,
                                            },
                                            finish_reason: stop_reason,
                                        }],
                                        usage,
                                    };
                                    if tx.send(Ok(chunk)).await.is_err() {
                                        return;
                                    }
                                }
                                "message_stop" => {
                                    // 消息结束，关闭流
                                    return;
                                }
                                "ping" => {
                                    // 心跳事件，忽略
                                }
                                "error" => {
                                    // 流式错误事件
                                    let error_msg =
                                        data["error"]["message"].as_str().unwrap_or("未知错误");
                                    log::error!(
                                        "Anthropic 流式错误, model={}, 错误: {}",
                                        model_name,
                                        error_msg
                                    );
                                    let _ = tx
                                        .send(Err(CommandError::llm(
                                            1000,
                                            format!("Anthropic 流式错误: {}", error_msg),
                                        )))
                                        .await;
                                    return;
                                }
                                _ => {
                                    log::debug!("忽略未知 Anthropic SSE 事件类型: {}", msg_type);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("流读取错误, model={}, 错误: {}", model_name, e);
                        let _ = tx
                            .send(Err(CommandError::llm(1000, format!("流读取错误: {}", e))))
                            .await;
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }

    /// 流式对话，支持覆盖 max_tokens 参数
    /// 用于响应截断时以更大的 max_tokens 重试
    async fn chat_stream_with_max_tokens(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        max_tokens_override: u32,
    ) -> Result<mpsc::Receiver<Result<StreamChunk, CommandError>>, CommandError> {
        log::info!(
            "发送 Anthropic 流式请求 (max_tokens={}), model={}",
            max_tokens_override,
            self.model
        );
        let url = self.build_api_url();
        let body = self.build_request_body(messages, tools, true, Some(max_tokens_override));
        // 使用流式专用客户端（禁用压缩），避免 bytes_stream 解码错误
        let response = self.send_streaming_with_retry(&url, &body).await?;

        let (tx, rx) = mpsc::channel(100);
        let model_name = self.model.clone();

        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();
            // tool_use 块的计数器，用于生成 OpenAI 兼容的 tool_call index
            let mut tool_call_counter: u32 = 0;
            // 收集 message_start 中的输入 usage（含缓存字段）
            let mut input_usage: Option<ChatUsage> = None;

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&text);

                        // 解析 SSE 事件（以空行分隔）
                        while let Some(pos) = buffer.find("\n\n") {
                            let event_text = buffer[..pos].to_string();
                            buffer = buffer[pos + 2..].to_string();

                            // 解析事件类型和数据
                            let mut event_type = String::new();
                            let mut event_data = String::new();

                            for line in event_text.lines() {
                                if let Some(et) = line.strip_prefix("event: ") {
                                    event_type = et.to_string();
                                } else if let Some(d) = line.strip_prefix("data: ") {
                                    event_data = d.to_string();
                                } else if let Some(d) = line.strip_prefix("data:") {
                                    event_data = d.to_string();
                                }
                            }

                            if event_data.is_empty() {
                                continue;
                            }

                            let data = event_data.trim();
                            let value: Value = match serde_json::from_str(data) {
                                Ok(v) => v,
                                Err(e) => {
                                    log::error!(
                                        "解析 Anthropic SSE 数据失败, model={}, 错误: {}",
                                        model_name,
                                        e
                                    );
                                    let _ = tx
                                        .send(Err(CommandError::llm(
                                            1000,
                                            format!("解析 SSE 数据失败: {}", e),
                                        )))
                                        .await;
                                    continue;
                                }
                            };

                            match event_type.as_str() {
                                "message_start" => {
                                    // 消息开始，提取 message id 和 usage（含缓存字段）
                                    let _ =
                                        value["message"]["id"].as_str().unwrap_or("").to_string();

                                    // 捕获输入 usage（含 cache 字段，兼容 Anthropic 和 DeepSeek 命名）
                                    if let Some(u) = value["message"]["usage"].as_object() {
                                        let (cache_hit, cache_miss, cache_creation) =
                                            extract_cache_fields(u);
                                        input_usage = Some(ChatUsage {
                                            prompt_tokens: u["input_tokens"].as_u64().unwrap_or(0),
                                            completion_tokens: 0,
                                            total_tokens: u["input_tokens"].as_u64().unwrap_or(0),
                                            prompt_cache_hit_tokens: cache_hit,
                                            prompt_cache_miss_tokens: cache_miss,
                                            cache_creation_input_tokens: cache_creation,
                                            cache_read_input_tokens: u["cache_read_input_tokens"]
                                                .as_u64()
                                                .unwrap_or(0),
                                            cached_content_token_count: 0,
                                        });
                                    }
                                }
                                "content_block_start" => {
                                    // 新内容块开始
                                    if let Some(content_block) = value.get("content_block") {
                                        if content_block["type"] == "tool_use" {
                                            // tool_use 块开始，提前发射 tool_call 事件
                                            let tool_name = content_block["name"]
                                                .as_str()
                                                .unwrap_or("")
                                                .to_string();
                                            let tool_id = content_block["id"]
                                                .as_str()
                                                .unwrap_or("")
                                                .to_string();
                                            let index = tool_call_counter;
                                            tool_call_counter += 1;

                                            let chunk = StreamChunk {
                                                id: String::new(),
                                                choices: vec![StreamChoice {
                                                    index: 0,
                                                    delta: StreamDelta {
                                                        role: None,
                                                        content: None,
                                                        reasoning_content: None,
                                                        tool_calls: Some(vec![LlmToolCall {
                                                            index,
                                                            id: tool_id,
                                                            name: tool_name,
                                                            arguments: String::new(),
                                                        }]),
                                                    },
                                                    finish_reason: None,
                                                }],
                                                usage: None,
                                            };
                                            if tx.send(Ok(chunk)).await.is_err() {
                                                return;
                                            }
                                        }
                                    }
                                }
                                "content_block_delta" => {
                                    // 内容块增量
                                    if let Some(delta) = value.get("delta") {
                                        let delta_type = delta["type"].as_str().unwrap_or("");

                                        match delta_type {
                                            "text_delta" => {
                                                // 文本增量
                                                let text = delta["text"].as_str().unwrap_or("");
                                                let chunk = StreamChunk {
                                                    id: String::new(),
                                                    choices: vec![StreamChoice {
                                                        index: 0,
                                                        delta: StreamDelta {
                                                            role: None,
                                                            content: Some(text.to_string()),
                                                            reasoning_content: None,
                                                            tool_calls: None,
                                                        },
                                                        finish_reason: None,
                                                    }],
                                                    usage: None,
                                                };
                                                if tx.send(Ok(chunk)).await.is_err() {
                                                    return;
                                                }
                                            }
                                            "thinking_delta" => {
                                                // 思考增量（Extended Thinking）
                                                let thought =
                                                    delta["thinking"].as_str().unwrap_or("");
                                                let chunk = StreamChunk {
                                                    id: String::new(),
                                                    choices: vec![StreamChoice {
                                                        index: 0,
                                                        delta: StreamDelta {
                                                            role: None,
                                                            content: None,
                                                            reasoning_content: Some(
                                                                thought.to_string(),
                                                            ),
                                                            tool_calls: None,
                                                        },
                                                        finish_reason: None,
                                                    }],
                                                    usage: None,
                                                };
                                                if tx.send(Ok(chunk)).await.is_err() {
                                                    return;
                                                }
                                            }
                                            "input_json_delta" => {
                                                // tool_use 参数增量
                                                let partial_json =
                                                    delta["partial_json"].as_str().unwrap_or("");
                                                // 找到当前 tool_call 的索引（最后一个）
                                                let index = tool_call_counter.saturating_sub(1);
                                                let chunk = StreamChunk {
                                                    id: String::new(),
                                                    choices: vec![StreamChoice {
                                                        index: 0,
                                                        delta: StreamDelta {
                                                            role: None,
                                                            content: None,
                                                            reasoning_content: None,
                                                            tool_calls: Some(vec![LlmToolCall {
                                                                index,
                                                                id: String::new(),
                                                                name: String::new(),
                                                                arguments: partial_json.to_string(),
                                                            }]),
                                                        },
                                                        finish_reason: None,
                                                    }],
                                                    usage: None,
                                                };
                                                if tx.send(Ok(chunk)).await.is_err() {
                                                    return;
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                "content_block_stop" => {
                                    // 内容块结束，无需特殊处理
                                }
                                "message_delta" => {
                                    // 消息级增量（包含 stop_reason 和 usage）
                                    if let Some(delta) = value.get("delta") {
                                        let stop_reason =
                                            delta["stop_reason"].as_str().map(|r| match r {
                                                "end_turn" => "stop".to_string(),
                                                "tool_use" => "tool_calls".to_string(),
                                                "max_tokens" => "length".to_string(),
                                                "stop_sequence" => "stop".to_string(),
                                                other => other.to_string(),
                                            });

                                        // 从 message_delta 提取 output 用量，与 input_usage 合并
                                        let usage = value
                                            .get("usage")
                                            .and_then(|u| u.as_object())
                                            .map(|u| {
                                                let output_tokens =
                                                    u["output_tokens"].as_u64().unwrap_or(0);
                                                let total_input = input_usage
                                                    .as_ref()
                                                    .map(|i| i.prompt_tokens)
                                                    .unwrap_or(0);
                                                ChatUsage {
                                                    prompt_tokens: total_input,
                                                    completion_tokens: output_tokens,
                                                    total_tokens: total_input + output_tokens,
                                                    prompt_cache_hit_tokens: input_usage
                                                        .as_ref()
                                                        .map(|i| i.prompt_cache_hit_tokens)
                                                        .unwrap_or(0),
                                                    prompt_cache_miss_tokens: input_usage
                                                        .as_ref()
                                                        .map(|i| i.prompt_cache_miss_tokens)
                                                        .unwrap_or(0),
                                                    cache_creation_input_tokens: input_usage
                                                        .as_ref()
                                                        .map(|i| i.cache_creation_input_tokens)
                                                        .unwrap_or(0),
                                                    cache_read_input_tokens: input_usage
                                                        .as_ref()
                                                        .map(|i| i.cache_read_input_tokens)
                                                        .unwrap_or(0),
                                                    cached_content_token_count: 0,
                                                }
                                            });

                                        let chunk = StreamChunk {
                                            id: String::new(),
                                            choices: vec![StreamChoice {
                                                index: 0,
                                                delta: StreamDelta {
                                                    role: None,
                                                    content: None,
                                                    reasoning_content: None,
                                                    tool_calls: None,
                                                },
                                                finish_reason: stop_reason,
                                            }],
                                            usage,
                                        };
                                        if tx.send(Ok(chunk)).await.is_err() {
                                            return;
                                        }
                                    }
                                }
                                "message_stop" => {
                                    // 消息结束
                                    return;
                                }
                                "ping" => {
                                    // 心跳，忽略
                                }
                                "error" => {
                                    let error_msg =
                                        value["error"]["message"].as_str().unwrap_or("未知错误");
                                    log::error!(
                                        "Anthropic SSE 错误事件, model={}, 错误: {}",
                                        model_name,
                                        error_msg
                                    );
                                    let _ = tx
                                        .send(Err(CommandError::llm(
                                            1000,
                                            format!("API 错误: {}", error_msg),
                                        )))
                                        .await;
                                    return;
                                }
                                _ => {
                                    log::debug!("忽略未知 Anthropic SSE 事件类型: {}", event_type);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("流读取错误, model={}, 错误: {}", model_name, e);
                        let _ = tx
                            .send(Err(CommandError::llm(1000, format!("流读取错误: {}", e))))
                            .await;
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }

    async fn test_connection(&self) -> Result<ConnectionResult, CommandError> {
        log::info!("测试 Anthropic 连接, model={}", self.model);
        let start = std::time::Instant::now();
        let test_messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "Hi".to_string(),
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            attachments: None,
            metadata: None,
        }];
        let url = self.build_api_url();
        let body = self.build_request_body(&test_messages, &[], false, None);

        match self.send_with_retry(&url, &body).await {
            Ok(response) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                let value: Value = response.json().await.unwrap_or_default();
                let model_name = value["model"].as_str().unwrap_or(&self.model).to_string();
                log::info!(
                    "Anthropic 连接测试成功, model={}, 延迟={}ms",
                    model_name,
                    latency_ms
                );
                Ok(ConnectionResult {
                    success: true,
                    provider_id: None,
                    latency_ms,
                    model_info: None,
                    model: Some(model_name),
                    error_message: None,
                    error: None,
                })
            }
            Err(e) => {
                log::error!(
                    "Anthropic 连接测试失败, model={}, 错误: {}",
                    self.model,
                    e.message
                );
                Ok(ConnectionResult {
                    success: false,
                    provider_id: None,
                    latency_ms: start.elapsed().as_millis() as u64,
                    model_info: None,
                    model: None,
                    error_message: Some(e.message.clone()),
                    error: Some(e.message.clone()),
                })
            }
        }
    }

    fn rebuild_client(&mut self) {
        let timeout = Duration::from_secs(self.advanced.timeout_seconds as u64);
        self.client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        self.streaming_client = Client::builder()
            .timeout(Duration::from_secs(300))
            .no_gzip()
            .no_deflate()
            .no_brotli()
            .tcp_keepalive(Some(Duration::from_secs(60)))
            .build()
            .unwrap_or_default();

        log::info!("Anthropic 适配器客户端已重建");
    }

    fn get_max_tokens(&self) -> u32 {
        self.advanced.max_tokens
    }

    /// 轻量级健康检查：仅发送 HEAD 请求到 /v1/messages 端点
    /// 返回 405(方法不允许) 也说明网络可达
    async fn lightweight_health_check(&self) -> Result<ConnectionResult, CommandError> {
        let start = std::time::Instant::now();
        let url = self.build_api_url();

        let result = self
            .client
            .head(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .timeout(Duration::from_secs(10))
            .send()
            .await;

        match result {
            Ok(response) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                let status = response.status().as_u16();
                // 200/401/403/404/405 都说明网络可达
                let reachable = status < 500;
                log::info!(
                    "轻量级健康检查, model={}, status={}, 可达={}, 延迟={}ms",
                    self.model,
                    status,
                    reachable,
                    latency_ms
                );
                Ok(ConnectionResult {
                    success: reachable,
                    provider_id: None,
                    latency_ms,
                    model_info: None,
                    model: None,
                    error_message: if reachable {
                        None
                    } else {
                        Some(format!("服务端错误 ({})", status))
                    },
                    error: None,
                })
            }
            Err(e) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                log::warn!("轻量级健康检查失败, model={}, 错误: {}", self.model, e);
                Ok(ConnectionResult {
                    success: false,
                    provider_id: None,
                    latency_ms,
                    model_info: None,
                    model: None,
                    error_message: Some(format!("连接失败: {}", e)),
                    error: None,
                })
            }
        }
    }
}
