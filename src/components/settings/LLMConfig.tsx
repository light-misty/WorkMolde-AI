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

  const handleAdd = () => {
    setEditingProvider(null);
    setDialogMode("add");
  };

  const handleEdit = (provider: ProviderInfo) => {
    setEditingProvider(provider);
    setDialogMode("edit");
  };

  const handleTest = async (providerId: string) => {
    try {
      const result = await tauriCmd.testConnection(providerId);
      if (result.success) {
        alert(`连接成功！延迟: ${result.latencyMs}ms${result.model ? `\n模型: ${result.model}` : ""}`);
      } else {
        alert(`连接失败: ${result.errorMessage || result.error || "未知错误"}`);
      }
    } catch (err) {
      alert(`连接测试出错: ${err}`);
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
      <div className="mb-6">
        <div className="text-[13px] font-semibold text-text-secondary uppercase tracking-[.3px] mb-3">已配置的 Provider</div>

        {llmProviders.length === 0 && (
          <div className="text-[13px] text-text-tertiary text-center py-6">
            暂无 Provider，请点击下方按钮添加
          </div>
        )}

        {llmProviders.map((p) => (
          <div key={p.id} className="px-3 py-3 border border-border rounded-[var(--radius-md)] mb-2 transition-colors duration-150 hover:border-[#D0D3D9]">
            <div className="flex items-center gap-2 mb-2">
              <span className="font-semibold text-[13px]">{p.name}</span>
              <span className="text-[10px] font-semibold px-[6px] py-[2px] rounded-[3px] bg-accent-light text-accent uppercase">{p.providerType}</span>
              {p.isDefault && (
                <span className="text-[11px] text-success ml-1">默认</span>
              )}
              <div className="ml-auto flex gap-1">
                {!p.isDefault && (
                  <button
                    className="px-2 py-[3px] rounded-[var(--radius-sm)] text-[11px] font-medium bg-bg-sub text-text-secondary hover:bg-bg-hover transition-all"
                    onClick={() => handleSetDefault(p.id)}
                  >
                    设为默认
                  </button>
                )}
                <button
                  className="px-2 py-[3px] rounded-[var(--radius-sm)] text-[11px] font-medium bg-bg-sub text-text-secondary hover:bg-bg-hover transition-all"
                  onClick={() => handleEdit(p)}
                >
                  编辑
                </button>
                <button
                  className="px-2 py-[3px] rounded-[var(--radius-sm)] text-[11px] font-medium bg-bg-sub text-text-secondary hover:bg-bg-hover transition-all"
                  onClick={() => handleTest(p.id)}
                >
                  测试
                </button>
                <button
                  className="px-2 py-[3px] rounded-[var(--radius-sm)] text-[11px] font-medium bg-bg-sub text-error hover:bg-red-50 transition-all"
                  onClick={() => { setDeletingId(p.id); setDeleteError(null); }}
                >
                  删除
                </button>
              </div>
            </div>
            <div className="font-mono text-[11px] text-text-tertiary">
              {p.model} &nbsp;|&nbsp; {p.apiBase} &nbsp;|&nbsp; {p.isAvailable ? "可用" : "不可用"}
            </div>

            {/* 删除确认 */}
            {deletingId === p.id && (
              <div className="mt-2 pt-2 border-t border-border-light">
                <div className="text-[12px] text-text-secondary mb-2">确定要删除此 Provider 吗？</div>
                {deleteError && (
                  <div className="text-[11px] text-error mb-2">{deleteError}</div>
                )}
                <div className="flex gap-2">
                  <button
                    className="px-3 py-[4px] rounded-[var(--radius-sm)] text-[11px] font-medium bg-error text-white hover:bg-red-600 transition-all"
                    onClick={() => handleDelete(p.id)}
                  >
                    确认删除
                  </button>
                  <button
                    className="px-3 py-[4px] rounded-[var(--radius-sm)] text-[11px] font-medium bg-bg-sub text-text-secondary hover:bg-bg-hover transition-all"
                    onClick={() => { setDeletingId(null); setDeleteError(null); }}
                  >
                    取消
                  </button>
                </div>
              </div>
            )}
          </div>
        ))}

        <button
          className="mt-2 px-[14px] py-[6px] rounded-[var(--radius-sm)] text-[12px] font-medium bg-accent text-white hover:bg-accent-hover transition-all"
          onClick={handleAdd}
        >
          + 添加 Provider
        </button>
      </div>

      <div>
        <div className="text-[13px] font-semibold text-text-secondary uppercase tracking-[.3px] mb-3">Fallback 顺序</div>
        <div className="text-[12px] text-text-secondary leading-[1.8]">
          {llmProviders.length === 0 && (
            <div className="text-text-tertiary">添加 Provider 后可配置 Fallback 顺序</div>
          )}
          {llmProviders.map((p, i) => (
            <div key={p.id} className="flex items-center gap-2 py-[6px]">
              <span className="text-accent font-semibold">{i + 1}.</span>
              {p.name}
              {p.isDefault && (
                <span className="text-[10px] px-[6px] py-[2px] bg-success-light text-success rounded-[3px]">默认</span>
              )}
            </div>
          ))}
        </div>
      </div>

      {/* Provider 表单对话框 */}
      {dialogMode && (
        <ProviderFormDialog
          mode={dialogMode}
          provider={editingProvider}
          onClose={() => { setDialogMode(null); setEditingProvider(null); }}
          onSaved={handleDialogSaved}
        />
      )}
    </div>
  );
}
