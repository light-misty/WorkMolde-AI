// ===== 设置相关类型定义 - 与 Rust 后端对齐 =====

export type SettingsTab = "llm" | "workspace" | "skill" | "template" | "usage" | "appearance" | "shortcuts" | "general";

// ----- 应用设置 -----

export type ConfirmationLevel = "always" | "editOnly" | "never";
export type ExceedAction = "warn" | "block" | "fallback";
export type RetentionPolicy = "byCount" | "byDays" | "both";
export type ThemeMode = "light" | "dark" | "system";

export interface GeneralSettings {
  authorName: string;
  confirmationLevel: ConfirmationLevel;
  language: string;
}

export interface AppearanceSettings {
  themeMode: ThemeMode;
  fontScale: number;
}

export interface TokenBudgetSettings {
  dailyLimit: number;
  monthlyLimit: number;
  exceedAction: ExceedAction;
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

export interface AppSettings {
  general: GeneralSettings;
  appearance: AppearanceSettings;
  tokenBudget: TokenBudgetSettings;
  versionSnapshot: VersionSnapshotSettings;
  workspace: WorkspaceDefaults;
  shortcuts: Shortcuts;
  disabledSkills: string[];
}

// ----- LLM 相关类型 -----

export type LLMProviderType = "openai" | "anthropic" | "ollama" | "custom";

export interface ProviderConfig {
  name: string;
  providerType: LLMProviderType;
  apiBase: string;
  apiKey: string;
  model: string;
  extraParams?: Record<string, unknown>;
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

// ----- Skill 相关类型 -----

export interface SkillInfo {
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

export interface CustomSkillConfig {
  id: string;
  name: string;
  description: string;
  category: string;
  /** 提示词模板，支持 {{param_name}} 占位符 */
  promptTemplate: string;
  supportedTypes: string[];
  paramsSchema?: unknown;
  version: string;
  createdAt: string;
  updatedAt: string;
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

// ----- Token 统计相关类型 -----

/** 每日用量统计项 */
export interface DailyUsageItem {
  date: string;
  inputTokens: number;
  outputTokens: number;
}

/** 按 Provider/Model 分组的用量统计项 */
export interface ProviderUsageItem {
  provider: string;
  model: string;
  inputTokens: number;
  outputTokens: number;
}

/** Token 用量概览 */
export interface TokenUsageOverview {
  totalInput: number;
  totalOutput: number;
  todayInput: number;
  todayOutput: number;
  monthInput: number;
  monthOutput: number;
}
