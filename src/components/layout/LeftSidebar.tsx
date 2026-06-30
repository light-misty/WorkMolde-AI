import { useState, useRef, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import i18n from "../../i18n";
import { AgentInfoSection } from "../sidebar/AgentInfoSection";
import { FileTreeSection } from "../sidebar/FileTreeSection";
import { SessionListSection } from "../sidebar/SessionListSection";
import { Icon } from "../common/Icon";
import { useWorkspaceStore } from "../../stores/useWorkspaceStore";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { useToastStore } from "../../stores/useToastStore";
import type { ThemeMode } from "../../types";
import * as tauriCmd from "../../services/tauri";

interface LeftSidebarProps {
  /** 文件预览回调 */
  onOpenPreview: (filePath: string, fileName: string) => void;
  /** 版本历史回调 */
  onOpenVersionHistory: (filePath: string, fileName: string) => void;
  /** 切换会话（父组件需同步切换工作区） */
  onSwitchSession: (sessionId: string, workspaceId?: string) => void;
  /** 为指定工作区新建会话（用于会话列表中按工作区新建） */
  onCreateSession: (workspaceId: string) => void;
  /** 直接新建会话：清空当前工作流进入待机状态，工作区由输入框内选择器切换 */
  onNewSession: () => void;
  /** 切换工作区并准备展示文件树 */
  onShowFiles: (workspaceId: string) => void;
  /** 删除当前会话后清理工作流 */
  onDeleteCurrentSession: (nextSessionId: string | null) => void;
}

type LeftSidebarView = "sessions" | "files";

/**
 * 左侧栏容器
 * 在「会话列表」与「工作区文件」两种视图之间切换。
 */
export function LeftSidebar({
  onOpenPreview,
  onOpenVersionHistory,
  onSwitchSession,
  onCreateSession,
  onNewSession,
  onShowFiles,
  onDeleteCurrentSession,
}: LeftSidebarProps) {
  const { t } = useTranslation();
  const [view, setView] = useState<LeftSidebarView>("sessions");
  const { workspaces, currentWorkspaceId } = useWorkspaceStore();
  const { openSettings, settings, updateSettings } = useSettingsStore();
  // 更多按钮下拉菜单
  const [moreOpen, setMoreOpen] = useState(false);
  const moreRef = useRef<HTMLDivElement>(null);
  // 语言子菜单
  const [langOpen, setLangOpen] = useState(false);
  const [langMenuStyle, setLangMenuStyle] = useState<React.CSSProperties>({});
  const langRef = useRef<HTMLDivElement>(null);
  // 语言子菜单 portal 容器 ref（用于点击外部关闭判定，子菜单通过 createPortal 渲染到 body，
  // 脱离了 langRef/moreRef 的 DOM 树，需要单独跟踪）
  const langDropdownRef = useRef<HTMLDivElement>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const addToast = useToastStore((s) => s.addToast);

  // 判断当前是否处于深色模式
  const isDarkMode = (() => {
    const { themeMode } = settings.appearance;
    if (themeMode === "dark") return true;
    if (themeMode === "system") return window.matchMedia("(prefers-color-scheme: dark)").matches;
    return false;
  })();

  // 切换主题：深色 ↔ 浅色
  const toggleTheme = useCallback(() => {
    const nextMode: ThemeMode = isDarkMode ? "light" : "dark";
    updateSettings({ appearance: { themeMode: nextMode } });
    setLangOpen(false);
  }, [isDarkMode, updateSettings]);

  // 检查更新
  const checkForUpdates = useCallback(async () => {
    if (checkingUpdate) return;
    setCheckingUpdate(true);
    try {
      const result = await tauriCmd.checkUpdate();
      if (result) {
        addToast("success", t('update.newVersionFound', { version: result.version }));
      } else {
        addToast("success", t('settings.general.upToDate'));
      }
    } catch (err) {
      const errMsg = err instanceof Error ? err.message : String(err);
      addToast("error", t('update.checkFailedWithError', { error: errMsg }));
    } finally {
      setCheckingUpdate(false);
    }
  }, [checkingUpdate, addToast, t]);

  // 切换语言
  const switchLanguage = useCallback((lang: string) => {
    i18n.changeLanguage(lang);
    localStorage.setItem('i18n-language', lang);
    updateSettings({ appearance: { language: lang, languageFollowSystem: false } });
    setLangOpen(false);
    setMoreOpen(false);
  }, [updateSettings]);

  const currentWorkspace = workspaces.find((w) => w.id === currentWorkspaceId);

  const handleShowFiles = (workspaceId: string) => {
    // 通知父组件切换活动工作区，保证文件树加载正确路径
    onShowFiles(workspaceId);
    setView("files");
  };

  const [fileTreeExiting, setFileTreeExiting] = useState(false);

  const handleBackToSessions = () => {
    setFileTreeExiting(true);
    setTimeout(() => {
      setView("sessions");
      setFileTreeExiting(false);
    }, 280);
  };

  // 点击外部或 Escape 关闭更多下拉菜单
  useEffect(() => {
    if (!moreOpen) return;
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      // 点击在语言子菜单内（通过 createPortal 渲染到 body，脱离 moreRef DOM 树），
      // 不关闭更多菜单，否则会在 mousedown 阶段卸载子菜单，导致后续 click 无法触发 switchLanguage
      if (langDropdownRef.current && langDropdownRef.current.contains(target)) {
        return;
      }
      if (moreRef.current && !moreRef.current.contains(target)) {
        setMoreOpen(false);
        setLangOpen(false);
      }
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setLangOpen(false);
        setMoreOpen(false);
      }
    };
    const timer = setTimeout(() => {
      document.addEventListener("mousedown", handleClickOutside);
      document.addEventListener("keydown", handleKeyDown);
    }, 0);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [moreOpen]);

  return (
    <div className="left-sidebar">
      {(view === "files" || fileTreeExiting) ? (
        <div className={`file-tree-panel ${fileTreeExiting ? 'exit' : 'enter'}`}>
          <div className="file-tree-header">
            <button
              className="file-tree-back-btn"
              onClick={handleBackToSessions}
              title={t("sessionList.backToSessions")}
              aria-label={t("sessionList.backToSessions")}
            >
              <Icon name="back" size={14} />
              <span>{t("sessionList.backToSessions")}</span>
            </button>
            <span className="file-tree-workspace-name" title={currentWorkspace?.name}>
              {currentWorkspace?.name || ""}
            </span>
          </div>
          <div className="file-tree-wrapper">
            <FileTreeSection
              onOpenPreview={onOpenPreview}
              onOpenVersionHistory={onOpenVersionHistory}
            />
          </div>
        </div>
      ) : (
        <>
          {/* 新建会话按钮：直接进入待机的新建会话页面，工作区由输入框内选择器切换 */}
          <div className="new-session-section">
            <button
              type="button"
              className="new-session-trigger"
              aria-label={t('topBar.newSession')}
              onClick={onNewSession}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="new-session-icon">
                <line x1="12" y1="5" x2="12" y2="19"/>
                <line x1="5" y1="12" x2="19" y2="12"/>
              </svg>
              <span>{t('topBar.newSession')}</span>
            </button>
          </div>

          {/* Agent 信息区置于会话列表上方，默认收缩，可点击展开 */}
          <AgentInfoSection />
          <SessionListSection
            onSwitchSession={onSwitchSession}
            onCreateSession={onCreateSession}
            onShowFiles={handleShowFiles}
            onDeleteCurrentSession={onDeleteCurrentSession}
          />

          {/* 更多按钮 + 下拉菜单 */}
          <div ref={moreRef} className="more-section">
            <button
              type="button"
              className="new-session-trigger"
              aria-haspopup="true"
              aria-expanded={moreOpen}
              onClick={() => setMoreOpen((prev) => !prev)}
            >
              <span>{t('sidebar.more')}</span>
              <Icon name="menu" size={14} style={{ marginLeft: 'auto' }} />
            </button>

            {moreOpen && (
              <div className="more-dropdown">
                <div ref={langRef} className="more-dropdown-item more-dropdown-item-with-sub" onClick={() => {
                  if (!langOpen && langRef.current) {
                    const rect = langRef.current.getBoundingClientRect();
                    setLangMenuStyle({ position: 'fixed', left: rect.right + 4, top: rect.top });
                  }
                  setLangOpen((prev) => !prev);
                }}>
                  <span className="more-dropdown-item-text">{t('sidebar.language')}</span>
                  <Icon name="chevron-right" size={12} className="more-dropdown-sub-chevron" />
                  {langOpen && createPortal(
                    <div ref={langDropdownRef} className="more-lang-dropdown" style={langMenuStyle}>
                      <div className="more-dropdown-item" onClick={(e) => { e.stopPropagation(); switchLanguage('zh-CN'); }}>
                        <span className="more-dropdown-item-text">{t('settings.appearance.zhCN')}</span>
                        {settings.appearance.language === 'zh-CN' && <Icon name="check" size={14} className="more-dropdown-check" />}
                      </div>
                      <div className="more-dropdown-item" onClick={(e) => { e.stopPropagation(); switchLanguage('en-US'); }}>
                        <span className="more-dropdown-item-text">{t('settings.appearance.enUS')}</span>
                        {settings.appearance.language === 'en-US' && <Icon name="check" size={14} className="more-dropdown-check" />}
                      </div>
                    </div>,
                    document.body
                  )}
                </div>
                <div className="more-dropdown-item" onClick={toggleTheme}>
                  <Icon name={isDarkMode ? "theme" : "moon"} size={14} />
                  <span>{isDarkMode ? t('topBar.switchToLight') : t('topBar.switchToDark')}</span>
                </div>
                <div className="more-dropdown-item" onClick={() => { checkForUpdates(); setMoreOpen(false); setLangOpen(false); }}>
                  <Icon name="refresh" size={14} />
                  <span>{t('sidebar.checkUpdate')}</span>
                </div>
                <div className="more-dropdown-item" onClick={() => { openSettings("appearance"); setMoreOpen(false); setLangOpen(false); }}>
                  <Icon name="settings" size={14} />
                  <span>{t('topBar.settings')}</span>
                </div>
              </div>
            )}
          </div>
        </>
      )}

      <style>{`
        .left-sidebar {
          position: relative;
          display: flex;
          flex-direction: column;
          height: 100%;
          width: 100%;
          overflow: hidden;
          --color-bg-hover: #e8eaef;
        }
        .dark .left-sidebar {
          --color-bg-hover: #242627;
        }
        /* 新建会话按钮区: 悬停背景与智能体信息标题栏一致 */
        .new-session-section {
          position: relative;
          flex-shrink: 0;
          margin: 4px 8px 0;
        }
        .new-session-trigger {
          display: flex;
          align-items: center;
          gap: 6px;
          width: 100%;
          padding: 8px 12px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          user-select: none;
          transition: background 0.15s;
          background: transparent;
          border: none;
          font-size: 14px;
          font-weight: 400;
          color: var(--color-text-primary);
        }
        .new-session-icon {
          flex-shrink: 0;
          color: var(--color-text-primary);
        }
        .new-session-trigger:hover {
          background: var(--color-bg-hover);
        }
        /* 删除全局 button:active 的 scale 缩小动画反馈 */
        .new-session-trigger:active {
          transform: none;
        }
        .file-tree-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 10px 12px;
          border-bottom: 1px solid var(--color-border-light);
          flex-shrink: 0;
        }
        .file-tree-back-btn {
          display: flex;
          align-items: center;
          gap: 6px;
          padding: 4px 8px;
          border-radius: var(--radius-sm);
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-secondary);
          background: transparent;
          border: none;
          cursor: pointer;
          transition: all 0.15s;
        }
        .file-tree-back-btn:hover {
          background: var(--color-bg-hover);
        }
        .file-tree-workspace-name {
          font-size: 13px;
          font-weight: 600;
          color: var(--color-text-primary);
          max-width: 140px;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .file-tree-wrapper {
          flex: 1;
          min-height: 0;
          overflow-y: auto;
        }
        .file-tree-panel {
          position: absolute;
          inset: 0;
          display: flex;
          flex-direction: column;
          background: var(--color-bg-primary);
          z-index: 10;
          overflow: hidden;
        }
        .file-tree-panel.enter {
          animation: file-tree-slide-in 0.28s cubic-bezier(0.4, 0, 0.2, 1);
        }
        .file-tree-panel.exit {
          animation: file-tree-slide-out 0.28s cubic-bezier(0.4, 0, 0.2, 1) forwards;
        }
        @keyframes file-tree-slide-in {
          from {
            opacity: 0;
            transform: translateX(-100%);
          }
          to {
            opacity: 1;
            transform: translateX(0);
          }
        }
        @keyframes file-tree-slide-out {
          from {
            opacity: 1;
            transform: translateX(0);
          }
          to {
            opacity: 0;
            transform: translateX(-100%);
          }
        }
        /* 更多按钮区：固定在底部，不与会话列表重叠 */
        .more-section {
          position: relative;
          flex-shrink: 0;
          margin: 0 8px 4px;
          margin-top: auto;
        }
        /* 更多下拉菜单：显示在上方（有足够的空间） */
        .more-dropdown {
          position: absolute;
          bottom: calc(100% + 4px);
          left: 0;
          right: 0;
          min-width: 200px;
          background: var(--color-bg-elevated);
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-md);
          box-shadow: var(--shadow-lg);
          z-index: 200;
          animation: more-dropdown-in 0.15s ease-out;
          padding: 4px;
        }
        @keyframes more-dropdown-in {
          from {
            opacity: 0;
            transform: scale(0.96) translateY(4px);
          }
          to {
            opacity: 1;
            transform: scale(1) translateY(0);
          }
        }
        .more-dropdown-item {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 8px 10px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          user-select: none;
          transition: background 0.12s;
          font-size: 13px;
          color: var(--color-text-primary);
        }
        .more-dropdown-item:hover {
          background: var(--color-bg-hover);
        }
        .more-dropdown-item-text {
          flex: 1;
        }
        .more-dropdown-check {
          color: var(--color-accent);
        }
        .more-dropdown-item-with-sub {
          position: relative;
        }
        .more-dropdown-sub-chevron {
          color: var(--color-text-quaternary);
        }
        /* 语言子菜单：显示在主菜单右侧 */
        .more-lang-dropdown {
          position: absolute;
          left: 100%;
          top: 0;
          min-width: 160px;
          background: var(--color-bg-elevated);
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-md);
          box-shadow: var(--shadow-lg);
          z-index: 210;
          animation: more-lang-in 0.12s ease-out;
          overflow: hidden;
          padding: 4px;
          margin-left: 4px;
        }
        @keyframes more-lang-in {
          from {
            opacity: 0;
            transform: translateX(-4px);
          }
          to {
            opacity: 1;
            transform: translateX(0);
          }
        }
      `}</style>
    </div>
  );
}
