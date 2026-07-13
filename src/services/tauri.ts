/**
 * Tauri 命令调用封装
 * 为每个后端 Tauri 命令提供对应的 TypeScript 异步函数
 * 函数名使用 camelCase，调用 invoke 时使用 snake_case 命令名
 * 统一使用 safeInvoke 进行错误处理和 Toast 通知
 */
import { invoke } from "@tauri-apps/api/core";
import { safeInvoke } from "./errorHandler";

import type {
  ConnectionResult,
  ProviderConfig,
  ProviderInfo,
  CreateSessionParams,
  SessionFilter,
  Session,
  SessionSummary,
  SessionDetail,
  Message,
  WorkspaceInfo,
  FileNode,
  SearchOptions,
  SearchResult,
  PreviewContent,
  VersionInfo,
  HandlerInfo,
  ToolInfo,
  AppSettings,
  PromptTemplate,
  CreateTemplateParams,
  UpdateTemplateParams,
  ContextUsageInfo,
  PermissionRule,
  AddPermissionRuleParams,
  UpdatePermissionRuleParams,
  LspServerInfo,
} from "../types";

// ================================================================
// LLM 命令
// ================================================================

/** 测试 LLM Provider 连接 */
export async function testConnection(providerId: string): Promise<ConnectionResult> {
  const result = await safeInvoke(() => invoke<ConnectionResult>("test_connection", { providerId }), { context: "testConnection" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 使用临时配置测试 LLM Provider 连接（用于添加/编辑模式，编辑时传入 providerId 以便空 API Key 时查找已保存密钥） */
export async function testConnectionWithConfig(config: ProviderConfig, providerId?: string): Promise<ConnectionResult> {
  const result = await safeInvoke(() => invoke<ConnectionResult>("test_connection_with_config", { config, providerId: providerId ?? null }), { context: "testConnectionWithConfig" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 列出所有 LLM Provider */
export async function listProviders(): Promise<ProviderInfo[]> {
  const result = await safeInvoke(() => invoke<ProviderInfo[]>("list_providers"), { context: "listProviders" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 添加 LLM Provider */
export async function addProvider(config: ProviderConfig): Promise<void> {
  const result = await safeInvoke(() => invoke("add_provider", { config }), { context: "addProvider" });
  if (!result.ok) throw result.error.raw;
}

/** 更新 LLM Provider */
export async function updateProvider(providerId: string, config: ProviderConfig): Promise<void> {
  const result = await safeInvoke(() => invoke("update_provider", { providerId, config }), { context: "updateProvider" });
  if (!result.ok) throw result.error.raw;
}

/** 删除 LLM Provider */
export async function deleteProvider(providerId: string): Promise<void> {
  const result = await safeInvoke(() => invoke("delete_provider", { providerId }), { context: "deleteProvider" });
  if (!result.ok) throw result.error.raw;
}

/** 对所有 LLM Provider 执行健康检查 */
export async function healthCheckProviders(): Promise<Record<string, ConnectionResult>> {
  const result = await safeInvoke(() => invoke<Record<string, ConnectionResult>>("health_check_providers"), { context: "healthCheckProviders" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 强制恢复所有 Provider 为可用状态并重建 HTTP 客户端 */
export async function forceRecoverProviders(): Promise<void> {
  const result = await safeInvoke(() => invoke("force_recover_providers"), { context: "forceRecoverProviders" });
  if (!result.ok) throw result.error.raw;
}

/** 获取当前网络状态 */
export async function getNetworkStatus(): Promise<string> {
  const result = await safeInvoke(() => invoke<string>("get_network_status"), { context: "getNetworkStatus" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

// ================================================================
// 会话命令
// ================================================================

/** 创建新会话 */
export async function createSession(params: CreateSessionParams): Promise<Session> {
  const result = await safeInvoke(() => invoke<Session>("create_session", { params }), { context: "createSession" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 列出会话 */
export async function listSessions(filter?: SessionFilter): Promise<SessionSummary[]> {
  const result = await safeInvoke(() => invoke<SessionSummary[]>("list_sessions", { filter: filter ?? null }), { context: "listSessions" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 获取会话详情 */
export async function getSession(sessionId: string): Promise<SessionDetail> {
  const result = await safeInvoke(() => invoke<SessionDetail>("get_session", { sessionId }), { context: "getSession" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 删除会话 */
export async function deleteSession(sessionId: string): Promise<void> {
  const result = await safeInvoke(() => invoke("delete_session", { sessionId }), { context: "deleteSession" });
  if (!result.ok) throw result.error.raw;
}

/** 更新会话标题 */
export async function updateSessionTitle(sessionId: string, title: string): Promise<void> {
  const result = await safeInvoke(() => invoke("update_session_title", { sessionId, title }), { context: "updateSessionTitle" });
  if (!result.ok) throw result.error.raw;
}

/** 清除所有会话数据 */
export async function clearAllSessions(): Promise<number> {
  const result = await safeInvoke(() => invoke<number>("clear_all_sessions"), { context: "clearAllSessions" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 清除指定工作区下的所有会话 */
export async function clearWorkspaceSessions(workspaceId: string): Promise<number> {
  const result = await safeInvoke(() => invoke<number>("clear_workspace_sessions", { workspaceId }), { context: "clearWorkspaceSessions" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 更新会话的工作区 ID（用于修复旧数据中 workspace_id 为空的会话） */
export async function updateSessionWorkspace(sessionId: string, workspaceId: string): Promise<void> {
  const result = await safeInvoke(() => invoke("update_session_workspace", { sessionId, workspaceId }), { context: "updateSessionWorkspace" });
  if (!result.ok) throw result.error.raw;
}

// ================================================================
// 工作区命令
// ================================================================

/** 列出所有工作区 */
export async function listWorkspaces(): Promise<WorkspaceInfo[]> {
  const result = await safeInvoke(() => invoke<WorkspaceInfo[]>("list_workspaces"), { context: "listWorkspaces" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 添加工作区 */
export async function addWorkspace(path: string, name?: string): Promise<WorkspaceInfo> {
  const result = await safeInvoke(() => invoke<WorkspaceInfo>("add_workspace", { path, name: name ?? null }), { context: "addWorkspace" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 移除工作区 */
export async function removeWorkspace(workspaceId: string): Promise<void> {
  const result = await safeInvoke(() => invoke("remove_workspace", { workspaceId }), { context: "removeWorkspace" });
  if (!result.ok) throw result.error.raw;
}

/** 设置活动工作区 */
export async function setActiveWorkspace(workspaceId: string): Promise<void> {
  const result = await safeInvoke(() => invoke("set_active_workspace", { workspaceId }), { context: "setActiveWorkspace" });
  if (!result.ok) throw result.error.raw;
}

/** 获取文件树 */
export async function getFileTree(
  workspaceId: string,
  path?: string,
  depth?: number,
): Promise<FileNode[]> {
  const result = await safeInvoke(() => invoke<FileNode[]>("get_file_tree", {
    workspaceId,
    path: path ?? null,
    depth: depth ?? null,
  }), { context: "getFileTree" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 搜索文件 */
export async function searchFiles(
  workspaceId: string,
  query: string,
  options?: SearchOptions,
): Promise<SearchResult[]> {
  const result = await safeInvoke(() => invoke<SearchResult[]>("search_files", {
    workspaceId,
    query,
    options: options ?? null,
  }), { context: "searchFiles" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

// ================================================================
// 文档命令
// ================================================================

/** 预览文档 */
export async function previewDocument(
  workspaceId: string,
  path: string,
): Promise<PreviewContent> {
  const result = await safeInvoke(() => invoke<PreviewContent>("preview_document", { workspaceId, path }), { context: "previewDocument" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 获取 PDF 文件的 base64 编码数据，用于 pdfjs-dist 渲染 */
export async function getPdfData(
  workspaceId: string,
  path: string,
): Promise<string> {
  const result = await safeInvoke(() => invoke<string>("get_pdf_data", { workspaceId, path }), { context: "getPdfData" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 获取文档版本历史 */
export async function getDocumentVersions(
  workspaceId: string,
  path: string,
): Promise<VersionInfo[]> {
  const result = await safeInvoke(() => invoke<VersionInfo[]>("get_document_versions", { workspaceId, path }), { context: "getDocumentVersions" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 回滚到指定版本 */
export async function rollbackVersion(
  workspaceId: string,
  path: string,
  versionId: string,
): Promise<void> {
  const result = await safeInvoke(() => invoke("rollback_version", { workspaceId, path, versionId }), { context: "rollbackVersion" });
  if (!result.ok) throw result.error.raw;
}

/** 获取指定版本快照的文档内容，用于版本预览和差异对比 */
export async function getVersionContent(
  workspaceId: string,
  path: string,
  versionId: string,
): Promise<PreviewContent> {
  const result = await safeInvoke(() => invoke<PreviewContent>("get_version_content", { workspaceId, path, versionId }), { context: "getVersionContent" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 创建空文件 */
export async function createFile(
  workspaceId: string,
  path: string,
): Promise<void> {
  const result = await safeInvoke(() => invoke("create_file", { workspaceId, path }), { context: "createFile" });
  if (!result.ok) throw result.error.raw;
}

/** 创建目录 */
export async function createDirectory(
  workspaceId: string,
  path: string,
): Promise<void> {
  const result = await safeInvoke(() => invoke("create_directory", { workspaceId, path }), { context: "createDirectory" });
  if (!result.ok) throw result.error.raw;
}

/** 重命名文件或目录 */
export async function renameFile(
  workspaceId: string,
  oldPath: string,
  newPath: string,
): Promise<void> {
  const result = await safeInvoke(() => invoke("rename_file", { workspaceId, oldPath, newPath }), { context: "renameFile" });
  if (!result.ok) throw result.error.raw;
}

/** 删除文件或目录（永久删除） */
export async function deleteFile(
  workspaceId: string,
  path: string,
): Promise<void> {
  const result = await safeInvoke(() => invoke("delete_file", { workspaceId, path }), { context: "deleteFile" });
  if (!result.ok) throw result.error.raw;
}

/** 在系统文件管理器中显示 */
export async function showInFileManager(
  workspaceId: string,
  path: string,
): Promise<void> {
  const result = await safeInvoke(() => invoke("show_in_file_manager", { workspaceId, path }), { context: "showInFileManager" });
  if (!result.ok) throw result.error.raw;
}

// ================================================================
// Tool 命令
// ================================================================

/** 列出所有 Tool */
export async function listTools(): Promise<ToolInfo[]> {
  const result = await safeInvoke(() => invoke<ToolInfo[]>("list_tools"), { context: "listTools" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

// ================================================================
// Handler 命令
// ================================================================

/** 列出所有 Handler */
export async function listHandlers(): Promise<HandlerInfo[]> {
  const result = await safeInvoke(() => invoke<HandlerInfo[]>("list_handlers"), { context: "listHandlers" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

// ================================================================
// 设置命令
// ================================================================

/** 获取应用设置 */
export async function getSettings(): Promise<AppSettings> {
  const result = await safeInvoke(() => invoke<AppSettings>("get_settings"), { context: "getSettings" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 更新应用设置 */
export async function updateSettings(settings: Record<string, unknown>): Promise<void> {
  const result = await safeInvoke(() => invoke("update_settings", { settings }), { context: "updateSettings" });
  if (!result.ok) throw result.error.raw;
}

// ================================================================
// Agent 命令
// ================================================================

/** 启动 Agent */
export async function startAgent(
  sessionId: string,
  prompt: string,
  options?: Record<string, unknown>,
): Promise<void> {
  const result = await safeInvoke(() => invoke("start_agent", { sessionId, prompt, options: options ?? null }), {
    context: "startAgent",
    showToast: false, // Agent 错误通过事件系统处理，不重复显示 Toast
  });
  if (!result.ok) throw result.error.raw;
}

/** 停止 Agent */
export async function stopAgent(sessionId: string): Promise<void> {
  const result = await safeInvoke(() => invoke("stop_agent", { sessionId }), { context: "stopAgent" });
  if (!result.ok) throw result.error.raw;
}

/** 确认 Agent 操作 */
export async function confirmOperation(
  sessionId: string,
  operationId: string,
  approved: boolean,
  feedback?: string,
): Promise<void> {
  const result = await safeInvoke(() => invoke("confirm_operation", {
    sessionId,
    operationId,
    approved,
    feedback: feedback ?? null,
  }), { context: "confirmOperation" });
  if (!result.ok) throw result.error.raw;
}

/** 权限审批回复（双态权限系统：once/reject） */
export async function permissionRespond(
  sessionId: string,
  operationId: string,
  response: 'once' | 'reject',
  feedback?: string,
): Promise<void> {
  const result = await safeInvoke(() => invoke("permission_respond", {
    sessionId,
    operationId,
    response,
    feedback: feedback ?? null,
  }), { context: "permissionRespond" });
  if (!result.ok) throw result.error.raw;
}

/** 切换 Agent 模式（Plan/Build/Document），由前端按钮触发 */
export async function switchAgentMode(sessionId: string, mode: 'plan' | 'build' | 'document'): Promise<void> {
  const result = await safeInvoke(() => invoke("switch_agent_mode", { sessionId, mode }), {
    context: "switchAgentMode",
  });
  if (!result.ok) throw result.error.raw;
}

/** 获取上下文窗口使用信息 */
export async function getContextUsage(sessionId: string): Promise<ContextUsageInfo> {
  const result = await safeInvoke(() => invoke<ContextUsageInfo>("get_context_usage", { sessionId }), { context: "getContextUsage" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 检查指定会话的 Agent 是否正在运行 */
export async function isAgentRunning(sessionId: string): Promise<boolean> {
  const result = await safeInvoke(() => invoke<boolean>("is_agent_running", { sessionId }), { context: "isAgentRunning" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 提交用户对 Agent 提问的回答 */
export async function submitQuestionAnswer(
  questionId: string,
  answers: Array<{ questionIndex: number; selectedOptions: string[] }>,
): Promise<void> {
  const result = await safeInvoke(() => invoke("submit_question_answer", { questionId, answers }), { context: "submitQuestionAnswer" });
  if (!result.ok) throw result.error.raw;
}

/** 查询指定子 Agent 的所有持久化消息 */
export async function listSubAgentMessages(agentId: string): Promise<Message[]> {
  const result = await safeInvoke(() => invoke<Message[]>("list_sub_agent_messages", { agentId }), { context: "listSubAgentMessages" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

// ================================================================
// 模板命令
// ================================================================

/** 列出所有 Prompt 模板 */
export async function listTemplates(): Promise<PromptTemplate[]> {
  const result = await safeInvoke(() => invoke<PromptTemplate[]>("list_templates"), { context: "listTemplates" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 获取单个 Prompt 模板 */
export async function getTemplate(templateId: string): Promise<PromptTemplate> {
  const result = await safeInvoke(() => invoke<PromptTemplate>("get_template", { templateId }), { context: "getTemplate" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 创建 Prompt 模板 */
export async function createTemplate(params: CreateTemplateParams): Promise<PromptTemplate> {
  const result = await safeInvoke(() => invoke<PromptTemplate>("create_template", { params }), { context: "createTemplate" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 更新 Prompt 模板 */
export async function updateTemplate(templateId: string, params: UpdateTemplateParams): Promise<PromptTemplate> {
  const result = await safeInvoke(() => invoke<PromptTemplate>("update_template", { templateId, params }), { context: "updateTemplate" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 删除 Prompt 模板 */
export async function deleteTemplate(templateId: string): Promise<void> {
  const result = await safeInvoke(() => invoke("delete_template", { templateId }), { context: "deleteTemplate" });
  if (!result.ok) throw result.error.raw;
}

// ================================================================
// 日志命令
// ================================================================

/** 获取日志目录路径 */
export async function getLogPath(): Promise<{ logSource: string }> {
  const result = await safeInvoke(() => invoke<{ logSource: string }>("get_log_path"), { context: "getLogPath" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 在系统文件管理器中打开指定目录 */
export async function openDirectory(path: string): Promise<void> {
  const result = await safeInvoke(() => invoke<void>("open_directory", { path }), { context: "openDirectory" });
  if (!result.ok) throw result.error.raw;
}

// ================================================================
// 更新命令
// ================================================================

/** 更新信息 */
export interface UpdateInfo {
  /** 新版本号 */
  version: string;
  /** 当前版本号 */
  currentVersion: string;
  /** 发布日期 */
  date?: string;
  /** 更新说明 */
  body?: string;
}

/** 下载更新进度事件 */
export type DownloadUpdateEvent =
  | { event: "progress"; data: { downloaded: number; contentLength?: number } }
  | { event: "finished" };

/** 下载更新结果 */
export interface DownloadUpdateResult {
  /** 安装包临时文件路径 */
  installerPath: string;
}

/** 检查更新 */
export async function checkUpdate(): Promise<UpdateInfo | null> {
  const result = await safeInvoke(() => invoke<UpdateInfo | null>("check_update"), { context: "checkUpdate" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 下载更新（保存到临时文件，不安装） */
export async function downloadUpdate(
  onEvent: (event: DownloadUpdateEvent) => void,
): Promise<DownloadUpdateResult> {
  const { Channel } = await import("@tauri-apps/api/core");
  const channel = new Channel<DownloadUpdateEvent>();
  channel.onmessage = onEvent;
  const result = await safeInvoke(
    () => invoke<DownloadUpdateResult>("download_update", { onEvent: channel }),
    { context: "downloadUpdate" },
  );
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 安装已下载的更新 */
export async function installDownloadedUpdate(
  installerPath: string,
  restart: boolean,
): Promise<void> {
  const result = await safeInvoke(
    () => invoke("install_downloaded_update", { installerPath, restart }),
    { context: "installDownloadedUpdate" },
  );
  if (!result.ok) throw result.error.raw;
}

// ================================================================
// 权限规则命令
// ================================================================

/** 列出权限规则（默认规则 + 用户规则） */
export async function listPermissionRules(
  scope?: string,
  workspaceId?: string,
  sessionId?: string,
  permissionType?: string,
  enabledOnly?: boolean,
  includeDefaults?: boolean,
): Promise<PermissionRule[]> {
  const result = await safeInvoke(() => invoke<PermissionRule[]>("list_permission_rules", {
    scope: scope ?? null,
    workspaceId: workspaceId ?? null,
    sessionId: sessionId ?? null,
    permissionType: permissionType ?? null,
    enabledOnly: enabledOnly ?? null,
    includeDefaults: includeDefaults ?? null,
  }), { context: "listPermissionRules" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 添加权限规则 */
export async function addPermissionRule(params: AddPermissionRuleParams): Promise<PermissionRule> {
  const result = await safeInvoke(() => invoke<PermissionRule>("add_permission_rule", {
    scope: params.scope,
    permissionType: params.permissionType,
    pattern: params.pattern,
    action: params.action,
    description: params.description ?? null,
    workspaceId: params.workspaceId ?? null,
    sessionId: params.sessionId ?? null,
  }), { context: "addPermissionRule" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 更新权限规则 */
export async function updatePermissionRule(ruleId: string, params: UpdatePermissionRuleParams): Promise<PermissionRule> {
  const result = await safeInvoke(() => invoke<PermissionRule>("update_permission_rule", {
    ruleId,
    scope: params.scope ?? null,
    permissionType: params.permissionType ?? null,
    pattern: params.pattern ?? null,
    action: params.action ?? null,
    description: params.description ?? null,
    enabled: params.enabled ?? null,
    workspaceId: params.workspaceId ?? null,
    sessionId: params.sessionId ?? null,
  }), { context: "updatePermissionRule" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 删除权限规则 */
export async function deletePermissionRule(ruleId: string): Promise<void> {
  const result = await safeInvoke(() => invoke("delete_permission_rule", { ruleId }), { context: "deletePermissionRule" });
  if (!result.ok) throw result.error.raw;
}

// ================================================================
// LSP 命令
// ================================================================

/** 获取所有 LSP 服务器状态 */
export async function lspGetStatus(): Promise<LspServerInfo[]> {
  const result = await safeInvoke(() => invoke<LspServerInfo[]>("lsp_get_status"), { context: "lspGetStatus" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 重启指定语言的 LSP 服务器 */
export async function lspRestartServer(language: string): Promise<void> {
  // 禁用默认 Toast，由前端 handleRestart 统一处理错误提示，避免双重 Toast
  const result = await safeInvoke(() => invoke("lsp_restart_server", { language }), { context: "lspRestartServer", showToast: false });
  if (!result.ok) throw result.error.raw;
}

/** 停止所有 LSP 服务器 */
export async function lspStopAll(): Promise<void> {
  const result = await safeInvoke(() => invoke("lsp_stop_all"), { context: "lspStopAll" });
  if (!result.ok) throw result.error.raw;
}

/** 初始化 LSP：注册并启动所有启用的语言服务器 */
export async function lspInitialize(): Promise<LspServerInfo[]> {
  const result = await safeInvoke(() => invoke<LspServerInfo[]>("lsp_initialize"), { context: "lspInitialize" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}
