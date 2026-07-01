import { useTranslation } from 'react-i18next';
import { useSettingsStore } from "../../stores/useSettingsStore";
import { WindowControls } from "./WindowControls";

interface TopBarProps {
  sidebarVisible: boolean;
  onToggleSidebar: () => void;
}

export function TopBar({ sidebarVisible, onToggleSidebar }: TopBarProps) {
  const { t } = useTranslation();
  const { llmProviders, activeProviderId, preferredProviderId } = useSettingsStore();
  // 优先显示用户选择的首选 Provider，回退到默认 Provider，与 InputArea 发送逻辑一致
  const activeProvider = llmProviders.find((p) => p.id === (preferredProviderId || activeProviderId));

  const hasProvider = !!activeProvider;
  const statusText = hasProvider ? activeProvider.model : t('topBar.disconnected');
  const statusColor = hasProvider ? "bg-success" : "bg-text-tertiary";

  return (
    <div role="banner" className="flex items-center h-[52px] bg-bg-sub flex-shrink-0 z-[100]" style={{ position: 'relative' }}>
      <div data-tauri-drag-region className="flex items-center flex-1 self-stretch gap-3" style={{ paddingLeft: '24px', paddingRight: '144px' }}>
        {/* 侧边栏收缩/展开按钮 */}
        <button
          type="button"
          className="topbar-btn"
          onClick={onToggleSidebar}
          title={sidebarVisible ? t('topBar.collapseSidebar') : t('topBar.expandSidebar')}
          aria-label={sidebarVisible ? t('topBar.collapseSidebar') : t('topBar.expandSidebar')}
        >
          {sidebarVisible ? (
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
              <line x1="15" y1="3" x2="15" y2="21"/>
              <polyline points="11 10 8 13 11 16"/>
            </svg>
          ) : (
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
              <line x1="9" y1="3" x2="9" y2="21"/>
              <polyline points="13 10 16 13 13 16"/>
            </svg>
          )}
        </button>

        {/* 左侧留白，工作区选择器已移至输入框左下角 */}
        <div className="flex-1" />

        {/* 状态指示器 - 对接实际 LLM Provider 状态 */}
        <div className="flex items-center gap-[6px] text-[11px] text-text-tertiary" aria-label={hasProvider ? t('topBar.connected') : t('topBar.disconnected')}>
          <span className={`w-[6px] h-[6px] rounded-full ${statusColor}`} />
          <span>{statusText}</span>
        </div>
      </div>

      {/* 窗口控制按钮 */}
      <WindowControls />
    </div>
  );
}
