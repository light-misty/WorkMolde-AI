use std::sync::Arc;
use std::time::Instant;

use serde_json::json;
use tauri::Runtime;

use crate::errors::CommandError;
use crate::events::emitter::AgentEmitter;
use crate::events::types::*;
use crate::models::llm::LlmToolCall;
use crate::services::llm::router::LlmRouter;
use crate::services::skill::registry::SkillRegistry;
use super::context::AgentContext;

/// Agent 执行器
/// 实现 Tool Calling 循环：调用 LLM -> 检查 tool_calls -> 执行 Skill -> 反馈结果 -> 继续循环
pub struct AgentExecutor<R: Runtime> {
    /// LLM 路由器
    router: Arc<LlmRouter>,
    /// Skill 注册表
    registry: Arc<SkillRegistry>,
    /// 事件发射器
    emitter: AgentEmitter<R>,
    /// 最大迭代次数
    max_iterations: u32,
    /// 停止标志检查：返回 true 表示 Agent 应该停止
    should_stop: Arc<dyn Fn(&str) -> bool + Send + Sync>,
}

impl<R: Runtime> AgentExecutor<R> {
    pub fn new(
        router: Arc<LlmRouter>,
        registry: Arc<SkillRegistry>,
        emitter: AgentEmitter<R>,
    ) -> Self {
        Self {
            router,
            registry,
            emitter,
            max_iterations: 20,
            should_stop: Arc::new(|_| false),
        }
    }

    /// 设置停止检查回调
    pub fn with_stop_check(
        mut self,
        check: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    ) -> Self {
        self.should_stop = check;
        self
    }

    /// 设置最大迭代次数
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    /// 检查是否应该停止
    fn check_stopped(&self, session_id: &str) -> bool {
        (self.should_stop)(session_id)
    }

    /// 执行 Agent 循环
    pub async fn execute(&self, ctx: &mut AgentContext) -> Result<String, CommandError> {
        let start_time = Instant::now();
        let mut total_steps = 0u32;
        let total_input_tokens = 0u64;
        let total_output_tokens = 0u64;

        log::info!("Agent 开始执行, session_id={}", ctx.session_id);

        let tool_defs_json = self.registry.tool_definitions();
        let tools: Vec<crate::models::llm::ToolDefinition> = tool_defs_json
            .iter()
            .filter_map(|v| {
                let func = v.get("function")?;
                Some(crate::models::llm::ToolDefinition {
                    name: func["name"].as_str()?.to_string(),
                    description: func["description"].as_str()?.to_string(),
                    parameters: func["parameters"].clone(),
                })
            })
            .collect();

        for iteration in 0..self.max_iterations {
            // 检查是否被用户停止
            if self.check_stopped(&ctx.session_id) {
                log::info!("Agent 被用户停止, session_id={}, iteration={}", ctx.session_id, iteration);
                self.emitter.emit_stopped(StoppedPayload {
                    session_id: ctx.session_id.clone(),
                    reason: "用户手动停止".to_string(),
                    completed_steps: total_steps,
                }).ok();
                return Ok("Agent 已被用户停止".to_string());
            }

            total_steps += 1;
            log::debug!("Agent 迭代 #{}, session_id={}", iteration + 1, ctx.session_id);

            let messages = ctx.get_messages();
            log::debug!("调用 LLM 流式接口, session_id={}, 消息数={}", ctx.session_id, messages.len());
            let mut stream_rx = match self.router.chat_stream(&messages, &tools).await {
                Ok(rx) => rx,
                Err(e) => {
                    log::error!("LLM 流式调用失败, session_id={}, 错误: {}", ctx.session_id, e.message);
                    self.emitter.emit_error(ErrorPayload {
                        session_id: ctx.session_id.clone(),
                        code: e.code,
                        message: e.message.clone(),
                        recoverable: true,
                    }).ok();
                    return Err(e);
                }
            };

            // 收集流式响应
            let mut assistant_content = String::new();
            let mut collected_tool_calls: Vec<LlmToolCall> = Vec::new();
            let mut message_id = String::new();

            while let Some(chunk_result) = stream_rx.recv().await {
                match chunk_result {
                    Ok(chunk) => {
                        message_id = chunk.id.clone();
                        for choice in chunk.choices {
                            // 处理内容增量
                            if let Some(content) = &choice.delta.content {
                                assistant_content.push_str(content);
                                self.emitter.emit_content(ContentPayload {
                                    session_id: ctx.session_id.clone(),
                                    message_id: message_id.clone(),
                                    content: content.clone(),
                                    is_streaming: true,
                                }).ok();
                            }

                            // 收集 tool_calls 增量
                            if let Some(delta_tool_calls) = choice.delta.tool_calls {
                                for tc in delta_tool_calls {
                                    if let Some(existing) = collected_tool_calls.iter_mut()
                                        .find(|c| c.id == tc.id && !tc.id.is_empty())
                                    {
                                        existing.name.push_str(&tc.name);
                                        existing.arguments.push_str(&tc.arguments);
                                    } else {
                                        collected_tool_calls.push(tc);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if e.message == "stream_done" {
                            break;
                        }
                        log::warn!("流式响应错误: {}", e.message);
                        break;
                    }
                }
            }

            // 发送内容结束事件
            if !assistant_content.is_empty() {
                self.emitter.emit_content(ContentPayload {
                    session_id: ctx.session_id.clone(),
                    message_id: message_id.clone(),
                    content: String::new(),
                    is_streaming: false,
                }).ok();
            }

            // 检查是否有 tool_calls
            let has_tool_calls = !collected_tool_calls.is_empty();
            log::debug!("LLM 响应解析完成, session_id={}, tool_calls数={}, 内容长度={}", ctx.session_id, collected_tool_calls.len(), assistant_content.len());

            if has_tool_calls {
                // 将助手消息（含 tool_calls）添加到上下文
                let tool_calls_for_message = if collected_tool_calls.is_empty() {
                    None
                } else {
                    Some(collected_tool_calls.clone())
                };
                ctx.add_assistant_message(&assistant_content, tool_calls_for_message);

                // 执行每个 tool_call
                for tool_call in &collected_tool_calls {
                    // 检查是否被停止
                    if self.check_stopped(&ctx.session_id) {
                        log::info!("Agent 在 Tool 执行前被停止, session_id={}", ctx.session_id);
                        self.emitter.emit_stopped(StoppedPayload {
                            session_id: ctx.session_id.clone(),
                            reason: "用户手动停止".to_string(),
                            completed_steps: total_steps,
                        }).ok();
                        return Ok("Agent 已被用户停止".to_string());
                    }

                    log::info!("执行 Tool, session_id={}, tool={}, call_id={}", ctx.session_id, tool_call.name, tool_call.id);

                    self.emitter.emit_tool_call(ToolCallPayload {
                        session_id: ctx.session_id.clone(),
                        call_id: tool_call.id.clone(),
                        tool_name: tool_call.name.clone(),
                        arguments: serde_json::from_str(&tool_call.arguments)
                            .unwrap_or(json!({})),
                    }).ok();

                    let tool_start = Instant::now();

                    // 执行 Skill
                    let params = serde_json::from_str(&tool_call.arguments)
                        .unwrap_or(json!({}));
                    let result = self.registry.execute(&tool_call.name, params).await;

                    let duration_ms = tool_start.elapsed().as_millis() as u64;
                    log::debug!("Tool 执行完成, session_id={}, tool={}, 成功={}, 耗时={}ms", ctx.session_id, tool_call.name, result.success, duration_ms);

                    self.emitter.emit_tool_result(ToolResultPayload {
                        session_id: ctx.session_id.clone(),
                        call_id: tool_call.id.clone(),
                        success: result.success,
                        result: result.output.clone().unwrap_or(json!(null)),
                        error: result.error.clone(),
                        duration_ms,
                    }).ok();

                    // 将工具结果添加到上下文
                    let result_content = if result.success {
                        serde_json::to_string(&result.output).unwrap_or_default()
                    } else {
                        format!("错误: {}", result.error.unwrap_or_default())
                    };
                    ctx.add_tool_result(&tool_call.id, &result_content);
                }

                // 继续循环，让 LLM 处理工具结果
                continue;
            }

            // 没有 tool_calls，表示 LLM 已完成回复
            if !assistant_content.is_empty() {
                ctx.add_assistant_message(&assistant_content, None);
            }

            let total_duration_ms = start_time.elapsed().as_millis() as u64;
            log::info!("Agent 执行完成, session_id={}, 总步骤={}, 总耗时={}ms", ctx.session_id, total_steps, total_duration_ms);
            self.emitter.emit_done(DonePayload {
                session_id: ctx.session_id.clone(),
                summary: assistant_content.clone(),
                total_steps,
                total_tokens: total_input_tokens + total_output_tokens,
                duration_ms: total_duration_ms,
            }).ok();

            return Ok(assistant_content);
        }

        // 超过最大迭代次数
        let error = CommandError::agent(2001, format!("Agent 执行超过最大迭代次数 ({})", self.max_iterations));
        log::error!("Agent 执行超过最大迭代次数, session_id={}, max_iterations={}", ctx.session_id, self.max_iterations);
        self.emitter.emit_error(ErrorPayload {
            session_id: ctx.session_id.clone(),
            code: error.code,
            message: error.message.clone(),
            recoverable: false,
        }).ok();

        Err(error)
    }
}
