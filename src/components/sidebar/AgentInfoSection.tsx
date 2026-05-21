import { useState } from "react";
import { SidebarSection } from "../layout/Sidebar";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { Icon } from "../common/Icon";

const confirmationLevelLabels: Record<string, string> = {
  always: "全部需确认",
  editOnly: "仅编辑操作确认",
  never: "全部自动确认",
};

export function AgentInfoSection() {
  const { settings, llmProviders, activeProviderId, updateSettings } = useSettingsStore();
  const activeProvider = llmProviders.find((p) => p.id === activeProviderId);

  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState(settings.general.authorName);

  const handleSave = () => {
    if (editValue.trim()) {
      updateSettings({ general: { authorName: editValue.trim() } });
    }
    setEditing(false);
  };

  return (
    <SidebarSection title="Agent 信息">
      <div className="ai-grid">
        {/* 当前模型 */}
        <div className="ai-field">
          <span className="ai-field-label">当前模型</span>
          <div className={`ai-model-badge ${activeProvider ? "online" : "offline"}`}>
            <span className="ai-status-dot" />
            <span className="ai-model-name">
              {activeProvider?.model ?? "未配置"}
            </span>
          </div>
        </div>

        {/* 作者名 */}
        <div className="ai-field">
          <span className="ai-field-label">作者名</span>
          {editing ? (
            <input
              className="ai-field-edit"
              value={editValue}
              onChange={(e) => setEditValue(e.target.value)}
              onBlur={handleSave}
              onKeyDown={(e) => { if (e.key === "Enter") handleSave(); }}
              autoFocus
            />
          ) : (
            <button
              className="ai-field-value-btn"
              onClick={() => { setEditValue(settings.general.authorName); setEditing(true); }}
            >
              <span>{settings.general.authorName || "未设置"}</span>
              <Icon name="code" size={12} />
            </button>
          )}
        </div>

        {/* 确认级别 */}
        <div className="ai-field">
          <span className="ai-field-label">确认级别</span>
          <span className="ai-field-value">
            {confirmationLevelLabels[settings.general.confirmationLevel] ?? settings.general.confirmationLevel}
          </span>
        </div>
      </div>

      <style>{`
        .ai-grid {
          display: flex;
          flex-direction: column;
          gap: 10px;
        }
        .ai-field {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 8px;
          padding: 6px 0;
        }
        .ai-field-label {
          font-size: 12px;
          color: var(--color-text-quaternary);
          flex-shrink: 0;
        }
        .ai-model-badge {
          display: flex;
          align-items: center;
          gap: 6px;
          padding: 3px 10px;
          background: var(--color-bg-sub);
          border-radius: var(--radius-sm);
          font-size: 12px;
          font-weight: 500;
          transition: background 0.2s;
        }
        .ai-model-badge.online {
          background: var(--color-success-bg);
        }
        .ai-status-dot {
          width: 6px;
          height: 6px;
          border-radius: 50%;
          flex-shrink: 0;
          background: var(--color-text-quaternary);
          transition: background 0.3s, box-shadow 0.3s;
        }
        .ai-model-badge.online .ai-status-dot {
          background: var(--color-success);
          box-shadow: 0 0 4px rgba(52, 199, 36, 0.4);
        }
        .ai-model-name {
          font-weight: 500;
          color: var(--color-text-primary);
        }
        .ai-field-value {
          font-size: 12px;
          font-weight: 500;
          color: var(--color-text-primary);
        }
        .ai-field-value-btn {
          display: inline-flex;
          align-items: center;
          gap: 4px;
          padding: 2px 8px;
          border-radius: var(--radius-sm);
          font-size: 12px;
          font-weight: 500;
          color: var(--color-text-primary);
          border: 1px solid transparent;
          transition: all 0.2s;
          cursor: pointer;
          background: none;
        }
        .ai-field-value-btn:hover {
          border-color: var(--color-border);
          background: var(--color-bg);
          color: var(--color-accent);
        }
        .ai-field-value-btn:hover svg {
          opacity: 1;
        }
        .ai-field-value-btn svg {
          opacity: 0;
          transition: opacity 0.2s;
          color: var(--color-text-quaternary);
        }
        .ai-field-edit {
          font-size: 12px;
          font-weight: 500;
          padding: 2px 8px;
          border: 1.5px solid var(--color-accent);
          border-radius: var(--radius-sm);
          width: 120px;
          background: var(--color-bg);
          box-shadow: 0 0 0 3px var(--color-accent-lighter);
          transition: all 0.2s;
          outline: none;
          color: var(--color-text-primary);
        }
      `}</style>
    </SidebarSection>
  );
}
