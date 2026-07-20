import { create } from "zustand";
import type { WorkflowNode, WorkflowNodeType, NodeStatus, ExecutionStatus, NodeDataMap, SubAgentNodeData, UserNodeData } from "../types";
import type { Message, BranchGroupInfo } from "../types/session";
import type { ContextUsageInfo } from "../types/settings";

import { extractToolPath } from "../utils/format";
import { useWorkspaceStore } from "./useWorkspaceStore";
import { onAgentContextUpdate, type QuestionItem } from "../services/event";
import * as tauriCmd from "../services/tauri";
import i18n from "../i18n";

/** 获取当前工作区的根目录绝对路径 */
function getWorkspaceRoot(): string {
  const { workspaces, currentWorkspaceId } = useWorkspaceStore.getState();
  const ws = workspaces.find((w) => w.id === currentWorkspaceId);
  return ws?.path || '';
}

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
  /** 后台流式状态：当前压缩节点 ID（compaction_start 创建，compaction_done 更新） */
  bgCompactionNodeId: string | null;
  /** 最后访问时间，用于 LRU 淘汰 */
  lastAccessedAt: number;
  /** 分支组列表快照，restore 时恢复 */
  branchGroups: BranchGroupInfo[];
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
  | { type: "stopped"; completedSteps: number; reason: string }
  | { type: "compaction_start"; tokensBefore: number }
  | { type: "compaction_done"; tokensBefore: number; tokensAfter: number; compacted: boolean; error?: string }
  | { type: "sub_agent_status"; agentId: string; status: string; message?: string; iteration: number; taskDescription: string }
  | { type: "sub_agent_tool_call"; agentId: string; toolName: string; arguments: Record<string, unknown>; iteration: number }
  | { type: "question"; questionId: string; questions: QuestionItem[] };

interface WorkflowState {
  nodes: WorkflowNode[];
  executionStatus: ExecutionStatus;
  error: string | null;
  confirmHandler: ((approved: boolean, feedback?: string) => Promise<void>) | null;
  /** 权限审批回调（once/reject 双态） */
  permissionHandler: ((response: 'once' | 'reject', feedback?: string) => Promise<void>) | null;
  /** 上下文窗口使用信息（Agent 运行时实时更新） */
  contextUsage: ContextUsageInfo | null;
  /** 按会话缓存的状态映射 */
  sessionCache: Map<string, SessionCacheEntry>;
  /** 当前查看的子 Agent ID，null 表示显示主工作流 */
  currentSubAgentId: string | null;
  /** 子 Agent 工作流节点列表 */
  subAgentNodes: WorkflowNode[];
  /** 当前会话的所有分支组信息，用于分支切换器渲染 */
  branchGroups: BranchGroupInfo[];
  /** 当前会话的活跃分支 ID（loadFromMessages 时保存，供组件查询） */
  activeBranchId: string;
  /** 创建分支后待发送的消息（由 UserNode 设置，App.tsx 监听消费） */
  pendingBranchSend: { content: string; branchGroupId: string } | null;

  addNode: <T extends WorkflowNodeType>(type: T, data: NodeDataMap[T], status?: NodeStatus, iteration?: number) => string;
  updateNode: (id: string, updates: Partial<WorkflowNode>) => void;
  removeNode: (id: string) => void;
  clearNodes: () => void;
  setExecutionStatus: (status: ExecutionStatus) => void;
  setError: (error: string | null) => void;
  toggleNode: (id: string) => void;
  setConfirmHandler: (handler: ((approved: boolean, feedback?: string) => Promise<void>) | null) => void;
  /** 设置权限审批回调（与 setConfirmHandler 并存，permissionHandler 优先） */
  setPermissionHandler: (handler: ((response: 'once' | 'reject', feedback?: string) => Promise<void>) | null) => void;
  loadFromMessages: (messages: Message[], branchGroups: BranchGroupInfo[], activeBranchId: string) => void;
  /** 设置当前查看的子 Agent ID */
  setCurrentSubAgentId: (agentId: string | null) => void;
  /** 将子 Agent 消息转换为工作流节点，设置 subAgentNodes */
  loadSubAgentMessages: (messages: Message[]) => void;
  /** 清空子 Agent 工作流状态 */
  clearSubAgentWorkflow: () => void;
  /** 子 Agent 工作流:追加思考内容（流式） */
  appendSubAgentThinking: (agentId: string, content: string, isStreaming: boolean, iteration: number) => void;
  /** 子 Agent 工作流:追加内容（流式） */
  appendSubAgentContent: (agentId: string, content: string, isStreaming: boolean, iteration: number) => void;
  /** 子 Agent 工作流:添加工具调用节点 */
  addSubAgentToolNode: (agentId: string, toolCallId: string, toolName: string, args: Record<string, unknown>, iteration: number) => void;
  /** 子 Agent 工作流:更新工具执行结果 */
  updateSubAgentToolResult: (agentId: string, toolCallId: string, result: string | undefined, error: string | undefined, success: boolean) => void;
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
  /** 设置/清除待发送的分支消息（UserNode 创建分支后设置，App.tsx 消费后清空） */
  setPendingBranchSend: (data: { content: string; branchGroupId: string } | null) => void;
}

/** 流式状态引用快照，切换会话时保存/恢复 */
export interface StreamingRefSnapshot {
  streamingNodeId: string | null;
  thinkingNodeId: string | null;
  confirmNodeId: string | null;
  currentIteration: number | undefined;
}

let nodeCounter = 0;

// 子 Agent 工作流节点计数器（独立于主工作流的 nodeCounter）
let subAgentNodeCounter = 0;

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

/** 将消息列表转换为工作流节点列表的核心逻辑（供 loadFromMessages 和 loadSubAgentMessages 复用） */
function convertMessagesToNodes(
  messages: Message[],
  branchGroups: BranchGroupInfo[] = [],
  activeBranchId: string = ""
): WorkflowNode[] {
  nodeCounter = 0;
  const nodes: WorkflowNode[] = [];
  let iterationCounter = 0;

  // 第一遍：收集 tool 消息的执行结果，按 callId 索引
  // tc.result 为实际值表示成功（JSON 解析成功）；为 null 时结合 msg.content 判断
  // 同时收集 metadata，用于恢复 question/confirm/sub_agent 节点
  const toolResultMap = new Map<string, { success: boolean; error?: string; metadata?: Record<string, unknown> }>();
  for (const msg of messages) {
    if (msg.role === "tool" && msg.toolCalls) {
      for (const tc of msg.toolCalls) {
        if (tc.id) {
          const failed = tc.result == null && msg.content.startsWith("错误:");
          toolResultMap.set(tc.id, {
            success: !failed,
            error: failed ? msg.content : undefined,
            metadata: msg.metadata,
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

      // 计算分支切换器信息
      let branchIndex: number | undefined;
      let branchTotal: number | undefined;
      if (msg.branchGroupId && activeBranchId) {
        const group = branchGroups.find((g) => g.branchGroupId === msg.branchGroupId);
        if (group && group.branches.length > 1) {
          branchTotal = group.branches.length;
          // 找到 activeBranchId 在组内的位置（1-based）
          const idx = group.branches.findIndex((b) => b.branchId === activeBranchId);
          if (idx >= 0) {
            branchIndex = idx + 1;
          }
        }
      }

      nodes.push({
        id: `node_${++nodeCounter}`,
        type: "user",
        status: "completed",
        timestamp: msgTimestamp,
        data: {
          content: msg.content,
          attachments: nodeAttachments,
          messageId: msg.id,
          branchId: msg.branchId,
          branchGroupId: msg.branchGroupId,
          branchIndex,
          branchTotal,
        } as UserNodeData,
        isExpanded: true,
      });
    } else if (msg.role === "assistant") {
      // 检查是否为 error 节点
      if (msg.metadata?.nodeType === "error") {
        nodes.push({
          id: `node_${++nodeCounter}`,
          type: "error",
          status: "failed",
          timestamp: msgTimestamp,
          data: {
            code: (msg.metadata.code as number) ?? 0,
            message: (msg.metadata.message as string) ?? msg.content,
            recoverable: (msg.metadata.recoverable as boolean) ?? false,
            module: "",
            messageId: msg.id,
          },
          isExpanded: true,
        });
        continue; // 跳过 thinking/content/tool 节点创建
      }

      // 每条 assistant 消息递增迭代计数
      iterationCounter += 1;
      const currentIteration = iterationCounter;

      if (msg.reasoningContent && msg.reasoningContent.trim()) {
        nodes.push({
          id: `node_${++nodeCounter}`,
          type: "thinking",
          status: "completed",
          timestamp: msgTimestamp,
          data: { content: msg.reasoningContent, duration: 0, isStreaming: false, messageId: msg.id },
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
          data: { content: msg.content, messageId: msg.id },
          isExpanded: true,
          iteration: currentIteration,
        });
      }
      if (msg.toolCalls && msg.toolCalls.length > 0) {
        for (const tc of msg.toolCalls) {
          const { success, error, metadata } = toolResultMap.get(tc.id) ?? { success: true };

          // 根据 metadata.nodeType 创建不同类型的节点
          if (metadata?.nodeType === "sub_agent") {
            // 创建 sub_agent 节点（从持久化消息恢复）
            // 根据 metadata.success 设置节点状态（兼容旧数据，默认为成功）
            const subSuccess = (metadata.success as boolean) ?? true;
            const subStatus: NodeStatus = subSuccess ? "completed" : "failed";
            // 从 metadata.error 恢复错误信息（失败时）
            const subMessage = subSuccess ? undefined : (metadata.error as string | null) ?? undefined;
            // 从 metadata.iterations 恢复迭代次数
            const subIteration = (metadata.iterations as number) ?? 0;
            // 从 metadata.toolCalls 恢复工具调用列表（兼容旧数据：若为数字则回退为空数组）
            const rawToolCalls = metadata.toolCalls;
            const subToolCalls = Array.isArray(rawToolCalls)
              ? (rawToolCalls as Array<{ toolName: string; arguments: Record<string, unknown> }>)
              : [];
            nodes.push({
              id: `subagent-${tc.id}`,
              type: "sub_agent",
              status: subStatus,
              timestamp: msgTimestamp,
              iteration: undefined, // 不显示迭代次数
              data: {
                agentId: (metadata.agentId as string) ?? "",
                taskDescription: (metadata.taskDescription as string) ?? "",
                status: subStatus,
                iteration: subIteration,
                toolCalls: subToolCalls,
                message: subMessage,
                messageId: msg.id,
              } as SubAgentNodeData,
              isExpanded: true,
            });
          } else if (metadata?.nodeType === "question") {
            // 创建 question 节点
            nodes.push({
              id: `node_${++nodeCounter}`,
              type: "question",
              status: "completed",
              timestamp: msgTimestamp,
              data: {
                questionId: (metadata.questionId as string) ?? tc.id,
                questions: (metadata.questions as any[]) ?? [],
                answers: (metadata.answers as any[]) ?? [],
                answered: true,
                messageId: msg.id,
              },
              isExpanded: true,
              iteration: currentIteration,
            });
          } else if (metadata?.nodeType === "confirm") {
            // 创建 confirm 节点
            nodes.push({
              id: `node_${++nodeCounter}`,
              type: "confirm",
              status: "completed",
              timestamp: msgTimestamp,
              data: {
                title: (metadata.operationType as string) ?? tc.name,
                description: (metadata.description as string) ?? "",
                confirmLabel: "确认",
                cancelLabel: "取消",
                confirmed: (metadata.approved as boolean) ?? false,
                riskLevel: (metadata.riskLevel as string) ?? "normal",
                messageId: msg.id,
              },
              isExpanded: true,
              iteration: currentIteration,
            });
          } else {
            // 普通 tool 节点（现有逻辑）
            nodes.push({
              id: `node_${++nodeCounter}`,
              type: "tool",
              status: success ? "completed" as NodeStatus : "failed" as NodeStatus,
              timestamp: msgTimestamp,
              data: {
                toolName: tc.name,
                filePath: extractToolPath(tc.name, (tc.arguments ?? {}) as Record<string, unknown>, getWorkspaceRoot()),
                input: (tc.arguments ?? {}) as Record<string, unknown>,
                callId: tc.id,
                success,
                messageId: msg.id,
                ...(error ? { error } : {}),
              },
              isExpanded: true,
              iteration: currentIteration,
            });
          }
        }
      }
    }
  }

  return nodes;
}

export const useWorkflowStore = create<WorkflowState>((set, get) => ({
  nodes: [],
  executionStatus: "idle",
  error: null,
  confirmHandler: null,
  permissionHandler: null,
  contextUsage: null,
  sessionCache: new Map(),
  currentSubAgentId: null,
  subAgentNodes: [],
  branchGroups: [],
  activeBranchId: "",
  pendingBranchSend: null,

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
    set({ nodes: [], error: null, executionStatus: "idle", confirmHandler: null, permissionHandler: null, contextUsage: null, currentSubAgentId: null, subAgentNodes: [] });
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

  setPermissionHandler: (handler) => {
    set({ permissionHandler: handler });
  },

  loadFromMessages: (messages, branchGroups, activeBranchId) => {
    const nodes = convertMessagesToNodes(messages, branchGroups, activeBranchId);
    set({ nodes, error: null, executionStatus: "idle", confirmHandler: null, permissionHandler: null, branchGroups, activeBranchId });
  },

  setCurrentSubAgentId: (agentId) => {
    set({ currentSubAgentId: agentId });
  },

  loadSubAgentMessages: (messages) => {
    // 保存主工作流的 nodeCounter，避免子 Agent 节点生成影响主工作流 ID 序列
    const savedCounter = nodeCounter;
    subAgentNodeCounter = 0;  // 重置子 Agent 节点计数器
    const nodes = convertMessagesToNodes(messages);
    nodeCounter = savedCounter;
    // 保留当前 running 状态的流式节点，避免竞态条件导致节点丢失或位置错误
    // 竞态场景：用户进入子 Agent 页面 → 流式事件创建 running 节点 →
    // listSubAgentMessages 返回并覆盖 subAgentNodes → running 节点丢失
    const currentNodes = get().subAgentNodes;
    const runningNodes = currentNodes.filter(n => n.status === "running");
    if (runningNodes.length > 0) {
      let maxCounter = 0;
      for (const runningNode of runningNodes) {
        // 提取 running 节点的 counter 值，用于后续更新 subAgentNodeCounter
        const match = runningNode.id.match(/^subagent_node_(\d+)$/);
        if (match) {
          const counter = parseInt(match[1], 10);
          if (counter > maxCounter) maxCounter = counter;
        }
        // 若数据库节点已包含同迭代同类型的 completed 节点，说明该节点已完成并持久化，跳过避免重复
        const hasCompleted = nodes.some(n =>
          n.type === runningNode.type &&
          n.iteration === runningNode.iteration &&
          n.status === "completed"
        );
        if (!hasCompleted) {
          nodes.push(runningNode);
        }
      }
      // 确保 subAgentNodeCounter 不与保留的 running 节点 ID 冲突
      subAgentNodeCounter = Math.max(subAgentNodeCounter, maxCounter);
    }
    set({ subAgentNodes: nodes });
  },

  clearSubAgentWorkflow: () => {
    subAgentNodeCounter = 0;
    set({ currentSubAgentId: null, subAgentNodes: [] });
  },

  appendSubAgentThinking: (_agentId, content, isStreaming, iteration) => {
    const state = get();
    const nodes = [...state.subAgentNodes];
    // 查找最后一个 running 的 thinking 节点
    let thinkingNodeIdx = -1;
    for (let i = nodes.length - 1; i >= 0; i--) {
      if (nodes[i].type === "thinking" && nodes[i].status === "running") {
        thinkingNodeIdx = i;
        break;
      }
    }
    // 跨迭代不复用旧 running 节点：新迭代开始意味着旧迭代的 thinking 已完成
    // 避免上一迭代的孤立 running 节点被新迭代复用导致位置错误
    if (thinkingNodeIdx >= 0 && nodes[thinkingNodeIdx].iteration !== iteration) {
      const existing = nodes[thinkingNodeIdx];
      nodes[thinkingNodeIdx] = {
        ...existing,
        status: "completed",
        data: { ...existing.data, isStreaming: false },
      };
      thinkingNodeIdx = -1;
    }
    if (thinkingNodeIdx >= 0) {
      // 追加到已有节点
      const existing = nodes[thinkingNodeIdx];
      const existingContent = (existing.data as { content: string }).content || "";
      nodes[thinkingNodeIdx] = {
        ...existing,
        data: {
          ...existing.data,
          content: existingContent + content,
          isStreaming,
        },
        status: isStreaming ? "running" : "completed",
        iteration,
      };
    } else {
      // 空内容不创建新节点（close 事件和流式空 delta 均跳过）
      if (content.length === 0) {
        return;
      }
      // 创建新 thinking 节点
      const nodeId = `subagent_node_${++subAgentNodeCounter}`;
      nodes.push({
        id: nodeId,
        type: "thinking",
        status: isStreaming ? "running" : "completed",
        timestamp: Date.now(),
        data: { content, duration: 0, isStreaming },
        isExpanded: true,
        iteration,
      });
    }
    set({ subAgentNodes: nodes });
  },

  appendSubAgentContent: (_agentId, content, isStreaming, iteration) => {
    const state = get();
    const nodes = [...state.subAgentNodes];
    // 查找最后一个 running 的 content 节点
    let contentNodeIdx = -1;
    for (let i = nodes.length - 1; i >= 0; i--) {
      if (nodes[i].type === "content" && nodes[i].status === "running") {
        contentNodeIdx = i;
        break;
      }
    }
    // 跨迭代不复用旧 running 节点：新迭代开始意味着旧迭代的 content 已完成
    // 避免上一迭代的孤立 running content 节点被新迭代复用导致位置错误
    if (contentNodeIdx >= 0 && nodes[contentNodeIdx].iteration !== iteration) {
      const existing = nodes[contentNodeIdx];
      nodes[contentNodeIdx] = {
        ...existing,
        status: "completed",
        data: { ...existing.data, isStreaming: false },
      };
      contentNodeIdx = -1;
    }
    if (contentNodeIdx >= 0) {
      const existing = nodes[contentNodeIdx];
      const existingContent = (existing.data as { content: string }).content || "";
      nodes[contentNodeIdx] = {
        ...existing,
        data: {
          ...existing.data,
          content: existingContent + content,
          isStreaming,
        },
        status: isStreaming ? "running" : "completed",
        iteration,
      };
    } else {
      // 空内容不创建新节点（close 事件和流式空 delta 均跳过）
      if (content.length === 0) {
        return;
      }
      const nodeId = `subagent_node_${++subAgentNodeCounter}`;
      nodes.push({
        id: nodeId,
        type: "content",
        status: isStreaming ? "running" : "completed",
        timestamp: Date.now(),
        data: { content, isStreaming },
        isExpanded: true,
        iteration,
      });
    }
    set({ subAgentNodes: nodes });
  },

  addSubAgentToolNode: (_agentId, toolCallId, toolName, args, iteration) => {
    const state = get();
    const nodes = [...state.subAgentNodes];
    const nodeId = `subagent_node_${++subAgentNodeCounter}`;
    nodes.push({
      id: nodeId,
      type: "tool",
      status: "running",
      timestamp: Date.now(),
      data: {
        toolName,
        callId: toolCallId,
        input: args,
        filePath: extractToolPath(toolName, args, getWorkspaceRoot()),
      },
      isExpanded: true,
      iteration,
    });
    set({ subAgentNodes: nodes });
  },

  updateSubAgentToolResult: (_agentId, toolCallId, result, error, success) => {
    const state = get();
    const nodes = [...state.subAgentNodes];
    // 按 callId 匹配 tool 节点
    const toolNodeIdx = nodes.findIndex(
      (n) => n.type === "tool" && (n.data as { callId?: string }).callId === toolCallId
    );
    if (toolNodeIdx >= 0) {
      const existing = nodes[toolNodeIdx];
      // 解析 result 字符串为 JSON 对象
      let parsedResult: unknown = undefined;
      if (result) {
        try {
          parsedResult = JSON.parse(result);
        } catch {
          parsedResult = result;
        }
      }
      nodes[toolNodeIdx] = {
        ...existing,
        status: success ? "completed" : "failed",
        data: {
          ...existing.data,
          success,
          error: success ? undefined : (error || i18n.t("toolNode.executionFailed")),
          result: success && parsedResult ? (parsedResult as Record<string, unknown>) : (existing.data as { result?: Record<string, unknown> }).result,
        },
      };
      set({ subAgentNodes: nodes });
    }
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
      bgCompactionNodeId: cache.get(sessionId)?.bgCompactionNodeId ?? null,
      lastAccessedAt: Date.now(),
      branchGroups: state.branchGroups,
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
      permissionHandler: null,
      branchGroups: entry.branchGroups,
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
    let bgCompactionNodeId = entry.bgCompactionNodeId;
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
                    filePath: extractToolPath(event.toolName, event.arguments, getWorkspaceRoot()),
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
              filePath: extractToolPath(event.toolName, event.arguments, getWorkspaceRoot()),
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
                    // 保存工具执行结果（如 bash 的 stdout/stderr/exit_code）
                    result: event.success && event.result
                      ? (event.result as Record<string, unknown>)
                      : (n.data as { result?: Record<string, unknown> }).result,
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
      case "compaction_start": {
        // 创建压缩节点，状态为 running，等待 compaction_done 更新
        const nodeId = `bg_node_${++bgNodeCounter}`;
        nodes.push({
          id: nodeId,
          type: "compaction",
          status: "running",
          timestamp: now,
          data: {
            tokensBefore: event.tokensBefore,
          },
          isExpanded: true,
        });
        bgCompactionNodeId = nodeId;
        break;
      }
      case "compaction_done": {
        // 更新压缩节点结果
        if (bgCompactionNodeId) {
          const isFailed = !event.compacted || !!event.error;
          nodes = nodes.map((n) =>
            n.id === bgCompactionNodeId
              ? {
                  ...n,
                  status: isFailed ? ("failed" as NodeStatus) : ("completed" as NodeStatus),
                  data: {
                    ...n.data,
                    tokensBefore: event.tokensBefore,
                    tokensAfter: event.tokensAfter,
                    compacted: event.compacted,
                    ...(event.error ? { error: event.error } : {}),
                  },
                }
              : n
          );
          bgCompactionNodeId = null;
        } else {
          // 未找到压缩开始节点（可能缓存被清理过），直接创建一个已完成节点
          const nodeId = `bg_node_${++bgNodeCounter}`;
          const isFailed = !event.compacted || !!event.error;
          nodes.push({
            id: nodeId,
            type: "compaction",
            status: isFailed ? ("failed" as NodeStatus) : ("completed" as NodeStatus),
            timestamp: now,
            data: {
              tokensBefore: event.tokensBefore,
              tokensAfter: event.tokensAfter,
              compacted: event.compacted,
              ...(event.error ? { error: event.error } : {}),
            },
            isExpanded: true,
          });
        }
        break;
      }
      case "sub_agent_status": {
        // 查找已有的 sub_agent 节点（按 agentId 匹配）
        const existingNode = nodes.find(
          (n) => n.type === "sub_agent" && (n.data as SubAgentNodeData).agentId === event.agentId
        );
        if (existingNode) {
          // 更新已有节点：保留 taskDescription 和 toolCalls，更新状态相关字段
          const existingData = existingNode.data as SubAgentNodeData;
          nodes = nodes.map((n) =>
            n.id === existingNode.id
              ? {
                  ...n,
                  status: event.status as NodeStatus,
                  data: {
                    ...existingData,
                    status: event.status,
                    iteration: event.iteration,
                    message: event.message,
                  },
                }
              : n
          );
        } else {
          // 首次事件：创建节点
          const nodeId = `bg_node_${++bgNodeCounter}`;
          nodes.push({
            id: nodeId,
            type: "sub_agent",
            status: event.status as NodeStatus,
            timestamp: now,
            data: {
              agentId: event.agentId,
              taskDescription: event.taskDescription,
              status: event.status,
              iteration: event.iteration,
              toolCalls: [],
              message: event.message,
            },
            isExpanded: true,
          });
        }
        break;
      }
      case "sub_agent_tool_call": {
        // 查找已有的 sub_agent 节点（按 agentId 匹配）
        const existingNode = nodes.find(
          (n) => n.type === "sub_agent" && (n.data as SubAgentNodeData).agentId === event.agentId
        );
        if (existingNode) {
          // 在 toolCalls 数组中追加工具调用记录
          const existingData = existingNode.data as SubAgentNodeData;
          nodes = nodes.map((n) =>
            n.id === existingNode.id
              ? {
                  ...n,
                  data: {
                    ...existingData,
                    toolCalls: [
                      ...existingData.toolCalls,
                      { toolName: event.toolName, arguments: event.arguments },
                    ],
                    iteration: event.iteration,
                  },
                }
              : n
          );
        } else {
          // 未找到对应节点，创建新节点（使用空字符串作为默认 taskDescription）
          const nodeId = `bg_node_${++bgNodeCounter}`;
          nodes.push({
            id: nodeId,
            type: "sub_agent",
            status: "running" as NodeStatus,
            timestamp: now,
            data: {
              agentId: event.agentId,
              taskDescription: "",
              status: "running",
              iteration: event.iteration,
              toolCalls: [{ toolName: event.toolName, arguments: event.arguments }],
            },
            isExpanded: true,
          });
        }
        break;
      }
      case "question": {
        // 创建 question 节点（status="running"）
        const nodeId = `bg_node_${++bgNodeCounter}`;
        nodes.push({
          id: nodeId,
          type: "question",
          status: "running",
          timestamp: now,
          data: {
            questionId: event.questionId,
            questions: event.questions,
            answered: false,
          },
          isExpanded: true,
        });
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
      bgCompactionNodeId,
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

  // 设置/清除待发送的分支消息
  setPendingBranchSend: (data) => set({ pendingBranchSend: data }),
}));

/** 当前会话 ID 的外部追踪，供 initContextUsageListener 区分当前/后台会话 */
// 使用模块级变量而非 store 状态，避免不必要的重渲染
let _currentSessionId: string | null = null;

/** 设置当前会话 ID（由 useAgent hook 在 sessionId 变化时调用） */
export function setCurrentSessionId(sessionId: string | null) {
  _currentSessionId = sessionId;
}
