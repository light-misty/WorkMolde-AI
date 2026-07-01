import { create } from "zustand";
import type { WorkspaceInfo } from "../types";
import * as tauriCmd from "../services/tauri";
import { useSessionStore } from "./useSessionStore";

interface WorkspaceState {
  currentWorkspaceId: string | null;
  workspaces: WorkspaceInfo[];
  isLoading: boolean;

  addWorkspace: (path: string, name?: string) => Promise<string>;
  switchWorkspace: (id: string) => Promise<void>;
  removeWorkspace: (id: string) => Promise<void>;
  /** 处理工作区目录被外部删除：从 store 中移除并调用后端清理配置 */
  handleWorkspaceDirectoryDeleted: (workspaceId: string) => Promise<void>;
  loadWorkspaces: () => Promise<void>;
}

export const useWorkspaceStore = create<WorkspaceState>((set, get) => ({
  currentWorkspaceId: null,
  workspaces: [],
  isLoading: false,

  // 添加工作区，调用后端 API
  addWorkspace: async (path, name) => {
    try {
      const workspace = await tauriCmd.addWorkspace(path, name);
      const newCurrentId = get().currentWorkspaceId || workspace.id;
      set((state) => ({
        workspaces: [...state.workspaces, workspace],
        currentWorkspaceId: newCurrentId,
      }));
      // 同步后端活动工作区状态，确保文件监听启动
      if (newCurrentId) {
        await tauriCmd.setActiveWorkspace(newCurrentId).catch((err) => {
          console.warn("[WorkspaceStore] 同步活动工作区失败:", err);
        });
      }
      return workspace.id;
    } catch (error) {
      console.error("[WorkspaceStore] 添加工作区失败:", error);
      throw error;
    }
  },

  // 切换工作区，调用后端 API
  switchWorkspace: async (id) => {
    try {
      await tauriCmd.setActiveWorkspace(id);
      set({ currentWorkspaceId: id });
    } catch (error) {
      console.error("[WorkspaceStore] 切换工作区失败:", error);
    }
  },

  // 移除工作区，调用后端 API
  // 后端会同时删除该工作区下的所有会话，返回被删除的会话 ID 列表供调用方清理本地状态
  removeWorkspace: async (id) => {
    try {
      await tauriCmd.removeWorkspace(id);
      let newCurrentId: string | null = null;
      set((state) => {
        // 先过滤得到剩余列表，再从剩余列表中取回退值，避免回退到已删除的工作区
        const remaining = state.workspaces.filter((w) => w.id !== id);
        newCurrentId =
          state.currentWorkspaceId === id
            ? remaining[0]?.id ?? null
            : state.currentWorkspaceId;
        return {
          workspaces: remaining,
          currentWorkspaceId: newCurrentId,
        };
      });
      // 同步后端活动工作区状态，确保文件监听切换
      if (newCurrentId) {
        await tauriCmd.setActiveWorkspace(newCurrentId).catch((err) => {
          console.warn("[WorkspaceStore] 同步活动工作区失败:", err);
        });
      }
      // 后端已删除该工作区下的所有关联会话，重新加载会话列表使本地状态与数据库一致
      // 避免出现孤儿会话被前端兜底逻辑错误归入其他工作区
      await useSessionStore.getState().loadSessions();
    } catch (error) {
      console.error("[WorkspaceStore] 移除工作区失败:", error);
      // 重新抛出，让调用方感知失败并提示用户（避免静默失败导致 UI 误以为成功）
      throw error;
    }
  },

  // 处理工作区目录被外部删除：从 store 中移除并调用后端清理配置
  // 后端 remove_workspace 会同时清理该工作区下的所有会话，前端通过会话删除事件感知
  handleWorkspaceDirectoryDeleted: async (workspaceId) => {
    console.warn("[WorkspaceStore] 工作区目录已被外部删除, id=", workspaceId);
    let newCurrentId: string | null = null;
    set((state) => {
      // 从列表中移除该工作区
      const remaining = state.workspaces.filter((w) => w.id !== workspaceId);
      // 如果被删除的是当前活动工作区，自动切换到第一个可用工作区
      newCurrentId =
        state.currentWorkspaceId === workspaceId
          ? remaining[0]?.id ?? null
          : state.currentWorkspaceId;
      return {
        workspaces: remaining,
        currentWorkspaceId: newCurrentId,
      };
    });

    // 调用后端移除工作区配置（清理 workspaces.json 中的条目，同时清理关联会话）
    try {
      await tauriCmd.removeWorkspace(workspaceId);
    } catch (err) {
      console.warn("[WorkspaceStore] 后端移除工作区配置失败:", err);
    }

    // 同步后端活动工作区状态，确保文件监听切换到新的工作区
    if (newCurrentId) {
      await tauriCmd.setActiveWorkspace(newCurrentId).catch((err) => {
        console.warn("[WorkspaceStore] 同步活动工作区失败:", err);
      });
    }
    // 后端已删除该工作区下的所有关联会话，重新加载会话列表使本地状态与数据库一致
    await useSessionStore.getState().loadSessions();
  },

  // 从后端加载工作区列表
  loadWorkspaces: async () => {
    set({ isLoading: true });
    try {
      const workspaces = await tauriCmd.listWorkspaces();

      // 自动清理目录已不存在的工作区
      const deletedWorkspaces = workspaces.filter((w) => !w.pathExists);
      const validWorkspaces = workspaces.filter((w) => w.pathExists);

      // 对目录已不存在的工作区，调用后端移除配置（后端会同时清理关联会话）
      let hasDeletedWorkspace = false;
      for (const ws of deletedWorkspaces) {
        try {
          await tauriCmd.removeWorkspace(ws.id);
          hasDeletedWorkspace = true;
          console.warn("[WorkspaceStore] 已自动清理目录不存在的工作区:", ws.name, ws.path);
        } catch (err) {
          console.warn("[WorkspaceStore] 清理工作区配置失败:", err);
        }
      }

      const activeWorkspace = validWorkspaces.find((w) => w.isActive);
      const currentId = activeWorkspace?.id ?? validWorkspaces[0]?.id ?? null;
      set({
        workspaces: validWorkspaces,
        currentWorkspaceId: currentId,
        isLoading: false,
      });
      // 同步后端活动工作区状态，确保文件监听启动
      if (currentId) {
        await tauriCmd.setActiveWorkspace(currentId).catch((err) => {
          console.warn("[WorkspaceStore] 同步活动工作区失败:", err);
        });
      }
      // 如果清理了目录不存在的工作区，后端已删除其关联会话，需重新加载会话列表
      if (hasDeletedWorkspace) {
        await useSessionStore.getState().loadSessions();
      }
    } catch (error) {
      console.error("[WorkspaceStore] 加载工作区列表失败:", error);
      set({ isLoading: false });
    }
  },
}));
