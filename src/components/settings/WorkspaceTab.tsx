import { useState } from "react";
import { useWorkspaceStore } from "../../stores/useWorkspaceStore";
import { AddWorkspaceDialog } from "./AddWorkspaceDialog";

export function WorkspaceTab() {
  const { workspaces, currentWorkspaceId, switchWorkspace, removeWorkspace, loadWorkspaces } = useWorkspaceStore();
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [removingId, setRemovingId] = useState<string | null>(null);
  const [removeError, setRemoveError] = useState<string | null>(null);

  const handleSwitch = async (id: string) => {
    await switchWorkspace(id);
  };

  const handleRemove = async (id: string) => {
    setRemoveError(null);
    try {
      await removeWorkspace(id);
      setRemovingId(null);
      await loadWorkspaces();
    } catch (err) {
      setRemoveError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleAddSaved = async () => {
    setShowAddDialog(false);
    await loadWorkspaces();
  };

  return (
    <div>
      <div className="section-header">
        <span className="section-title">工作区列表</span>
        <span className="section-badge">{workspaces.length}</span>
      </div>

      {workspaces.length === 0 && (
        <div className="empty-state-lg">
          <span>暂无工作区，请点击下方按钮添加</span>
        </div>
      )}

      {workspaces.map((ws) => (
        <div key={ws.id} className="workspace-card">
          <div className="workspace-card-header">
            <div className="workspace-card-left">
              <span className="workspace-name">{ws.name}</span>
              {ws.id === currentWorkspaceId && (
                <span className="workspace-current-badge">当前</span>
              )}
              {ws.id !== currentWorkspaceId && (
                <button className="switch-btn" onClick={() => handleSwitch(ws.id)}>切换</button>
              )}
            </div>
            <div className="workspace-actions">
              <button
                className="action-btn action-btn-danger"
                onClick={() => { setRemovingId(ws.id); setRemoveError(null); }}
              >
                移除
              </button>
            </div>
          </div>
          <div className="workspace-card-info">
            <span className="workspace-path">{ws.path}</span>
            <span className="info-sep">|</span>
            <span className="workspace-date">创建于 {new Date(ws.createdAt).toLocaleDateString("zh-CN")}</span>
          </div>

          {removingId === ws.id && (
            <div className="confirm-bar">
              <div className="confirm-bar-text">确定要移除此工作区吗？（不会删除本地文件）</div>
              {removeError && (
                <div className="error-text">{removeError}</div>
              )}
              <div className="confirm-bar-actions">
                <button className="confirm-btn confirm-btn-danger" onClick={() => handleRemove(ws.id)}>确认移除</button>
                <button className="confirm-btn confirm-btn-ghost" onClick={() => { setRemovingId(null); setRemoveError(null); }}>取消</button>
              </div>
            </div>
          )}
        </div>
      ))}

      <button className="add-btn" onClick={() => setShowAddDialog(true)}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
        添加工作区
      </button>

      {showAddDialog && (
        <AddWorkspaceDialog
          onClose={() => setShowAddDialog(false)}
          onSaved={handleAddSaved}
        />
      )}

      <style>{`
        .section-header {
          display: flex;
          align-items: center;
          gap: 8px;
          margin-bottom: 16px;
        }
        .section-title {
          font-size: 13px;
          font-weight: 600;
          color: var(--color-text-secondary);
          text-transform: uppercase;
          letter-spacing: 0.3px;
        }
        .section-badge {
          font-size: 11px;
          font-weight: 500;
          padding: 1px 8px;
          border-radius: 10px;
          background: var(--color-accent-light);
          color: var(--color-accent);
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
        .confirm-bar {
          margin-top: 12px;
          padding-top: 12px;
          border-top: 1px solid var(--color-border-light);
        }
        .confirm-bar-text {
          font-size: 12px;
          color: var(--color-text-secondary);
          margin-bottom: 8px;
        }
        .error-text {
          font-size: 11px;
          color: var(--color-error);
          margin-bottom: 8px;
        }
        .confirm-bar-actions {
          display: flex;
          gap: 8px;
        }
        .confirm-btn {
          padding: 4px 12px;
          border-radius: var(--radius-xs);
          font-size: 11px;
          font-weight: 500;
          border: none;
          cursor: pointer;
          transition: all 0.15s;
        }
        .confirm-btn-danger {
          background: var(--color-error);
          color: white;
        }
        .confirm-btn-danger:hover {
          background: var(--color-error);
          filter: brightness(0.9);
        }
        .confirm-btn-ghost {
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
        }
        .confirm-btn-ghost:hover {
          background: var(--color-bg-hover);
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
