import { useTranslation } from 'react-i18next';
import { Icon } from "../common/Icon";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { WindowControls } from "./WindowControls";
import { WorkspaceSelector } from "./WorkspaceSelector";
import type { ThemeMode } from "../../types";

interface TopBarProps {
  onNewSession: () => void;
}

export function TopBar({ onNewSession }: TopBarProps) {
  const { t } = useTranslation();
  const { openSettings, llmProviders, activeProviderId, settings, updateSettings } = useSettingsStore();
  const activeProvider = llmProviders.find((p) => p.id === activeProviderId);

  const hasProvider = !!activeProvider;
  const statusText = hasProvider ? activeProvider.model : t('topBar.disconnected');
  const statusColor = hasProvider ? "bg-success" : "bg-text-tertiary";

  // 判断当前是否处于深色模式（考虑 system 模式下的系统偏好）
  const isDarkMode = (() => {
    const { themeMode } = settings.appearance;
    if (themeMode === "dark") return true;
    if (themeMode === "system") return window.matchMedia("(prefers-color-scheme: dark)").matches;
    return false;
  })();

  // 切换主题：深色 → 浅色，浅色 → 深色
  const toggleTheme = () => {
    const nextMode: ThemeMode = isDarkMode ? "light" : "dark";
    updateSettings({ appearance: { themeMode: nextMode } });
  };

  return (
    <div role="banner" data-tauri-drag-region className="flex items-center h-[52px] pr-4 bg-bg-sub flex-shrink-0 gap-3 z-[100]" style={{ paddingLeft: '24px' }}>
      {/* 工作区选择器 */}
      <WorkspaceSelector />

      <div className="flex-1" />

      {/* 状态指示器 - 对接实际 LLM Provider 状态 */}
      <div className="flex items-center gap-[6px] text-[11px] text-text-tertiary" aria-label={hasProvider ? t('topBar.connected') : t('topBar.disconnected')}>
        <span className={`w-[6px] h-[6px] rounded-full ${statusColor}`} />
        <span>{statusText}</span>
      </div>

      {/* 操作按钮 */}
      <div className="flex items-center gap-1" role="toolbar" aria-label={t('topBar.toolbar')}>
        <button
          className="topbar-btn"
          title={isDarkMode ? t('topBar.switchToLight') : t('topBar.switchToDark')}
          aria-label={isDarkMode ? t('topBar.switchToLight') : t('topBar.switchToDark')}
          onClick={toggleTheme}
        >
          <Icon name={isDarkMode ? "theme" : "moon"} />
        </button>
        <button
          className="topbar-btn"
          title={t('topBar.newSession')}
          aria-label={t('topBar.newSession')}
          onClick={onNewSession}
        >
          <Icon name="plus" />
        </button>
        <button
          className="topbar-btn"
          title={t('topBar.settings')}
          aria-label={t('topBar.settings')}
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
