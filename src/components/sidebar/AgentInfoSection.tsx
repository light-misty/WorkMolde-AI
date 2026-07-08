import { useState } from "react";
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from "../../stores/useSettingsStore";
import { useWorkflowStore } from "../../stores/useWorkflowStore";
import { Icon } from "../common/Icon";

/** 格式化 Token 数量为可读字符串 */
function formatTokens(tokens: number): string {
  if (tokens >= 1_000_000) {
    return `${(tokens / 1_000_000).toFixed(1)}M`;
  }
  if (tokens >= 1000) {
    return `${(tokens / 1000).toFixed(1)}K`;
  }
  return String(tokens);
}

/** 根据使用百分比返回对应的显示信息 */
function getUsageInfo(usagePercent: number, t: (key: string) => string): { label: string; color: string } {
  if (usagePercent >= 95) {
    return { label: t('contextWindow.approachingLimit'), color: "var(--color-error)" };
  } else if (usagePercent >= 80) {
    return { label: t('contextWindow.normal'), color: "var(--color-warning)" };
  } else {
    return { label: t('contextWindow.normal'), color: "var(--color-success)" };
  }
}

/** 上下文各部分定义：key（用于翻译）、颜色变量名、对应字段 */
const CONTEXT_SECTIONS = [
  { key: "system", labelKey: "contextWindow.systemPrompt", colorVar: "--color-context-system" },
  { key: "functions", labelKey: "contextWindow.toolDefinitions", colorVar: "--color-context-functions" },
  { key: "history", labelKey: "contextWindow.conversationHistory", colorVar: "--color-context-history" },
  { key: "response", labelKey: "contextWindow.llmResponse", colorVar: "--color-context-response" },
] as const;

/** 缓存命中率文字显示 */
function CacheHitRateBar({ hitRate }: { hitRate: number }) {
  const { t } = useTranslation();
  const percent = Math.round(hitRate * 100);
  const color =
    percent >= 70
      ? "var(--color-success)"
      : percent >= 40
        ? "var(--color-warning)"
        : "var(--color-error)";

  return (
    <div className="ai-cache-simple">
      <span className="ai-cache-label">{t('contextWindow.cacheHitRate')}</span>
      <span className="ai-cache-value" style={{ color }}>{percent}%</span>
    </div>
  );
}

export function AgentInfoSection() {
  const { t } = useTranslation();
  const { settings, llmProviders, activeProviderId, preferredProviderId, updateSettings, openSettings } = useSettingsStore();
  const { contextUsage } = useWorkflowStore();

  // 默认收缩状态
  const [open, setOpen] = useState(false);

  // 确认级别选项（移入组件内部以使用 t() 翻译）
  const confirmationLevelOptions: { value: string; label: string }[] = [
    { value: "always", label: t('agentInfo.confirmAlways') },
    { value: "editOnly", label: t('agentInfo.confirmEditOnly') },
    { value: "never", label: t('agentInfo.confirmNever') },
  ];
  // 优先显示用户选择的首选 Provider，回退到默认 Provider，与 InputArea/TopBar 显示逻辑一致
  const activeProvider = llmProviders.find((p) => p.id === (preferredProviderId || activeProviderId));

  // 计算上下文使用数据
  // 仅当后端返回了实际的上下文使用数据时才显示，新会话（无消息）不显示
  const hasContextInfo = !!contextUsage;

  let contextBar = null;
  if (contextUsage) {
    const {
      contextWindow,
      systemPromptTokens,
      functionDefinitionsTokens,
      conversationTokens,
      responseTokens,
      totalUsedTokens,
      totalMessageCount,
      cacheHitRate,
      providerCacheType,
    } = contextUsage;

    const usagePercent = contextWindow > 0 ? Math.round((totalUsedTokens / contextWindow) * 100) : 0;
    const usageInfo = getUsageInfo(usagePercent, t);
    const systemPct = contextWindow > 0 ? (systemPromptTokens / contextWindow) * 100 : 0;
    const funcPct = contextWindow > 0 ? (functionDefinitionsTokens / contextWindow) * 100 : 0;
    const convPct = contextWindow > 0 ? (conversationTokens / contextWindow) * 100 : 0;
    const respPct = contextWindow > 0 ? (responseTokens / contextWindow) * 100 : 0;
    const sectionTokens = [systemPromptTokens, functionDefinitionsTokens, conversationTokens, responseTokens];
    const sectionPcts = [systemPct, funcPct, convPct, respPct];

    contextBar = (
      <div className="ai-context-block">
        <div className="ai-context-header">
          <span className="ai-context-label">{t('contextWindow.sectionTitle')}</span>
          <span className="ai-context-value" style={usagePercent >= 95 ? { color: "var(--color-error)" } : undefined}>
            {formatTokens(totalUsedTokens)} / {formatTokens(contextWindow)}
          </span>
        </div>

        <div className="ai-context-bar-track">
          {CONTEXT_SECTIONS.map((section, i) => (
            <div
              key={section.key}
              className="ai-context-bar-segment"
              style={{ width: `${sectionPcts[i]}%`, background: `var(${section.colorVar})` }}
              title={`${t(section.labelKey)}: ${formatTokens(sectionTokens[i])} (${sectionPcts[i].toFixed(1)}%)`}
            />
          ))}
        </div>

        <div className="ai-context-footer">
          <span className="ai-context-status" style={{ color: usageInfo.color }}>
            {usageInfo.label}
          </span>
          <span className="ai-context-percent">{usagePercent}%</span>
        </div>

        {usagePercent >= 80 && (
          <div className="ai-context-compressed">
            <span className="ai-context-dot" />
            <span>
              {t('contextWindow.approachingLimitDetail', { total: totalMessageCount })}
            </span>
          </div>
        )}

        {providerCacheType !== "none" && <CacheHitRateBar hitRate={cacheHitRate} />}
      </div>
    );
  }

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
              <span className="ai-field-author-summary">
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
              onChange={(e) => updateSettings({ general: { confirmationLevel: e.target.value as "always" | "editOnly" | "never" } })}
            >
              {confirmationLevelOptions.map((opt) => (
                <option key={opt.value} value={opt.value}>{opt.label}</option>
              ))}
            </select>
          </div>

          {hasContextInfo && <div className="ai-context-divider" />}
          {contextBar}
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

        /* 合并后的上下文窗口区域 */
        .ai-context-divider {
          height: 1px;
          background: var(--color-border-light);
          margin: 6px 0 4px;
        }
        .ai-context-block {
          display: flex;
          flex-direction: column;
          gap: 4px;
          padding: 2px 0;
        }
        .ai-context-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
        }
        .ai-context-label {
          font-size: 12px;
          color: var(--color-text-quaternary);
          font-weight: 500;
        }
        .ai-context-value {
          font-size: 12px;
          font-weight: 600;
          color: var(--color-text-secondary);
          font-variant-numeric: tabular-nums;
        }
        .ai-context-bar-track {
          height: 4px;
          background: var(--color-context-idle);
          border-radius: 2px;
          overflow: hidden;
          display: flex;
        }
        .ai-context-bar-segment {
          height: 100%;
          transition: width 0.5s ease;
          min-width: 0;
        }
        .ai-context-footer {
          display: flex;
          justify-content: space-between;
          align-items: center;
        }
        .ai-context-status {
          font-size: 11px;
          font-weight: 500;
        }
        .ai-context-percent {
          font-size: 11px;
          color: var(--color-text-quaternary);
          font-variant-numeric: tabular-nums;
        }
        .ai-context-compressed {
          display: flex;
          align-items: center;
          gap: 4px;
          padding: 3px 6px;
          background: var(--color-warning-bg, rgba(250, 173, 20, 0.1));
          border-radius: var(--radius-sm);
          font-size: 11px;
          color: var(--color-warning, #faad14);
        }
        .ai-context-dot {
          width: 5px;
          height: 5px;
          border-radius: 50%;
          background: var(--color-warning, #faad14);
          flex-shrink: 0;
        }
        .ai-cache-simple {
          display: flex;
          gap: 4px;
          align-items: baseline;
        }
        .ai-cache-label {
          font-size: 11px;
          color: var(--color-text-tertiary);
        }
        .ai-cache-value {
          font-size: 12px;
          font-weight: 600;
          font-variant-numeric: tabular-nums;
        }
      `}</style>
    </div>
  );
}
