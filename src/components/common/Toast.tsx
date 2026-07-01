import { useEffect, useCallback, type ReactElement } from "react";
import { useToastStore, type ToastType } from "../../stores/useToastStore";

/** 自动消失时间（毫秒） */
const AUTO_DISMISS_MS = 3000;

/** 类型对应的图标 SVG */
const typeIcons: Record<ToastType, ReactElement> = {
  error: (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
      <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" />
      <path d="M5.5 5.5L10.5 10.5M10.5 5.5L5.5 10.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  ),
  success: (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
      <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" />
      <path d="M5 8.5L7 10.5L11 6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  ),
  warning: (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
      <path d="M8 2L14.5 13H1.5L8 2Z" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
      <path d="M8 6.5V9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      <circle cx="8" cy="11" r="0.75" fill="currentColor" />
    </svg>
  ),
  info: (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
      <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" />
      <path d="M8 7V11.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      <circle cx="8" cy="5" r="0.75" fill="currentColor" />
    </svg>
  ),
};

/** 单条 Toast 组件 */
function ToastItem({ id, type, message }: { id: string; type: ToastType; message: string }) {
  const removeToast = useToastStore((s) => s.removeToast);

  // 自动消失定时器（info 类型需要手动关闭）
  useEffect(() => {
    if (type === "info") return;
    const timer = setTimeout(() => removeToast(id), AUTO_DISMISS_MS);
    return () => clearTimeout(timer);
  }, [id, type, removeToast]);

  // 手动关闭
  const handleClose = useCallback(() => {
    removeToast(id);
  }, [id, removeToast]);

  return (
    <div className={`toast-item toast-${type}`}>
      <span className="toast-icon">{typeIcons[type]}</span>
      <span className="toast-message">{message}</span>
      <button className="toast-close" onClick={handleClose}>
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
          <path d="M2 2L10 10M10 2L2 10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
      </button>
    </div>
  );
}

/** 全局 Toast 容器，渲染在屏幕右上角 */
export function ToastContainer() {
  const toasts = useToastStore((s) => s.toasts);

  return (
    <div className="toast-container">
      {toasts.map((toast) => (
        <ToastItem key={toast.id} id={toast.id} type={toast.type} message={toast.message} />
      ))}

      <style>{`
        .toast-container {
          position: fixed;
          top: 16px;
          right: 16px;
          z-index: 9999;
          display: flex;
          flex-direction: column;
          gap: 8px;
          pointer-events: none;
          max-width: 380px;
        }

        .toast-item {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 10px 14px;
          border-radius: var(--radius-md);
          background: var(--color-bg-elevated);
          border: 1px solid var(--color-border);
          box-shadow: var(--shadow-lg);
          pointer-events: auto;
          animation: toastSlideIn 0.3s ease forwards;
          min-width: 240px;
          max-width: 380px;
        }

        /* 退场动画：通过 CSS 类控制 */
        .toast-item.toast-exit {
          animation: toastSlideOut 0.25s ease forwards;
        }

        /* 类型样式 */
        .toast-error {
          border-left: 3px solid var(--color-error);
        }
        .toast-error .toast-icon {
          color: var(--color-error);
        }

        .toast-success {
          border-left: 3px solid var(--color-success);
        }
        .toast-success .toast-icon {
          color: var(--color-success);
        }

        .toast-warning {
          border-left: 3px solid var(--color-warning);
        }
        .toast-warning .toast-icon {
          color: var(--color-warning);
        }

        .toast-info {
          border-left: 3px solid var(--color-info);
        }
        .toast-info .toast-icon {
          color: var(--color-info);
        }

        .toast-icon {
          flex-shrink: 0;
          display: flex;
          align-items: center;
          justify-content: center;
        }

        .toast-message {
          flex: 1;
          font-size: 13px;
          line-height: 1.5;
          color: var(--color-text-primary);
          word-break: break-word;
        }

        .toast-close {
          flex-shrink: 0;
          display: flex;
          align-items: center;
          justify-content: center;
          width: 20px;
          height: 20px;
          border-radius: 4px;
          color: var(--color-text-quaternary);
          transition: all 0.15s;
          cursor: pointer;
        }

        .toast-close:hover {
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
        }

        /* 入场动画：从右侧滑入 */
        @keyframes toastSlideIn {
          from {
            opacity: 0;
            transform: translateX(100%);
          }
          to {
            opacity: 1;
            transform: translateX(0);
          }
        }

        /* 退场动画：向右滑出 */
        @keyframes toastSlideOut {
          from {
            opacity: 1;
            transform: translateX(0);
          }
          to {
            opacity: 0;
            transform: translateX(100%);
          }
        }
      `}</style>
    </div>
  );
}
