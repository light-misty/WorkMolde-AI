import { create } from "zustand";
import type { WorkflowNode, WorkflowNodeType, NodeStatus, ExecutionStatus, NodeDataMap } from "../types";
import type { Message } from "../types/session";

interface WorkflowState {
  nodes: WorkflowNode[];
  executionStatus: ExecutionStatus;
  error: string | null;
  autoScroll: boolean;
  confirmHandler: ((approved: boolean) => Promise<void>) | null;

  addNode: <T extends WorkflowNodeType>(type: T, data: NodeDataMap[T], status?: NodeStatus) => string;
  updateNode: (id: string, updates: Partial<WorkflowNode>) => void;
  removeNode: (id: string) => void;
  clearNodes: () => void;
  setExecutionStatus: (status: ExecutionStatus) => void;
  setError: (error: string | null) => void;
  toggleNode: (id: string) => void;
  setAutoScroll: (autoScroll: boolean) => void;
  setConfirmHandler: (handler: ((approved: boolean) => Promise<void>) | null) => void;
  loadFromMessages: (messages: Message[]) => void;
}

let nodeCounter = 0;

export const useWorkflowStore = create<WorkflowState>((set) => ({
  nodes: [],
  executionStatus: "idle",
  error: null,
  autoScroll: true,
  confirmHandler: null,

  addNode: (type, data, status = "completed") => {
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
    set({ nodes: [], error: null, executionStatus: "idle", confirmHandler: null });
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

  setAutoScroll: (autoScroll) => {
    set({ autoScroll });
  },

  setConfirmHandler: (handler) => {
    set({ confirmHandler: handler });
  },

  // 从后端消息列表加载工作流节点（用于历史会话恢复）
  loadFromMessages: (messages) => {
    nodeCounter = 0;
    const nodes: WorkflowNode[] = [];

    for (const msg of messages) {
      // 使用消息的实际创建时间，而非当前时间
      const msgTimestamp = new Date(msg.createdAt).getTime();

      if (msg.role === "user") {
        // 用户消息 -> user 节点
        nodes.push({
          id: `node_${++nodeCounter}`,
          type: "user",
          status: "completed",
          timestamp: msgTimestamp,
          data: { content: msg.content, attachments: [] },
          isExpanded: true,
        });
      } else if (msg.role === "assistant") {
        // 助手消息可能包含 tool_calls 和/或文本内容
        if (msg.toolCalls && msg.toolCalls.length > 0) {
          // 每个 tool_call 生成一个 tool 节点
          for (const tc of msg.toolCalls) {
            nodes.push({
              id: `node_${++nodeCounter}`,
              type: "tool",
              status: "completed",
              timestamp: msgTimestamp,
              data: {
                toolName: tc.name,
                input: (tc.arguments ?? {}) as Record<string, unknown>,
              },
              isExpanded: true,
            });
          }
        }
        // 有文本内容时生成 reply 节点
        if (msg.content && msg.content.trim()) {
          nodes.push({
            id: `node_${++nodeCounter}`,
            type: "reply",
            status: "completed",
            timestamp: msgTimestamp,
            data: { content: msg.content },
            isExpanded: true,
          });
        }
      } else if (msg.role === "tool") {
        // 工具结果 -> result 节点
        const isSuccess = !msg.content.startsWith("错误:");
        nodes.push({
          id: `node_${++nodeCounter}`,
          type: "result",
          status: "completed",
          timestamp: msgTimestamp,
          data: {
            content: msg.content,
            success: isSuccess,
            filePaths: [],
          },
          isExpanded: true,
        });
      }
    }

    set({ nodes, error: null, executionStatus: "idle", confirmHandler: null });
  },
}));
