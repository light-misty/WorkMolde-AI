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
}

export interface SessionSummary {
  id: string;
  title: string;
  status: string;
  messageCount: number;
  lastMessagePreview?: string;
  createdAt: string;
  updatedAt: string;
}

export interface SessionDetail {
  session: Session;
  messages: Message[];
}

export interface Message {
  id: string;
  role: string;
  content: string;
  toolCalls?: ToolCall[];
  reasoningContent?: string;
  /** 附件元信息列表 */
  attachments?: AttachmentMeta[];
  createdAt: string;
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
