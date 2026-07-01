import { create } from "zustand";

/** Toast 类型 */
export type ToastType = "error" | "success" | "warning" | "info";

/** 单条 Toast 数据 */
export interface ToastItem {
  id: string;
  type: ToastType;
  message: string;
  timestamp: number;
}

/** 最大同时显示条数 */
const MAX_TOASTS = 5;

/** 自增 ID 计数器 */
let toastCounter = 0;

interface ToastState {
  toasts: ToastItem[];
  addToast: (type: ToastType, message: string) => string;
  removeToast: (id: string) => void;
}

export const useToastStore = create<ToastState>((set) => ({
  toasts: [],

  // 添加 Toast，超过上限时移除最早的
  addToast: (type, message) => {
    const id = `toast-${++toastCounter}-${Date.now()}`;
    set((state) => {
      const next = [...state.toasts, { id, type, message, timestamp: Date.now() }];
      // 超过最大条数时，移除最早的
      if (next.length > MAX_TOASTS) {
        return { toasts: next.slice(next.length - MAX_TOASTS) };
      }
      return { toasts: next };
    });
    return id;
  },

  // 移除指定 Toast
  removeToast: (id) => {
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    }));
  },
}));
