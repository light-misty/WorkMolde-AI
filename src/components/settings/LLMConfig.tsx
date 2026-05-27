import { useState } from "react";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { ProviderFormDialog } from "./ProviderFormDialog";
import type { ProviderInfo } from "../../types";
import * as tauriCmd from "../../services/tauri";

export function LLMConfigTab() {
  const { llmProviders, loadProviders } = useSettingsStore();
  const [dialogMode, setDialogMode] = useState<"add" | "edit" | null>(null);
  const [editingProvider, setEditingProvider] = useState<ProviderInfo | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  // 正在测试连接的 provider ID，用于显示加载动画
  const [testingId, setTestingId] = useState<string | null>(null);

  const handleAdd = () => {
    setEditingProvider(null);
    setDialogMode("add");
  };

  const handleEdit = (provider: ProviderInfo) => {
    setEditingProvider(provider);
    setDialogMode("edit");
  };

  const handleTest = async (providerId: string) => {
    setTestingId(providerId);
    try {
      const result = await tauriCmd.testConnection(providerId);
      if (result.success) {
        alert(`连接成功！延迟: ${result.latencyMs}ms${result.model ? `\n模型: ${result.model}` : ""}`);
      } else {
        alert(`连接失败: ${result.errorMessage || result.error || "未知错误"}`);
      }
    } catch (err) {
      alert(`连接测试出错: ${err}`);
    } finally {
      setTestingId(null);
    }
  };

  const handleDelete = async (providerId: string) => {
    setDeleteError(null);
    try {
      await tauriCmd.deleteProvider(providerId);
      setDeletingId(null);
      await loadProviders();
    } catch (err) {
      setDeleteError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleSetDefault = async (providerId: string) => {
    try {
      await tauriCmd.setDefaultProvider(providerId);
      await loadProviders();
    } catch (err) {
      console.error("[LLMConfigTab] 设置默认 Provider 失败:", err);
    }
  };

  const handleDialogSaved = async () => {
    setDialogMode(null);
    setEditingProvider(null);
    await loadProviders();
  };

  return (
    <div>
      <div className="mb-8">
        <div className="section-header">
          <span className="section-title">已配置的 Provider</span>
          <span className="section-badge">{llmProviders.length}</span>
          <button className="add-btn" onClick={handleAdd}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
            添加 Provider
          </button>
        </div>

        {llmProviders.length === 0 && (
          <div className="empty-state-lg">
            <span>暂无 Provider，请点击右侧按钮添加</span>
          </div>
        )}

        {llmProviders.map((p) => (
          <div key={p.id} className="provider-card">
            <div className="provider-card-header">
              <div className="provider-card-left">
                <span className="provider-name">{p.name}</span>
                <span className="provider-type-badge">{p.providerType}</span>
                {p.isDefault && (
                  <span className="provider-default-badge">
                    <svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor"><polyline points="20 6 9 17 4 12" /></svg>
                    默认
                  </span>
                )}
              </div>
              <div className="provider-actions">
                {!p.isDefault && (
                  <button className="action-btn" onClick={() => handleSetDefault(p.id)}>设为默认</button>
                )}
                <button className="action-btn" onClick={() => handleEdit(p)}>编辑</button>
                <button 
                  className="action-btn" 
                  onClick={() => handleTest(p.id)}
                  disabled={testingId === p.id}
                >
                  {testingId === p.id ? (
                    <span className="test-loading">
                      <span className="test-spinner"></span>
                      测试中
                    </span>
                  ) : "测试"}
                </button>
                <button
                  className="action-btn action-btn-danger"
                  onClick={() => { setDeletingId(p.id); setDeleteError(null); }}
                >
                  删除
                </button>
              </div>
            </div>
            <div className="provider-card-info">
              <span className="provider-model">{p.model}</span>
              <span className="info-sep">|</span>
              <span className="provider-url">{p.apiBase}</span>
              <span className="info-sep">|</span>
              <span className={`status-badge ${p.isAvailable ? "status-available" : "status-unavailable"}`}>
                {p.isAvailable ? "可用" : "不可用"}
              </span>
            </div>

            {deletingId === p.id && (
              <div className="confirm-bar">
                <div className="confirm-bar-text">确定要删除此 Provider 吗？</div>
                {deleteError && (
                  <div className="error-text">{deleteError}</div>
                )}
                <div className="confirm-bar-actions">
                  <button className="confirm-btn confirm-btn-danger" onClick={() => handleDelete(p.id)}>确认删除</button>
                  <button className="confirm-btn confirm-btn-ghost" onClick={() => { setDeletingId(null); setDeleteError(null); }}>取消</button>
                </div>
              </div>
            )}
          </div>
        ))}

      </div>

      <div>
        <div className="section-header">
          <span className="section-title">Fallback 顺序</span>
        </div>
        <div className="fallback-list">
          {llmProviders.length === 0 && (
            <div className="empty-state-lg">添加 Provider 后可配置 Fallback 顺序</div>
          )}
          {llmProviders.map((p, i) => (
            <div key={p.id} className="fallback-item">
              <span className="fallback-index">{i + 1}.</span>
              <span className="fallback-name">{p.name}</span>
              {p.isDefault && (
                <span className="fallback-default-badge">默认</span>
              )}
            </div>
          ))}
        </div>
      </div>

      {dialogMode && (
        <ProviderFormDialog
          mode={dialogMode}
          provider={editingProvider}
          onClose={() => { setDialogMode(null); setEditingProvider(null); }}
          onSaved={handleDialogSaved}
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
        .provider-card {
          padding: 14px 16px;
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-md);
          margin-bottom: 8px;
          transition: all 0.15s;
        }
        .provider-card:hover {
          border-color: var(--color-border-strong);
        }
        .provider-card-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 8px;
          margin-bottom: 8px;
          flex-wrap: wrap;
        }
        .provider-card-left {
          display: flex;
          align-items: center;
          gap: 8px;
          flex-wrap: wrap;
        }
        .provider-name {
          font-size: 13px;
          font-weight: 600;
          color: var(--color-text-primary);
        }
        .provider-type-badge {
          font-size: 10px;
          font-weight: 600;
          padding: 2px 8px;
          border-radius: 4px;
          background: var(--color-accent-light);
          color: var(--color-accent);
          text-transform: uppercase;
        }
        .provider-default-badge {
          font-size: 11px;
          color: var(--color-success);
          display: flex;
          align-items: center;
          gap: 3px;
          font-weight: 500;
        }
        .provider-actions {
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
        .action-btn:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }
        .test-loading {
          display: inline-flex;
          align-items: center;
          gap: 4px;
        }
        .test-spinner {
          width: 10px;
          height: 10px;
          border: 2px solid var(--color-text-quaternary);
          border-top-color: var(--color-text-secondary);
          border-radius: 50%;
          animation: spin 0.8s linear infinite;
        }
        @keyframes spin {
          to { transform: rotate(360deg); }
        }
        .provider-card-info {
          font-size: 11px;
          color: var(--color-text-quaternary);
          display: flex;
          align-items: center;
          gap: 6px;
          flex-wrap: wrap;
        }
        .provider-model {
          font-family: var(--font-mono);
          font-weight: 500;
          color: var(--color-text-tertiary);
        }
        .provider-url {
          font-family: var(--font-mono);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
          max-width: 200px;
        }
        .info-sep {
          color: var(--color-border);
        }
        .status-badge {
          padding: 1px 6px;
          border-radius: 3px;
          font-size: 10px;
          font-weight: 500;
        }
        .status-available {
          background: var(--color-success-light);
          color: var(--color-success);
        }
        .status-unavailable {
          background: var(--color-error-light);
          color: var(--color-error);
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
        .fallback-list {
          display: flex;
          flex-direction: column;
          gap: 4px;
        }
        .fallback-item {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 8px 12px;
          font-size: 12px;
          color: var(--color-text-secondary);
          border-radius: var(--radius-sm);
          transition: background 0.15s;
        }
        .fallback-item:hover {
          background: var(--color-accent-bg);
        }
        .fallback-index {
          font-weight: 600;
          color: var(--color-accent);
          min-width: 20px;
        }
        .fallback-name {
          flex: 1;
          font-weight: 500;
        }
        .fallback-default-badge {
          font-size: 10px;
          padding: 1px 6px;
          border-radius: 3px;
          background: var(--color-success-light);
          color: var(--color-success);
          font-weight: 500;
        }
      `}</style>
    </div>
  );
}
