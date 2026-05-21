import { useEffect, useRef, useCallback } from "react";
import { Icon } from "./Icon";

export interface ContextMenuItem {
  label: string;
  icon?: string;
  onClick: () => void;
  danger?: boolean;
  separator?: boolean;
}

interface ContextMenuProps {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

export function ContextMenu({ x, y, items, onClose }: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  /* 点击菜单外部关闭 */
  const handleClickOutside = useCallback(
    (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    },
    [onClose],
  );

  /* 按 Escape 关闭 */
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    },
    [onClose],
  );

  useEffect(() => {
    /* 延迟添加监听，避免当前右键事件立即触发关闭 */
    const timer = setTimeout(() => {
      document.addEventListener("mousedown", handleClickOutside);
      document.addEventListener("keydown", handleKeyDown);
    }, 0);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [handleClickOutside, handleKeyDown]);

  /* 计算菜单位置，处理边界溢出 */
  const menuStyle = (() => {
    const menuWidth = 200;
    const menuHeight = items.length * 32;
    const viewportW = window.innerWidth;
    const viewportH = window.innerHeight;

    let posX = x;
    let posY = y;

    if (x + menuWidth > viewportW) {
      posX = viewportW - menuWidth - 8;
    }
    if (y + menuHeight > viewportH) {
      posY = viewportH - menuHeight - 8;
    }

    posX = Math.max(8, posX);
    posY = Math.max(8, posY);

    return { left: posX, top: posY };
  })();

  /* 过滤掉连续的分隔线 */
  const filteredItems = items.reduce<ContextMenuItem[]>((acc, item, idx) => {
    if (item.separator) {
      /* 跳过首尾的分隔线和连续分隔线 */
      if (acc.length === 0 || idx === items.length - 1) return acc;
      if (acc[acc.length - 1].separator) return acc;
    }
    acc.push(item);
    return acc;
  }, []);

  return (
    <div className="ctx-menu-overlay">
      <div
        ref={menuRef}
        className="ctx-menu"
        style={menuStyle}
      >
        {filteredItems.map((item, idx) => {
          if (item.separator) {
            return <div key={`sep-${idx}`} className="ctx-menu-separator" />;
          }
          return (
            <button
              key={`item-${idx}`}
              className={`ctx-menu-item ${item.danger ? "ctx-menu-item-danger" : ""}`}
              onClick={() => {
                item.onClick();
                onClose();
              }}
            >
              {item.icon && (
                <span className="ctx-menu-item-icon">
                  <Icon name={item.icon as never} size={14} />
                </span>
              )}
              <span className="ctx-menu-item-label">{item.label}</span>
            </button>
          );
        })}
      </div>

      <style>{`
        .ctx-menu-overlay {
          position: fixed;
          inset: 0;
          z-index: 9999;
        }
        .ctx-menu {
          position: fixed;
          min-width: 180px;
          max-width: 260px;
          background: var(--color-bg-elevated, #fff);
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-md, 8px);
          box-shadow: 0 6px 16px rgba(0, 0, 0, 0.08), 0 2px 8px rgba(0, 0, 0, 0.04);
          padding: 4px;
          z-index: 10000;
          animation: ctx-menu-in 0.12s ease-out;
        }
        @keyframes ctx-menu-in {
          from {
            opacity: 0;
            transform: scale(0.95) translateY(-4px);
          }
          to {
            opacity: 1;
            transform: scale(1) translateY(0);
          }
        }
        .ctx-menu-item {
          display: flex;
          align-items: center;
          gap: 8px;
          width: 100%;
          padding: 6px 10px;
          border: none;
          background: none;
          border-radius: var(--radius-sm, 4px);
          cursor: pointer;
          font-size: 12px;
          color: var(--color-text-primary);
          transition: background 0.12s;
          text-align: left;
        }
        .ctx-menu-item:hover {
          background: var(--color-bg-hover, rgba(0, 0, 0, 0.04));
        }
        .ctx-menu-item-danger {
          color: var(--color-error, #e53e3e);
        }
        .ctx-menu-item-danger:hover {
          background: rgba(229, 62, 62, 0.08);
        }
        .ctx-menu-item-icon {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 16px;
          height: 16px;
          flex-shrink: 0;
          color: var(--color-text-tertiary);
        }
        .ctx-menu-item-danger .ctx-menu-item-icon {
          color: var(--color-error, #e53e3e);
        }
        .ctx-menu-item-label {
          flex: 1;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        .ctx-menu-separator {
          height: 1px;
          margin: 4px 8px;
          background: var(--color-border-light);
        }
      `}</style>
    </div>
  );
}
