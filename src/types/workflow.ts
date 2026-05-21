// ===== 工作流节点类型定义 =====

export type NodeStatus = "pending" | "running" | "completed" | "failed" | "cancelled";

export type ExecutionStatus = "idle" | "running" | "stopping" | "paused" | "completed" | "failed" | "cancelled";

export type WorkflowNodeType = "user" | "thinking" | "tool" | "result" | "reply" | "confirm";

export interface Attachment {
  id: string;
  name: string;
  path: string;
  size: number;
  mimeType: string;
}

export interface UserNodeData {
  content: string;
  attachments: Attachment[];
}

export interface ThinkingNodeData {
  content: string;
  duration: number;
}

export interface ToolNodeData {
  toolName: string;
  toolBadge?: string;
  input: Record<string, unknown>;
  output?: Record<string, unknown>;
}

export interface ResultNodeData {
  content: string;
  success: boolean;
  filePaths: string[];
}

export interface ReplyNodeData {
  content: string;
}

export interface ConfirmNodeData {
  title: string;
  description: string;
  confirmLabel: string;
  cancelLabel: string;
  confirmed: boolean | null;
}

export interface NodeDataMap {
  user: UserNodeData;
  thinking: ThinkingNodeData;
  tool: ToolNodeData;
  result: ResultNodeData;
  reply: ReplyNodeData;
  confirm: ConfirmNodeData;
}

export interface WorkflowNode<T extends WorkflowNodeType = WorkflowNodeType> {
  id: string;
  type: T;
  status: NodeStatus;
  timestamp: number;
  data: NodeDataMap[T];
  isExpanded: boolean;
  error?: string;
}

export interface DiffStats {
  additions: number;
  deletions: number;
  files: number;
}
