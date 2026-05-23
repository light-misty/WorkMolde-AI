/**
 * 统一错误处理服务
 * 集中管理所有 Tauri 命令调用的错误解析、用户友好消息映射和 Toast 通知
 * 替代各处分散的 try/catch + console.error 模式
 */
import { useToastStore } from "../stores/useToastStore";

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
  1001: "无法连接到 AI 服务，请检查网络连接",
  1002: "API 密钥无效或已过期，请在设置中更新",
  1003: "请求过于频繁，请稍后重试",
  1004: "API 配额已用尽，请检查账户余额",
  1005: "指定的模型不可用，请更换模型",
  1006: "AI 服务响应超时，请稍后重试",
  1007: "请求参数无效，请检查输入内容",
  1008: "流式响应中断，请重试",
  1009: "AI 服务暂时不可用，已自动切换到备用服务",
  1010: "AI 响应解析失败，请重试",

  // Agent 相关 (2000-2999)
  2001: "Agent 正在执行中，请等待完成后再试",
  2002: "Agent 未在运行",
  2003: "任务执行步骤过多，已自动停止",
  2004: "操作确认超时，请重新发送请求",
  2005: "操作已被用户拒绝",
  2006: "请求的功能不存在",
  2007: "该功能已被禁用，请在设置中启用",
  2008: "Agent 执行出错，请重试",
  2009: "Token 用量已超出预算限制",
  2010: "会话不存在，请创建新会话",

  // 文档处理 (3000-3999)
  3001: "文件不存在，请检查文件路径",
  3002: "不支持的文档格式",
  3003: "文档解析失败，文件可能已损坏",
  3004: "文档写入失败，请检查文件权限",
  3005: "格式转换失败，请检查源文件",
  3006: "模板不存在",
  3007: "模板处理失败",
  3008: "版本记录不存在",
  3009: "版本回滚失败",
  3010: "文档处理服务异常，正在自动恢复...",
  3011: "没有文件操作权限",
  3012: "文件过大，无法处理",

  // 数据库 (4000-4999)
  4001: "数据库连接失败，请重启应用",
  4002: "数据查询失败",
  4003: "记录不存在",
  4004: "记录已存在",
  4005: "数据约束冲突",
  4006: "数据库迁移失败，请重启应用",
  4007: "数据库损坏，请重启应用尝试修复",

  // 配置 (5000-5999)
  5001: "配置文件格式无效",
  5002: "配置缺少必要字段",
  5003: "配置值无效",
  5004: "配置导入失败",
  5005: "配置导出失败",
  5006: "未找到指定的 AI 服务配置",
  5007: "请先设置默认 AI 服务",

  // 文件系统 (6000-6999)
  6001: "路径不存在",
  6002: "没有操作权限",
  6003: "文件已存在",
  6004: "路径不是目录",
  6005: "磁盘空间不足",
  6006: "文件操作失败",
  6007: "文件监听失败",
  6008: "文件编码错误",

  // 运行时 (7000-7999)
  7001: "内部通信错误",

  // 更新相关 (8000-8999)
  8001: "检查更新失败，请检查网络连接",
  8002: "更新下载失败，请重试",
  8003: "更新安装失败，请重新启动应用",
  8004: "当前已是最新版本",
  8005: "更新服务网络错误，请稍后重试",
};

/** 可恢复的错误码集合（网络超时、临时不可用等） */
const RECOVERABLE_CODES = new Set([
  1001, 1003, 1006, 1008, 1009, 1010,
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
    userMessage: "未知错误",
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
