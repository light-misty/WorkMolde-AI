/**
 * Tauri 命令调用封装
 * 为每个后端 Tauri 命令提供对应的 TypeScript 异步函数
 * 函数名使用 camelCase，调用 invoke 时使用 snake_case 命令名
 * 类型定义统一从 types/ 目录导入，确保前后端类型一致
 */
import { invoke } from "@tauri-apps/api/core";

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
  CustomSkillConfig,
  AppSettings,
} from "../types";

/** 命令错误 */
export interface CommandError {
  code: number;
  message: string;
}

// ================================================================
// LLM 命令
// ================================================================

/** 测试 LLM Provider 连接 */
export async function testConnection(providerId: string): Promise<ConnectionResult> {
  try {
    return await invoke<ConnectionResult>("test_connection", { providerId });
  } catch (error) {
    console.error("[tauri] testConnection 失败:", error);
    throw error;
  }
}

/** 列出所有 LLM Provider */
export async function listProviders(): Promise<ProviderInfo[]> {
  try {
    return await invoke<ProviderInfo[]>("list_providers");
  } catch (error) {
    console.error("[tauri] listProviders 失败:", error);
    throw error;
  }
}

/** 添加 LLM Provider */
export async function addProvider(config: ProviderConfig): Promise<void> {
  try {
    await invoke("add_provider", { config });
  } catch (error) {
    console.error("[tauri] addProvider 失败:", error);
    throw error;
  }
}

/** 更新 LLM Provider */
export async function updateProvider(providerId: string, config: ProviderConfig): Promise<void> {
  try {
    await invoke("update_provider", { providerId, config });
  } catch (error) {
    console.error("[tauri] updateProvider 失败:", error);
    throw error;
  }
}

/** 删除 LLM Provider */
export async function deleteProvider(providerId: string): Promise<void> {
  try {
    await invoke("delete_provider", { providerId });
  } catch (error) {
    console.error("[tauri] deleteProvider 失败:", error);
    throw error;
  }
}

/** 设置默认 LLM Provider */
export async function setDefaultProvider(providerId: string): Promise<void> {
  try {
    await invoke("set_default_provider", { providerId });
  } catch (error) {
    console.error("[tauri] setDefaultProvider 失败:", error);
    throw error;
  }
}

/** 对所有 LLM Provider 执行健康检查 */
export async function healthCheckProviders(): Promise<Record<string, ConnectionResult>> {
  try {
    const result = await invoke<Record<string, ConnectionResult>>("health_check_providers");
    return result;
  } catch (error) {
    console.error("[tauri] healthCheckProviders 失败:", error);
    throw error;
  }
}

// ================================================================
// 会话命令
// ================================================================

/** 创建新会话 */
export async function createSession(params: CreateSessionParams): Promise<Session> {
  try {
    return await invoke<Session>("create_session", { params });
  } catch (error) {
    console.error("[tauri] createSession 失败:", error);
    throw error;
  }
}

/** 列出会话 */
export async function listSessions(filter?: SessionFilter): Promise<SessionSummary[]> {
  try {
    return await invoke<SessionSummary[]>("list_sessions", { filter: filter ?? null });
  } catch (error) {
    console.error("[tauri] listSessions 失败:", error);
    throw error;
  }
}

/** 获取会话详情 */
export async function getSession(sessionId: string): Promise<SessionDetail> {
  try {
    return await invoke<SessionDetail>("get_session", { sessionId });
  } catch (error) {
    console.error("[tauri] getSession 失败:", error);
    throw error;
  }
}

/** 删除会话 */
export async function deleteSession(sessionId: string): Promise<void> {
  try {
    await invoke("delete_session", { sessionId });
  } catch (error) {
    console.error("[tauri] deleteSession 失败:", error);
    throw error;
  }
}

/** 更新会话标题 */
export async function updateSessionTitle(sessionId: string, title: string): Promise<void> {
  try {
    await invoke("update_session_title", { sessionId, title });
  } catch (error) {
    console.error("[tauri] updateSessionTitle 失败:", error);
    throw error;
  }
}

// ================================================================
// 工作区命令
// ================================================================

/** 列出所有工作区 */
export async function listWorkspaces(): Promise<WorkspaceInfo[]> {
  try {
    return await invoke<WorkspaceInfo[]>("list_workspaces");
  } catch (error) {
    console.error("[tauri] listWorkspaces 失败:", error);
    throw error;
  }
}

/** 添加工作区 */
export async function addWorkspace(path: string, name?: string): Promise<WorkspaceInfo> {
  try {
    return await invoke<WorkspaceInfo>("add_workspace", { path, name: name ?? null });
  } catch (error) {
    console.error("[tauri] addWorkspace 失败:", error);
    throw error;
  }
}

/** 移除工作区 */
export async function removeWorkspace(workspaceId: string): Promise<void> {
  try {
    await invoke("remove_workspace", { workspaceId });
  } catch (error) {
    console.error("[tauri] removeWorkspace 失败:", error);
    throw error;
  }
}

/** 设置活动工作区 */
export async function setActiveWorkspace(workspaceId: string): Promise<void> {
  try {
    await invoke("set_active_workspace", { workspaceId });
  } catch (error) {
    console.error("[tauri] setActiveWorkspace 失败:", error);
    throw error;
  }
}

/** 获取文件树 */
export async function getFileTree(
  workspaceId: string,
  path?: string,
  depth?: number,
): Promise<FileNode[]> {
  try {
    return await invoke<FileNode[]>("get_file_tree", {
      workspaceId,
      path: path ?? null,
      depth: depth ?? null,
    });
  } catch (error) {
    console.error("[tauri] getFileTree 失败:", error);
    throw error;
  }
}

/** 搜索文件 */
export async function searchFiles(
  workspaceId: string,
  query: string,
  options?: SearchOptions,
): Promise<SearchResult[]> {
  try {
    return await invoke<SearchResult[]>("search_files", {
      workspaceId,
      query,
      options: options ?? null,
    });
  } catch (error) {
    console.error("[tauri] searchFiles 失败:", error);
    throw error;
  }
}

// ================================================================
// 文档命令
// ================================================================

/** 预览文档 */
export async function previewDocument(
  workspaceId: string,
  path: string,
): Promise<PreviewContent> {
  try {
    return await invoke<PreviewContent>("preview_document", { workspaceId, path });
  } catch (error) {
    console.error("[tauri] previewDocument 失败:", error);
    throw error;
  }
}

/** 获取文档版本历史 */
export async function getDocumentVersions(
  workspaceId: string,
  path: string,
): Promise<VersionInfo[]> {
  try {
    return await invoke<VersionInfo[]>("get_document_versions", { workspaceId, path });
  } catch (error) {
    console.error("[tauri] getDocumentVersions 失败:", error);
    throw error;
  }
}

/** 回滚到指定版本 */
export async function rollbackVersion(
  workspaceId: string,
  path: string,
  versionId: string,
): Promise<void> {
  try {
    await invoke("rollback_version", { workspaceId, path, versionId });
  } catch (error) {
    console.error("[tauri] rollbackVersion 失败:", error);
    throw error;
  }
}

/** 创建空文件 */
export async function createFile(
  workspaceId: string,
  path: string,
): Promise<void> {
  try {
    await invoke("create_file", { workspaceId, path });
  } catch (error) {
    console.error("[tauri] createFile 失败:", error);
    throw error;
  }
}

/** 创建目录 */
export async function createDirectory(
  workspaceId: string,
  path: string,
): Promise<void> {
  try {
    await invoke("create_directory", { workspaceId, path });
  } catch (error) {
    console.error("[tauri] createDirectory 失败:", error);
    throw error;
  }
}

/** 重命名文件或目录 */
export async function renameFile(
  workspaceId: string,
  oldPath: string,
  newPath: string,
): Promise<void> {
  try {
    await invoke("rename_file", { workspaceId, oldPath, newPath });
  } catch (error) {
    console.error("[tauri] renameFile 失败:", error);
    throw error;
  }
}

/** 删除文件或目录（永久删除） */
export async function deleteFile(
  workspaceId: string,
  path: string,
): Promise<void> {
  try {
    await invoke("delete_file", { workspaceId, path });
  } catch (error) {
    console.error("[tauri] deleteFile 失败:", error);
    throw error;
  }
}

/** 在系统文件管理器中显示 */
export async function showInFileManager(
  workspaceId: string,
  path: string,
): Promise<void> {
  try {
    await invoke("show_in_file_manager", { workspaceId, path });
  } catch (error) {
    console.error("[tauri] showInFileManager 失败:", error);
    throw error;
  }
}

// ================================================================
// Skill 命令
// ================================================================

/** 列出所有 Skill */
export async function listSkills(): Promise<SkillInfo[]> {
  try {
    return await invoke<SkillInfo[]>("list_skills");
  } catch (error) {
    console.error("[tauri] listSkills 失败:", error);
    throw error;
  }
}

/** 切换 Skill 启用/禁用 */
export async function toggleSkill(skillId: string, enabled: boolean): Promise<void> {
  try {
    await invoke("toggle_skill", { skillId, enabled });
  } catch (error) {
    console.error("[tauri] toggleSkill 失败:", error);
    throw error;
  }
}

/** 添加自定义 Skill */
export async function addCustomSkill(config: CustomSkillConfig): Promise<void> {
  try {
    await invoke("add_custom_skill", { config });
  } catch (error) {
    console.error("[tauri] addCustomSkill 失败:", error);
    throw error;
  }
}

/** 删除自定义 Skill */
export async function deleteCustomSkill(skillId: string): Promise<void> {
  try {
    await invoke("delete_custom_skill", { skillId });
  } catch (error) {
    console.error("[tauri] deleteCustomSkill 失败:", error);
    throw error;
  }
}

// ================================================================
// 设置命令
// ================================================================

/** 获取应用设置 */
export async function getSettings(): Promise<AppSettings> {
  try {
    return await invoke<AppSettings>("get_settings");
  } catch (error) {
    console.error("[tauri] getSettings 失败:", error);
    throw error;
  }
}

/** 更新应用设置 */
export async function updateSettings(settings: Record<string, unknown>): Promise<void> {
  try {
    await invoke("update_settings", { settings });
  } catch (error) {
    console.error("[tauri] updateSettings 失败:", error);
    throw error;
  }
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
  try {
    await invoke("start_agent", { sessionId, prompt, options: options ?? null });
  } catch (error) {
    console.error("[tauri] startAgent 失败:", error);
    throw error;
  }
}

/** 停止 Agent */
export async function stopAgent(sessionId: string): Promise<void> {
  try {
    await invoke("stop_agent", { sessionId });
  } catch (error) {
    console.error("[tauri] stopAgent 失败:", error);
    throw error;
  }
}

/** 确认 Agent 操作 */
export async function confirmOperation(
  sessionId: string,
  operationId: string,
  approved: boolean,
  feedback?: string,
): Promise<void> {
  try {
    await invoke("confirm_operation", {
      sessionId,
      operationId,
      approved,
      feedback: feedback ?? null,
    });
  } catch (error) {
    console.error("[tauri] confirmOperation 失败:", error);
    throw error;
  }
}
