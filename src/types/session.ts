// ===== 会话类型定义 - 与 Rust 后端对齐 =====

/** 附件类型枚举 */
export type AttachmentType = "image" | "document" | "text";

/** 附件元信息 */
export interface AttachmentMeta {
  /** 文件在工作区中的相对路径 */
  path?: string;
  /** 文件绝对路径 */
  absolutePath?: string;
  /** 文件名 */
  name: string;
  /** MIME 类型 */
  mimeType: string;
  /** 文件大小 (字节) */
  size: number;
  /** 附件类型 */
  type: AttachmentType;
  /** 文件内容 base64 编码 (浏览器端读取后传入) */
  data?: string;
}

export interface Session {
  id: string;
  title: string;
  workspaceId?: string;
  providerId: string;
  templateId?: string;
  createdAt: string;
  updatedAt: string;
  status: string;
  /** 当前活跃分支 ID */
  activeBranchId?: string;
}

/** 消息分支（用于对话分支管理） */
export interface Branch {
  id: string;
  sessionId: string;
  parentBranchId?: string;
  forkMessageId?: string;
  branchGroupId?: string;
  name: string;
  sortOrder: number;
  createdAt: string;
}

/** 分支组内的单条分支信息 */
export interface BranchInfo {
  branchId: string;
  name: string;
  sortOrder: number;
}

/** 分支组信息（用于前端渲染切换器） */
export interface BranchGroupInfo {
  branchGroupId: string;
  forkMessageId?: string;
  branches: BranchInfo[];
}

/** 创建分支命令返回结果 */
export interface CreateBranchResult {
  branchId: string;
  branchGroupId: string;
}

/** 分支内的用户消息简要信息（用于跨分支搜索） */
export interface BranchUserMessage {
  messageId: string;
  sessionId: string;
  branchId: string;
  content: string;
  createdAt: string;
}

export interface SessionSummary {
  id: string;
  title: string;
  workspaceId?: string;
  status: string;
  messageCount: number;
  lastMessagePreview?: string;
  createdAt: string;
  updatedAt: string;
}

export interface SessionDetail {
  session: Session;
  messages: Message[];
  /** 会话所有分支列表 */
  branches: Branch[];
  /** 当前活跃分支 ID */
  activeBranchId: string;
}

export interface Message {
  id: string;
  role: string;
  content: string;
  toolCalls?: ToolCall[];
  reasoningContent?: string;
  /** 附件元信息列表 */
  attachments?: AttachmentMeta[];
  /** 工作流节点扩展信息 (用于持久化 question/confirm/error 节点详情) */
  metadata?: Record<string, unknown>;
  createdAt: string;
  /** 消息所属分支 ID */
  branchId?: string;
  /** 分支组 ID（同一父消息下的多个分支共享） */
  branchGroupId?: string;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
  result?: unknown;
}

export interface CreateSessionParams {
  title?: string;
  workspaceId?: string;
  providerId?: string;
  templateId?: string;
}

export interface SessionFilter {
  workspaceId?: string;
  status?: string;
  search?: string;
  limit?: number;
  offset?: number;
}
