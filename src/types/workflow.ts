export type NodeStatus = "pending" | "running" | "completed" | "failed" | "cancelled";

export type ExecutionStatus = "idle" | "running" | "stopping" | "paused" | "completed" | "failed" | "cancelled";

export type WorkflowNodeType = "user" | "thinking" | "content" | "tool" | "confirm" | "error" | "compaction" | "sub_agent" | "question";

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
  isStreaming?: boolean;
}

export interface ContentNodeData {
  content: string;
  isStreaming?: boolean;
}

export interface ToolNodeData {
  toolName: string;
  briefDescription: string;
  input: Record<string, unknown>;
  /** 工具调用的唯一标识，用于流式阶段提前发射后去重更新 */
  callId?: string;
  success?: boolean;
  error?: string;
  /** 工具执行结果（如 bash 的 stdout/stderr/exit_code），成功时填充 */
  result?: Record<string, unknown>;
}

export interface ConfirmNodeData {
  title: string;
  description: string;
  confirmLabel: string;
  cancelLabel: string;
  confirmed: boolean | null;
  /** 用户拒绝时填写的反馈原因 */
  feedback?: string;
  /** 风险等级（critical/high/medium/normal） */
  riskLevel?: string;
  /** 权限审批回复（once/reject） */
  permissionResponse?: 'once' | 'reject' | null;
}

export interface ErrorNodeData {
  code: number;
  message: string;
  recoverable: boolean;
  module: string;
}

/** 上下文压缩节点数据 */
export interface CompactionNodeData {
  /** 压缩前 token 数 */
  tokensBefore: number;
  /** 压缩后 token 数（compaction_done 事件到达后填充） */
  tokensAfter?: number;
  /** 是否实际执行了压缩（compaction_done 事件到达后填充） */
  compacted?: boolean;
  /** 压缩失败时的错误信息 */
  error?: string;
}

/** 子 Agent 节点数据 */
export interface SubAgentNodeData {
  /** 子 Agent 唯一标识 */
  agentId: string;
  /** 任务描述 */
  taskDescription: string;
  /** 状态: "running" | "completed" | "failed" | "cancelled" */
  status: string;
  /** 当前迭代次数 */
  iteration: number;
  /** 工具调用记录 */
  toolCalls: Array<{ toolName: string; arguments: Record<string, unknown> }>;
  /** 附加消息（错误信息或结果摘要） */
  message?: string;
}

/** 提问节点数据 */
export interface QuestionNodeData {
  /** 提问唯一标识，提交回答时使用 */
  questionId: string;
  /** 问题列表 */
  questions: Array<{
    /** 短标签 */
    header: string;
    /** 完整问题文本 */
    question: string;
    /** 选项列表 */
    options: Array<{ label: string; description: string }>;
    /** 是否允许多选 */
    multiSelect: boolean;
  }>;
  /** 用户回答（提交后填充） */
  answers?: Array<{ questionIndex: number; selectedOptions: string[] }>;
  /** 是否已回答 */
  answered: boolean;
}

export interface NodeDataMap {
  user: UserNodeData;
  thinking: ThinkingNodeData;
  content: ContentNodeData;
  tool: ToolNodeData;
  confirm: ConfirmNodeData;
  error: ErrorNodeData;
  compaction: CompactionNodeData;
  sub_agent: SubAgentNodeData;
  question: QuestionNodeData;
}

export interface WorkflowNode<T extends WorkflowNodeType = WorkflowNodeType> {
  id: string;
  type: T;
  status: NodeStatus;
  timestamp: number;
  data: NodeDataMap[T];
  isExpanded: boolean;
  error?: string;
  /** 当前迭代轮次序号（从 1 开始），用于前端按迭代分组展示 */
  iteration?: number;
}

export interface DiffStats {
  additions: number;
  deletions: number;
  files: number;
}
