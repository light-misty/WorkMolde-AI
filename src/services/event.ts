/**
 * Tauri 事件监听封装
 * 为每个事件类型创建类型定义和监听函数
 * 事件名使用 namespace:action 格式，与 Rust 端一致
 */
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ================================================================
// Agent 事件 Payload 类型 - 与 Rust 端 serde camelCase 输出一致
// ================================================================

/** Agent 思考链增量 */
export interface ThinkingPayload {
  sessionId: string;
  step: number;
  thought: string;
}

/** Agent 深度思考链增量（Extended Thinking / reasoning_content） */
export interface DeepThinkingPayload {
  sessionId: string;
  step: number;
  thought: string;
  isStreaming: boolean;
  /** 当前迭代轮次序号（从 1 开始），用于前端按迭代分组展示 */
  iteration?: number;
}

/** Agent 回复内容增量 */
export interface ContentPayload {
  sessionId: string;
  messageId: string;
  content: string;
  isStreaming: boolean;
  /** 当前迭代轮次序号（从 1 开始），用于前端按迭代分组展示 */
  iteration?: number;
}

/** Tool 调用开始 */
export interface ToolCallPayload {
  sessionId: string;
  callId: string;
  toolName: string;
  arguments: Record<string, unknown>;
  /** 当前迭代轮次序号（从 1 开始），用于前端按迭代分组展示 */
  iteration?: number;
}

/** Tool 执行结果 */
export interface ToolResultPayload {
  sessionId: string;
  callId: string;
  success: boolean;
  result: unknown;
  error?: string;
  durationMs: number;
}

/** 需要用户确认的操作 */
export interface ConfirmPayload {
  sessionId: string;
  operationId: string;
  operationType: string;
  description: string;
  details: unknown;
  riskLevel: string;
}

/** Agent 执行完成 */
export interface DonePayload {
  sessionId: string;
  summary: string;
  totalSteps: number;
  durationMs: number;
}

/** Agent 执行错误 */
export interface ErrorPayload {
  sessionId: string;
  code: number;
  message: string;
  recoverable: boolean;
}

/** Agent 执行中断 */
export interface StoppedPayload {
  sessionId: string;
  completedSteps: number;
  reason: string;
}

/** Agent 网络重试事件 */
export interface NetworkRetryPayload {
  sessionId: string;
  attempt: number;
  maxAttempts: number;
  reason: string;
}

/** 代码流式事件（仅 code_interpreter_handler 触发） */
export interface CodeStreamingPayload {
  sessionId: string;
  /** 关联的 tool_call ID */
  callId: string;
  /** 完整的代码内容（非增量，前端直接替换） */
  codeDelta: string;
  /** 是否为流式输出的最后一个事件 */
  isFinal: boolean;
}

// ================================================================
// 系统事件 Payload 类型
// ================================================================

/** 会话更新事件 */
export interface SessionUpdatePayload {
  sessionId: string;
  changeType: string;
  data?: unknown;
}

/** 工作区目录被外部删除事件 */
export interface WorkspaceDirectoryDeletedPayload {
  workspaceId: string;
  workspaceName: string;
  workspacePath: string;
}

/** 文件变更事件 */
export interface FileChangePayload {
  workspaceId: string;
  changeType: string;
  path: string;
  oldPath?: string;
}

/** 网络状态变化事件 */
export interface NetworkChangePayload {
  /** 当前网络状态: "online" | "offline" */
  status: string;
  /** 之前的网络状态 */
  previousStatus: string;
}

// ================================================================
// LLM 事件 Payload 类型
// ================================================================

/** LLM Provider 切换通知 */
export interface ProviderSwitchPayload {
  fromProviderId: string;
  toProviderId: string;
  reason: string;
  isAutomatic: boolean;
}

// ================================================================
// 上下文窗口事件 Payload 类型
// ================================================================

/** 上下文窗口使用情况更新事件 */
export interface ContextUsagePayload {
  sessionId: string;
  /** 上下文使用详情 */
  contextUsage: import("../types/settings").ContextUsageInfo;
}

// ================================================================
// Agent 事件监听函数
// ================================================================

/** 监听 Agent 思考链增量事件 */
export function onAgentThinking(
  handler: (payload: ThinkingPayload) => void,
): Promise<UnlistenFn> {
  return listen<ThinkingPayload>("agent:thinking", (event) => {
    handler(event.payload);
  });
}

/** 监听 Agent 深度思考链增量事件 */
export function onAgentDeepThinking(
  handler: (payload: DeepThinkingPayload) => void,
): Promise<UnlistenFn> {
  return listen<DeepThinkingPayload>("agent:deep_thinking", (event) => {
    handler(event.payload);
  });
}

/** 监听 Agent 回复内容增量事件 */
export function onAgentContent(
  handler: (payload: ContentPayload) => void,
): Promise<UnlistenFn> {
  return listen<ContentPayload>("agent:content", (event) => {
    handler(event.payload);
  });
}

/** 监听 Tool 调用开始事件 */
export function onAgentToolCall(
  handler: (payload: ToolCallPayload) => void,
): Promise<UnlistenFn> {
  return listen<ToolCallPayload>("agent:tool_call", (event) => {
    handler(event.payload);
  });
}

/** 监听 Tool 执行结果事件 */
export function onAgentToolResult(
  handler: (payload: ToolResultPayload) => void,
): Promise<UnlistenFn> {
  return listen<ToolResultPayload>("agent:tool_result", (event) => {
    handler(event.payload);
  });
}

/** 监听需要用户确认的事件 */
export function onAgentConfirm(
  handler: (payload: ConfirmPayload) => void,
): Promise<UnlistenFn> {
  return listen<ConfirmPayload>("agent:confirm", (event) => {
    handler(event.payload);
  });
}

/** 监听 Agent 执行完成事件 */
export function onAgentDone(
  handler: (payload: DonePayload) => void,
): Promise<UnlistenFn> {
  return listen<DonePayload>("agent:done", (event) => {
    handler(event.payload);
  });
}

/** 监听 Agent 执行错误事件 */
export function onAgentError(
  handler: (payload: ErrorPayload) => void,
): Promise<UnlistenFn> {
  return listen<ErrorPayload>("agent:error", (event) => {
    handler(event.payload);
  });
}

/** 监听 Agent 执行中断事件 */
export function onAgentStopped(
  handler: (payload: StoppedPayload) => void,
): Promise<UnlistenFn> {
  return listen<StoppedPayload>("agent:stopped", (event) => {
    handler(event.payload);
  });
}

/** 监听 Agent 网络重试事件 */
export function onAgentNetworkRetry(
  handler: (payload: NetworkRetryPayload) => void,
): Promise<UnlistenFn> {
  return listen<NetworkRetryPayload>("agent:network_retry", (event) => {
    handler(event.payload);
  });
}

/** 监听代码流式增量事件 */
export function onAgentCodeStreaming(
  handler: (payload: CodeStreamingPayload) => void,
): Promise<UnlistenFn> {
  return listen<CodeStreamingPayload>("agent:code_streaming", (event) => {
    handler(event.payload);
  });
}

// ================================================================
// 系统事件监听函数
// ================================================================

/** 监听会话更新事件 */
export function onSessionUpdated(
  handler: (payload: SessionUpdatePayload) => void,
): Promise<UnlistenFn> {
  return listen<SessionUpdatePayload>("session:updated", (event) => {
    handler(event.payload);
  });
}

/** 监听工作区目录被外部删除事件 */
export function onWorkspaceDirectoryDeleted(
  handler: (payload: WorkspaceDirectoryDeletedPayload) => void,
): Promise<UnlistenFn> {
  return listen<WorkspaceDirectoryDeletedPayload>("workspace:directory_deleted", (event) => {
    handler(event.payload);
  });
}

/** 监听文件变更事件 */
export function onFileChange(
  handler: (payload: FileChangePayload) => void,
): Promise<UnlistenFn> {
  return listen<FileChangePayload>("file:change", (event) => {
    handler(event.payload);
  });
}

/** 监听网络状态变化事件 */
export function onSystemNetworkChange(
  handler: (payload: NetworkChangePayload) => void,
): Promise<UnlistenFn> {
  return listen<NetworkChangePayload>("system:network_change", (event) => {
    handler(event.payload);
  });
}

// ================================================================
// LLM 事件监听函数
// ================================================================

/** 监听 LLM Provider 切换通知事件 */
export function onLlmProviderSwitch(
  handler: (payload: ProviderSwitchPayload) => void,
): Promise<UnlistenFn> {
  return listen<ProviderSwitchPayload>("llm:provider_switch", (event) => {
    handler(event.payload);
  });
}

// ================================================================
// 上下文窗口事件监听函数
// ================================================================

/** 监听上下文窗口使用情况更新事件 */
export function onAgentContextUpdate(
  handler: (payload: ContextUsagePayload) => void,
): Promise<UnlistenFn> {
  return listen<ContextUsagePayload>("agent:context_update", (event) => {
    handler(event.payload);
  });
}
