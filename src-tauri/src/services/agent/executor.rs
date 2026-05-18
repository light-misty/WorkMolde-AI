use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tauri::Runtime;

use crate::errors::CommandError;
use crate::events::emitter::AgentEmitter;
use crate::events::types::*;
use crate::models::llm::LlmToolCall;
use crate::services::llm::router::LlmRouter;
use crate::services::skill::registry::SkillRegistry;
use crate::ConfirmDecision;
use super::context::AgentContext;

const HIGH_RISK_SKILLS: &[&str] = &["delete_document", "modify_document", "batch_process"];
const CONFIRM_TIMEOUT_SECS: u64 = 300;

pub struct ExecutionResult {
    pub summary: String,
    pub total_steps: u32,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub duration_ms: u64,
}

pub struct AgentExecutor<R: Runtime> {
    router: Arc<LlmRouter>,
    registry: Arc<tokio::sync::Mutex<SkillRegistry>>,
    emitter: AgentEmitter<R>,
    confirm_channels: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<ConfirmDecision>>>>,
    max_iterations: u32,
    should_stop: Arc<dyn Fn(&str) -> bool + Send + Sync>,
}

impl<R: Runtime> AgentExecutor<R> {
    pub fn new(
        router: Arc<LlmRouter>,
        registry: Arc<tokio::sync::Mutex<SkillRegistry>>,
        emitter: AgentEmitter<R>,
        confirm_channels: Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<ConfirmDecision>>>>,
    ) -> Self {
        Self {
            router,
            registry,
            emitter,
            confirm_channels,
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

    fn is_high_risk_skill(name: &str) -> bool {
        HIGH_RISK_SKILLS.contains(&name)
    }

    async fn request_confirmation(
        &self,
        session_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<bool, CommandError> {
        let operation_id = format!("confirm_{}", uuid::Uuid::new_v4());

        let risk_level = if tool_name == "delete_document" {
            "critical"
        } else {
            "high"
        };

        let description = match tool_name {
            "delete_document" => format!("删除文件: {}", arguments["path"].as_str().unwrap_or("未知")),
            "modify_document" => format!("修改文件: {}", arguments["path"].as_str().unwrap_or("未知")),
            "batch_process" => format!("批量处理 {} 个文件", arguments["paths"].as_array().map(|a| a.len()).unwrap_or(0)),
            _ => format!("执行高风险操作: {}", tool_name),
        };

        self.emitter.emit_confirm(ConfirmPayload {
            session_id: session_id.to_string(),
            operation_id: operation_id.clone(),
            operation_type: tool_name.to_string(),
            description,
            details: arguments.clone(),
            risk_level: risk_level.to_string(),
        }).ok();

        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut channels = self.confirm_channels.lock().await;
            channels.insert(operation_id.clone(), tx);
        }

        match tokio::time::timeout(Duration::from_secs(CONFIRM_TIMEOUT_SECS), rx).await {
            Ok(Ok(decision)) => {
                let mut channels = self.confirm_channels.lock().await;
                channels.remove(&operation_id);
                if decision.approved {
                    log::info!("用户确认操作: operation_id={}, tool={}", operation_id, tool_name);
                    Ok(true)
                } else {
                    log::info!("用户拒绝操作: operation_id={}, tool={}, feedback={:?}", operation_id, tool_name, decision.feedback);
                    Ok(false)
                }
            }
            Ok(Err(_)) => {
                let mut channels = self.confirm_channels.lock().await;
                channels.remove(&operation_id);
                log::warn!("确认通道关闭: operation_id={}", operation_id);
                Ok(false)
            }
            Err(_) => {
                let mut channels = self.confirm_channels.lock().await;
                channels.remove(&operation_id);
                log::warn!("确认超时: operation_id={}", operation_id);
                self.emitter.emit_error(ErrorPayload {
                    session_id: session_id.to_string(),
                    code: crate::errors::AGENT_CONFIRMATION_TIMEOUT,
                    message: format!("操作确认超时 ({}秒)", CONFIRM_TIMEOUT_SECS),
                    recoverable: true,
                }).ok();
                Ok(false)
            }
        }
    }

    fn emit_todo_progress(
        &self,
        session_id: &str,
        current_step: u32,
        total_possible: u32,
        tool_name: &str,
    ) {
        let mut todos = Vec::new();

        if current_step > 1 {
            todos.push(TodoItem {
                id: format!("step_{}", current_step - 1),
                content: format!("步骤 {} 已完成", current_step - 1),
                status: "completed".to_string(),
            });
        }

        todos.push(TodoItem {
            id: format!("step_{}", current_step),
            content: format!("正在执行: {}", tool_name),
            status: "in_progress".to_string(),
        });

        if current_step < total_possible {
            todos.push(TodoItem {
                id: format!("step_{}", current_step + 1),
                content: format!("步骤 {} 待执行", current_step + 1),
                status: "pending".to_string(),
            });
        }

        self.emitter.emit_todo_update(TodoUpdatePayload {
            session_id: session_id.to_string(),
            todos,
        }).ok();
    }

    pub async fn execute(&self, ctx: &mut AgentContext) -> Result<ExecutionResult, CommandError> {
        let start_time = std::time::Instant::now();
        let mut total_steps = 0u32;
        let mut total_input_tokens = 0u64;
        let mut total_output_tokens = 0u64;

        log::info!("Agent 开始执行, session_id={}", ctx.session_id);

        let tool_defs_json = {
            let reg = self.registry.lock().await;
            reg.tool_definitions()
        };
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

        self.emitter.emit_todo_update(TodoUpdatePayload {
            session_id: ctx.session_id.clone(),
            todos: vec![TodoItem {
                id: "step_0".to_string(),
                content: "正在分析用户请求...".to_string(),
                status: "in_progress".to_string(),
            }],
        }).ok();

        for iteration in 0..self.max_iterations {
            // 检查是否被用户停止
            if self.check_stopped(&ctx.session_id) {
                log::info!("Agent 被用户停止, session_id={}, iteration={}", ctx.session_id, iteration);
                self.emitter.emit_stopped(StoppedPayload {
                    session_id: ctx.session_id.clone(),
                    reason: "用户手动停止".to_string(),
                    completed_steps: total_steps,
                }).ok();
                return Ok(ExecutionResult {
                    summary: "Agent 已被用户停止".to_string(),
                    total_steps,
                    total_input_tokens,
                    total_output_tokens,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                });
            }

            total_steps += 1;
            log::debug!("Agent 迭代 #{}, session_id={}", iteration + 1, ctx.session_id);

            self.emitter.emit_thinking(ThinkingPayload {
                session_id: ctx.session_id.clone(),
                step: total_steps,
                thought: format!("正在分析用户请求并规划操作步骤... (第{}轮)", iteration + 1),
            }).ok();

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
                        if e.code == 9999 && e.message == "stream_done" {
                            log::debug!("流式响应正常结束, session_id={}", ctx.session_id);
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

            let input_chars: usize = messages.iter().map(|m| m.content.len()).sum();
            let output_chars = assistant_content.len();
            let estimated_input = (input_chars as u64) / 3;
            let estimated_output = (output_chars as u64) / 3;
            total_input_tokens += estimated_input;
            total_output_tokens += estimated_output;

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

                for (tc_index, tool_call) in collected_tool_calls.iter().enumerate() {
                    if self.check_stopped(&ctx.session_id) {
                        log::info!("Agent 在 Tool 执行前被停止, session_id={}", ctx.session_id);
                        self.emitter.emit_stopped(StoppedPayload {
                            session_id: ctx.session_id.clone(),
                            reason: "用户手动停止".to_string(),
                            completed_steps: total_steps,
                        }).ok();
                        return Ok(ExecutionResult {
                            summary: "Agent 已被用户停止".to_string(),
                            total_steps,
                            total_input_tokens,
                            total_output_tokens,
                            duration_ms: start_time.elapsed().as_millis() as u64,
                        });
                    }

                    log::info!("执行 Tool, session_id={}, tool={}, call_id={}", ctx.session_id, tool_call.name, tool_call.id);

                    self.emit_todo_progress(
                        &ctx.session_id,
                        total_steps,
                        self.max_iterations,
                        &tool_call.name,
                    );

                    let params = serde_json::from_str(&tool_call.arguments)
                        .unwrap_or(json!({}));

                    if Self::is_high_risk_skill(&tool_call.name) {
                        self.emitter.emit_tool_call(ToolCallPayload {
                            session_id: ctx.session_id.clone(),
                            call_id: tool_call.id.clone(),
                            tool_name: format!("{} (等待确认)", tool_call.name),
                            arguments: params.clone(),
                        }).ok();

                        let approved = self.request_confirmation(
                            &ctx.session_id,
                            &tool_call.name,
                            &params,
                        ).await?;

                        if !approved {
                            let skip_msg = format!("用户拒绝了操作: {}", tool_call.name);
                            log::info!("操作被拒绝: session_id={}, tool={}", ctx.session_id, tool_call.name);

                            self.emitter.emit_tool_result(ToolResultPayload {
                                session_id: ctx.session_id.clone(),
                                call_id: tool_call.id.clone(),
                                success: false,
                                result: json!(null),
                                error: Some(skip_msg.clone()),
                                duration_ms: 0,
                            }).ok();

                            ctx.add_tool_result(&tool_call.id, &skip_msg);
                            continue;
                        }
                    } else {
                        self.emitter.emit_tool_call(ToolCallPayload {
                            session_id: ctx.session_id.clone(),
                            call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                            arguments: params.clone(),
                        }).ok();
                    }

                    let tool_start = std::time::Instant::now();

                    let result = {
                        let reg = self.registry.lock().await;
                        reg.execute(&tool_call.name, params).await
                    };

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

                    let _ = tc_index;
                }

                // 继续循环，让 LLM 处理工具结果
                continue;
            }

            if !assistant_content.is_empty() {
                ctx.add_assistant_message(&assistant_content, None);
            }

            self.emitter.emit_todo_update(TodoUpdatePayload {
                session_id: ctx.session_id.clone(),
                todos: vec![TodoItem {
                    id: "done".to_string(),
                    content: "任务完成".to_string(),
                    status: "completed".to_string(),
                }],
            }).ok();

            let total_duration_ms = start_time.elapsed().as_millis() as u64;
            log::info!("Agent 执行完成, session_id={}, 总步骤={}, 总耗时={}ms", ctx.session_id, total_steps, total_duration_ms);
            self.emitter.emit_done(DonePayload {
                session_id: ctx.session_id.clone(),
                summary: assistant_content.clone(),
                total_steps,
                total_tokens: total_input_tokens + total_output_tokens,
                duration_ms: total_duration_ms,
            }).ok();

            return Ok(ExecutionResult {
                summary: assistant_content,
                total_steps,
                total_input_tokens,
                total_output_tokens,
                duration_ms: total_duration_ms,
            });
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
