import { useSettingsStore } from "../../stores/useSettingsStore";
import type { ThemeMode } from "../../types";

export function AppearanceTab() {
  const { settings, updateSettings } = useSettingsStore();

  return (
    <div>
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">主题</span>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">主题模式</div>
            <div className="setting-desc">选择应用的配色方案</div>
          </div>
          <div className="theme-switcher">
            {([
              { value: "light" as ThemeMode, label: "浅色" },
              { value: "dark" as ThemeMode, label: "深色" },
              { value: "system" as ThemeMode, label: "跟随系统" },
            ]).map((opt) => (
              <button
                key={opt.value}
                className={`theme-btn ${settings.appearance.themeMode === opt.value ? "active" : ""}`}
                onClick={() => updateSettings({ appearance: { themeMode: opt.value } })}
              >
                {opt.label}
              </button>
            ))}
          </div>
        </div>
      </div>

      <style>{`
        .theme-switcher {
          display: flex;
          gap: 0;
          border: 1px solid var(--color-border);
          border-radius: var(--radius-sm);
          overflow: hidden;
          flex-shrink: 0;
        }
        .theme-btn {
          padding: 6px 14px;
          font-size: 12px;
          font-weight: 500;
          color: var(--color-text-secondary);
          background: var(--color-bg);
          border: none;
          border-right: 1px solid var(--color-border);
          cursor: pointer;
          transition: all 0.15s;
          white-space: nowrap;
        }
        .theme-btn:last-child {
          border-right: none;
        }
        .theme-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
        }
        .theme-btn.active {
          background: var(--color-accent);
          color: #fff;
        }
        .theme-btn.active:hover {
          background: var(--color-accent-hover);
        }
      `}</style>
    </div>
  );
}
