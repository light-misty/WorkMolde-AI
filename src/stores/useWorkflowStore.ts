import { create } from "zustand";
import type { WorkflowNode, WorkflowNodeType, NodeStatus, ExecutionStatus, NodeDataMap } from "../types";
import type { Message } from "../types/session";
import type { ContextUsageInfo } from "../types/settings";

import { generateToolBrief } from "../utils/format";
import { onAgentContextUpdate } from "../services/event";
import * as tauriCmd from "../services/tauri";
import i18n from "../i18n";

/** 按会话缓存的状态条目，切换会话时保存/恢复 */
export interface SessionCacheEntry {
  nodes: WorkflowNode[];
  executionStatus: ExecutionStatus;
  error: string | null;
  nodeCounter: number;
  contextUsage: ContextUsageInfo | null;
  /** 流式状态引用快照 */
  streamingNodeId: string | null;
  thinkingNodeId: string | null;
  confirmNodeId: string | null;
  currentIteration: number | undefined;
  /** 后台流式状态：深度思考内容累积 */
  deepThinkingContent: string;
  /** 后台流式状态：上一次深度思考 step */
  lastDeepThinkingStep: number;
  /** 后台流式状态：已见 tool call ID 集合 */
  seenToolCallIds: string[];
  /** 后台流式状态：后台节点计数器（独立于主 nodeCounter） */
  backgroundNodeCounter: number;
  /** 后台流式状态：当前 streaming 节点 ID */
  bgStreamingNodeId: string | null;
  /** 后台流式状态：当前 thinking 节点 ID */
  bgThinkingNodeId: string | null;
  /** 后台流式状态：被 tool_call 关闭的 streaming 节点 ID（用于修复内容截断） */
  bgLastClosedStreamingNodeId: string | null;
  /** 最后访问时间，用于 LRU 淘汰 */
  lastAccessedAt: number;
}

/** 缓存上限：最多保留 20 个会话的缓存 */
const MAX_CACHE_SIZE = 20;

/** 后台 Agent 事件类型，用于 applyBackgroundEvent */
export type BackgroundAgentEvent =
  | { type: "deep_thinking"; step: number; thought: string; isStreaming: boolean; iteration?: number }
  | { type: "content"; content: string; isStreaming: boolean; iteration?: number }
  | { type: "tool_call"; callId: string; toolName: string; arguments: Record<string, unknown>; iteration?: number }
  | { type: "tool_result"; callId: string; success: boolean; result: unknown; error?: string; durationMs: number }
  | { type: "context_update"; contextUsage: ContextUsageInfo }
  | { type: "done"; summary: string; totalSteps: number; durationMs: number }
  | { type: "error"; code: number; message: string; recoverable: boolean }
  | { type: "stopped"; completedSteps: number; reason: string };

interface WorkflowState {
  nodes: WorkflowNode[];
  executionStatus: ExecutionStatus;
  error: string | null;
  confirmHandler: ((approved: boolean, feedback?: string) => Promise<void>) | null;
  /** 上下文窗口使用信息（Agent 运行时实时更新） */
  contextUsage: ContextUsageInfo | null;
  /** 按会话缓存的状态映射 */
  sessionCache: Map<string, SessionCacheEntry>;

  addNode: <T extends WorkflowNodeType>(type: T, data: NodeDataMap[T], status?: NodeStatus, iteration?: number) => string;
  updateNode: (id: string, updates: Partial<WorkflowNode>) => void;
  removeNode: (id: string) => void;
  clearNodes: () => void;
  setExecutionStatus: (status: ExecutionStatus) => void;
  setError: (error: string | null) => void;
  toggleNode: (id: string) => void;
  setConfirmHandler: (handler: ((approved: boolean, feedback?: string) => Promise<void>) | null) => void;
  loadFromMessages: (messages: Message[]) => void;
  /** 初始化上下文窗口使用情况事件监听 */
  initContextUsageListener: () => Promise<() => void>;
  /** 从后端加载指定会话的上下文窗口使用信息 */
  loadContextUsage: (sessionId: string) => Promise<void>;
  /** 清除上下文窗口使用信息（新会话/切换会话时调用） */
  clearContextUsage: () => void;
  /** 将当前状态保存到指定会话的缓存 */
  saveSessionToCache: (sessionId: string, streamingRef: StreamingRefSnapshot) => void;
  /** 从指定会话的缓存恢复状态，返回是否命中缓存 */
  restoreSessionFromCache: (sessionId: string) => boolean;
  /** 删除指定会话的缓存 */
  clearSessionCache: (sessionId: string) => void;
  /** 获取指定会话缓存的流式引用快照 */
  getCachedStreamingRefs: (sessionId: string) => StreamingRefSnapshot | null;
  /** 将后台 Agent 事件应用到指定会话的缓存 */
  applyBackgroundEvent: (sessionId: string, event: BackgroundAgentEvent) => void;
  /** 获取指定会话缓存的 contextUsage */
  getCachedContextUsage: (sessionId: string) => ContextUsageInfo | null;
}

/** 流式状态引用快照，切换会话时保存/恢复 */
export interface StreamingRefSnapshot {
  streamingNodeId: string | null;
  thinkingNodeId: string | null;
  confirmNodeId: string | null;
  currentIteration: number | undefined;
}

let nodeCounter = 0;

/** LRU 淘汰：当缓存超过上限时，移除最久未访问的条目 */
function evictCacheIfNeeded(cache: Map<string, SessionCacheEntry>) {
  if (cache.size <= MAX_CACHE_SIZE) return;
  let oldestKey: string | null = null;
  let oldestTime = Infinity;
  for (const [key, entry] of cache) {
    if (entry.lastAccessedAt < oldestTime) {
      oldestTime = entry.lastAccessedAt;
      oldestKey = key;
    }
  }
  if (oldestKey) {
    cache.delete(oldestKey);
  }
}

export const useWorkflowStore = create<WorkflowState>((set, get) => ({
  nodes: [],
  executionStatus: "idle",
  error: null,
  confirmHandler: null,
  contextUsage: null,
  sessionCache: new Map(),

  addNode: (type, data, status = "completed", iteration) => {
    const id = `node_${++nodeCounter}`;
    set((state) => ({
      nodes: [
        ...state.nodes,
        {
          id,
          type,
          status,
          timestamp: Date.now(),
          data: data as NodeDataMap[typeof type],
          isExpanded: true,
          iteration,
        } as WorkflowNode,
      ],
    }));
    return id;
  },

  updateNode: (id, updates) => {
    set((state) => ({
      nodes: state.nodes.map((n) => (n.id === id ? { ...n, ...updates } : n)),
    }));
  },

  removeNode: (id) => {
    set((state) => ({
      nodes: state.nodes.filter((n) => n.id !== id),
    }));
  },

  clearNodes: () => {
    nodeCounter = 0;
    // 不重置 sessionCache，它按会话管理
    set({ nodes: [], error: null, executionStatus: "idle", confirmHandler: null, contextUsage: null });
  },

  setExecutionStatus: (status) => {
    set({ executionStatus: status });
  },

  setError: (error) => {
    set({ error });
  },

  toggleNode: (id) => {
    set((state) => ({
      nodes: state.nodes.map((n) =>
        n.id === id ? { ...n, isExpanded: !n.isExpanded } : n
      ),
    }));
  },

  setConfirmHandler: (handler) => {
    set({ confirmHandler: handler });
  },

  loadFromMessages: (messages) => {
    nodeCounter = 0;
    const nodes: WorkflowNode[] = [];
    let iterationCounter = 0;

    // 第一遍：收集 tool 消息的执行结果，按 callId 索引
    // tc.result 为实际值表示成功（JSON 解析成功）；为 null 时结合 msg.content 判断
    const toolResultMap = new Map<string, { success: boolean; error?: string }>();
    for (const msg of messages) {
      if (msg.role === "tool" && msg.toolCalls) {
        for (const tc of msg.toolCalls) {
          if (tc.id) {
            const failed = tc.result == null && msg.content.startsWith("错误:");
            toolResultMap.set(tc.id, {
              success: !failed,
              error: failed ? msg.content : undefined,
            });
          }
        }
      }
    }

    for (const msg of messages) {
      const msgTimestamp = new Date(msg.createdAt).getTime();

      if (msg.role === "user") {
        // 将消息附件映射为工作流节点附件格式
        const nodeAttachments = (msg.attachments || []).map((att, idx) => ({
          id: `att_${idx}`,
          name: att.name,
          path: att.path || att.absolutePath || "",
          size: att.size,
          mimeType: att.mimeType,
        }));
        nodes.push({
          id: `node_${++nodeCounter}`,
          type: "user",
          status: "completed",
          timestamp: msgTimestamp,
          data: { content: msg.content, attachments: nodeAttachments },
          isExpanded: true,
        });
      } else if (msg.role === "assistant") {
        // 每条 assistant 消息递增迭代计数
        iterationCounter += 1;
        const currentIteration = iterationCounter;

        if (msg.reasoningContent && msg.reasoningContent.trim()) {
          nodes.push({
            id: `node_${++nodeCounter}`,
            type: "thinking",
            status: "completed",
            timestamp: msgTimestamp,
            data: { content: msg.reasoningContent, duration: 0, isStreaming: false },
            isExpanded: true,
            iteration: currentIteration,
          });
        }
        // LLM 响应中 content 在 tool_calls 之前输出，因此 content 节点应排在 tool 节点之前
        if (msg.content && msg.content.trim()) {
          nodes.push({
            id: `node_${++nodeCounter}`,
            type: "content",
            status: "completed",
            timestamp: msgTimestamp,
            data: { content: msg.content },
            isExpanded: true,
            iteration: currentIteration,
          });
        }
        if (msg.toolCalls && msg.toolCalls.length > 0) {
          for (const tc of msg.toolCalls) {
            const { success, error } = toolResultMap.get(tc.id) ?? { success: true };
            nodes.push({
              id: `node_${++nodeCounter}`,
              type: "tool",
              status: success ? "completed" as NodeStatus : "failed" as NodeStatus,
              timestamp: msgTimestamp,
              data: {
                toolName: tc.name,
                briefDescription: generateToolBrief(tc.name, (tc.arguments ?? {}) as Record<string, unknown>),
                input: (tc.arguments ?? {}) as Record<string, unknown>,
                callId: tc.id,
                success,
                ...(error ? { error } : {}),
              },
              isExpanded: true,
              iteration: currentIteration,
            });
          }
        }
      }
    }

    set({ nodes, error: null, executionStatus: "idle", confirmHandler: null });
  },

  // 初始化上下文窗口使用情况事件监听，返回取消监听函数
  initContextUsageListener: async () => {
    const unlisten = await onAgentContextUpdate((payload) => {
      const state = get();
      // 如果事件属于当前会话，直接更新 contextUsage
      // 否则更新后台会话缓存
      const currentSessionId = _currentSessionId;
      if (currentSessionId && payload.sessionId !== currentSessionId) {
        // 后台会话：更新缓存中的 contextUsage
        const cache = new Map(state.sessionCache);
        const entry = cache.get(payload.sessionId);
        if (entry) {
          cache.set(payload.sessionId, {
            ...entry,
            contextUsage: payload.contextUsage,
            lastAccessedAt: entry.lastAccessedAt,
          });
          set({ sessionCache: cache });
        }
      } else {
        // 当前会话：直接更新
        set({ contextUsage: payload.contextUsage });
      }
    });
    return unlisten;
  },

  // 从后端加载指定会话的上下文窗口使用信息
  loadContextUsage: async (sessionId: string) => {
    try {
      const usage = await tauriCmd.getContextUsage(sessionId);
      set({ contextUsage: usage });
    } catch {
      // 会话无消息或后端计算失败时，清除上下文使用信息
      set({ contextUsage: null });
    }
  },

  // 清除上下文窗口使用信息（新会话/切换会话时调用）
  clearContextUsage: () => {
    set({ contextUsage: null });
  },

  // 将当前状态保存到指定会话的缓存
  saveSessionToCache: (sessionId: string, streamingRef: StreamingRefSnapshot) => {
    const state = get();
    const cache = new Map(state.sessionCache);

    cache.set(sessionId, {
      nodes: [...state.nodes],
      executionStatus: state.executionStatus,
      error: state.error,
      nodeCounter,
      contextUsage: state.contextUsage,
      streamingNodeId: streamingRef.streamingNodeId,
      thinkingNodeId: streamingRef.thinkingNodeId,
      confirmNodeId: streamingRef.confirmNodeId,
      currentIteration: streamingRef.currentIteration,
      // 保留已有的后台流式状态，如果没有则初始化
      deepThinkingContent: cache.get(sessionId)?.deepThinkingContent ?? "",
      lastDeepThinkingStep: cache.get(sessionId)?.lastDeepThinkingStep ?? 0,
      seenToolCallIds: cache.get(sessionId)?.seenToolCallIds ?? [],
      backgroundNodeCounter: cache.get(sessionId)?.backgroundNodeCounter ?? 0,
      bgStreamingNodeId: cache.get(sessionId)?.bgStreamingNodeId ?? null,
      bgThinkingNodeId: cache.get(sessionId)?.bgThinkingNodeId ?? null,
      bgLastClosedStreamingNodeId: cache.get(sessionId)?.bgLastClosedStreamingNodeId ?? null,
      lastAccessedAt: Date.now(),
    });

    evictCacheIfNeeded(cache);
    set({ sessionCache: cache });
  },

  // 从指定会话的缓存恢复状态，返回是否命中缓存
  restoreSessionFromCache: (sessionId: string) => {
    const state = get();
    const entry = state.sessionCache.get(sessionId);
    if (!entry) return false;

    nodeCounter = entry.nodeCounter;
    set({
      nodes: [...entry.nodes],
      executionStatus: entry.executionStatus,
      error: entry.error,
      contextUsage: entry.contextUsage,
      confirmHandler: null,
    });

    // 更新缓存访问时间
    const cache = new Map(state.sessionCache);
    cache.set(sessionId, { ...entry, lastAccessedAt: Date.now() });
    set({ sessionCache: cache });

    return true;
  },

  // 删除指定会话的缓存
  clearSessionCache: (sessionId: string) => {
    const cache = new Map(get().sessionCache);
    cache.delete(sessionId);
    set({ sessionCache: cache });
  },

  // 获取指定会话缓存的流式引用快照
  getCachedStreamingRefs: (sessionId: string) => {
    const entry = get().sessionCache.get(sessionId);
    if (!entry) return null;
    return {
      streamingNodeId: entry.streamingNodeId,
      thinkingNodeId: entry.thinkingNodeId,
      confirmNodeId: entry.confirmNodeId,
      currentIteration: entry.currentIteration,
    };
  },

  // 将后台 Agent 事件应用到指定会话的缓存
  applyBackgroundEvent: (sessionId: string, event: BackgroundAgentEvent) => {
    const state = get();
    const cache = new Map(state.sessionCache);
    const entry = cache.get(sessionId);
    if (!entry) return; // 无缓存则跳过

    // 深拷贝节点列表以避免引用问题
    let nodes = [...entry.nodes];
    let bgNodeCounter = entry.backgroundNodeCounter;
    let bgStreamingNodeId = entry.bgStreamingNodeId;
    let bgThinkingNodeId = entry.bgThinkingNodeId;
    let bgLastClosedStreamingNodeId = entry.bgLastClosedStreamingNodeId;
    let deepThinkingContent = entry.deepThinkingContent;
    let lastDeepThinkingStep = entry.lastDeepThinkingStep;
    let seenToolCallIds = [...entry.seenToolCallIds];
    let executionStatus = entry.executionStatus;
    let contextUsage = entry.contextUsage;

    const now = Date.now();

    switch (event.type) {
      case "deep_thinking": {
        // 更新迭代追踪
        const iteration = event.iteration;
        // step 变化表示新一轮思考开始，重置累积内容
        if (event.step !== lastDeepThinkingStep) {
          lastDeepThinkingStep = event.step;
          deepThinkingContent = "";
        }
        if (event.isStreaming) {
          deepThinkingContent += event.thought;
        }
        // 关闭当前 streaming 节点
        if (bgStreamingNodeId) {
          nodes = nodes.map((n) =>
            n.id === bgStreamingNodeId
              ? { ...n, status: "completed" as NodeStatus, data: { ...n.data, isStreaming: false } }
              : n
          );
          bgStreamingNodeId = null;
        }
        // 更新或创建 thinking 节点
        if (!bgThinkingNodeId) {
          const nodeId = `bg_node_${++bgNodeCounter}`;
          nodes.push({
            id: nodeId,
            type: "thinking",
            status: event.isStreaming ? "running" : "completed",
            timestamp: now,
            data: { content: deepThinkingContent, duration: 0, isStreaming: event.isStreaming },
            isExpanded: true,
            iteration,
          });
          bgThinkingNodeId = nodeId;
        } else {
          nodes = nodes.map((n) =>
            n.id === bgThinkingNodeId
              ? {
                  ...n,
                  data: { content: deepThinkingContent, duration: 0, isStreaming: event.isStreaming },
                  status: event.isStreaming ? "running" as NodeStatus : "completed" as NodeStatus,
                  iteration,
                }
              : n
          );
          if (!event.isStreaming) {
            bgThinkingNodeId = null;
          }
        }
        break;
      }
      case "content": {
        // 关闭当前 thinking 节点
        if (bgThinkingNodeId) {
          nodes = nodes.map((n) =>
            n.id === bgThinkingNodeId
              ? { ...n, status: "completed" as NodeStatus, data: { ...n.data, isStreaming: false } }
              : n
          );
          bgThinkingNodeId = null;
        }
        if (!bgStreamingNodeId) {
          if (event.content) {
            // 后端在流式结束后发射 is_streaming=false 的完整内容事件，
            // 用于更新被 tool_call 关闭的 content 节点（修复 LLM 在 tool_use 块后
            // 继续输出文本内容导致的截断问题）
            if (!event.isStreaming && bgLastClosedStreamingNodeId) {
              nodes = nodes.map((n) =>
                n.id === bgLastClosedStreamingNodeId
                  ? { ...n, data: { content: event.content, isStreaming: false } }
                  : n
              );
              bgLastClosedStreamingNodeId = null;
            } else {
              const nodeId = `bg_node_${++bgNodeCounter}`;
              nodes.push({
                id: nodeId,
                type: "content",
                status: "running",
                timestamp: now,
                data: { content: event.content, isStreaming: event.isStreaming },
                isExpanded: true,
                iteration: event.iteration,
              });
              bgStreamingNodeId = nodeId;
            }
          }
        } else {
          nodes = nodes.map((n) =>
            n.id === bgStreamingNodeId
              ? { ...n, data: { content: event.content, isStreaming: event.isStreaming } }
              : n
          );
        }
        // 流式结束时关闭节点
        if (!event.isStreaming && bgStreamingNodeId) {
          nodes = nodes.map((n) =>
            n.id === bgStreamingNodeId
              ? { ...n, status: "completed" as NodeStatus, data: { ...n.data, isStreaming: false } }
              : n
          );
          bgStreamingNodeId = null;
        }
        break;
      }
      case "tool_call": {
        // 关闭当前 thinking/streaming 节点
        if (bgThinkingNodeId) {
          nodes = nodes.map((n) =>
            n.id === bgThinkingNodeId
              ? { ...n, status: "completed" as NodeStatus, data: { ...n.data, isStreaming: false } }
              : n
          );
          bgThinkingNodeId = null;
        }
        if (bgStreamingNodeId) {
          // 保存被关闭的 streaming 节点 ID，用于后续最终内容事件更新（修复截断）
          bgLastClosedStreamingNodeId = bgStreamingNodeId;
          nodes = nodes.map((n) =>
            n.id === bgStreamingNodeId
              ? { ...n, status: "completed" as NodeStatus, data: { ...n.data, isStreaming: false } }
              : n
          );
          bgStreamingNodeId = null;
        }
        // 通过 callId 去重
        const existingToolNode = event.callId
          ? nodes.find((n) => n.type === "tool" && (n.data as { callId?: string }).callId === event.callId)
          : undefined;
        if (existingToolNode) {
          nodes = nodes.map((n) =>
            n.id === existingToolNode.id
              ? {
                  ...n,
                  data: {
                    ...n.data,
                    toolName: event.toolName,
                    input: event.arguments,
                    briefDescription: generateToolBrief(event.toolName, event.arguments),
                  },
                }
              : n
          );
        } else {
          if (!seenToolCallIds.includes(event.callId)) {
            seenToolCallIds.push(event.callId);
          }
          const nodeId = `bg_node_${++bgNodeCounter}`;
          nodes.push({
            id: nodeId,
            type: "tool",
            status: "running",
            timestamp: now,
            data: {
              toolName: event.toolName,
              input: event.arguments,
              briefDescription: generateToolBrief(event.toolName, event.arguments),
              callId: event.callId,
            },
            isExpanded: true,
            iteration: event.iteration,
          });
        }
        break;
      }
      case "tool_result": {
        // 通过 callId 匹配工具节点
        const toolNode = event.callId
          ? nodes.find((n) => n.type === "tool" && n.status === "running" && (n.data as { callId?: string }).callId === event.callId)
          : undefined;
        const targetNode = toolNode ?? (() => {
          const runningTools = nodes.filter((n) => n.type === "tool" && n.status === "running");
          return runningTools.length > 0 ? runningTools[runningTools.length - 1] : undefined;
        })();
        if (targetNode) {
          nodes = nodes.map((n) =>
            n.id === targetNode.id
              ? {
                  ...n,
                  status: event.success ? "completed" as NodeStatus : "failed" as NodeStatus,
                  data: {
                    ...n.data,
                    success: event.success,
                    error: event.success ? undefined : (event.error || i18n.t("toolNode.executionFailed")),
                  },
                }
              : n
          );
        }
        break;
      }
      case "context_update": {
        contextUsage = event.contextUsage;
        break;
      }
      case "done": {
        // 关闭所有 running 节点
        if (bgThinkingNodeId) {
          nodes = nodes.map((n) =>
            n.id === bgThinkingNodeId
              ? { ...n, status: "completed" as NodeStatus, data: { ...n.data, isStreaming: false } }
              : n
          );
          bgThinkingNodeId = null;
        }
        if (bgStreamingNodeId) {
          nodes = nodes.map((n) =>
            n.id === bgStreamingNodeId
              ? { ...n, status: "completed" as NodeStatus, data: { content: event.summary, isStreaming: false } }
              : n
          );
          bgStreamingNodeId = null;
        } else if (event.summary) {
          const nodeId = `bg_node_${++bgNodeCounter}`;
          nodes.push({
            id: nodeId,
            type: "content",
            status: "completed",
            timestamp: now,
            data: { content: event.summary, isStreaming: false },
            isExpanded: true,
          });
        }
        executionStatus = "completed";
        bgLastClosedStreamingNodeId = null;
        break;
      }
      case "error": {
        if (bgThinkingNodeId) {
          nodes = nodes.map((n) =>
            n.id === bgThinkingNodeId
              ? { ...n, status: "failed" as NodeStatus, data: { ...n.data, isStreaming: false } }
              : n
          );
          bgThinkingNodeId = null;
        }
        if (bgStreamingNodeId) {
          nodes = nodes.map((n) =>
            n.id === bgStreamingNodeId
              ? { ...n, status: "failed" as NodeStatus }
              : n
          );
          bgStreamingNodeId = null;
        }
        const errorNodeId = `bg_node_${++bgNodeCounter}`;
        nodes.push({
          id: errorNodeId,
          type: "error",
          status: "completed",
          timestamp: now,
          data: {
            code: event.code,
            message: event.message,
            recoverable: event.recoverable,
            module: "",
          },
          isExpanded: true,
        });
        executionStatus = "failed";
        bgLastClosedStreamingNodeId = null;
        break;
      }
      case "stopped": {
        if (bgThinkingNodeId) {
          nodes = nodes.map((n) =>
            n.id === bgThinkingNodeId
              ? { ...n, status: "cancelled" as NodeStatus, data: { ...n.data, isStreaming: false } }
              : n
          );
          bgThinkingNodeId = null;
        }
        if (bgStreamingNodeId) {
          nodes = nodes.map((n) =>
            n.id === bgStreamingNodeId
              ? { ...n, status: "cancelled" as NodeStatus }
              : n
          );
          bgStreamingNodeId = null;
        }
        executionStatus = "cancelled";
        bgLastClosedStreamingNodeId = null;
        break;
      }
    }

    cache.set(sessionId, {
      ...entry,
      nodes,
      executionStatus,
      contextUsage,
      backgroundNodeCounter: bgNodeCounter,
      bgStreamingNodeId,
      bgThinkingNodeId,
      bgLastClosedStreamingNodeId,
      deepThinkingContent,
      lastDeepThinkingStep,
      seenToolCallIds,
      lastAccessedAt: Date.now(),
    });

    set({ sessionCache: cache });
  },

  // 获取指定会话缓存的 contextUsage
  getCachedContextUsage: (sessionId: string) => {
    const entry = get().sessionCache.get(sessionId);
    return entry?.contextUsage ?? null;
  },
}));

/** 当前会话 ID 的外部追踪，供 initContextUsageListener 区分当前/后台会话 */
// 使用模块级变量而非 store 状态，避免不必要的重渲染
let _currentSessionId: string | null = null;

/** 设置当前会话 ID（由 useAgent hook 在 sessionId 变化时调用） */
export function setCurrentSessionId(sessionId: string | null) {
  _currentSessionId = sessionId;
}
