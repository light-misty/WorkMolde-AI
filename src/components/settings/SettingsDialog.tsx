import { useEffect } from "react";
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from "../../stores/useSettingsStore";
import { Icon } from "../common/Icon";
import { LLMConfigTab } from "./LLMConfig";
import { WorkspaceTab } from "./WorkspaceTab";
import { HandlersTab } from "./HandlersTab";
import { TemplatesTab } from "./TemplatesTab";
import { AppearanceTab } from "./AppearanceTab";
import { ShortcutsTab } from "./ShortcutsTab";
import { GeneralTab } from "./GeneralTab";
import { HelpTab } from "./HelpTab";

export function SettingsDialog() {
  const { t } = useTranslation();
  const { isSettingsOpen, activeSettingsTab, closeSettings, setActiveTab } = useSettingsStore();

  // 将 tabs 数组移到组件内部，以便使用 t() 函数
  const tabs = [
    { id: "llm" as const, label: t('settings.tabs.llm'), icon: "settings" },
    { id: "workspace" as const, label: t('settings.tabs.workspace'), icon: "folder" },
    { id: "handler" as const, label: t('settings.tabs.handler'), icon: "tool" },
    { id: "template" as const, label: t('settings.tabs.template'), icon: "template" },
    { id: "appearance" as const, label: t('settings.tabs.appearance'), icon: "theme" },
    { id: "shortcuts" as const, label: t('settings.tabs.shortcuts'), icon: "keyboard" },
    { id: "general" as const, label: t('settings.tabs.general'), icon: "code" },
    { id: "help" as const, label: t('settings.tabs.help'), icon: "info" },
  ];

  useEffect(() => {
    if (!isSettingsOpen) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") closeSettings();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [isSettingsOpen, closeSettings]);

  if (!isSettingsOpen) return null;

  const renderTab = () => {
    switch (activeSettingsTab) {
      case "llm": return <LLMConfigTab />;
      case "workspace": return <WorkspaceTab />;
      case "handler": return <HandlersTab />;
      case "template": return <TemplatesTab />;
      case "appearance": return <AppearanceTab />;
      case "shortcuts": return <ShortcutsTab />;
      case "general": return <GeneralTab />;
      case "help": return <HelpTab />;
    }
  };

  return (
    <div className="settings-page">
      <div className="settings-header">
        <h2 className="settings-title">{t('settings.title')}</h2>
        <button
          className="settings-close-btn"
          onClick={closeSettings}
        >
          <Icon name="close" size={18} />
        </button>
      </div>

      <div className="settings-body">
        <div className="settings-nav">
          {tabs.map((tab) => (
            <div
              key={tab.id}
              className={`settings-nav-item ${activeSettingsTab === tab.id ? "active" : ""}`}
              onClick={() => setActiveTab(tab.id)}
            >
              <span className="settings-nav-icon">
                <Icon name={tab.icon as any} size={16} />
              </span>
              <span className="settings-nav-label">{tab.label}</span>
            </div>
          ))}
        </div>

        <div className="settings-content">
          {renderTab()}
        </div>
      </div>

      <style>{`
        .settings-page {
          position: fixed;
          inset: 0;
          z-index: 300;
          background: var(--color-bg-elevated);
          display: flex;
          flex-direction: column;
          animation: fadeIn 0.2s ease;
        }
        .settings-header {
          padding: 16px 24px;
          border-bottom: 1px solid var(--color-border-light);
          display: flex;
          align-items: center;
          gap: 12px;
          flex-shrink: 0;
        }
        .settings-title {
          font-size: 16px;
          font-weight: 700;
          color: var(--color-text-primary);
          flex: 1;
        }
        .settings-close-btn {
          width: 32px;
          height: 32px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-sm);
          color: var(--color-text-secondary);
          transition: all 0.15s;
        }
        .settings-close-btn:hover {
          background: var(--color-bg-sub);
          color: var(--color-text-primary);
        }
        .settings-body {
          display: flex;
          flex: 1;
          overflow: hidden;
        }
        .settings-nav {
          width: 200px;
          flex-shrink: 0;
          border-right: 1px solid var(--color-border-light);
          padding: 12px 8px;
          overflow-y: auto;
          background: var(--color-bg-sub);
        }
        .settings-nav-item {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 10px 12px;
          border-radius: var(--radius-sm);
          font-size: 13px;
          color: var(--color-text-secondary);
          cursor: pointer;
          transition: all 0.15s;
          margin-bottom: 2px;
        }
        .settings-nav-item:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
        }
        .settings-nav-item.active {
          background: var(--color-accent-light);
          color: var(--color-accent);
          font-weight: 500;
        }
        .settings-nav-icon {
          width: 18px;
          height: 18px;
          display: flex;
          align-items: center;
          justify-content: center;
          flex-shrink: 0;
        }
        .settings-nav-label {
          flex: 1;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .settings-content {
          flex: 1;
          overflow-y: auto;
          padding: 24px 32px;
        }
        .settings-section {
          margin-bottom: 24px;
        }
        .settings-section:last-child {
          margin-bottom: 0;
        }
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
        .setting-row {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 10px 12px;
          border-bottom: 1px solid var(--color-border-light);
          gap: 16px;
        }
        .setting-row:last-child {
          border-bottom: none;
        }
        .setting-info {
          flex: 1;
          min-width: 0;
        }
        .setting-label {
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-primary);
        }
        .setting-desc {
          font-size: 11px;
          color: var(--color-text-quaternary);
          margin-top: 2px;
        }
        .log-path-hint {
          display: block;
          margin-top: 2px;
          word-break: break-all;
          font-family: monospace;
          font-size: 10px;
          color: var(--color-text-quaternary);
          opacity: 0.8;
        }
      `}</style>
    </div>
  );
}
