// ===== 设置相关类型定义 - 与 Rust 后端对齐 =====

export type SettingsTab = "llm" | "workspace" | "handler" | "template" | "appearance" | "shortcuts" | "general" | "help";

// ----- 应用设置 -----

export type ConfirmationLevel = "always" | "editOnly" | "never";
export type RetentionPolicy = "byCount" | "byDays" | "both";
export type ThemeMode = "light" | "dark" | "system";

export interface GeneralSettings {
  authorName: string;
  /** 作者邮箱 */
  authorEmail: string;
  /** 作者公司/组织 */
  authorCompany: string;
  confirmationLevel: ConfirmationLevel;
}

export interface AppearanceSettings {
  themeMode: ThemeMode;
  /** 界面语言 */
  language: string;
  /** 是否跟随系统语言 */
  languageFollowSystem: boolean;
}

export interface VersionSnapshotSettings {
  retentionPolicy: RetentionPolicy;
  maxCount: number;
  maxDays: number;
}

export interface WorkspaceDefaults {
  defaultWorkspaceId: string;
}

export interface Shortcuts {
  newSession: string;
  closeSession: string;
  sendMessage: string;
  toggleSidebar: string;
  quickPrompt: string;
}

export interface UpdateSettings {
  autoCheck: boolean;
}

export interface AppSettings {
  general: GeneralSettings;
  appearance: AppearanceSettings;
  versionSnapshot: VersionSnapshotSettings;
  workspace: WorkspaceDefaults;
  shortcuts: Shortcuts;
  update: UpdateSettings;
}

// ----- LLM 相关类型 -----

export type LLMProviderType = "openai" | "anthropic" | "ollama" | "gemini" | "custom";

export interface ProviderConfig {
  name: string;
  providerType: LLMProviderType;
  apiBase: string;
  apiKey: string;
  model: string;
  extraParams?: Record<string, unknown>;
  /** 上下文窗口大小 (tokens)，undefined 表示自动推断 */
  contextWindow?: number;
  /** 是否支持视觉/图片多模态 */
  supportsVision: boolean;
}

export interface ProviderInfo {
  id: string;
  name: string;
  providerType: LLMProviderType;
  apiBase: string;
  model: string;
  isDefault: boolean;
  isAvailable: boolean;
  isConnected?: boolean;
  createdAt: string;
  /** 上下文窗口大小 (tokens)，运行时计算后的最终值 */
  contextWindow: number;
  /** 是否支持视觉/图片多模态 */
  supportsVision: boolean;
}

export interface ConnectionResult {
  success: boolean;
  providerId?: string;
  latencyMs: number;
  modelInfo?: ModelInfo;
  model?: string;
  errorMessage?: string;
  error?: string;
}

export interface ModelInfo {
  modelName: string;
  maxTokens: number;
  supportsStreaming: boolean;
  supportsToolCall: boolean;
}

// ----- Handler 相关类型 -----

export interface HandlerInfo {
  id: string;
  name: string;
  description: string;
  category: string;
  isBuiltin: boolean;
  enabled: boolean;
  version: string;
  paramsSchema?: unknown;
  supportedTypes: string[];
}

// ----- Tool 相关类型 -----

export interface ToolInfo {
  id: string;
  name: string;
  description: string;
  category: string;
  isBuiltin: boolean;
  enabled: boolean;
  version: string;
  paramsSchema?: unknown;
}

// ----- 上下文窗口相关类型 -----

/** 上下文窗口使用信息，与 Rust ContextUsageInfo 对齐 */
export interface ContextUsageInfo {
  /** 上下文窗口总大小 (tokens) */
  contextWindow: number;
  /** 系统提示词估算 Token 数 */
  systemPromptTokens: number;
  /** 工具定义估算 Token 数（包含 Tool + Handler 两部分） */
  functionDefinitionsTokens: number;
  /** 对话历史估算 Token 数 */
  conversationTokens: number;
  /** LLM 响应估算 Token 数（当前轮，迭代完成后估算） */
  responseTokens: number;
  /** 已使用 Token 总数 */
  totalUsedTokens: number;
  /** 压缩状态: "normal" | "compressed" | "critical" */
  compressionStatus: string;
  /** 当前活跃 Provider 的模型名称 */
  modelName: string;
  /** 对话历史消息总数（压缩前） */
  totalMessageCount: number;
  /** 压缩后保留的消息数 */
  retainedMessageCount: number;
}

// ----- 模板相关类型 -----

export interface TemplateVariable {
  name: string;
  type: "string" | "number" | "boolean" | "select";
  label: string;
  defaultValue?: unknown;
  options?: string[];
}

export interface PromptTemplate {
  id: string;
  name: string;
  description: string;
  content: string;
  category: string;
  isBuiltin: boolean;
  variables?: TemplateVariable[];
  createdAt: string;
  updatedAt: string;
}

/** 创建模板参数 */
export interface CreateTemplateParams {
  name: string;
  description: string;
  content: string;
  category: string;
  variables?: TemplateVariable[];
}

/** 更新模板参数 */
export interface UpdateTemplateParams {
  name?: string;
  description?: string;
  content?: string;
  category?: string;
  variables?: TemplateVariable[];
}
