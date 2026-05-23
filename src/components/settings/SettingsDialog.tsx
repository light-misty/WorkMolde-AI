import { useEffect } from "react";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { Icon } from "../common/Icon";
import { LLMConfigTab } from "./LLMConfig";
import { WorkspaceTab } from "./WorkspaceTab";
import { SkillsTab } from "./SkillsTab";
import { TemplatesTab } from "./TemplatesTab";
import { AppearanceTab } from "./AppearanceTab";
import { ShortcutsTab } from "./ShortcutsTab";
import { GeneralTab } from "./GeneralTab";
import { TokenUsageTab } from "./TokenUsageTab";

const tabs = [
  { id: "llm" as const, label: "LLM 配置", icon: "settings" },
  { id: "workspace" as const, label: "工作区管理", icon: "folder" },
  { id: "skill" as const, label: "Skills 管理", icon: "tool" },
  { id: "template" as const, label: "Prompt 模板", icon: "template" },
  { id: "usage" as const, label: "Token 用量", icon: "chart" },
  { id: "appearance" as const, label: "外观设置", icon: "theme" },
  { id: "shortcuts" as const, label: "快捷键", icon: "keyboard" },
  { id: "general" as const, label: "通用设置", icon: "code" },
];

export function SettingsDialog() {
  const { isSettingsOpen, activeSettingsTab, closeSettings, setActiveTab } = useSettingsStore();

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
      case "skill": return <SkillsTab />;
      case "template": return <TemplatesTab />;
      case "usage": return <TokenUsageTab />;
      case "appearance": return <AppearanceTab />;
      case "shortcuts": return <ShortcutsTab />;
      case "general": return <GeneralTab />;
    }
  };

  return (
    <div
      className="fixed inset-0 bg-overlay z-[300] flex items-center justify-center animate-fade-in"
      onClick={(e) => { if (e.target === e.currentTarget) closeSettings(); }}
    >
      <div
        className="settings-modal"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="settings-header">
          <h2 className="settings-title">设置</h2>
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
          .settings-modal {
            width: 760px;
            max-height: 80vh;
            background: var(--color-bg-elevated);
            border-radius: var(--radius-xl);
            box-shadow: var(--shadow-xl);
            display: flex;
            flex-direction: column;
            overflow: hidden;
            animation: scaleIn 0.2s ease;
          }
          .settings-header {
            padding: 20px 24px;
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
            width: 180px;
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
            padding: 24px;
          }
        `}</style>
      </div>
    </div>
  );
}
