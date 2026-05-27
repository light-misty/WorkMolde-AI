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
  WorkspaceInfo,
  FileNode,
  SearchOptions,
  SearchResult,
  PreviewContent,
  VersionInfo,
  SkillInfo,
  ToolInfo,
  CustomSkillConfig,
  AppSettings,
  PromptTemplate,
  CreateTemplateParams,
  UpdateTemplateParams,
  DailyUsageItem,
  ProviderUsageItem,
  TokenUsageOverview,
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

/** 使用临时配置测试 LLM Provider 连接（用于添加模式，不需要已保存的 provider） */
export async function testConnectionWithConfig(config: ProviderConfig): Promise<ConnectionResult> {
  const result = await safeInvoke(() => invoke<ConnectionResult>("test_connection_with_config", { config }), { context: "testConnectionWithConfig" });
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

/** 设置默认 LLM Provider */
export async function setDefaultProvider(providerId: string): Promise<void> {
  const result = await safeInvoke(() => invoke("set_default_provider", { providerId }), { context: "setDefaultProvider" });
  if (!result.ok) throw result.error.raw;
}

/** 对所有 LLM Provider 执行健康检查 */
export async function healthCheckProviders(): Promise<Record<string, ConnectionResult>> {
  const result = await safeInvoke(() => invoke<Record<string, ConnectionResult>>("health_check_providers"), { context: "healthCheckProviders" });
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
// Skill 命令
// ================================================================

/** 列出所有 Skill */
export async function listSkills(): Promise<SkillInfo[]> {
  const result = await safeInvoke(() => invoke<SkillInfo[]>("list_skills"), { context: "listSkills" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 列出所有自定义 Skill 配置 */
export async function listCustomSkills(): Promise<CustomSkillConfig[]> {
  const result = await safeInvoke(() => invoke<CustomSkillConfig[]>("list_custom_skills"), { context: "listCustomSkills" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 切换 Skill 启用/禁用 */
export async function toggleSkill(skillId: string, enabled: boolean): Promise<void> {
  const result = await safeInvoke(() => invoke("toggle_skill", { skillId, enabled }), { context: "toggleSkill" });
  if (!result.ok) throw result.error.raw;
}

/** 添加自定义 Skill */
export async function addCustomSkill(config: CustomSkillConfig): Promise<CustomSkillConfig> {
  const result = await safeInvoke(() => invoke<CustomSkillConfig>("add_custom_skill", { config }), { context: "addCustomSkill" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 更新自定义 Skill */
export async function updateCustomSkill(config: CustomSkillConfig): Promise<CustomSkillConfig> {
  const result = await safeInvoke(() => invoke<CustomSkillConfig>("update_custom_skill", { config }), { context: "updateCustomSkill" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 删除自定义 Skill */
export async function deleteCustomSkill(skillId: string): Promise<void> {
  const result = await safeInvoke(() => invoke("delete_custom_skill", { skillId }), { context: "deleteCustomSkill" });
  if (!result.ok) throw result.error.raw;
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

/** 导出应用配置 */
export async function exportConfig(includeSecrets?: boolean): Promise<string> {
  const result = await safeInvoke(() => invoke<string>("export_config", { includeSecrets: includeSecrets ?? null }), { context: "exportConfig" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 导入应用配置 */
export async function importConfig(configJson: string, overwrite?: boolean): Promise<void> {
  const result = await safeInvoke(() => invoke("import_config", { configJson, overwrite: overwrite ?? null }), { context: "importConfig" });
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
// Token 统计命令
// ================================================================

/** 获取最近 N 天的 Token 用量趋势 */
export async function getTokenUsageTrend(
  workspaceId?: string,
  days?: number,
): Promise<DailyUsageItem[]> {
  const result = await safeInvoke(() => invoke<DailyUsageItem[]>("get_token_usage_trend", {
    workspaceId: workspaceId ?? null,
    days: days ?? null,
  }), { context: "getTokenUsageTrend" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 按 Provider/Model 分组获取 Token 用量 */
export async function getTokenProviderUsage(
  startDate?: string,
  endDate?: string,
): Promise<ProviderUsageItem[]> {
  const result = await safeInvoke(() => invoke<ProviderUsageItem[]>("get_token_provider_usage", {
    startDate: startDate ?? null,
    endDate: endDate ?? null,
  }), { context: "getTokenProviderUsage" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

/** 获取 Token 用量概览 */
export async function getTokenUsageOverview(
  workspaceId?: string,
): Promise<TokenUsageOverview> {
  const result = await safeInvoke(() => invoke<TokenUsageOverview>("get_token_usage_overview", {
    workspaceId: workspaceId ?? null,
  }), { context: "getTokenUsageOverview" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

// ================================================================
// 日志命令
// ================================================================

/** 获取错误日志文件内容 */
export async function getErrorLog(): Promise<string> {
  const result = await safeInvoke(() => invoke<string>("get_error_log"), { context: "getErrorLog" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}

// ================================================================
// 开发人员工具命令
// ================================================================

/** 切换开发人员工具（DevTools）的显示状态 */
export async function toggleDevtools(): Promise<boolean> {
  const result = await safeInvoke(() => invoke<boolean>("toggle_devtools"), { context: "toggleDevtools" });
  if (!result.ok) throw result.error.raw;
  return result.data;
}
