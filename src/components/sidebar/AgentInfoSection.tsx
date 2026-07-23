import { useState } from "react";
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from "../../stores/useSettingsStore";
import { Icon } from "../common/Icon";

export function AgentInfoSection() {
  const { t } = useTranslation();
  const { settings, llmProviders, activeProviderId, preferredProviderId, updateSettings, openSettings } = useSettingsStore();

  // 默认收缩状态
  const [open, setOpen] = useState(false);

  // 确认级别选项（移入组件内部以使用 t() 翻译）
  const confirmationLevelOptions: { value: string; label: string }[] = [
    { value: "always", label: t('agentInfo.confirmAlways') },
    { value: "deleteOnly", label: t('agentInfo.confirmEditOnly') },
    { value: "never", label: t('agentInfo.confirmNever') },
  ];
  // 优先显示用户选择的首选 Provider，回退到默认 Provider，与 InputArea/TopBar 显示逻辑一致
  const activeProvider = llmProviders.find((p) => p.id === (preferredProviderId || activeProviderId));

  return (
    <div className="agent-info-section">
      {/* 标题栏：样式与会话列表标题一致，可点击折叠 */}
      <div
        className="agent-info-header"
        style={{ borderRadius: "var(--radius-sm)" }}
        role="button"
        aria-expanded={open}
        aria-label={t('agentInfo.sectionTitle')}
        tabIndex={0}
        onClick={() => setOpen(!open)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setOpen(!open);
          }
        }}
      >
        <div className="agent-info-title">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="agent-info-title-icon">
            <circle cx="12" cy="8" r="4"/>
            <path d="M20 21a8 8 0 1 0-16 0"/>
          </svg>
          <span>{t('agentInfo.sectionTitle')}</span>
        </div>
        {/* 折叠箭头：默认隐藏，鼠标悬停时在右侧显示 */}
        <span
          className="agent-info-chevron"
          style={{ transform: open ? "rotate(0deg)" : "rotate(-90deg)" }}
        >
          <Icon name="chevron-down" size={12} />
        </span>
      </div>

      {/* 可折叠内容区 */}
      <div
        className="agent-info-body"
        role="region"
        aria-label={t('agentInfo.sectionTitle')}
        style={{
          maxHeight: open ? "2000px" : "0px",
          opacity: open ? 1 : 0,
        }}
      >
        <div className="agent-info-content" role="region" aria-label={t('agentInfo.sectionTitle')}>
          {/* 当前模型 */}
          <div className="ai-field">
            <span className="ai-field-label">{t('agentInfo.currentModel')}</span>
            <div className={`ai-model-badge ${activeProvider ? "online" : "offline"}`} aria-label={activeProvider ? t('agentInfo.modelConnected') : t('agentInfo.modelDisconnected')}>
              <span className="ai-status-dot" />
              <span className="ai-model-name">
                {activeProvider?.model ?? t('agentInfo.notConfigured')}
              </span>
            </div>
          </div>

          {/* 未配置 Provider 时的引导提示 */}
          {!activeProvider && (
            <button className="ai-setup-hint" onClick={() => openSettings("llm")}>
              <Icon name="settings" size={12} />
              <span>{t('agentInfo.configureLLM')}</span>
            </button>
          )}

          {/* 作者信息 */}
          <div className="ai-field">
            <span className="ai-field-label">{t('agentInfo.authorInfo')}</span>
            <div className="ai-field-author-info">
              <span className="ai-field-author-summary" title={t('agentInfo.authorInfoTooltip')}>
                {settings.general.authorName || t('agentInfo.notSet')}
              </span>
              <button
                className="ai-field-edit-btn"
                aria-label={t('agentInfo.editAuthorInfo')}
                onClick={() => openSettings("general")}
                title={t('agentInfo.editAuthorInfo')}
              >
                <Icon name="edit" size={14} />
              </button>
            </div>
          </div>

          {/* 确认级别 */}
          <div className="ai-field">
            <span className="ai-field-label">{t('agentInfo.confirmLevel')}</span>
            <select
              className="ai-field-select"
              aria-label={t('agentInfo.confirmLevel')}
              value={settings.general.confirmationLevel}
              onChange={(e) => updateSettings({ general: { confirmationLevel: e.target.value as "always" | "deleteOnly" | "never" } })}
            >
              {confirmationLevelOptions.map((opt) => (
                <option key={opt.value} value={opt.value}>{opt.label}</option>
              ))}
            </select>
          </div>

        </div>
      </div>

      <style>{`
        .agent-info-section {
          display: flex;
          flex-direction: column;
          flex-shrink: 0;
          border-radius: var(--radius-sm);
          overflow: hidden;
          background: var(--color-bg-sub);
          margin: 4px 8px;
        }
        /* 标题栏样式与会话列表标题（session-list-header / session-list-title）保持一致 */
        .agent-info-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 8px 12px;
          cursor: pointer;
          user-select: none;
          transition: background 0.15s;
        }
        .agent-info-header:hover {
          background: var(--color-bg-hover);
        }
        .agent-info-title {
          display: flex;
          align-items: center;
          gap: 6px;
          font-size: 14px;
          font-weight: 400;
          color: var(--color-text-primary);
        }
        .agent-info-title-icon {
          flex-shrink: 0;
          color: var(--color-text-primary);
        }
        /* 折叠箭头：默认隐藏，鼠标悬停时在右侧显示 */
        .agent-info-chevron {
          display: flex;
          align-items: center;
          justify-content: center;
          color: var(--color-text-quaternary);
          transition: transform 0.2s, opacity 0.15s;
          flex-shrink: 0;
          opacity: 0;
        }
        .agent-info-header:hover .agent-info-chevron {
          opacity: 1;
        }
        .agent-info-body {
          overflow: hidden;
          transition: max-height 0.25s ease, opacity 0.2s ease;
        }
        .agent-info-content {
          display: flex;
          flex-direction: column;
          gap: 2px;
          padding: 4px 12px 10px;
        }
        .ai-field {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 8px;
          padding: 3px 0;
        }
        .ai-field-label {
          font-size: 13px;
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
          font-size: 13px;
          font-weight: 500;
          transition: background 0.2s;
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
          box-shadow: 0 0 4px var(--color-success-bg);
        }
        .ai-model-name {
          font-weight: 500;
          color: var(--color-text-primary);
        }
        .ai-field-value {
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-primary);
        }
        .ai-field-value-btn {
          display: inline-flex;
          align-items: center;
          gap: 4px;
          padding: 2px 8px;
          border-radius: var(--radius-sm);
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-primary);
          border: 1px solid transparent;
          transition: all 0.15s;
          cursor: pointer;
          background: none;
        }
        .ai-field-value-btn:hover {
          border-color: var(--color-border);
          background: var(--color-bg);
          color: var(--color-accent);
        }
        .ai-field-edit {
          font-size: 13px;
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
        .ai-field-edit:focus-visible {
          outline: none;
        }
        .ai-field-select {
          font-size: 13px;
          font-weight: 500;
          padding: 2px 8px;
          border: 1px solid transparent;
          border-radius: var(--radius-sm);
          background: none;
          color: var(--color-text-primary);
          cursor: pointer;
          transition: all 0.15s;
          outline: none;
          -webkit-appearance: none;
          appearance: none;
        }
        .ai-field-select:hover {
          border-color: var(--color-border);
          background: var(--color-bg);
          color: var(--color-accent);
        }
        .ai-field-select option {
          color: var(--color-text-primary);
          background: var(--color-bg);
        }
        .ai-field-select:focus {
          border-color: var(--color-accent);
          box-shadow: 0 0 0 2px var(--color-accent-lighter);
        }
        .ai-field-author-info {
          display: flex;
          align-items: center;
          gap: 4px;
          min-width: 0;
          flex: 1;
          justify-content: flex-end;
        }
        .ai-field-author-summary {
          font-size: 12px;
          color: var(--color-text-secondary);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
          max-width: 160px;
        }
        .ai-field-edit-btn {
          display: inline-flex;
          align-items: center;
          justify-content: center;
          width: 24px;
          height: 24px;
          border-radius: var(--radius-sm);
          color: var(--color-text-quaternary);
          background: none;
          border: 1px solid transparent;
          cursor: pointer;
          transition: all 0.15s;
          flex-shrink: 0;
          padding: 0;
        }
        .ai-field-edit-btn:hover {
          color: var(--color-accent);
          border-color: var(--color-border);
          background: var(--color-bg);
        }
        .ai-setup-hint {
          display: flex;
          align-items: center;
          gap: 6px;
          padding: 6px 10px;
          border-radius: var(--radius-sm);
          background: var(--color-accent-bg);
          border: 1px solid var(--color-accent-light);
          font-size: 13px;
          color: var(--color-accent);
          cursor: pointer;
          transition: all 0.2s;
          width: 100%;
        }
        .ai-setup-hint:hover {
          background: var(--color-accent-light);
          border-color: var(--color-accent);
        }
      `}</style>
    </div>
  );
}
