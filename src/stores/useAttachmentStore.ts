import { create } from "zustand";
import type { AttachmentMeta, AttachmentType } from "../types/session";

interface AttachmentState {
  /** 当前待发送的附件列表 */
  attachments: AttachmentMeta[];
  /** 添加附件 */
  addAttachment: (attachment: AttachmentMeta) => void;
  /** 移除附件 */
  removeAttachment: (index: number) => void;
  /** 清空所有附件 */
  clearAttachments: () => void;
  /** 设置附件列表 */
  setAttachments: (attachments: AttachmentMeta[]) => void;
}

export const useAttachmentStore = create<AttachmentState>((set) => ({
  attachments: [],

  addAttachment: (attachment) =>
    set((state) => ({
      attachments: [...state.attachments, attachment],
    })),

  removeAttachment: (index) =>
    set((state) => ({
      attachments: state.attachments.filter((_, i) => i !== index),
    })),

  clearAttachments: () => set({ attachments: [] }),

  setAttachments: (attachments) => set({ attachments }),
}));

/** 根据 MIME 类型推断附件类型 */
export function inferAttachmentType(mimeType: string): AttachmentType {
  if (mimeType.startsWith("image/")) {
    return "image";
  }
  const textTypes = [
    "text/plain", "text/markdown", "text/csv", "text/html",
    "application/json", "text/xml", "application/xml",
    "text/yaml", "text/x-yaml", "text/toml", "text/ini", "text/log",
  ];
  if (textTypes.includes(mimeType)) {
    return "text";
  }
  // 精确匹配文档 MIME 类型
  const documentTypes = [
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "application/pdf",
  ];
  if (documentTypes.includes(mimeType)) {
    return "document";
  }
  // 模糊匹配文档格式
  if (
    mimeType.includes("pdf") ||
    mimeType.includes("word") ||
    mimeType.includes("excel") ||
    mimeType.includes("spreadsheet") ||
    mimeType.includes("presentation") ||
    mimeType.includes("document")
  ) {
    return "document";
  }
  return "text";
}

/** 支持的附件 MIME 类型 */
export const SUPPORTED_ATTACHMENT_MIME_TYPES = [
  // 图片
  "image/png", "image/jpeg", "image/jpg", "image/gif", "image/webp",
  // 文本
  "text/plain", "text/markdown", "text/csv", "text/html",
  "application/json", "text/xml", "application/xml",
  "text/yaml", "text/x-yaml", "text/toml", "text/ini", "text/log",
  // 文档（通过 Sidecar 解析）
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  "application/vnd.openxmlformats-officedocument.presentationml.presentation",
  "application/pdf",
];

/** 图片文件大小上限 (20MB) */
export const MAX_IMAGE_SIZE = 20 * 1024 * 1024;

/** 文本文件大小上限 (1MB) */
export const MAX_TEXT_SIZE = 1024 * 1024;

/** 文档文件大小上限 (10MB) */
export const MAX_DOCUMENT_SIZE = 10 * 1024 * 1024;

/** 单次发送附件数量上限 */
export const MAX_ATTACHMENT_COUNT = 10;

/** 检查附件是否包含图片 */
export function hasImageAttachments(attachments: AttachmentMeta[]): boolean {
  return attachments.some((a) => a.type === "image");
}
