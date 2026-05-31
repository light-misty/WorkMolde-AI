use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::config::llm_config::AdvancedConfig;
use crate::errors::CommandError;
use crate::models::llm::*;
use super::provider::LlmProvider;

/// Google Gemini API 适配器
/// 支持 Gemini 系列模型的原生 API 格式
pub struct GeminiAdapter {
    api_base_url: String,
    api_key: String,
    model: String,
    advanced: AdvancedConfig,
    /// 用于非流式请求的客户端（支持压缩）
    client: Client,
    /// 用于流式请求的客户端（禁用压缩，避免 bytes_stream 解码错误）
    streaming_client: Client,
}

impl GeminiAdapter {
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

    /// 构建 Gemini API 请求体
    /// 将内部 ChatMessage 格式转换为 Gemini contents 格式
    fn build_request_body(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Value {
        let mut system_parts: Vec<Value> = Vec::new();
        let mut contents: Vec<Value> = Vec::new();

        // 构建 tool_call_id -> function_name 的映射表
        // 用于将 tool 结果消息中的 tool_call_id 转换为 Gemini 需要的 function name
        let mut call_id_to_name: HashMap<String, String> = HashMap::new();
        for msg in messages {
            if let Some(tool_calls) = &msg.tool_calls {
                for tc in tool_calls {
                    call_id_to_name.insert(tc.id.clone(), tc.name.clone());
                }
            }
        }

        for msg in messages {
            match msg.role.as_str() {
                // system 消息提取到 systemInstruction 字段
                "system" => {
                    if let Some(ref content_parts) = msg.content_parts {
                        if !content_parts.is_empty() {
                            // 多模态 system 消息：将 ContentPart 映射为 Gemini API 格式
                            for cp in content_parts {
                                match cp {
                                    ContentPart::Text { text } => system_parts.push(json!({"text": text})),
                                    ContentPart::Image { mime_type, data } => {
                                        system_parts.push(json!({
                                            "inline_data": {
                                                "mime_type": mime_type,
                                                "data": data
                                            }
                                        }));
                                    }
                                }
                            }
                        } else if !msg.content.is_empty() {
                            system_parts.push(json!({"text": msg.content}));
                        }
                    } else if !msg.content.is_empty() {
                        system_parts.push(json!({"text": msg.content}));
                    }
                }
                // user 消息转换为 Gemini user 角色
                "user" => {
                    let parts = if let Some(ref content_parts) = msg.content_parts {
                        if !content_parts.is_empty() {
                            // 多模态消息：将 ContentPart 映射为 Gemini API 格式
                            content_parts
                                .iter()
                                .map(|cp| match cp {
                                    ContentPart::Text { text } => json!({"text": text}),
                                    ContentPart::Image { mime_type, data } => json!({
                                        "inline_data": {
                                            "mime_type": mime_type,
                                            "data": data
                                        }
                                    }),
                                })
                                .collect::<Vec<Value>>()
                        } else {
                            vec![json!({"text": msg.content})]
                        }
                    } else {
                        vec![json!({"text": msg.content})]
                    };
                    contents.push(json!({
                        "role": "user",
                        "parts": parts
                    }));
                }
                // assistant 消息转换为 Gemini model 角色
                "assistant" => {
                    let mut parts: Vec<Value> = Vec::new();
                    if let Some(rc) = &msg.reasoning_content {
                        if !rc.is_empty() {
                            parts.push(json!({"text": rc, "thought": true}));
                        }
                    }
                    if !msg.content.is_empty() {
                        parts.push(json!({"text": msg.content}));
                    }
                    // 将 tool_calls 转换为 functionCall parts
                    if let Some(tool_calls) = &msg.tool_calls {
                        for tc in tool_calls {
                            // Gemini 使用 args (JSON 对象) 而非 arguments (字符串)
                            let args: Value =
                                serde_json::from_str(&tc.arguments).unwrap_or(json!({}));
                            parts.push(json!({
                                "functionCall": {
                                    "name": tc.name,
                                    "args": args
                                }
                            }));
                        }
                    }
                    if !parts.is_empty() {
                        contents.push(json!({
                            "role": "model",
                            "parts": parts
                        }));
                    }
                }
                // tool 消息转换为 Gemini function 角色
                "tool" => {
                    // 通过 tool_call_id 查找对应的函数名
                    let func_name = msg
                        .tool_call_id
                        .as_ref()
                        .and_then(|id| call_id_to_name.get(id))
                        .cloned()
                        .unwrap_or_default();
                    contents.push(json!({
                        "role": "function",
                        "parts": [{
                            "functionResponse": {
                                "name": func_name,
                                "response": {"content": msg.content}
                            }
                        }]
                    }));
                }
                _ => {
                    // 未知角色按 user 处理
                    log::warn!("未知消息角色: {}, 按 user 处理", msg.role);
                    let parts = if let Some(ref content_parts) = msg.content_parts {
                        if !content_parts.is_empty() {
                            // 多模态消息：将 ContentPart 映射为 Gemini API 格式
                            content_parts
                                .iter()
                                .map(|cp| match cp {
                                    ContentPart::Text { text } => json!({"text": text}),
                                    ContentPart::Image { mime_type, data } => json!({
                                        "inline_data": {
                                            "mime_type": mime_type,
                                            "data": data
                                        }
                                    }),
                                })
                                .collect::<Vec<Value>>()
                        } else {
                            vec![json!({"text": msg.content})]
                        }
                    } else {
                        vec![json!({"text": msg.content})]
                    };
                    contents.push(json!({
                        "role": "user",
                        "parts": parts
                    }));
                }
            }
        }

        let mut body = json!({
            "contents": contents,
        });

        // 添加 systemInstruction（如果存在 system 消息）
        if !system_parts.is_empty() {
            body["systemInstruction"] = json!({"parts": system_parts});
        }

        // 添加 tools（functionDeclarations 格式）
        if !tools.is_empty() {
            body["tools"] = json!([{
                "functionDeclarations": tools.iter().map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    })
                }).collect::<Vec<_>>()
            }]);
        }

        // 添加 generationConfig
        body["generationConfig"] = json!({
            "temperature": self.advanced.temperature,
            "topP": self.advanced.top_p,
            "maxOutputTokens": self.advanced.max_tokens,
            "thinkingConfig": {
                "includeThoughts": true
            }
        });

        body
    }

    /// 构建非流式请求 URL
    /// Gemini API Key 通过 URL 查询参数传递
    fn build_url(&self) -> String {
        format!(
            "{}/models/{}:generateContent?key={}",
            self.api_base_url.trim_end_matches('/'),
            self.model,
            self.api_key
        )
    }

    /// 构建流式请求 URL
    /// 流式请求需要额外添加 alt=sse 参数
    fn build_streaming_url(&self) -> String {
        format!(
            "{}/models/{}:streamGenerateContent?key={}&alt=sse",
            self.api_base_url.trim_end_matches('/'),
            self.model,
            self.api_key
        )
    }

    /// 发送请求，带重试逻辑（使用普通客户端，支持压缩）
    async fn send_with_retry(
        &self,
        url: &str,
        body: &Value,
    ) -> Result<reqwest::Response, CommandError> {
        self.send_with_retry_internal(url, body, &self.client)
            .await
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
    /// Gemini API Key 通过 URL 参数传递，不使用 Authorization 请求头
    async fn send_with_retry_internal(
        &self,
        url: &str,
        body: &Value,
        client: &Client,
    ) -> Result<reqwest::Response, CommandError> {
        let max_retries = self.advanced.max_retries;
        let mut last_error = None;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                log::warn!(
                    "请求重试, model={}, 第{}次重试, 延迟{}ms",
                    self.model,
                    attempt,
                    delay.as_millis()
                );
                tokio::time::sleep(delay).await;
            }

            let mut request = client.post(url);
            request = request.header("Content-Type", "application/json");

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

                    // 解析 Gemini 错误响应体
                    let error_body = response.text().await.unwrap_or_default();
                    let error_message = Self::extract_error_message(&error_body);

                    if status.as_u16() == 401 || status.as_u16() == 403 {
                        log::error!("认证失败({}), model={}", status, self.model);
                        return Err(CommandError::llm(
                            1002,
                            format!("认证失败: {}", error_message),
                        ));
                    }
                    if status.as_u16() == 429 {
                        if attempt < max_retries {
                            log::warn!("请求频率受限(429), model={}, 准备重试", self.model);
                            last_error = Some(CommandError::llm(
                                1003,
                                "请求频率受限，正在重试".to_string(),
                            ));
                            continue;
                        }
                        return Err(CommandError::llm(
                            1003,
                            format!("请求频率受限: {}", error_message),
                        ));
                    }
                    if status.as_u16() == 404 {
                        log::error!("模型不存在(404), model={}", self.model);
                        return Err(CommandError::llm(
                            1005,
                            format!("模型不存在: {}", error_message),
                        ));
                    }
                    if status.as_u16() == 400 {
                        log::error!("请求参数无效(400), model={}", self.model);
                        return Err(CommandError::llm(
                            1007,
                            format!("请求参数无效: {}", error_message),
                        ));
                    }

                    last_error = Some(CommandError::llm(
                        1000,
                        format!("API 请求失败 ({}): {}", status, error_message),
                    ));
                }
                Err(e) => {
                    if e.is_timeout() {
                        if attempt < max_retries {
                            log::warn!("请求超时, model={}, 准备重试", self.model);
                            last_error =
                                Some(CommandError::llm(1006, "请求超时，正在重试".to_string()));
                            continue;
                        }
                        return Err(CommandError::llm(1006, "请求超时".to_string()));
                    }
                    last_error = Some(CommandError::llm(1000, format!("网络错误: {}", e)));
                }
            }
        }

        let err = last_error.unwrap_or_else(|| CommandError::llm(1000, "未知错误".to_string()));
        log::error!(
            "请求最终失败, model={}, 重试耗尽, 错误: {}",
            self.model,
            err.message
        );
        Err(err)
    }

    /// 从 Gemini 错误响应体中提取错误消息
    /// Gemini 错误格式: {"error": {"code": 400, "message": "...", "status": "..."}}
    fn extract_error_message(body: &str) -> String {
        serde_json::from_str::<Value>(body)
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(String::from))
            .unwrap_or_else(|| body.to_string())
    }

    /// 解析 Gemini 非流式响应为内部 ChatResponse 格式
    fn parse_response(&self, value: Value) -> Result<ChatResponse, CommandError> {
        // Gemini 响应不包含 id 字段，生成唯一标识
        let id = format!("gemini-{}", uuid::Uuid::new_v4());

        let choices = value["candidates"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        let index = c["index"].as_u64().unwrap_or(0) as u32;
                        let content_obj = &c["content"];
                        let parts = content_obj["parts"].as_array()?;

                        let mut text_content = String::new();
                        let mut reasoning_content = String::new();
                        let mut tool_calls: Vec<LlmToolCall> = Vec::new();
                        let mut tc_index = 0u32;

                        for part in parts {
                            if let Some(text) = part["text"].as_str() {
                                if part["thought"].as_bool().unwrap_or(false) {
                                    reasoning_content.push_str(text);
                                } else {
                                    text_content.push_str(text);
                                }
                            }
                            // 提取 functionCall
                            if let Some(fc) = part.get("functionCall") {
                                let name = fc["name"].as_str().unwrap_or("").to_string();
                                // Gemini 使用 args (JSON 对象)，转换为 arguments (字符串)
                                let args = fc["args"].clone();
                                let arguments =
                                    serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
                                // Gemini 没有 call ID，生成唯一标识用于内部追踪
                                let call_id = format!("gemini_{}_{}", name, tc_index);
                                tool_calls.push(LlmToolCall {
                                    index: tc_index,
                                    id: call_id,
                                    name,
                                    arguments,
                                });
                                tc_index += 1;
                            }
                        }

                        // 映射 Gemini finishReason 到内部格式
                        let finish_reason = c["finishReason"].as_str().map(|r| match r {
                            "STOP" => "stop".to_string(),
                            "FUNCTION_CALL" => "tool_calls".to_string(),
                            "MAX_TOKENS" => "length".to_string(),
                            "SAFETY" => "content_filter".to_string(),
                            other => other.to_lowercase(),
                        });

                        Some(ChatChoice {
                            index,
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
                                reasoning_content: if reasoning_content.is_empty() {
                                    None
                                } else {
                                    Some(reasoning_content)
                                },
                                attachments: None,
                            },
                            finish_reason,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // 映射 Gemini usageMetadata 到内部 ChatUsage 格式
        let usage = value["usageMetadata"].as_object().map(|u| ChatUsage {
            prompt_tokens: u["promptTokenCount"].as_u64().unwrap_or(0),
            completion_tokens: u["candidatesTokenCount"].as_u64().unwrap_or(0),
            total_tokens: u["totalTokenCount"].as_u64().unwrap_or(0),
        });

        Ok(ChatResponse { id, choices, usage })
    }

    /// 解析 Gemini 流式响应块为内部 StreamChunk 格式
    fn parse_stream_chunk(value: &Value) -> Result<StreamChunk, CommandError> {
        let id = format!("gemini-{}", uuid::Uuid::new_v4());

        let choices = value["candidates"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        let index = c["index"].as_u64().unwrap_or(0) as u32;
                        let content_obj = &c["content"];
                        let parts = content_obj["parts"].as_array()?;

                        let mut content: Option<String> = None;
                        let mut reasoning_content: Option<String> = None;
                        let mut tool_calls: Vec<LlmToolCall> = Vec::new();
                        let mut tc_index = 0u32;

                        for part in parts {
                            if let Some(text) = part["text"].as_str() {
                                if part["thought"].as_bool().unwrap_or(false) {
                                    reasoning_content = Some(text.to_string());
                                } else {
                                    content = Some(text.to_string());
                                }
                            }
                            if let Some(fc) = part.get("functionCall") {
                                let name = fc["name"].as_str().unwrap_or("").to_string();
                                let args = fc["args"].clone();
                                let arguments =
                                    serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
                                let call_id = format!("gemini_{}_{}", name, tc_index);
                                tool_calls.push(LlmToolCall {
                                    index: tc_index,
                                    id: call_id,
                                    name,
                                    arguments,
                                });
                                tc_index += 1;
                            }
                        }

                        // 流式响应中 role 只在第一个块中出现
                        let role = content_obj["role"]
                            .as_str()
                            .map(|r| match r {
                                "model" => "assistant".to_string(),
                                other => other.to_string(),
                            });

                        let finish_reason = c["finishReason"].as_str().map(|r| match r {
                            "STOP" => "stop".to_string(),
                            "FUNCTION_CALL" => "tool_calls".to_string(),
                            "MAX_TOKENS" => "length".to_string(),
                            "SAFETY" => "content_filter".to_string(),
                            other => other.to_lowercase(),
                        });

                        Some(StreamChoice {
                            index,
                            delta: StreamDelta {
                                role,
                                content,
                                reasoning_content,
                                tool_calls: if tool_calls.is_empty() {
                                    None
                                } else {
                                    Some(tool_calls)
                                },
                            },
                            finish_reason,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(StreamChunk { id, choices })
    }
}

#[async_trait]
impl LlmProvider for GeminiAdapter {
    fn provider_name(&self) -> &str {
        &self.model
    }

    async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ChatResponse, CommandError> {
        log::info!("发送非流式请求 (Gemini), model={}", self.model);
        let url = self.build_url();
        let body = self.build_request_body(messages, tools);
        let response = self.send_with_retry(&url, &body).await?;
        let value: Value = response.json().await.map_err(|e| {
            log::error!(
                "解析非流式响应失败 (Gemini), model={}, 错误: {}",
                self.model,
                e
            );
            CommandError::llm(1000, format!("解析响应失败: {}", e))
        })?;
        log::info!("非流式请求完成 (Gemini), model={}", self.model);
        self.parse_response(value)
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<mpsc::Receiver<Result<StreamChunk, CommandError>>, CommandError> {
        log::info!("发送流式请求 (Gemini), model={}", self.model);
        let url = self.build_streaming_url();
        let body = self.build_request_body(messages, tools);
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
                                    // Gemini 流式响应没有 [DONE] 标记，流结束即完成

                                    match serde_json::from_str::<Value>(data) {
                                        Ok(value) => {
                                            // 检查是否为错误响应
                                            if let Some(error) = value.get("error") {
                                                let error_msg = error["message"]
                                                    .as_str()
                                                    .unwrap_or("未知错误");
                                                log::error!(
                                                    "Gemini 流式响应错误, model={}, 错误: {}",
                                                    model_name,
                                                    error_msg
                                                );
                                                let _ = tx
                                                    .send(Err(CommandError::llm(
                                                        1000,
                                                        format!("Gemini API 错误: {}", error_msg),
                                                    )))
                                                    .await;
                                                return;
                                            }

                                            match Self::parse_stream_chunk(&value) {
                                                Ok(chunk) => {
                                                    if tx.send(Ok(chunk)).await.is_err() {
                                                        return;
                                                    }
                                                }
                                                Err(e) => {
                                                    log::error!(
                                                        "解析 Gemini 流式数据失败, model={}, 错误: {}",
                                                        model_name,
                                                        e.message
                                                    );
                                                    let _ = tx.send(Err(e)).await;
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            log::error!(
                                                "解析 SSE JSON 失败 (Gemini), model={}, 错误: {}",
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
                        log::error!("流读取错误 (Gemini), model={}, 错误: {}", model_name, e);
                        let _ = tx
                            .send(Err(CommandError::llm(
                                1000,
                                format!("流读取错误: {}", e),
                            )))
                            .await;
                        return;
                    }
                }
            }
            // Gemini 流式响应正常结束，流自然关闭
            // executor 侧通过 recv() 返回 None 检测流结束
        });

        Ok(rx)
    }

    async fn test_connection(&self) -> Result<ConnectionResult, CommandError> {
        log::info!("测试连接 (Gemini), model={}", self.model);
        let start = std::time::Instant::now();
        let test_messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "Hi".to_string(),
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            attachments: None,
        }];
        let url = self.build_url();
        let body = self.build_request_body(&test_messages, &[]);

        match self.send_with_retry(&url, &body).await {
            Ok(response) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                let value: Value = response.json().await.unwrap_or_default();
                // 从 Gemini 响应中提取模型名称
                let model_name = value["modelVersion"]
                    .as_str()
                    .unwrap_or(&self.model)
                    .to_string();
                log::info!(
                    "连接测试成功 (Gemini), model={}, 延迟={}ms",
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
                    "连接测试失败 (Gemini), model={}, 错误: {}",
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
}
