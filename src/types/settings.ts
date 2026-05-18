// ===== 设置相关类型定义 - 与 Rust 后端对齐 =====

export type SettingsTab = "llm" | "workspace" | "skill" | "template" | "general";

// ----- 应用设置 -----

export type ConfirmationLevel = "always" | "editOnly" | "never";
export type ExceedAction = "warn" | "block" | "fallback";
export type RetentionPolicy = "byCount" | "byDays" | "both";

export interface GeneralSettings {
  authorName: string;
  confirmationLevel: ConfirmationLevel;
  language: string;
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
  name: string;
  description: string;
  category: string;
  promptTemplate: string;
  supportedTypes: string[];
  paramsSchema?: unknown;
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
  variables?: TemplateVariable[];
  createdAt: string;
  updatedAt: string;
}
