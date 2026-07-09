// 权限规则类型 - 与 Rust 后端 models::permission::PermissionRule 对齐
// 注意: PermissionType 使用 snake_case 以匹配 Rust serde 序列化格式

export type PermissionScope = 'global' | 'project' | 'session';
export type PermissionAction = 'allow' | 'deny' | 'ask';
export type PermissionType =
  | 'wildcard' | 'read' | 'edit' | 'glob' | 'grep' | 'list'
  | 'bash' | 'write_script' | 'task' | 'skill' | 'lsp'
  | 'web_fetch' | 'web_search' | 'external_directory' | 'doom_loop'
  | 'document' | 'question';

export interface PermissionRule {
  id: string;
  scope: PermissionScope;
  workspaceId?: string;
  sessionId?: string;
  permissionType: PermissionType;
  pattern: string;
  action: PermissionAction;
  description: string;
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
}

/** 添加权限规则参数 */
export interface AddPermissionRuleParams {
  scope: PermissionScope;
  permissionType: PermissionType;
  pattern: string;
  action: PermissionAction;
  description?: string;
  workspaceId?: string;
  sessionId?: string;
}

/** 更新权限规则参数 */
export interface UpdatePermissionRuleParams {
  scope?: PermissionScope;
  permissionType?: PermissionType;
  pattern?: string;
  action?: PermissionAction;
  description?: string;
  enabled?: boolean;
  workspaceId?: string;
  sessionId?: string;
}
