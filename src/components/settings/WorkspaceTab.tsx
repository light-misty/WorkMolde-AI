import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../../stores/useWorkspaceStore";
import { AddWorkspaceDialog } from "./AddWorkspaceDialog";
import { DeleteConfirmDialog } from "../common/DeleteConfirmDialog";
import type { WorkspaceInfo } from "../../types/workspace";

export function WorkspaceTab() {
  const { t } = useTranslation();
  const { workspaces, currentWorkspaceId, switchWorkspace, removeWorkspace } = useWorkspaceStore();
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<WorkspaceInfo | null>(null);

  const handleSwitch = async (id: string) => {
    await switchWorkspace(id);
  };

  const handleRemove = async () => {
    if (!removeTarget) return;
    try {
      await removeWorkspace(removeTarget.id);
      setRemoveTarget(null);
    } catch (err) {
      setRemoveTarget(null);
      alert(err instanceof Error ? err.message : String(err));
    }
  };

  const handleAddSaved = () => {
    setShowAddDialog(false);
  };

  return (
    <div>
      <div className="section-header">
        <span className="section-title">{t('settings.workspace.workspaceList')}</span>
        <span className="section-badge">{workspaces.length}</span>
        <button className="add-btn" onClick={() => setShowAddDialog(true)}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
          {t('settings.workspace.addWorkspace')}
        </button>
      </div>

      {workspaces.length === 0 && (
        <div className="empty-state-lg">
          <span>{t('settings.workspace.emptyHint')}</span>
        </div>
      )}

      {workspaces.map((ws) => (
        <div key={ws.id} className="workspace-card">
          <div className="workspace-card-header">
            <div className="workspace-card-left">
              <span className="workspace-name">{ws.name}</span>
              {ws.id === currentWorkspaceId && (
                <span className="workspace-current-badge">{t('settings.workspace.current')}</span>
              )}
              {ws.id !== currentWorkspaceId && (
                <button className="switch-btn" onClick={() => handleSwitch(ws.id)}>{t('settings.workspace.switch')}</button>
              )}
            </div>
            <div className="workspace-actions">
              <button
                className="action-btn action-btn-danger"
                onClick={() => setRemoveTarget(ws)}
              >
                {t('settings.workspace.remove')}
              </button>
            </div>
          </div>
          <div className="workspace-card-info">
            <span className="workspace-path">{ws.path}</span>
            <span className="info-sep">|</span>
            <span className="workspace-date">{t('settings.workspace.createdAt')} {new Date(ws.createdAt).toLocaleDateString("zh-CN")}</span>
          </div>
        </div>
      ))}


      {removeTarget && (
        <DeleteConfirmDialog
          name={removeTarget.name}
          isDir={true}
          onConfirm={handleRemove}
          onCancel={() => setRemoveTarget(null)}
        />
      )}

      {showAddDialog && (
        <AddWorkspaceDialog
          onClose={() => setShowAddDialog(false)}
          onSaved={handleAddSaved}
        />
      )}

      <style>{`
        .section-header .add-btn {
          margin-left: auto;
        }
        .empty-state-lg {
          font-size: 13px;
          color: var(--color-text-quaternary);
          text-align: center;
          padding: 24px 16px;
        }
        .workspace-card {
          padding: 14px 16px;
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-md);
          margin-bottom: 8px;
          transition: all 0.15s;
        }
        .workspace-card:hover {
          border-color: var(--color-border-strong);
        }
        .workspace-card-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 8px;
          margin-bottom: 8px;
          flex-wrap: wrap;
        }
        .workspace-card-left {
          display: flex;
          align-items: center;
          gap: 8px;
          flex-wrap: wrap;
        }
        .workspace-name {
          font-size: 13px;
          font-weight: 600;
          color: var(--color-text-primary);
        }
        .workspace-current-badge {
          font-size: 11px;
          color: var(--color-success);
          font-weight: 500;
        }
        .switch-btn {
          padding: 2px 8px;
          border-radius: var(--radius-xs);
          font-size: 11px;
          font-weight: 500;
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
          transition: all 0.15s;
          border: none;
          cursor: pointer;
        }
        .switch-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
        }
        .workspace-actions {
          display: flex;
          gap: 4px;
          flex-shrink: 0;
          opacity: 1;
        }
        .action-btn {
          padding: 3px 8px;
          border-radius: var(--radius-xs);
          font-size: 11px;
          font-weight: 500;
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
          transition: all 0.15s;
          cursor: pointer;
          border: none;
        }
        .action-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
        }
        .action-btn-danger:hover {
          background: var(--color-error-light);
          color: var(--color-error);
        }
        .workspace-card-info {
          font-size: 11px;
          color: var(--color-text-quaternary);
          display: flex;
          align-items: center;
          gap: 6px;
          flex-wrap: wrap;
        }
        .workspace-path {
          font-family: var(--font-mono);
          color: var(--color-text-tertiary);
        }
        .info-sep {
          color: var(--color-border);
        }
        .workspace-date {
          color: var(--color-text-quaternary);
        }
        .add-btn {
          display: inline-flex;
          align-items: center;
          gap: 6px;
          padding: 6px 14px;
          border-radius: var(--radius-sm);
          font-size: 12px;
          font-weight: 500;
          background: var(--color-accent);
          color: white;
          border: none;
          cursor: pointer;
          transition: all 0.15s;
        }
        .add-btn:hover {
          background: var(--color-accent-hover);
        }
      `}</style>
    </div>
  );
}
