import { useTranslation } from "react-i18next";
import { useSettingsStore } from "../../stores/useSettingsStore";

export function HandlersTab() {
  const { t } = useTranslation();
  const { handlers, tools } = useSettingsStore();

  return (
    <div>
      {/* 内置 Tools */}
      <div className="section-header">
        <span className="section-title">{t('settings.tools.builtinTools')}</span>
        <span className="section-badge">{tools.length}</span>
      </div>

      <div className="handlers-list">
        {tools.map((tool) => (
          <div key={tool.id} className="handler-item">
            <div className="handler-item-info">
              <div className="handler-name-row">
                <span className="handler-name">{tool.name}</span>
                <span className="handler-tool-badge">{t('settings.handlers.toolBadge')}</span>
              </div>
              <div className="handler-desc">{tool.description}</div>
            </div>
            <div className="handler-always-on">
              {t('settings.handlers.alwaysEnabled')}
            </div>
          </div>
        ))}
      </div>

      {/* 内置 Handlers（始终启用） */}
      <div className="section-header" style={{ marginTop: 24 }}>
        <span className="section-title">{t('settings.handlers.builtinHandlers')}</span>
        <span className="section-badge">{handlers.length}</span>
      </div>

      <div className="handlers-list">
        {handlers.map((s) => (
          <div key={s.id} className="handler-item">
            <div className="handler-item-info">
              <div className="handler-name-row">
                <span className="handler-name">{s.name}</span>
                <span className="handler-handler-badge">{t('settings.handlers.handlerBadge')}</span>
              </div>
              <div className="handler-desc">{s.description}</div>
            </div>
            <div className="handler-always-on">
              {t('settings.handlers.alwaysEnabled')}
            </div>
          </div>
        ))}
      </div>

      <style>{`
        .handlers-list {
          display: flex;
          flex-direction: column;
          margin-bottom: 24px;
        }
        .handler-item {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 10px 12px;
          border-bottom: 1px solid var(--color-border-light);
          transition: background 0.15s;
        }
        .handler-item:hover {
          background: var(--color-accent-bg);
        }
        .handler-item:last-child {
          border-bottom: none;
        }
        .handler-item-info {
          flex: 1;
          min-width: 0;
        }
        .handler-name {
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-primary);
        }
        .handler-name-row {
          display: flex;
          align-items: center;
          gap: 6px;
        }
        .handler-tool-badge {
          font-size: 10px;
          font-weight: 500;
          padding: 1px 6px;
          border-radius: 4px;
          background: var(--color-accent-bg);
          color: var(--color-accent);
        }
        .handler-handler-badge {
          font-size: 10px;
          font-weight: 500;
          padding: 1px 6px;
          border-radius: 4px;
          background: var(--color-purple-light);
          color: var(--color-purple);
        }
        .handler-always-on {
          font-size: 11px;
          color: var(--color-text-quaternary);
          flex-shrink: 0;
        }
        .handler-desc {
          font-size: 11px;
          color: var(--color-text-quaternary);
          margin-top: 2px;
          /* 限制描述最多显示两行 */
          display: -webkit-box;
          -webkit-line-clamp: 2;
          -webkit-box-orient: vertical;
          overflow: hidden;
        }
      `}</style>
    </div>
  );
}
