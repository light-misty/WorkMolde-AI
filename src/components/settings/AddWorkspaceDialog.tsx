import { useState } from "react";
import * as tauriCmd from "../../services/tauri";

interface AddWorkspaceDialogProps {
  onClose: () => void;
  onSaved: () => void;
}

export function AddWorkspaceDialog({ onClose, onSaved }: AddWorkspaceDialogProps) {
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSave = async () => {
    if (!path.trim()) {
      setError("请输入工作区路径");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await tauriCmd.addWorkspace(path.trim(), name.trim() || undefined);
      onSaved();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : typeof err === "string" ? err : "添加工作区失败";
      setError(msg);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div
      className="fixed inset-0 bg-black/35 z-[400] flex items-center justify-center animate-fade-in"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div
        className="w-[480px] bg-bg rounded-[var(--radius-lg)] shadow-lg flex flex-col overflow-hidden animate-slide-up"
        onClick={(e) => e.stopPropagation()}
      >
        {/* 标题栏 */}
        <div className="flex items-center px-6 py-4 border-b border-border gap-3 flex-shrink-0">
          <h3 className="text-[15px] font-bold flex-1">添加工作区</h3>
          <button
            className="w-[28px] h-[28px] flex items-center justify-center rounded-[var(--radius-sm)] transition-colors duration-150 text-text-secondary hover:bg-bg-sub"
            onClick={onClose}
          >
            x
          </button>
        </div>

        {/* 表单内容 */}
        <div className="flex-1 overflow-y-auto px-6 py-5 space-y-4">
          {/* 工作区路径 */}
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">工作区路径 *</label>
            <input
              className="w-full px-3 py-2 border border-border rounded-[var(--radius-sm)] text-[13px] font-mono transition-colors focus:border-accent focus:outline-none"
              placeholder="D:\Documents\ProjectDocs"
              value={path}
              onChange={(e) => setPath(e.target.value)}
            />
            <div className="text-[11px] text-text-tertiary mt-1">
              Agent 将在此目录下操作文档文件
            </div>
          </div>

          {/* 工作区名称 */}
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">工作区名称（可选）</label>
            <input
              className="w-full px-3 py-2 border border-border rounded-[var(--radius-sm)] text-[13px] transition-colors focus:border-accent focus:outline-none"
              placeholder="留空则使用目录名"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>

          {/* 错误信息 */}
          {error && (
            <div className="px-3 py-2 rounded-[var(--radius-sm)] text-[12px] bg-red-50 text-red-700 border border-red-200">
              {error}
            </div>
          )}
        </div>

        {/* 底部操作栏 */}
        <div className="flex items-center justify-end gap-2 px-6 py-4 border-t border-border flex-shrink-0">
          <button
            className="px-4 py-[6px] rounded-[var(--radius-sm)] text-[12px] font-medium bg-bg-sub text-text-secondary hover:bg-bg-hover transition-all"
            onClick={onClose}
          >
            取消
          </button>
          <button
            className="px-4 py-[6px] rounded-[var(--radius-sm)] text-[12px] font-medium bg-accent text-white hover:bg-accent-hover transition-all disabled:opacity-50"
            onClick={handleSave}
            disabled={saving}
          >
            {saving ? "添加中..." : "添加"}
          </button>
        </div>
      </div>
    </div>
  );
}
