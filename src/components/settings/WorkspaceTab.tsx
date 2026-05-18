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
      <div className="text-[13px] font-semibold text-text-secondary uppercase tracking-[.3px] mb-3">工作区列表</div>

      {workspaces.length === 0 && (
        <div className="text-[13px] text-text-tertiary text-center py-6">
          暂无工作区，请点击下方按钮添加
        </div>
      )}

      {workspaces.map((ws) => (
        <div key={ws.id} className="px-3 py-3 border border-border rounded-[var(--radius-md)] mb-2 transition-colors duration-150 hover:border-[#D0D3D9]">
          <div className="flex items-center gap-2 mb-2">
            <span className="font-semibold text-[13px]">{ws.name}</span>
            {ws.id === currentWorkspaceId && (
              <span className="text-[11px] text-success">当前</span>
            )}
            {ws.id !== currentWorkspaceId && (
              <button
                className="ml-1 px-2 py-[2px] rounded-[var(--radius-sm)] text-[11px] font-medium bg-bg-sub text-text-secondary hover:bg-bg-hover transition-all"
                onClick={() => handleSwitch(ws.id)}
              >
                切换
              </button>
            )}
            <div className="ml-auto flex gap-1">
              <button
                className="px-2 py-[3px] rounded-[var(--radius-sm)] text-[11px] font-medium bg-bg-sub text-error hover:bg-red-50 transition-all"
                onClick={() => { setRemovingId(ws.id); setRemoveError(null); }}
              >
                移除
              </button>
            </div>
          </div>
          <div className="font-mono text-[11px] text-text-tertiary">
            {ws.path} &nbsp;|&nbsp; 创建于 {new Date(ws.createdAt).toLocaleDateString("zh-CN")}
          </div>

          {/* 移除确认 */}
          {removingId === ws.id && (
            <div className="mt-2 pt-2 border-t border-border-light">
              <div className="text-[12px] text-text-secondary mb-2">确定要移除此工作区吗？（不会删除本地文件）</div>
              {removeError && (
                <div className="text-[11px] text-error mb-2">{removeError}</div>
              )}
              <div className="flex gap-2">
                <button
                  className="px-3 py-[4px] rounded-[var(--radius-sm)] text-[11px] font-medium bg-error text-white hover:bg-red-600 transition-all"
                  onClick={() => handleRemove(ws.id)}
                >
                  确认移除
                </button>
                <button
                  className="px-3 py-[4px] rounded-[var(--radius-sm)] text-[11px] font-medium bg-bg-sub text-text-secondary hover:bg-bg-hover transition-all"
                  onClick={() => { setRemovingId(null); setRemoveError(null); }}
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
        onClick={() => setShowAddDialog(true)}
      >
        + 添加工作区
      </button>

      {/* 添加工作区对话框 */}
      {showAddDialog && (
        <AddWorkspaceDialog
          onClose={() => setShowAddDialog(false)}
          onSaved={handleAddSaved}
        />
      )}
    </div>
  );
}
