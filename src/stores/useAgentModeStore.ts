import { create } from 'zustand'

/**
 * Agent 执行模式类型
 * plan: 只读规划模式，禁止修改类操作
 * build: 完整执行模式，允许所有编程操作（受权限规则约束）
 * document: Build 超集 + 4 个文档 Handler 动态加入工具列表
 */
export type AgentMode = 'plan' | 'build' | 'document'

interface AgentModeStore {
  /** 当前 Agent 模式（默认 build） */
  mode: AgentMode
  /** 设置当前模式 */
  setMode: (mode: AgentMode) => void
  /** 切换到 Plan 模式 */
  switchToPlan: () => void
  /** 切换到 Build 模式 */
  switchToBuild: () => void
  /** 切换到 Document 模式 */
  switchToDocument: () => void
}

/**
 * Agent 模式状态管理
 * 模式切换仅由前端按钮触发，不提供 LLM 工具切换模式
 */
export const useAgentModeStore = create<AgentModeStore>((set) => ({
  mode: 'build',
  setMode: (mode) => set({ mode }),
  switchToPlan: () => set({ mode: 'plan' }),
  switchToBuild: () => set({ mode: 'build' }),
  switchToDocument: () => set({ mode: 'document' }),
}))
