// ===== 工作区类型定义 - 与 Rust 后端对齐 =====

/** Git 仓库状态（与 Rust 后端 WorkspaceGitStatus 对齐） */
export interface GitStatus {
  isGitRepo: boolean;
  branchName: string | null;
  changedFileCount: number;
}

export interface WorkspaceInfo {
  id: string;
  name: string;
  path: string;
  isActive: boolean;
  /** 工作区目录是否存在于文件系统中 */
  pathExists: boolean;
  fileCount: number;
  createdAt: string;
  lastAccessed: string;
}

export interface WorkspaceConfig {
  name: string;
  path: string;
}

export interface FileNode {
  name: string;
  path: string;
  isDir: boolean;
  size?: number;
  modified?: string;
  extension?: string;
  children?: FileNode[];
}

export interface SearchOptions {
  extensions?: string[];
  maxResults?: number;
  includeContent?: boolean;
}

export interface SearchResult {
  path: string;
  name: string;
  extension: string;
  size: number;
  modified: string;
  matchType: string;
  matchPreview?: string;
  lineNumber?: number;
}
