import { create } from "zustand";
import type {
  AppSettings,
  ProviderInfo,
  SkillInfo,
  ToolInfo,
  PromptTemplate,
  CreateTemplateParams,
  UpdateTemplateParams,
  SettingsTab,
  UpdateChannel,
} from "../types";
import type { ProviderSwitchPayload } from "../services/event";
import { onLlmProviderSwitch } from "../services/event";
import * as tauriCmd from "../services/tauri";

// 深层部分类型
type DeepPartial<T> = {
  [P in keyof T]?: T[P] extends object ? DeepPartial<T[P]> : T[P];
};

// 深层合并工具，支持部分更新嵌套对象
function deepMerge<T extends object>(target: T, source: DeepPartial<T>): T {
  const result = { ...target } as Record<string, unknown>;
  for (const key of Object.keys(source as object)) {
    const sourceVal = (source as Record<string, unknown>)[key];
    const targetVal = result[key];
    if (
      sourceVal !== undefined &&
      typeof sourceVal === "object" &&
      sourceVal !== null &&
      !Array.isArray(sourceVal) &&
      typeof targetVal === "object" &&
      targetVal !== null &&
      !Array.isArray(targetVal)
    ) {
      result[key] = deepMerge(
        targetVal as object,
        sourceVal as DeepPartial<object>,
      );
    } else if (sourceVal !== undefined) {
      result[key] = sourceVal;
    }
  }
  return result as T;
}

const defaultSettings: AppSettings = {
  general: {
    authorName: "",
    authorEmail: "",
    authorCompany: "",
    confirmationLevel: "editOnly",
    language: "zh-CN",
  },
  appearance: {
    themeMode: "system",
  },
  versionSnapshot: {
    retentionPolicy: "byCount",
    maxCount: 50,
    maxDays: 30,
  },
  workspace: {
    defaultWorkspaceId: "",
  },
  shortcuts: {
    newSession: "Ctrl+N",
    closeSession: "Ctrl+W",
    sendMessage: "Ctrl+Enter",
    toggleSidebar: "Ctrl+B",
    quickPrompt: "Ctrl+/",
  },
  disabledSkills: [],
  update: {
    channel: "stable" as UpdateChannel,
    autoCheck: true,
  },
};

interface SettingsState {
  settings: AppSettings;
  llmProviders: ProviderInfo[];
  activeProviderId: string | null;
  skills: SkillInfo[];
  tools: ToolInfo[];
  templates: PromptTemplate[];
  isSettingsOpen: boolean;
  activeSettingsTab: SettingsTab;
  /** 最近一次 Provider 切换事件 */
  lastProviderSwitch: ProviderSwitchPayload | null;

  updateSettings: (updates: DeepPartial<AppSettings>) => void;
  openSettings: (tab?: SettingsTab) => void;
  closeSettings: () => void;
  setActiveTab: (tab: SettingsTab) => void;
  loadSettings: () => Promise<void>;
  loadProviders: () => Promise<void>;
  loadSkills: () => Promise<void>;
  loadTools: () => Promise<void>;
  /** 刷新 Skill 列表（loadSkills 的别名，语义更清晰） */
  refreshSkills: () => Promise<void>;
  /** 刷新 Tool 列表（loadTools 的别名，语义更清晰） */
  refreshTools: () => Promise<void>;
  /** 初始化 Provider 切换事件监听 */
  initProviderSwitchListener: () => Promise<() => void>;
  /** 从后端加载模板列表 */
  loadTemplates: () => Promise<void>;
  /** 创建模板 */
  createTemplate: (params: CreateTemplateParams) => Promise<PromptTemplate | null>;
  /** 更新模板 */
  updateTemplate: (id: string, params: UpdateTemplateParams) => Promise<PromptTemplate | null>;
  /** 删除模板 */
  deleteTemplate: (id: string) => Promise<boolean>;
  /** 应用外观设置到 DOM */
  applyAppearance: () => void;
  /** 初始化系统主题偏好监听（跟随系统模式时自动响应变化） */
  initThemeListener: () => () => void;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: defaultSettings,
  llmProviders: [],
  activeProviderId: null,
  skills: [],
  tools: [],
  templates: [],
  isSettingsOpen: false,
  activeSettingsTab: "llm",
  lastProviderSwitch: null,

  // 更新设置（深层合并，支持部分更新嵌套对象），并持久化到后端
  updateSettings: (updates) => {
    set((state) => {
      const merged = deepMerge(state.settings, updates);
      // 异步持久化到后端，不阻塞 UI 更新
      tauriCmd.updateSettings(merged as unknown as Record<string, unknown>).catch((err) => {
        console.error("[SettingsStore] 持久化设置失败:", err);
      });
      return { settings: merged };
    });
    // 如果更新了外观设置，立即应用到 DOM
    if (updates.appearance) {
      // 使用 setTimeout 确保 state 已更新
      setTimeout(() => get().applyAppearance(), 0);
    }
  },

  // 打开设置对话框
  openSettings: (tab) => {
    set({ isSettingsOpen: true, activeSettingsTab: tab || "llm" });
  },

  // 关闭设置对话框
  closeSettings: () => {
    set({ isSettingsOpen: false });
  },

  // 切换设置标签页
  setActiveTab: (tab) => {
    set({ activeSettingsTab: tab });
  },

  // 从后端加载设置、Provider 列表和 Skill 列表
  loadSettings: async () => {
    try {
      const [settings, providers, skills, tools] = await Promise.all([
        tauriCmd.getSettings(),
        tauriCmd.listProviders(),
        tauriCmd.listSkills(),
        tauriCmd.listTools(),
      ]);
      const defaultProvider = providers.find((p) => p.isDefault);
      set({
        settings,
        llmProviders: providers,
        activeProviderId: defaultProvider?.id ?? null,
        skills,
        tools,
      });
      // 设置加载完成后应用外观
      get().applyAppearance();
      // 异步加载模板列表（不阻塞设置加载）
      get().loadTemplates();
    } catch (error) {
      console.error("[SettingsStore] 加载设置失败:", error);
    }
  },

  // 从后端加载 Provider 列表
  loadProviders: async () => {
    try {
      const providers = await tauriCmd.listProviders();
      const defaultProvider = providers.find((p) => p.isDefault);
      set({
        llmProviders: providers,
        activeProviderId: defaultProvider?.id ?? null,
      });
    } catch (error) {
      console.error("[SettingsStore] 加载 Provider 列表失败:", error);
    }
  },

  // 从后端加载 Skill 列表
  loadSkills: async () => {
    try {
      const skills = await tauriCmd.listSkills();
      set({ skills });
    } catch (error) {
      console.error("[SettingsStore] 加载 Skill 列表失败:", error);
    }
  },

  // 刷新 Skill 列表
  refreshSkills: async () => {
    await get().loadSkills();
  },

  // 从后端加载 Tool 列表
  loadTools: async () => {
    try {
      const tools = await tauriCmd.listTools();
      set({ tools });
    } catch (error) {
      console.error("[SettingsStore] 加载 Tool 列表失败:", error);
    }
  },

  // 刷新 Tool 列表
  refreshTools: async () => {
    await get().loadTools();
  },

  // 初始化 Provider 切换事件监听，返回取消监听函数
  initProviderSwitchListener: async () => {
    const unlisten = await onLlmProviderSwitch((payload) => {
      console.info(
        "[SettingsStore] Provider 切换: %s -> %s, 原因: %s, 自动: %s",
        payload.fromProviderId,
        payload.toProviderId,
        payload.reason,
        payload.isAutomatic,
      );
      set({ lastProviderSwitch: payload });
    });
    return unlisten;
  },

  // 从后端加载模板列表
  loadTemplates: async () => {
    try {
      const templates = await tauriCmd.listTemplates();
      set({ templates });
    } catch (error) {
      console.error("[SettingsStore] 加载模板列表失败:", error);
    }
  },

  // 创建模板
  createTemplate: async (params) => {
    try {
      const newTemplate = await tauriCmd.createTemplate(params);
      set((state) => ({
        templates: [newTemplate, ...state.templates],
      }));
      return newTemplate;
    } catch (error) {
      console.error("[SettingsStore] 创建模板失败:", error);
      return null;
    }
  },

  // 更新模板
  updateTemplate: async (id, params) => {
    try {
      const updated = await tauriCmd.updateTemplate(id, params);
      set((state) => ({
        templates: state.templates.map((t) => (t.id === id ? updated : t)),
      }));
      return updated;
    } catch (error) {
      console.error("[SettingsStore] 更新模板失败:", error);
      return null;
    }
  },

  // 删除模板
  deleteTemplate: async (id) => {
    try {
      await tauriCmd.deleteTemplate(id);
      set((state) => ({
        templates: state.templates.filter((t) => t.id !== id),
      }));
      return true;
    } catch (error) {
      console.error("[SettingsStore] 删除模板失败:", error);
      return false;
    }
  },

  // 应用外观设置到 DOM（主题模式）
  applyAppearance: () => {
    const { settings } = get();
    const { themeMode } = settings.appearance;

    // 应用主题
    const root = document.documentElement;
    root.classList.remove("dark", "light");
    if (themeMode === "dark") {
      root.classList.add("dark");
    } else if (themeMode === "light") {
      root.classList.add("light");
    } else {
      // system: 跟随系统偏好
      const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      if (prefersDark) {
        root.classList.add("dark");
      }
    }
  },

  // 初始化系统主题偏好监听
  initThemeListener: () => {
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => {
      const { settings } = get();
      if (settings.appearance.themeMode === "system") {
        const root = document.documentElement;
        root.classList.remove("dark", "light");
        if (mql.matches) {
          root.classList.add("dark");
        }
      }
    };
    mql.addEventListener("change", handler);
    return () => mql.removeEventListener("change", handler);
  },
}));
