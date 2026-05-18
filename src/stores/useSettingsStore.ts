import { create } from "zustand";
import type {
  AppSettings,
  ProviderInfo,
  SkillInfo,
  PromptTemplate,
  SettingsTab,
} from "../types";
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
    confirmationLevel: "editOnly",
    language: "zh-CN",
  },
  tokenBudget: {
    dailyLimit: 0,
    monthlyLimit: 0,
    exceedAction: "warn",
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
};

interface SettingsState {
  settings: AppSettings;
  llmProviders: ProviderInfo[];
  activeProviderId: string | null;
  skills: SkillInfo[];
  templates: PromptTemplate[];
  isSettingsOpen: boolean;
  activeSettingsTab: SettingsTab;

  updateSettings: (updates: DeepPartial<AppSettings>) => void;
  openSettings: (tab?: SettingsTab) => void;
  closeSettings: () => void;
  setActiveTab: (tab: SettingsTab) => void;
  toggleSkill: (id: string) => Promise<void>;
  loadSettings: () => Promise<void>;
  loadProviders: () => Promise<void>;
  loadSkills: () => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: defaultSettings,
  llmProviders: [],
  activeProviderId: null,
  skills: [],
  templates: [],
  isSettingsOpen: false,
  activeSettingsTab: "llm",

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

  // 切换 Skill 启用/禁用，调用后端 API
  toggleSkill: async (id) => {
    const skill = get().skills.find((s) => s.id === id);
    if (!skill) return;
    try {
      await tauriCmd.toggleSkill(id, !skill.enabled);
      set((state) => ({
        skills: state.skills.map((s) =>
          s.id === id ? { ...s, enabled: !s.enabled } : s
        ),
      }));
    } catch (error) {
      console.error("[SettingsStore] 切换 Skill 失败:", error);
    }
  },

  // 从后端加载设置、Provider 列表和 Skill 列表
  loadSettings: async () => {
    try {
      const [settings, providers, skills] = await Promise.all([
        tauriCmd.getSettings(),
        tauriCmd.listProviders(),
        tauriCmd.listSkills(),
      ]);
      const defaultProvider = providers.find((p) => p.isDefault);
      set({
        settings,
        llmProviders: providers,
        activeProviderId: defaultProvider?.id ?? null,
        skills,
      });
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
}));
