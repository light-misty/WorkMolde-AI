/**
 * 统一错误处理服务
 * 集中管理所有 Tauri 命令调用的错误解析、用户友好消息映射和 Toast 通知
 * 替代各处分散的 try/catch + console.error 模式
 */
import { useToastStore } from "../stores/useToastStore";
import i18n from "../i18n";

/** 后端 CommandError 结构 */
export interface CommandError {
  code: number;
  message: string;
}

/** 解析后的结构化错误信息 */
export interface ParsedError {
  /** 原始错误码 */
  code: number;
  /** 用户友好的中文错误描述 */
  userMessage: string;
  /** 错误所属模块 */
  module: ErrorModule;
  /** 是否可恢复（可重试） */
  recoverable: boolean;
  /** 原始错误对象 */
  raw: unknown;
}

/** 错误模块分类 */
export type ErrorModule = "llm" | "agent" | "document" | "database" | "config" | "filesystem" | "runtime" | "update" | "unknown";

/** 错误码到用户友好消息的映射 */
const ERROR_MESSAGE_MAP: Record<number, string> = {
  // LLM 相关 (1000-1999)
  1001: i18n.t("errors.llm.connectionFailed"),
  1002: i18n.t("errors.llm.invalidApiKey"),
  1003: i18n.t("errors.llm.rateLimited"),
  1004: i18n.t("errors.llm.quotaExhausted"),
  1005: i18n.t("errors.llm.modelUnavailable"),
  1006: i18n.t("errors.llm.timeout"),
  1007: i18n.t("errors.llm.invalidParams"),
  1008: i18n.t("errors.llm.streamInterrupted"),
  1009: i18n.t("errors.llm.serviceUnavailable"),
  1010: i18n.t("errors.llm.parseFailed"),
  1011: i18n.t("errors.llm.dnsFailed"),
  1012: i18n.t("errors.llm.connectionRefused"),
  1013: i18n.t("errors.llm.tlsFailed"),
  1014: i18n.t("errors.llm.networkUnreachable"),

  // Agent 相关 (2000-2999)
  2001: i18n.t("errors.agent.alreadyRunning"),
  2002: i18n.t("errors.agent.notRunning"),
  2003: i18n.t("errors.agent.maxIterations"),
  2004: i18n.t("errors.agent.confirmTimeout"),
  2005: i18n.t("errors.agent.operationDenied"),
  2006: i18n.t("errors.agent.featureNotFound"),
  2008: i18n.t("errors.agent.executionError"),
  2010: i18n.t("errors.agent.sessionNotFound"),

  // 文档处理 (3000-3999)
  3001: i18n.t("errors.doc.fileNotFound"),
  3002: i18n.t("errors.doc.unsupportedFormat"),
  3003: i18n.t("errors.doc.parseFailed"),
  3004: i18n.t("errors.doc.writeFailed"),
  3005: i18n.t("errors.doc.convertFailed"),
  3006: i18n.t("errors.doc.templateNotFound"),
  3007: i18n.t("errors.doc.templateProcessFailed"),
  3008: i18n.t("errors.doc.versionNotFound"),
  3009: i18n.t("errors.doc.rollbackFailed"),
  3010: i18n.t("errors.doc.serviceError"),
  3011: i18n.t("errors.doc.noPermission"),
  3012: i18n.t("errors.doc.fileTooLarge"),

  // 数据库 (4000-4999)
  4001: i18n.t("errors.db.connectionFailed"),
  4002: i18n.t("errors.db.queryFailed"),
  4003: i18n.t("errors.db.recordNotFound"),
  4004: i18n.t("errors.db.recordExists"),
  4005: i18n.t("errors.db.constraintConflict"),
  4006: i18n.t("errors.db.migrationFailed"),
  4007: i18n.t("errors.db.corrupted"),

  // 配置 (5000-5999)
  5001: i18n.t("errors.config.invalidFormat"),
  5002: i18n.t("errors.config.missingField"),
  5003: i18n.t("errors.config.invalidValue"),
  5004: i18n.t("errors.config.importFailed"),
  5005: i18n.t("errors.config.exportFailed"),
  5006: i18n.t("errors.config.providerNotFound"),

  // 文件系统 (6000-6999)
  6001: i18n.t("errors.fs.pathNotFound"),
  6002: i18n.t("errors.fs.noPermission"),
  6003: i18n.t("errors.fs.fileExists"),
  6004: i18n.t("errors.fs.notDirectory"),
  6005: i18n.t("errors.fs.diskFull"),
  6006: i18n.t("errors.fs.operationFailed"),
  6007: i18n.t("errors.fs.watchFailed"),
  6008: i18n.t("errors.fs.encodingError"),

  // 运行时 (7000-7999)
  7001: i18n.t("errors.runtime.internalError"),

  // 更新相关 (8000-8999)
  8001: i18n.t("errors.update.checkFailed"),
  8002: i18n.t("errors.update.downloadFailed"),
  8003: i18n.t("errors.update.installFailed"),
  8004: i18n.t("errors.update.alreadyLatest"),
  8005: i18n.t("errors.update.networkError"),
};

/** 可恢复的错误码集合（网络超时、临时不可用等） */
const RECOVERABLE_CODES = new Set([
  1001, 1003, 1006, 1008, 1009, 1010, 1011, 1012, 1013, 1014,
  2004, 2008,
  3010,
  4001, 4002,
  7001,
  8001, 8002, 8005,
]);

/** 根据错误码判断所属模块 */
function getErrorModule(code: number): ErrorModule {
  if (code >= 1000 && code < 2000) return "llm";
  if (code >= 2000 && code < 3000) return "agent";
  if (code >= 3000 && code < 4000) return "document";
  if (code >= 4000 && code < 5000) return "database";
  if (code >= 5000 && code < 6000) return "config";
  if (code >= 6000 && code < 7000) return "filesystem";
  if (code >= 7000 && code < 8000) return "runtime";
  if (code >= 8000 && code < 9000) return "update";
  return "unknown";
}

/** 解析 Tauri invoke 抛出的错误为结构化错误信息 */
export function parseError(error: unknown): ParsedError {
  // 尝试解析为 CommandError 结构
  if (error && typeof error === "object") {
    const obj = error as Record<string, unknown>;
    if (typeof obj.code === "number" && typeof obj.message === "string") {
      const code = obj.code as number;
      return {
        code,
        userMessage: ERROR_MESSAGE_MAP[code] || obj.message as string,
        module: getErrorModule(code),
        recoverable: RECOVERABLE_CODES.has(code),
        raw: error,
      };
    }
  }

  // 字符串错误
  if (typeof error === "string") {
    return {
      code: 0,
      userMessage: error,
      module: "unknown",
      recoverable: false,
      raw: error,
    };
  }

  // Error 实例
  if (error instanceof Error) {
    return {
      code: 0,
      userMessage: error.message,
      module: "unknown",
      recoverable: false,
      raw: error,
    };
  }

  // 未知错误
  return {
    code: 0,
    userMessage: i18n.t("errors.unknown"),
    module: "unknown",
    recoverable: false,
    raw: error,
  };
}

/** 错误通知配置 */
export interface ErrorNotifyOptions {
  /** 是否显示 Toast 通知，默认 true */
  showToast?: boolean;
  /** Toast 类型覆盖，默认根据模块自动选择 */
  toastType?: "error" | "warning";
  /** 是否在控制台输出日志，默认 true */
  log?: boolean;
  /** 上下文描述，用于日志 */
  context?: string;
}

/**
 * 统一错误处理函数
 * 解析错误、记录日志、显示 Toast 通知
 * 返回结构化的 ParsedError 供调用方进一步处理
 */
export function handleError(error: unknown, options: ErrorNotifyOptions = {}): ParsedError {
  const {
    showToast = true,
    toastType,
    log: shouldLog = true,
    context,
  } = options;

  const parsed = parseError(error);

  if (shouldLog) {
    const prefix = context ? `[${context}]` : "[ErrorHandler]";
    console.error(`${prefix} 错误码=${parsed.code}, 模块=${parsed.module}, 可恢复=${parsed.recoverable}`, error);
  }

  if (showToast) {
    // 可恢复的错误用 warning 类型，不可恢复的用 error 类型
    const type = toastType || (parsed.recoverable ? "warning" : "error");
    useToastStore.getState().addToast(type, parsed.userMessage);
  }

  return parsed;
}

/**
 * 创建带统一错误处理的 invoke 包装函数
 * 自动捕获错误、解析、通知，并返回 Result 类型
 */
export async function safeInvoke<T>(
  fn: () => Promise<T>,
  options: ErrorNotifyOptions & {
    /** 错误时是否重新抛出，默认 false */
    rethrow?: boolean;
  } = {},
): Promise<{ ok: true; data: T } | { ok: false; error: ParsedError }> {
  const { rethrow = false, ...errorOptions } = options;

  try {
    const data = await fn();
    return { ok: true, data };
  } catch (error) {
    const parsed = handleError(error, errorOptions);
    if (rethrow) {
      throw error;
    }
    return { ok: false, error: parsed };
  }
}
