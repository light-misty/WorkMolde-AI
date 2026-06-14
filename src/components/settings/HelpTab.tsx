import { useTranslation } from "react-i18next";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { deriveNewLineShortcut } from "../../utils/format";

export function HelpTab() {
  const { t } = useTranslation();
  const shortcuts = useSettingsStore((s) => s.settings.shortcuts);
  const newLineShortcut = deriveNewLineShortcut(shortcuts.sendMessage);

  // 固定的快捷键（非可配置项）
  const fixedShortcuts = [
    { keys: "Escape", desc: t('settings.help.closeDialogDesc') },
    { keys: "Ctrl+V", desc: t('settings.help.pasteImageDesc') },
  ];

  // 常见问题
  const faqList = [
    { q: t('settings.help.faqQ0'), a: t('settings.help.faqA0') },
    { q: t('settings.help.faqQ1'), a: t('settings.help.faqA1') },
    { q: t('settings.help.faqQ2'), a: t('settings.help.faqA2') },
    { q: t('settings.help.faqQ3'), a: t('settings.help.faqA3') },
    { q: t('settings.help.faqQ4'), a: t('settings.help.faqA4') },
    { q: t('settings.help.faqQ5'), a: t('settings.help.faqA5') },
    { q: t('settings.help.faqQ6'), a: t('settings.help.faqA6') },
  ];

  // 内置 Handler 列表
  const builtinHandlers = [
    { name: "docx_handler", desc: t('settings.help.handlerDocx') },
    { name: "xlsx_handler", desc: t('settings.help.handlerXlsx') },
    { name: "pptx_handler", desc: t('settings.help.handlerPptx') },
    { name: "pdf_handler", desc: t('settings.help.handlerPdf') },
  ];

  // 从设置中动态生成可配置快捷键列表
  const configurableShortcuts = [
    { keys: shortcuts.newSession, desc: t('settings.help.newSessionDesc') },
    { keys: shortcuts.closeSession, desc: t('settings.help.closeCurrentSessionDesc') },
    { keys: shortcuts.sendMessage, desc: t('settings.help.sendMessageDesc') },
    { keys: newLineShortcut, desc: t('settings.help.newLineDesc') },
    { keys: shortcuts.toggleSidebar, desc: t('settings.help.toggleSidebarDesc') },
    { keys: shortcuts.quickPrompt, desc: t('settings.help.quickPromptDesc') },
    { keys: "Ctrl+,", desc: t('settings.help.openSettingsDesc') },
  ];

  return (
    <div>
      {/* 快速入门 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">{t('settings.help.quickStart')}</span>
        </div>
        <div className="help-card">
          <div className="help-step">
            <span className="help-step-num">1</span>
            <div className="help-step-content">
              <div className="help-step-title">{t('settings.help.configureLLM')}</div>
              <div className="help-step-desc">{t('settings.help.configureLLMDesc')}</div>
            </div>
          </div>
          <div className="help-step">
            <span className="help-step-num">2</span>
            <div className="help-step-content">
              <div className="help-step-title">{t('settings.help.selectWorkspace')}</div>
              <div className="help-step-desc">{t('settings.help.selectWorkspaceDesc')}</div>
            </div>
          </div>
          <div className="help-step">
            <span className="help-step-num">3</span>
            <div className="help-step-content">
              <div className="help-step-title">{t('settings.help.startConversation')}</div>
              <div className="help-step-desc">{t('settings.help.startConversationDesc')}</div>
            </div>
          </div>
        </div>
      </div>

      {/* 快捷键 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">{t('settings.help.shortcuts')}</span>
        </div>
        <div className="help-shortcut-list">
          {configurableShortcuts.map((item) => (
            <div key={item.desc} className="help-shortcut-row">
              <span className="help-shortcut-desc">{item.desc}</span>
              <kbd className="help-shortcut-key">{item.keys}</kbd>
            </div>
          ))}
          {fixedShortcuts.map((item) => (
            <div key={item.desc} className="help-shortcut-row">
              <span className="help-shortcut-desc">{item.desc}</span>
              <kbd className="help-shortcut-key">{item.keys}</kbd>
            </div>
          ))}
        </div>
      </div>

      {/* 内置 Handler 列表 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">{t('settings.help.builtinHandlerList')}</span>
        </div>
        <div className="help-handler-list">
          {builtinHandlers.map((handler) => (
            <div key={handler.name} className="help-handler-row">
              <span className="help-handler-name">{handler.name}</span>
              <span className="help-handler-desc">{handler.desc}</span>
            </div>
          ))}
        </div>
      </div>

      {/* 常见问题 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">{t('settings.help.faqTitle')}</span>
        </div>
        <div className="help-faq-list">
          {faqList.map((item, i) => (
            <div key={i} className="help-faq-item">
              <div className="help-faq-q">{item.q}</div>
              <div className="help-faq-a">{item.a}</div>
            </div>
          ))}
        </div>
      </div>

      <style>{`
        .help-card {
          padding: 16px;
          background: var(--color-bg-sub);
          border-radius: var(--radius-md);
          border: 1px solid var(--color-border-light);
          display: flex;
          flex-direction: column;
          gap: 12px;
        }
        .help-step {
          display: flex;
          align-items: flex-start;
          gap: 12px;
        }
        .help-step-num {
          width: 24px;
          height: 24px;
          border-radius: 50%;
          background: var(--color-accent);
          color: #fff;
          display: flex;
          align-items: center;
          justify-content: center;
          font-size: 12px;
          font-weight: 700;
          flex-shrink: 0;
        }
        .help-step-content {
          flex: 1;
          min-width: 0;
        }
        .help-step-title {
          font-size: 13px;
          font-weight: 600;
          color: var(--color-text-primary);
        }
        .help-step-desc {
          font-size: 12px;
          color: var(--color-text-tertiary);
          margin-top: 2px;
        }
        .help-shortcut-list {
          display: flex;
          flex-direction: column;
        }
        .help-shortcut-row {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 8px 12px;
          border-bottom: 1px solid var(--color-border-light);
        }
        .help-shortcut-row:last-child {
          border-bottom: none;
        }
        .help-shortcut-desc {
          font-size: 13px;
          color: var(--color-text-primary);
        }
        .help-shortcut-key {
          padding: 2px 8px;
          border-radius: var(--radius-sm);
          background: var(--color-bg-sub);
          border: 1px solid var(--color-border);
          font-size: 11px;
          font-family: var(--font-mono);
          color: var(--color-text-secondary);
        }
        .help-handler-list {
          display: flex;
          flex-direction: column;
        }
        .help-handler-row {
          display: flex;
          align-items: center;
          padding: 8px 12px;
          border-bottom: 1px solid var(--color-border-light);
          gap: 12px;
        }
        .help-handler-row:last-child {
          border-bottom: none;
        }
        .help-handler-name {
          font-size: 12px;
          font-family: var(--font-mono);
          color: var(--color-accent);
          min-width: 160px;
        }
        .help-handler-desc {
          font-size: 12px;
          color: var(--color-text-tertiary);
        }
        .help-faq-list {
          display: flex;
          flex-direction: column;
          gap: 12px;
        }
        .help-faq-item {
          padding: 12px;
          background: var(--color-bg-sub);
          border-radius: var(--radius-sm);
          border: 1px solid var(--color-border-light);
        }
        .help-faq-q {
          font-size: 13px;
          font-weight: 600;
          color: var(--color-text-primary);
          margin-bottom: 4px;
        }
        .help-faq-a {
          font-size: 12px;
          color: var(--color-text-tertiary);
          line-height: 1.6;
        }
      `}</style>
    </div>
  );
}
