import { create } from 'zustand'

/** 更新 store 的状态接口 */
interface UpdateState {
  /** 待安装的更新安装包路径（下载后保存的临时文件路径） */
  pendingUpdatePath: string | null
  /** 设置待安装的更新路径 */
  setPendingUpdatePath: (path: string | null) => void
  /** 清除待安装的更新路径 */
  clearPendingUpdatePath: () => void
  /** 更新通知弹窗是否可见（控制 UpdateNotification 组件的开关） */
  updateNotificationOpen: boolean
  /** 设置更新通知弹窗的可见状态 */
  setUpdateNotificationOpen: (open: boolean) => void
}

/** 应用更新全局状态 store，管理更新流程中的状态 */
export const useUpdateStore = create<UpdateState>((set) => ({
  pendingUpdatePath: null,

  // 设置待安装的更新路径
  setPendingUpdatePath: (path: string | null) => {
    set({ pendingUpdatePath: path })
  },

  // 清除待安装的更新路径
  clearPendingUpdatePath: () => {
    set({ pendingUpdatePath: null })
  },

  // 更新通知弹窗可见状态
  updateNotificationOpen: false,
  setUpdateNotificationOpen: (open: boolean) => {
    set({ updateNotificationOpen: open })
  },
}))
