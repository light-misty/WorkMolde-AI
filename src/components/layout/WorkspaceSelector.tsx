import { useTranslation } from 'react-i18next';
import { useState, useRef, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import { Icon } from "../common/Icon";
import { useWorkspaceStore } from "../../stores/useWorkspaceStore";
import { AddWorkspaceDialog } from "../settings/AddWorkspaceDialog";
import { DeleteConfirmDialog } from "../common/DeleteConfirmDialog";

export function WorkspaceSelector() {
  const { t } = useTranslation();
  const { currentWorkspaceId, workspaces, removeWorkspace, switchWorkspace } = useWorkspaceStore();
  const [open, setOpen] = useState(false);
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [deleteWorkspaceId, setDeleteWorkspaceId] = useState<string | null>(null);
  const [deleteWorkspaceName, setDeleteWorkspaceName] = useState("");
  const containerRef = useRef<HTMLDivElement>(null);
  const currentWs = workspaces.find((w) => w.id === currentWorkspaceId);

  /* 点击外部关闭下拉框 */
  const handleClickOutside = useCallback((e: MouseEvent) => {
    if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
      setOpen(false);
    }
  }, []);

  /* 按 Escape 关闭下拉框 */
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === "Escape") {
      setOpen(false);
    }
  }, []);

  useEffect(() => {
    if (open) {
      /* 延迟添加监听，避免当前点击事件立即触发关闭 */
      const timer = setTimeout(() => {
        document.addEventListener("mousedown", handleClickOutside);
        document.addEventListener("keydown", handleKeyDown);
      }, 0);
      return () => {
        clearTimeout(timer);
        document.removeEventListener("mousedown", handleClickOutside);
        document.removeEventListener("keydown", handleKeyDown);
      };
    }
  }, [open, handleClickOutside, handleKeyDown]);

  /* 切换工作区：仅在目标工作区不同于当前工作区且目录存在时切换，切换后关闭下拉框 */
  const handleSwitch = async (id: string) => {
    if (id === currentWorkspaceId) {
      setOpen(false);
      return;
    }
    try {
      await switchWorkspace(id);
    } catch (err) {
      console.error("[WorkspaceSelector] 切换工作区失败:", err);
    }
    setOpen(false);
  };

  /* 移除工作区 */
  const handleRemove = async () => {
    if (!deleteWorkspaceId) return;
    try {
      await removeWorkspace(deleteWorkspaceId);
      setDeleteWorkspaceId(null);
      setDeleteWorkspaceName("");
    } catch (err) {
      console.error("[WorkspaceSelector] 移除工作区失败:", err);
      setDeleteWorkspaceId(null);
      setDeleteWorkspaceName("");
    }
  };

  /* 添加工作区完成后 */
  const handleAddSaved = () => {
    setShowAddDialog(false);
  };

  return (
    <div ref={containerRef} className="ws-selector-container">
      {/* 触发按钮 */}
      <div
        role="button"
        aria-label={t('workspace.selectWorkspace')}
        tabIndex={0}
        className={`ws-selector-trigger ${open ? "ws-selector-trigger-active" : ""}`}
        onClick={() => { setOpen((prev) => !prev); }}
        onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setOpen((prev) => !prev); } }}
      >
        <span className="ws-selector-label">{currentWs?.name ?? t('workspace.selectWorkspace')}</span>
        <Icon name={open ? "chevron-up" : "chevron-down"} size={14} />
      </div>

      {/* 下拉面板 */}
      {open && (
        <div className="ws-selector-dropdown">
          {/* 工作区列表 */}
          <div className="ws-selector-list">
            {workspaces.length === 0 && (
              <div className="ws-selector-empty">{t('workspace.noWorkspace')}</div>
            )}
            {workspaces.map((ws) => (
              <div key={ws.id} className="ws-selector-item-wrapper">
                <div
                  className={`ws-selector-item ${!ws.pathExists ? "ws-selector-item-deleted" : ""} ${ws.id === currentWorkspaceId ? "ws-selector-item-current" : ""}`}
                  role={ws.pathExists ? "button" : undefined}
                  tabIndex={ws.pathExists ? 0 : -1}
                  aria-current={ws.id === currentWorkspaceId ? "true" : undefined}
                  aria-label={t('workspace.selectWorkspace')}
                  onClick={() => { if (ws.pathExists) void handleSwitch(ws.id); }}
                  onKeyDown={(e) => {
                    if (ws.pathExists && (e.key === "Enter" || e.key === " ")) {
                      e.preventDefault();
                      void handleSwitch(ws.id);
                    }
                  }}
                >
                  <div className="ws-selector-item-left">
                    {/* 当前激活工作区标识 */}
                    <span className="ws-selector-current-mark" aria-hidden="true">
                      {ws.id === currentWorkspaceId && <Icon name="check" size={14} />}
                    </span>
                    <div className="ws-selector-item-info">
                      <span className="ws-selector-item-name">{ws.name}{!ws.pathExists ? ` (${t('workspace.directoryDeleted')})` : ""}</span>
                      <span className="ws-selector-item-path">{ws.path}</span>
                    </div>
                  </div>
                  <button
                    className="ws-selector-remove-btn"
                    title={t('workspace.removeWorkspace')}
                    aria-label={t('workspace.removeWorkspace')}
                    onClick={(e) => { e.stopPropagation(); setDeleteWorkspaceId(ws.id); setDeleteWorkspaceName(ws.name); }}
                  >
                    <Icon name="close" size={12} />
                  </button>
                </div>


              </div>
            ))}
          </div>

          {/* 添加工作区按钮 */}
          <div className="ws-selector-footer">
            <button
              className="ws-selector-add-btn"
              onClick={() => setShowAddDialog(true)}
            >
              <Icon name="plus" size={14} />
              <span>{t('workspace.addWorkspace')}</span>
            </button>
          </div>
        </div>
      )}

      {/* 移除工作区确认弹窗 */}
      {deleteWorkspaceId && createPortal(
        <DeleteConfirmDialog
          name={deleteWorkspaceName}
          isDir={true}
          onConfirm={handleRemove}
          onCancel={() => {
            setDeleteWorkspaceId(null);
            setDeleteWorkspaceName("");
          }}
        />,
        document.body
      )}

      {/* 添加工作区弹窗 */}
      {showAddDialog && (
        <AddWorkspaceDialog
          onClose={() => setShowAddDialog(false)}
          onSaved={handleAddSaved}
        />
      )}

      <style>{`
        .ws-selector-container {
          position: relative;
        }
        .ws-selector-trigger {
          display: flex;
          align-items: center;
          gap: 6px;
          padding: 5px 10px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          transition: background 0.15s;
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-secondary);
          white-space: nowrap;
          user-select: none;
        }
        .ws-selector-trigger:hover {
          background: var(--color-bg-sub);
        }
        .ws-selector-trigger-active {
          background: var(--color-bg-sub);
          color: var(--color-text-primary);
        }
        .ws-selector-label {
          max-width: 160px;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        .ws-selector-dropdown {
          position: absolute;
          top: calc(100% + 6px);
          left: 0;
          min-width: 280px;
          max-width: 360px;
          background: var(--color-bg-elevated);
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-md);
          box-shadow: var(--shadow-lg);
          z-index: 200;
          animation: ws-dropdown-in 0.15s ease-out;
          overflow: hidden;
        }
        @keyframes ws-dropdown-in {
          from {
            opacity: 0;
            transform: scale(0.96) translateY(-4px);
          }
          to {
            opacity: 1;
            transform: scale(1) translateY(0);
          }
        }
        .ws-selector-list {
          max-height: 320px;
          overflow-y: auto;
          padding: 4px;
        }
        .ws-selector-empty {
          padding: 20px 16px;
          text-align: center;
          font-size: 12px;
          color: var(--color-text-quaternary);
        }
        .ws-selector-item-wrapper {
          margin-bottom: 2px;
        }
        .ws-selector-item-wrapper:last-child {
          margin-bottom: 0;
        }
        .ws-selector-item {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 8px;
          padding: 8px 10px;
          border-radius: var(--radius-sm);
          cursor: pointer;
        }
        .ws-selector-item:hover {
          background: var(--color-bg-hover);
        }
        .ws-selector-item-current {
          background: var(--color-accent-bg);
        }
        .ws-selector-item-current:hover {
          background: var(--color-accent-bg);
        }
        .ws-selector-item-deleted {
          opacity: 0.5;
          cursor: not-allowed;
        }
        .ws-selector-item-deleted .ws-selector-item-name {
          color: var(--color-error);
          text-decoration: line-through;
        }
        .ws-selector-current-mark {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 16px;
          height: 16px;
          flex-shrink: 0;
          color: var(--color-accent);
        }
        .ws-selector-item-left {
          display: flex;
          align-items: center;
          gap: 8px;
          min-width: 0;
          flex: 1;
        }
        .ws-selector-item-info {
          display: flex;
          flex-direction: column;
          gap: 1px;
          min-width: 0;
          flex: 1;
        }
        .ws-selector-item-name {
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-primary);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .ws-selector-item-path {
          font-size: 11px;
          color: var(--color-text-quaternary);
          font-family: var(--font-mono);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .ws-selector-remove-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 22px;
          height: 22px;
          border-radius: var(--radius-xs);
          color: var(--color-text-quaternary);
          flex-shrink: 0;
          transition: all 0.12s;
          opacity: 0;
        }
        .ws-selector-item:hover .ws-selector-remove-btn {
          opacity: 1;
        }
        .ws-selector-remove-btn:hover {
          background: var(--color-error-bg);
          color: var(--color-error);
        }
        .ws-selector-footer {
          border-top: 1px solid var(--color-border-light);
          padding: 4px;
        }
        .ws-selector-add-btn {
          display: flex;
          align-items: center;
          gap: 6px;
          width: 100%;
          padding: 7px 10px;
          border-radius: var(--radius-sm);
          font-size: 12px;
          font-weight: 500;
          color: var(--color-accent);
          transition: background 0.12s;
          border: none;
          cursor: pointer;
          background: none;
        }
        .ws-selector-add-btn:hover {
          background: var(--color-accent-bg);
        }
      `}</style>
    </div>
  );
}
