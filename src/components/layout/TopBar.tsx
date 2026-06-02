import { Icon } from "../common/Icon";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { WindowControls } from "./WindowControls";
import { WorkspaceSelector } from "./WorkspaceSelector";

interface TopBarProps {
  onToggleHistory: () => void;
  onNewSession: () => void;
}

export function TopBar({ onToggleHistory, onNewSession }: TopBarProps) {
  const { openSettings, llmProviders, activeProviderId } = useSettingsStore();
  const activeProvider = llmProviders.find((p) => p.id === activeProviderId);

  const hasProvider = !!activeProvider;
  const statusText = hasProvider ? activeProvider.model : "未连接";
  const statusColor = hasProvider ? "bg-success" : "bg-text-tertiary";

  return (
    <div role="banner" data-tauri-drag-region className="flex items-center h-[52px] pr-4 border-b border-border bg-bg flex-shrink-0 gap-3 z-[100]" style={{ paddingLeft: '24px' }}>
      {/* 工作区选择器 */}
      <WorkspaceSelector />

      <div className="flex-1" />

      {/* 状态指示器 - 对接实际 LLM Provider 状态 */}
      <div className="flex items-center gap-[6px] text-[11px] text-text-tertiary" aria-label={hasProvider ? "已连接" : "未连接"}>
        <span className={`w-[6px] h-[6px] rounded-full ${statusColor}`} />
        <span>{statusText}</span>
      </div>

      {/* 操作按钮 */}
      <div className="flex items-center gap-1" role="toolbar" aria-label="操作工具栏">
        <button
          className="topbar-btn"
          title="历史会话"
          aria-label="历史会话"
          onClick={onToggleHistory}
        >
          <Icon name="history" />
        </button>
        <button
          className="topbar-btn"
          title="新建会话"
          aria-label="新建会话"
          onClick={onNewSession}
        >
          <Icon name="plus" />
        </button>
        <button
          className="topbar-btn"
          title="设置"
          aria-label="设置"
          onClick={() => openSettings()}
        >
          <Icon name="settings" />
        </button>
      </div>

      {/* 窗口控制按钮 */}
      <WindowControls />
    </div>
  );
}
