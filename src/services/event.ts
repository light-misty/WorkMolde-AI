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

/** Agent 回复内容增量 */
export interface ContentPayload {
  sessionId: string;
  messageId: string;
  content: string;
  isStreaming: boolean;
}

/** Tool 调用开始 */
export interface ToolCallPayload {
  sessionId: string;
  callId: string;
  toolName: string;
  arguments: Record<string, unknown>;
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

/** Todo 列表条目 */
export interface TodoItem {
  id: string;
  content: string;
  status: string;
}

/** Todo 列表更新 */
export interface TodoUpdatePayload {
  sessionId: string;
  todos: TodoItem[];
}

/** Agent 执行完成 */
export interface DonePayload {
  sessionId: string;
  summary: string;
  totalSteps: number;
  totalTokens: number;
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

// ================================================================
// 系统事件 Payload 类型
// ================================================================

/** 会话更新事件 */
export interface SessionUpdatePayload {
  sessionId: string;
  changeType: string;
  data?: unknown;
}

/** 工作区变更事件 */
export interface WorkspaceChangePayload {
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

/** Token 用量更新事件 */
export interface TokenUpdatePayload {
  sessionId: string;
  providerId: string;
  promptTokens: number;
  completionTokens: number;
  totalCost: number;
}

// ================================================================
// LLM 事件 Payload 类型
// ================================================================

/** LLM Provider 切换通知 */
export interface ProviderSwitchPayload {
  /** 原始 Provider ID */
  fromProviderId: string;
  /** 切换到的 Provider ID */
  toProviderId: string;
  /** 切换原因 */
  reason: string;
  /** 是否为自动切换 */
  isAutomatic: boolean;
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

/** 监听 Todo 列表更新事件 */
export function onAgentTodoUpdate(
  handler: (payload: TodoUpdatePayload) => void,
): Promise<UnlistenFn> {
  return listen<TodoUpdatePayload>("agent:todo_update", (event) => {
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

/** 监听工作区变更事件 */
export function onWorkspaceChange(
  handler: (payload: WorkspaceChangePayload) => void,
): Promise<UnlistenFn> {
  return listen<WorkspaceChangePayload>("workspace:change", (event) => {
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

/** 监听 Token 用量更新事件 */
export function onTokenUpdate(
  handler: (payload: TokenUpdatePayload) => void,
): Promise<UnlistenFn> {
  return listen<TokenUpdatePayload>("token:update", (event) => {
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
