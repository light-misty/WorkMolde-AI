import { create } from "zustand";
import i18n from "../i18n";
import type { SessionSummary } from "../types";
import * as tauriCmd from "../services/tauri";

interface SessionState {
  currentSessionId: string | null;
  sessions: SessionSummary[];
  isLoading: boolean;

  createSession: (title?: string, workspaceId?: string) => Promise<string>;
  switchSession: (sessionId: string) => void;
  clearCurrentSession: () => void;
  deleteSession: (sessionId: string) => Promise<void>;
  updateSessionTitle: (sessionId: string, title: string) => Promise<void>;
  updateSessionTitleLocal: (sessionId: string, title: string) => void;
  loadSessions: (workspaceId?: string) => Promise<void>;
  clearAllSessions: () => Promise<number>;
  clearWorkspaceSessions: (workspaceId: string) => Promise<number>;
}

export const useSessionStore = create<SessionState>((set) => ({
  currentSessionId: null,
  sessions: [],
  isLoading: false,

  // 创建新会话，调用后端 API
  createSession: async (title, workspaceId) => {
    try {
      const session = await tauriCmd.createSession({
        title: title || `${i18n.t('session.newSession')} ${new Date().toLocaleTimeString()}`,
        workspaceId,
      });
      set((state) => ({
        sessions: [
          {
            id: session.id,
            title: session.title,
            // 从后端返回值获取 workspaceId，确保本地状态与数据库一致
            workspaceId: session.workspaceId,
            status: session.status,
            messageCount: 0,
            createdAt: session.createdAt,
            updatedAt: session.updatedAt,
          },
          ...state.sessions,
        ],
        currentSessionId: session.id,
      }));
      return session.id;
    } catch (error) {
      console.error("[SessionStore] 创建会话失败:", error);
      throw error;
    }
  },

  // 切换当前会话
  switchSession: (sessionId) => {
    set({ currentSessionId: sessionId });
  },

  // 清除当前选中会话（新建会话但未执行智能体时使用）
  clearCurrentSession: () => {
    set({ currentSessionId: null });
  },

  // 删除会话，调用后端 API
  // 注意：不自动切换 currentSessionId，由调用方（App.tsx handleDeleteCurrentSession）统一管理
  deleteSession: async (sessionId) => {
    try {
      await tauriCmd.deleteSession(sessionId);
      set((state) => ({
        sessions: state.sessions.filter((s) => s.id !== sessionId),
        currentSessionId:
          state.currentSessionId === sessionId ? null : state.currentSessionId,
      }));
    } catch (error) {
      console.error("[SessionStore] 删除会话失败:", error);
      throw error;
    }
  },

  // 更新会话标题，调用后端 API
  updateSessionTitle: async (sessionId, title) => {
    try {
      await tauriCmd.updateSessionTitle(sessionId, title);
      set((state) => ({
        sessions: state.sessions.map((s) =>
          s.id === sessionId ? { ...s, title } : s
        ),
      }));
    } catch (error) {
      console.error("[SessionStore] 更新会话标题失败:", error);
      throw error;
    }
  },

  // 仅更新本地状态中的会话标题（不调用后端API，用于接收后端事件通知）
  updateSessionTitleLocal: (sessionId, title) => {
    set((state) => ({
      sessions: state.sessions.map((s) =>
        s.id === sessionId ? { ...s, title } : s
      ),
    }));
  },

  // 从后端加载会话列表
  loadSessions: async (workspaceId) => {
    set({ isLoading: true });
    try {
      const sessions = await tauriCmd.listSessions(
        workspaceId ? { workspaceId } : undefined
      );
      set({ sessions, isLoading: false });
    } catch (error) {
      console.error("[SessionStore] 加载会话列表失败:", error);
      set({ isLoading: false });
    }
  },

  // 清除所有会话数据，调用后端 API
  clearAllSessions: async () => {
    try {
      const count = await tauriCmd.clearAllSessions();
      set({ sessions: [], currentSessionId: null });
      return count;
    } catch (error) {
      console.error("[SessionStore] 清除所有会话失败:", error);
      throw error;
    }
  },

  // 清除指定工作区下的所有会话
  clearWorkspaceSessions: async (workspaceId) => {
    try {
      const count = await tauriCmd.clearWorkspaceSessions(workspaceId);
      set((state) => ({
        sessions: state.sessions.filter((s) => s.workspaceId !== workspaceId),
        // 不再置 currentSessionId 为 null，由 App.tsx 的会话失效检测 useEffect 统一处理 UI 清理
      }));
      return count;
    } catch (error) {
      console.error("[SessionStore] 清除工作区会话失败:", error);
      throw error;
    }
  },
}));
