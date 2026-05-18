import { Icon } from "../common/Icon";
import { useWorkspaceStore } from "../../stores/useWorkspaceStore";
import { useSettingsStore } from "../../stores/useSettingsStore";

interface TopBarProps {
  onToggleHistory: () => void;
  onNewSession: () => void;
}

export function TopBar({ onToggleHistory, onNewSession }: TopBarProps) {
  const { currentWorkspaceId, workspaces } = useWorkspaceStore();
  const { openSettings, llmProviders, activeProviderId } = useSettingsStore();
  const currentWs = workspaces.find((w) => w.id === currentWorkspaceId);
  const activeProvider = llmProviders.find((p) => p.id === activeProviderId);

  const hasProvider = !!activeProvider;
  const statusText = hasProvider ? activeProvider.model : "未连接";
  const statusColor = hasProvider ? "bg-success" : "bg-text-tertiary";

  return (
    <div data-tauri-drag-region className="flex items-center h-[52px] px-4 border-b border-border bg-bg flex-shrink-0 gap-3 z-[100]">
      {/* 工作区选择器 */}
      <div
        className="flex items-center gap-[6px] px-[10px] py-[5px] rounded-[var(--radius-sm)] cursor-pointer transition-colors duration-150 text-[13px] font-medium text-text-secondary whitespace-nowrap hover:bg-bg-sub"
        onClick={() => openSettings("workspace")}
      >
        <span className="w-2 h-2 rounded-full bg-accent" />
        <span>{currentWs?.name ?? "选择工作区"}</span>
        <Icon name="chevron-down" size={14} />
      </div>

      <div className="flex-1" />

      {/* 状态指示器 - 对接实际 LLM Provider 状态 */}
      <div className="flex items-center gap-[6px] text-[11px] text-text-tertiary">
        <span className={`w-[6px] h-[6px] rounded-full ${statusColor}`} />
        <span>{statusText}</span>
      </div>

      {/* 操作按钮 */}
      <div className="flex items-center gap-1">
        <button
          className="topbar-btn"
          title="历史会话"
          onClick={onToggleHistory}
        >
          <Icon name="history" />
        </button>
        <button
          className="topbar-btn"
          title="新建会话"
          onClick={onNewSession}
        >
          <Icon name="plus" />
        </button>
        <button
          className="topbar-btn"
          title="设置"
          onClick={() => openSettings()}
        >
          <Icon name="settings" />
        </button>
      </div>
    </div>
  );
}
