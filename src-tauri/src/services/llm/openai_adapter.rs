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

/// OpenAI 兼容 API 适配器
/// 支持 OpenAI、Azure OpenAI、以及所有兼容 OpenAI API 格式的服务
pub struct OpenAiAdapter {
    api_base_url: String,
    api_key: String,
    model: String,
    advanced: AdvancedConfig,
    /// 用于非流式请求的客户端（支持压缩）
    client: Client,
    /// 用于流式请求的客户端（禁用压缩，避免 bytes_stream 解码错误）
    streaming_client: Client,
}

impl OpenAiAdapter {
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

    /// 构建请求体
    /// max_tokens_override: 可选的 max_tokens 覆盖值，用于截断重试时增大输出限制
    fn build_request_body(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        stream: bool,
        max_tokens_override: Option<u32>,
    ) -> Value {
        let mut body = json!({
            "model": self.model,
            "messages": messages.iter().map(|m| {
                // 构建 content 字段：支持多模态消息
                let content_value = if let Some(parts) = &m.content_parts {
                    if !parts.is_empty() {
                        // 多模态消息：将 content_parts 转换为 OpenAI Vision API 格式的 JSON 数组
                        json!(parts.iter().map(|part| {
                            match part {
                                ContentPart::Text { text } => json!({
                                    "type": "text",
                                    "text": text,
                                }),
                                ContentPart::Image { mime_type, data } => json!({
                                    "type": "image_url",
                                    "image_url": {
                                        "url": format!("data:{};base64,{}", mime_type, data),
                                        "detail": "auto",
                                    },
                                }),
                            }
                        }).collect::<Vec<_>>())
                    } else {
                        json!(m.content)
                    }
                } else {
                    json!(m.content)
                };

                let mut msg = json!({
                    "role": m.role,
                    "content": content_value,
                });

                if let Some(rc) = &m.reasoning_content {
                    if self.advanced.reasoning_in_content {
                        // 不支持 reasoning_content 输入的 Provider（OpenAI/Ollama）：
                        // 将思考内容用 <agent-reasoning> 标签包裹后合并到 content 字段
                        // 使用不常见的标签名避免被 LLM 误解为用户指令
                        let merged_content = format!(
                            "<agent-reasoning>\n{}\n</agent-reasoning>\n{}",
                            rc,
                            m.content
                        );
                        msg["content"] = json!(merged_content);
                    } else {
                        // 支持 reasoning_content 输入的 Provider（DeepSeek）：
                        // 保持原样发送 reasoning_content 字段
                        msg["reasoning_content"] = json!(rc);
                    }
                }

                if let Some(tool_calls) = &m.tool_calls {
                    msg["tool_calls"] = json!(tool_calls.iter().map(|tc| {
                        json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": tc.arguments,
                            }
                        })
                    }).collect::<Vec<_>>());
                }
                if let Some(call_id) = &m.tool_call_id {
                    msg["tool_call_id"] = json!(call_id);
                }
                msg
            }).collect::<Vec<_>>(),
            "stream": stream,
        });

        if !tools.is_empty() {
            body["tools"] = json!(tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect::<Vec<_>>());
        }

        body["temperature"] = json!(self.advanced.temperature);
        body["max_tokens"] = json!(max_tokens_override.unwrap_or(self.advanced.max_tokens));
        body["top_p"] = json!(self.advanced.top_p);

        // 启用工具调用流式输出（tool_stream）
        // 智谱 GLM-5/GLM-4.7/GLM-4.6 系列模型默认 tool_stream=false，
        // 即流式响应中 tool_calls 不以增量方式返回，而是在参数完全生成后一次性返回，
        // 导致前端在 content 输出完毕后需等待很久才显示工具加载动画。
        // 设置 tool_stream=true 后，tool_calls 以增量 delta 方式流式输出，
        // 前端可立即检测到工具名称并显示加载状态。
        // 其他 OpenAI 兼容 API 会忽略未知参数，不影响兼容性。
        if stream && !tools.is_empty() {
            body["tool_stream"] = json!(true);
        }

        body
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
    /// DNS 解析失败时使用更短的重试间隔（200ms）并额外增加1次重试机会
    async fn send_with_retry_internal(
        &self,
        url: &str,
        body: &Value,
        client: &Client,
    ) -> Result<reqwest::Response, CommandError> {
        let max_retries = self.advanced.max_retries;
        let mut _last_error: Option<CommandError> = None;
        let mut dns_extra_retry = true;
        let mut is_dns_failure = false;
        let mut total_attempt: u32 = 0;

        loop {
            // 计算当前是第几次尝试（含 DNS 额外重试）
            total_attempt += 1;

            if total_attempt > 1 {
                let delay = if is_dns_failure {
                    // DNS 解析失败使用更短的重试间隔
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
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json");

            // 添加额外请求头
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
                    // 5xx 服务端错误，可重试
                    if status.as_u16() >= 500 && total_attempt <= max_retries {
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
                    // DNS 解析失败特殊处理：额外增加1次重试机会
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

            // 非可重试错误，直接跳出
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

    /// 解析非流式响应
    fn parse_response(&self, value: Value) -> Result<ChatResponse, CommandError> {
        let id = value["id"].as_str().unwrap_or("").to_string();
        let choices = value["choices"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|c| {
                        let index = c["index"].as_u64().unwrap_or(0) as u32;
                        let message = &c["message"];
                        let role = message["role"].as_str().unwrap_or("assistant").to_string();
                        let content = message["content"].as_str().unwrap_or("").to_string();

                        let tool_calls = message["tool_calls"].as_array().map(|tc_arr| {
                            tc_arr
                                .iter()
                                .map(|tc| {
                                    let index = tc["index"].as_u64().unwrap_or(0) as u32;
                                    let id = tc["id"].as_str().unwrap_or("").to_string();
                                    let func = &tc["function"];
                                    let name = func["name"].as_str().unwrap_or("").to_string();
                                    let arguments =
                                        func["arguments"].as_str().unwrap_or("{}").to_string();
                                    LlmToolCall {
                                        index,
                                        id,
                                        name,
                                        arguments,
                                    }
                                })
                                .collect::<Vec<_>>()
                        });

                        let finish_reason = c["finish_reason"].as_str().map(String::from);

                        ChatChoice {
                            index,
                            message: ChatMessage {
                                role,
                                content,
                                content_parts: None,
                                tool_calls,
                                tool_call_id: None,
                                reasoning_content: message["reasoning_content"]
                                    .as_str()
                                    .map(String::from),
                                attachments: None,
                                metadata: None,
                            },
                            finish_reason,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let usage = value["usage"].as_object().map(|u| ChatUsage {
            prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0),
            completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0),
            total_tokens: u["total_tokens"].as_u64().unwrap_or(0),
            prompt_cache_hit_tokens: u["prompt_cache_hit_tokens"].as_u64().unwrap_or(0),
            prompt_cache_miss_tokens: u["prompt_cache_miss_tokens"].as_u64().unwrap_or(0),
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            cached_content_token_count: 0,
        });

        Ok(ChatResponse { id, choices, usage })
    }
}

#[async_trait]
impl LlmProvider for OpenAiAdapter {
    fn provider_name(&self) -> &str {
        &self.model
    }

    async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ChatResponse, CommandError> {
        log::info!("发送非流式请求, model={}", self.model);
        let url = format!(
            "{}/chat/completions",
            self.api_base_url.trim_end_matches('/')
        );
        let body = self.build_request_body(messages, tools, false, None);
        let response = self.send_with_retry(&url, &body).await?;
        let value: Value = response.json().await.map_err(|e| {
            log::error!("解析非流式响应失败, model={}, 错误: {}", self.model, e);
            CommandError::llm(1000, format!("解析响应失败: {}", e))
        })?;
        log::info!("非流式请求完成, model={}", self.model);
        self.parse_response(value)
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<mpsc::Receiver<Result<StreamChunk, CommandError>>, CommandError> {
        log::info!("发送流式请求, model={}", self.model);
        let url = format!(
            "{}/chat/completions",
            self.api_base_url.trim_end_matches('/')
        );
        let body = self.build_request_body(messages, tools, true, None);
        // 使用流式专用客户端（禁用压缩），避免 bytes_stream 解码错误
        let response = self.send_streaming_with_retry(&url, &body).await?;

        let (tx, rx) = mpsc::channel(100);
        let model_name = self.model.clone();

        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&text);

                        // 解析 SSE 事件
                        while let Some(pos) = buffer.find("\n\n") {
                            let event_text = buffer[..pos].to_string();
                            buffer = buffer[pos + 2..].to_string();

                            for line in event_text.lines() {
                                // SSE 规范允许 data: 后有无空格，先尝试带空格再尝试无空格
                                let data = line
                                    .strip_prefix("data: ")
                                    .or_else(|| line.strip_prefix("data:"));
                                if let Some(data) = data {
                                    let data = data.trim();
                                    if data == "[DONE]" {
                                        // 流式响应正常结束，直接关闭 channel（drop sender）
                                        // executor 侧通过 recv() 返回 None 检测流结束
                                        return;
                                    }

                                    match serde_json::from_str::<Value>(data) {
                                        Ok(value) => {
                                            let id = value["id"].as_str().unwrap_or("").to_string();
                                            let choices = value["choices"]
                                                .as_array()
                                                .map(|arr| {
                                                    arr.iter()
                                                        .map(|c| {
                                                            let index =
                                                                c["index"].as_u64().unwrap_or(0)
                                                                    as u32;
                                                            let delta = &c["delta"];
                                                            let role = delta["role"]
                                                                .as_str()
                                                                .map(String::from);
                                                            let content = delta["content"]
                                                                .as_str()
                                                                .map(String::from);
                                                            let reasoning_content = delta
                                                                ["reasoning_content"]
                                                                .as_str()
                                                                .map(String::from);
                                                            let tool_calls = delta["tool_calls"]
                                                                .as_array()
                                                                .map(|tc_arr| {
                                                                    tc_arr
                                                                        .iter()
                                                                        .map(|tc| {
                                                                            let index = tc["index"]
                                                                                .as_u64()
                                                                                .unwrap_or(0)
                                                                                as u32;
                                                                            let id = tc["id"]
                                                                                .as_str()
                                                                                .unwrap_or("")
                                                                                .to_string();
                                                                            let func =
                                                                                &tc["function"];
                                                                            let name = func["name"]
                                                                                .as_str()
                                                                                .unwrap_or("")
                                                                                .to_string();
                                                                            let arguments = func
                                                                                ["arguments"]
                                                                                .as_str()
                                                                                .unwrap_or("")
                                                                                .to_string();
                                                                            LlmToolCall {
                                                                                index,
                                                                                id,
                                                                                name,
                                                                                arguments,
                                                                            }
                                                                        })
                                                                        .collect::<Vec<_>>()
                                                                });
                                                            let finish_reason = c["finish_reason"]
                                                                .as_str()
                                                                .map(String::from);

                                                            StreamChoice {
                                                                index,
                                                                delta: StreamDelta {
                                                                    role,
                                                                    content,
                                                                    reasoning_content,
                                                                    tool_calls,
                                                                },
                                                                finish_reason,
                                                            }
                                                        })
                                                        .collect::<Vec<_>>()
                                                })
                                                .unwrap_or_default();

                                            // 提取 usage（仅在最后一个 chunk 中存在）
                                            let usage = value.get("usage").map(|u| ChatUsage {
                                                prompt_tokens: u["prompt_tokens"]
                                                    .as_u64()
                                                    .unwrap_or(0),
                                                completion_tokens: u["completion_tokens"]
                                                    .as_u64()
                                                    .unwrap_or(0),
                                                total_tokens: u["total_tokens"]
                                                    .as_u64()
                                                    .unwrap_or(0),
                                                prompt_cache_hit_tokens: u
                                                    ["prompt_cache_hit_tokens"]
                                                    .as_u64()
                                                    .unwrap_or(0),
                                                prompt_cache_miss_tokens: u
                                                    ["prompt_cache_miss_tokens"]
                                                    .as_u64()
                                                    .unwrap_or(0),
                                                cache_creation_input_tokens: 0,
                                                cache_read_input_tokens: 0,
                                                cached_content_token_count: 0,
                                            });

                                            let chunk = StreamChunk { id, choices, usage };
                                            if tx.send(Ok(chunk)).await.is_err() {
                                                return;
                                            }
                                        }
                                        Err(e) => {
                                            log::error!(
                                                "解析 SSE 数据失败, model={}, 错误: {}",
                                                model_name,
                                                e
                                            );
                                            let _ = tx
                                                .send(Err(CommandError::llm(
                                                    1000,
                                                    format!("解析 SSE 数据失败: {}", e),
                                                )))
                                                .await;
                                        }
                                    }
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
            "发送流式请求 (max_tokens={}), model={}",
            max_tokens_override,
            self.model
        );
        let url = format!(
            "{}/chat/completions",
            self.api_base_url.trim_end_matches('/')
        );
        let body = self.build_request_body(messages, tools, true, Some(max_tokens_override));
        // 使用流式专用客户端（禁用压缩），避免 bytes_stream 解码错误
        let response = self.send_streaming_with_retry(&url, &body).await?;

        let (tx, rx) = mpsc::channel(100);
        let model_name = self.model.clone();

        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&text);

                        // 解析 SSE 事件（以空行分隔）
                        while let Some(pos) = buffer.find("\n\n") {
                            let event_text = buffer[..pos].to_string();
                            buffer = buffer[pos + 2..].to_string();

                            for line in event_text.lines() {
                                // SSE 规范允许 data: 后有无空格，先尝试带空格再尝试无空格
                                let data = line
                                    .strip_prefix("data: ")
                                    .or_else(|| line.strip_prefix("data:"));
                                if let Some(data) = data {
                                    let data = data.trim();
                                    if data == "[DONE]" {
                                        return;
                                    }

                                    match serde_json::from_str::<Value>(data) {
                                        Ok(value) => {
                                            let id = value["id"].as_str().unwrap_or("").to_string();
                                            let choices = value["choices"]
                                                .as_array()
                                                .map(|arr| {
                                                    arr.iter()
                                                        .map(|c| {
                                                            let index =
                                                                c["index"].as_u64().unwrap_or(0)
                                                                    as u32;
                                                            let delta = &c["delta"];
                                                            let role = delta["role"]
                                                                .as_str()
                                                                .map(String::from);
                                                            let content = delta["content"]
                                                                .as_str()
                                                                .map(String::from);
                                                            let reasoning_content = delta
                                                                ["reasoning_content"]
                                                                .as_str()
                                                                .map(String::from);
                                                            let tool_calls = delta["tool_calls"]
                                                                .as_array()
                                                                .map(|tc_arr| {
                                                                    tc_arr
                                                                        .iter()
                                                                        .map(|tc| {
                                                                            let index = tc["index"]
                                                                                .as_u64()
                                                                                .unwrap_or(0)
                                                                                as u32;
                                                                            let id = tc["id"]
                                                                                .as_str()
                                                                                .unwrap_or("")
                                                                                .to_string();
                                                                            let func =
                                                                                &tc["function"];
                                                                            let name = func["name"]
                                                                                .as_str()
                                                                                .unwrap_or("")
                                                                                .to_string();
                                                                            let arguments = func
                                                                                ["arguments"]
                                                                                .as_str()
                                                                                .unwrap_or("")
                                                                                .to_string();
                                                                            LlmToolCall {
                                                                                index,
                                                                                id,
                                                                                name,
                                                                                arguments,
                                                                            }
                                                                        })
                                                                        .collect::<Vec<_>>()
                                                                });
                                                            let finish_reason = c["finish_reason"]
                                                                .as_str()
                                                                .map(String::from);

                                                            StreamChoice {
                                                                index,
                                                                delta: StreamDelta {
                                                                    role,
                                                                    content,
                                                                    reasoning_content,
                                                                    tool_calls,
                                                                },
                                                                finish_reason,
                                                            }
                                                        })
                                                        .collect::<Vec<_>>()
                                                })
                                                .unwrap_or_default();

                                            // 提取 usage（仅在最后一个 chunk 中存在）
                                            let usage = value.get("usage").map(|u| ChatUsage {
                                                prompt_tokens: u["prompt_tokens"]
                                                    .as_u64()
                                                    .unwrap_or(0),
                                                completion_tokens: u["completion_tokens"]
                                                    .as_u64()
                                                    .unwrap_or(0),
                                                total_tokens: u["total_tokens"]
                                                    .as_u64()
                                                    .unwrap_or(0),
                                                prompt_cache_hit_tokens: u
                                                    ["prompt_cache_hit_tokens"]
                                                    .as_u64()
                                                    .unwrap_or(0),
                                                prompt_cache_miss_tokens: u
                                                    ["prompt_cache_miss_tokens"]
                                                    .as_u64()
                                                    .unwrap_or(0),
                                                cache_creation_input_tokens: 0,
                                                cache_read_input_tokens: 0,
                                                cached_content_token_count: 0,
                                            });

                                            let chunk = StreamChunk { id, choices, usage };
                                            if tx.send(Ok(chunk)).await.is_err() {
                                                return;
                                            }
                                        }
                                        Err(e) => {
                                            log::error!(
                                                "解析 SSE 数据失败, model={}, 错误: {}",
                                                model_name,
                                                e
                                            );
                                            let _ = tx
                                                .send(Err(CommandError::llm(
                                                    1000,
                                                    format!("解析 SSE 数据失败: {}", e),
                                                )))
                                                .await;
                                        }
                                    }
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
        log::info!("测试连接, model={}", self.model);
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
        let url = format!(
            "{}/chat/completions",
            self.api_base_url.trim_end_matches('/')
        );
        let body = self.build_request_body(&test_messages, &[], false, None);

        match self.send_with_retry(&url, &body).await {
            Ok(response) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                let value: Value = response.json().await.unwrap_or_default();
                let model_name = value["model"].as_str().unwrap_or(&self.model).to_string();
                log::info!("连接测试成功, model={}, 延迟={}ms", model_name, latency_ms);
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
                log::error!("连接测试失败, model={}, 错误: {}", self.model, e.message);
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

        log::info!("OpenAI 适配器客户端已重建");
    }

    fn get_max_tokens(&self) -> u32 {
        self.advanced.max_tokens
    }

    /// 轻量级健康检查：仅发送 HEAD 请求到 /v1/models 端点
    /// 如果返回 200/401/403/404 均视为网络可达（说明 API 端点在线）
    async fn lightweight_health_check(&self) -> Result<ConnectionResult, CommandError> {
        let start = std::time::Instant::now();
        let url = format!("{}/models", self.api_base_url.trim_end_matches('/'));

        let result = self
            .client
            .head(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .timeout(Duration::from_secs(10))
            .send()
            .await;

        match result {
            Ok(response) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                let status = response.status().as_u16();
                // 200/401/403/404/405 都说明网络可达，API 端点在线
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::llm_config::AdvancedConfig;

    /// 辅助函数：创建指定 reasoning_in_content 的 AdvancedConfig
    fn advanced_with_reasoning_in_content(val: bool) -> AdvancedConfig {
        AdvancedConfig {
            reasoning_in_content: val,
            ..AdvancedConfig::default()
        }
    }

    /// 辅助函数：创建 OpenAiAdapter
    fn create_adapter(advanced: AdvancedConfig) -> OpenAiAdapter {
        OpenAiAdapter::new(
            "https://api.openai.com/v1".to_string(),
            "test-key".to_string(),
            "gpt-4".to_string(),
            advanced,
        )
    }

    /// 测试 reasoning_in_content=true 时，reasoning_content 被折叠到 content 字段
    #[test]
    fn test_build_request_body_reasoning_in_content_true() {
        let adapter = create_adapter(advanced_with_reasoning_in_content(true));
        let messages = vec![ChatMessage {
            role: "assistant".to_string(),
            content: "这是回复内容".to_string(),
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: Some("这是思考过程".to_string()),
            attachments: None,
            metadata: None,
        }];

        let body = adapter.build_request_body(&messages, &[], false, None);
        let msg = &body["messages"][0];

        // reasoning_content 应该被合并到 content 中，使用 <agent-reasoning> 标签
        let content = msg["content"].as_str().unwrap();
        assert!(content.contains("<agent-reasoning>"));
        assert!(content.contains("这是思考过程"));
        assert!(content.contains("</agent-reasoning>"));
        assert!(content.contains("这是回复内容"));

        // 不应该有独立的 reasoning_content 字段
        assert!(msg.get("reasoning_content").is_none());
    }

    /// 测试 reasoning_in_content=false 时，reasoning_content 保持原样发送
    #[test]
    fn test_build_request_body_reasoning_in_content_false() {
        let adapter = create_adapter(advanced_with_reasoning_in_content(false));
        let messages = vec![ChatMessage {
            role: "assistant".to_string(),
            content: "这是回复内容".to_string(),
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: Some("这是思考过程".to_string()),
            attachments: None,
            metadata: None,
        }];

        let body = adapter.build_request_body(&messages, &[], false, None);
        let msg = &body["messages"][0];

        // content 应该保持原样
        assert_eq!(msg["content"].as_str().unwrap(), "这是回复内容");

        // reasoning_content 应该作为独立字段发送
        assert_eq!(msg["reasoning_content"].as_str().unwrap(), "这是思考过程");
    }

    /// 测试没有 reasoning_content 时正常构建请求体
    #[test]
    fn test_build_request_body_no_reasoning_content() {
        let adapter = create_adapter(AdvancedConfig::default());
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "你好".to_string(),
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            attachments: None,
            metadata: None,
        }];

        let body = adapter.build_request_body(&messages, &[], false, None);
        let msg = &body["messages"][0];

        // content 应该保持原样
        assert_eq!(msg["content"].as_str().unwrap(), "你好");

        // 不应该有 reasoning_content 字段
        assert!(msg.get("reasoning_content").is_none());
    }
}
