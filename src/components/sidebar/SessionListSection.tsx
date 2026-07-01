import type { MouseEvent } from "react";
import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { DeleteConfirmDialog } from "../common/DeleteConfirmDialog";
import { Icon } from "../common/Icon";
import { useSessionStore } from "../../stores/useSessionStore";
import { useWorkspaceStore } from "../../stores/useWorkspaceStore";
import { useToastStore } from "../../stores/useToastStore";
import type { SessionSummary } from "../../types/session";
import type { WorkspaceInfo } from "../../types/workspace";

interface SessionListSectionProps {
  /** 切换会话回调，父组件负责同步切换工作区 */
  onSwitchSession: (sessionId: string, workspaceId?: string) => void;
  /** 为指定工作区新建会话 */
  onCreateSession: (workspaceId: string) => void;
  /** 查看指定工作区文件 */
  onShowFiles: (workspaceId: string) => void;
  /** 删除当前会话后通知父组件清理工作流 */
  onDeleteCurrentSession: (nextSessionId: string | null) => void;
}

interface GroupedSessions {
  workspace: WorkspaceInfo;
  sessions: SessionSummary[];
}

/**
 * 右侧栏会话列表
 * 按工作区分组展示会话，支持折叠、新建会话、查看文件、重命名与删除。
 */
export function SessionListSection({
  onSwitchSession,
  onCreateSession,
  onShowFiles,
  onDeleteCurrentSession,
}: SessionListSectionProps) {
  const { t } = useTranslation();
  const { sessions, currentSessionId, deleteSession, updateSessionTitle } = useSessionStore();
  const { workspaces, currentWorkspaceId } = useWorkspaceStore();

  // 会话列表整体收缩状态: true 表示工作区和会话历史全部隐藏, 仅保留标题栏
  const [collapsed, setCollapsed] = useState(false);

  const [expanded, setExpanded] = useState<Record<string, boolean>>(() => {
    const init: Record<string, boolean> = {};
    workspaces.forEach((w) => {
      init[w.id] = w.id === currentWorkspaceId;
    });
    return init;
  });

  // 当前工作区变更时自动展开对应分组
  useEffect(() => {
    if (currentWorkspaceId) {
      setExpanded((prev) => ({ ...prev, [currentWorkspaceId]: true }));
    }
  }, [currentWorkspaceId]);

  // 组件卸载时清理 focus 定时器
  useEffect(() => {
    return () => {
      if (focusTimerRef.current !== null) {
        clearTimeout(focusTimerRef.current);
      }
    };
  }, []);

  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState("");
  const editInputRef = useRef<HTMLInputElement>(null);
  // 保存 focus 定时器，组件卸载时清理，避免在已卸载组件上执行回调
  const focusTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const [deleteConfirmTitle, setDeleteConfirmTitle] = useState("");
  const [deleteWorkspaceId, setDeleteWorkspaceId] = useState<string | null>(null);
  const [deleteWorkspaceName, setDeleteWorkspaceName] = useState("");

  // 按工作区分组并排序
  const grouped = useMemo(() => {
    const map = new Map<string, SessionSummary[]>();
    // 有效工作区 ID 集合，用于过滤无效的 workspaceId
    const validWorkspaceIds = new Set(workspaces.map((w) => w.id));

    sessions.forEach((s) => {
      let key: string | null = null;
      if (s.workspaceId && validWorkspaceIds.has(s.workspaceId)) {
        // 有有效 workspaceId 的会话归入对应工作区
        key = s.workspaceId;
      } else if (!s.workspaceId && workspaces.length > 0) {
        // 仅当 workspaceId 为空时（旧数据），归入第一个工作区
        // 数据修复 useEffect 会异步将其 workspace_id 持久化为正确值
        // 注意：workspaceId 非空但无效（指向已删除工作区）的孤儿会话不显示
        // 这些会话应由后端 remove_workspace 命令清理，不应在此兜底显示
        key = workspaces[0].id;
      }

      if (key !== null) {
        const list = map.get(key) || [];
        list.push(s);
        map.set(key, list);
      }
    });

    // 每个工作区内部按更新时间倒序
    map.forEach((list) => {
      list.sort((a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime());
    });

    const result: GroupedSessions[] = [];
    workspaces.forEach((w) => {
      result.push({ workspace: w, sessions: map.get(w.id) || [] });
    });

    return result;
  }, [sessions, workspaces, currentWorkspaceId]);

  const toggleWorkspace = (workspaceId: string) => {
    setExpanded((prev) => ({ ...prev, [workspaceId]: !prev[workspaceId] }));
  };

  const handleCreateClick = (e: MouseEvent, workspaceId: string) => {
    e.stopPropagation();
    onCreateSession(workspaceId);
  };

  const handleShowFilesClick = (e: MouseEvent, workspaceId: string) => {
    e.stopPropagation();
    onShowFiles(workspaceId);
  };

  const handleSessionClick = (session: SessionSummary) => {
    if (editingId === session.id) return;
    onSwitchSession(session.id, session.workspaceId);
  };

  const handleStartRename = (e: MouseEvent, session: SessionSummary) => {
    e.stopPropagation();
    setEditingId(session.id);
    setEditingTitle(session.title);
    // 保存定时器，组件卸载时清理
    focusTimerRef.current = setTimeout(() => editInputRef.current?.focus(), 0);
  };

  const confirmRename = async () => {
    if (editingId && editingTitle.trim()) {
      try {
        await updateSessionTitle(editingId, editingTitle.trim());
      } catch (err) {
        console.error("[SessionListSection] 重命名会话失败:", err);
        useToastStore.getState().addToast("error", t("sessionList.renameFailed"));
      }
    }
    setEditingId(null);
    setEditingTitle("");
  };

  const handleRenameKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      confirmRename();
    } else if (e.key === "Escape") {
      setEditingId(null);
      setEditingTitle("");
    }
  };

  const handleDeleteClick = (e: MouseEvent, session: SessionSummary) => {
    e.stopPropagation();
    setDeleteConfirmId(session.id);
    setDeleteConfirmTitle(session.title);
  };

  const handleConfirmDelete = async () => {
    if (!deleteConfirmId) return;

    const isDeletingCurrent = deleteConfirmId === currentSessionId;
    const flatSessions = grouped.flatMap((g) => g.sessions);
    const currentIndex = flatSessions.findIndex((s) => s.id === deleteConfirmId);
    const nextSessionId = flatSessions.length > 1
      ? flatSessions[currentIndex + 1]?.id || flatSessions[currentIndex - 1]?.id || null
      : null;

    try {
      await deleteSession(deleteConfirmId);
    } catch (err) {
      console.error("[SessionListSection] 删除会话失败:", err);
      useToastStore.getState().addToast("error", t("sessionList.deleteFailed"));
      // 删除失败时不关闭弹窗，让用户可以重试或取消
      return;
    }

    if (isDeletingCurrent) {
      onDeleteCurrentSession(nextSessionId || null);
    }

    setDeleteConfirmId(null);
    setDeleteConfirmTitle("");
  };

  const handleDeleteWorkspaceClick = (e: MouseEvent, workspace: WorkspaceInfo) => {
    e.stopPropagation();
    setDeleteWorkspaceId(workspace.id);
    setDeleteWorkspaceName(workspace.name);
  };

  const handleConfirmDeleteWorkspace = async () => {
    if (!deleteWorkspaceId) return;
    const { removeWorkspace } = useWorkspaceStore.getState();
    try {
      await removeWorkspace(deleteWorkspaceId);
    } catch (err) {
      console.error("[SessionListSection] 删除工作区失败:", err);
      useToastStore.getState().addToast("error", t("sessionList.deleteWorkspaceFailed"));
      // 删除失败时不关闭弹窗，让用户可以重试或取消
      return;
    }
    setDeleteWorkspaceId(null);
    setDeleteWorkspaceName("");
  };

  return (
    <>
      {deleteWorkspaceId && createPortal(
        <DeleteConfirmDialog
          name={deleteWorkspaceName}
          isDir={true}
          onConfirm={handleConfirmDeleteWorkspace}
          onCancel={() => {
            setDeleteWorkspaceId(null);
            setDeleteWorkspaceName("");
          }}
        />,
        document.body
      )}
      {deleteConfirmId && createPortal(
        <DeleteConfirmDialog
          name={deleteConfirmTitle}
          isDir={false}
          onConfirm={handleConfirmDelete}
          onCancel={() => {
            setDeleteConfirmId(null);
            setDeleteConfirmTitle("");
          }}
        />,
        document.body
      )}

      <div className={`session-list-section ${collapsed ? "session-list-section-collapsed" : ""}`}>
        <div
          className="session-list-header"
          role="button"
          aria-label={collapsed ? t('sessionList.expand') : t('sessionList.collapse')}
          aria-expanded={!collapsed}
          tabIndex={0}
          onClick={() => setCollapsed((prev) => !prev)}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              setCollapsed((prev) => !prev);
            }
          }}
        >
          <div className="session-list-title">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="session-list-title-icon">
              <line x1="8" y1="6" x2="21" y2="6"/>
              <line x1="8" y1="12" x2="21" y2="12"/>
              <line x1="8" y1="18" x2="21" y2="18"/>
              <line x1="3" y1="6" x2="3.01" y2="6"/>
              <line x1="3" y1="12" x2="3.01" y2="12"/>
              <line x1="3" y1="18" x2="3.01" y2="18"/>
            </svg>
            <span>{t("sessionList.title")}</span>
          </div>
          {/* 收缩/展开按钮: 斜对角双向直角, 展开时朝内(可收缩), 收缩时朝外(可展开) */}
          <button
            type="button"
            className={`session-list-collapse-btn ${collapsed ? "session-list-collapse-btn-visible" : ""}`}
            onClick={(e) => {
              e.stopPropagation();
              setCollapsed((prev) => !prev);
            }}
          >
            <Icon name={collapsed ? "chevron-diagonal-out" : "chevron-diagonal-in"} size={14} />
          </button>
        </div>

        <div className={`session-list-body ${collapsed ? "session-list-body-collapsed" : ""}`}>
          {grouped.length === 0 ? (
            <div className="session-list-empty">
              <Icon name="history" size={28} className="opacity-30" />
              <p>{t("sessionList.noSessionsInWorkspace")}</p>
            </div>
          ) : (
            grouped.map(({ workspace, sessions: groupSessions }) => {
              const isExpanded = expanded[workspace.id] ?? false;
              return (
                <div key={workspace.id} className="workspace-group">
                  <div
                    className="workspace-header"
                    role="button"
                    aria-expanded={isExpanded}
                    onClick={() => toggleWorkspace(workspace.id)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        toggleWorkspace(workspace.id);
                      }
                    }}
                    tabIndex={0}
                  >
                    <div className="workspace-header-left">
                      <span
                        className="workspace-chevron"
                        style={{ transform: isExpanded ? "rotate(0deg)" : "rotate(-90deg)" }}
                      >
                        <Icon name="chevron-down" size={12} />
                      </span>
                      <Icon name="folder" size={14} className="workspace-icon" />
                      <span className="workspace-name" title={workspace.name}>
                        {workspace.name}
                      </span>
                    </div>
                    <div className="workspace-actions">
                      <button
                        className="workspace-action-btn"
                        title={t("sessionList.newSessionForWorkspace", { workspace: workspace.name })}
                        aria-label={t("sessionList.newSessionForWorkspace", { workspace: workspace.name })}
                        onClick={(e) => handleCreateClick(e, workspace.id)}
                      >
                        <Icon name="plus" size={13} />
                      </button>
                      <button
                        className="workspace-action-btn"
                        title={t("sessionList.showFiles")}
                        aria-label={t("sessionList.showFiles")}
                        onClick={(e) => handleShowFilesClick(e, workspace.id)}
                      >
                        <Icon name="folder" size={13} />
                      </button>
                      <button
                        className="workspace-action-btn workspace-action-btn-danger"
                        title={t("workspace.removeWorkspace")}
                        aria-label={t("workspace.removeWorkspace")}
                        onClick={(e) => handleDeleteWorkspaceClick(e, workspace)}
                      >
                        <Icon name="trash" size={13} />
                      </button>
                    </div>
                  </div>

                  <div
                    className="workspace-sessions"
                    style={{
                      maxHeight: isExpanded ? "2000px" : "0px",
                      opacity: isExpanded ? 1 : 0,
                    }}
                  >
                    {groupSessions.map((s) => (
                      <div
                        key={s.id}
                        className={`session-item ${s.id === currentSessionId ? "active" : ""}`}
                        role="listitem"
                        aria-selected={s.id === currentSessionId}
                        onClick={() => handleSessionClick(s)}
                      >
                        {editingId === s.id ? (
                          <input
                            ref={editInputRef}
                            className="session-edit-input"
                            value={editingTitle}
                            onChange={(e) => setEditingTitle(e.target.value)}
                            onKeyDown={handleRenameKeyDown}
                            onBlur={confirmRename}
                            onClick={(e) => e.stopPropagation()}
                          />
                        ) : (
                          <>
                            <div className="session-item-title" title={s.title}>
                              {s.title}
                            </div>
                            <div className="session-item-actions">
                              <button
                                className="session-action-btn"
                                title={t("history.rename")}
                                aria-label={t("history.renameSession")}
                                onClick={(e) => handleStartRename(e, s)}
                              >
                                <Icon name="edit" size={12} />
                              </button>
                              <button
                                className="session-action-btn session-action-btn-danger"
                                title={t("history.deleteSession")}
                                aria-label={t("history.deleteSession")}
                                onClick={(e) => handleDeleteClick(e, s)}
                              >
                                <Icon name="trash" size={12} />
                              </button>
                            </div>
                          </>
                        )}
                      </div>
                    ))}
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>

      <style>{`
        .session-list-section {
          display: flex;
          flex-direction: column;
          flex: 1;
          min-height: 0;
          /* margin 与 agent-info-section / new-session-section 保持一致 */
          margin: 4px 8px;
        }
        /* 收缩状态: 仅保留标题栏高度, 不再占据剩余空间 */
        .session-list-section-collapsed {
          flex: 0;
        }
        .session-list-header {
          /* padding 与 agent-info-header / new-session-trigger 保持一致 */
          padding: 8px 12px;
          display: flex;
          align-items: center;
          justify-content: space-between;
          border-radius: var(--radius-sm);
          cursor: pointer;
          user-select: none;
          transition: background 0.15s;
        }
        /* 悬停背景与智能体信息标题栏一致 */
        .session-list-header:hover {
          background: var(--color-bg-hover);
        }
        .session-list-title {
          display: flex;
          align-items: center;
          gap: 6px;
          font-size: 14px;
          font-weight: 400;
          color: var(--color-text-primary);
        }
        .session-list-title-icon {
          flex-shrink: 0;
          color: var(--color-text-primary);
        }
        /* 收缩/展开按钮: 默认隐藏, 悬停时显示; 收缩状态始终可见 */
        .session-list-collapse-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0;
          border-radius: var(--radius-sm);
          color: var(--color-text-quaternary);
          background: transparent;
          border: none;
          cursor: pointer;
          transition: all 0.15s;
          opacity: 0;
          flex-shrink: 0;
        }
        .session-list-header:hover .session-list-collapse-btn,
        .session-list-collapse-btn-visible {
          opacity: 1;
        }
        .session-list-collapse-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
        }
        .session-list-body {
          flex: 1;
          /* min-height: 0 必需：允许 flex 子元素缩小到小于内容高度，
             否则会话列表内容会撑大 session-list-section，
             进而撑大 sb-scroll 导致 overflow-y: auto 失效 */
          min-height: 0;
          overflow-y: auto;
          padding: 0 8px 12px;
          transition: max-height 0.25s ease, opacity 0.2s ease, padding 0.25s ease;
        }
        /* 收缩状态: 工作区和会话历史全部隐藏 */
        .session-list-body-collapsed {
          flex: 0;
          max-height: 0;
          opacity: 0;
          padding-top: 0;
          padding-bottom: 0;
          overflow: hidden;
        }
        .session-list-empty {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          padding: 32px 12px;
          gap: 10px;
          color: var(--color-text-quaternary);
          font-size: 13px;
          text-align: center;
        }
        .workspace-group {
          margin-bottom: 2px;
        }
        .workspace-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 4px 8px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          user-select: none;
          transition: background 0.15s;
        }
        .workspace-header:hover {
          background: var(--color-bg-hover);
        }
        .workspace-header-left {
          display: flex;
          align-items: center;
          gap: 6px;
          min-width: 0;
          flex: 1;
        }
        .workspace-chevron {
          display: flex;
          align-items: center;
          justify-content: center;
          color: var(--color-text-quaternary);
          transition: transform 0.2s;
          flex-shrink: 0;
        }
        .workspace-icon {
          color: var(--color-text-tertiary);
          flex-shrink: 0;
        }
        .workspace-name {
          font-size: 14px;
          font-weight: 500;
          color: var(--color-text-primary);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .workspace-actions {
          display: flex;
          align-items: center;
          gap: 2px;
          opacity: 0;
          transition: opacity 0.15s;
          flex-shrink: 0;
        }
        .workspace-header:hover .workspace-actions {
          opacity: 1;
        }
        .workspace-action-btn {
          width: 24px;
          height: 24px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-sm);
          color: var(--color-text-tertiary);
          background: transparent;
          border: none;
          cursor: pointer;
          transition: all 0.15s;
        }
        .workspace-action-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
        }
        .workspace-sessions {
          overflow: hidden;
          transition: max-height 0.25s ease, opacity 0.2s ease;
        }
        .session-item {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 6px;
          padding: 4px 8px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          margin-bottom: 1px;
          border: 1px solid transparent;
          transition: all 0.15s;
        }
        .session-item:hover {
          background: var(--color-bg-hover);
          border-color: transparent;
        }
        .session-item.active {
          background: var(--color-bg-hover);
          border-color: transparent;
        }
        .session-item-title {
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-primary);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
          flex: 1;
          min-width: 0;
          padding-left: 20px;
        }
        .session-item.active .session-item-title {
          color: var(--color-text-primary);
        }
        .session-item-actions {
          display: flex;
          align-items: center;
          gap: 1px;
          opacity: 0;
          transition: opacity 0.15s;
          flex-shrink: 0;
        }
        .session-item:hover .session-item-actions {
          opacity: 1;
        }
        .session-action-btn {
          width: 22px;
          height: 22px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-sm);
          color: var(--color-text-tertiary);
          background: transparent;
          border: none;
          cursor: pointer;
          transition: all 0.15s;
        }
        .session-action-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
        }
        .session-action-btn-danger:hover {
          background: var(--color-error-bg);
          color: var(--color-error);
        }
        .session-edit-input {
          width: 100%;
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-primary);
          padding: 2px 6px;
          border: 1px solid var(--color-accent);
          border-radius: var(--radius-sm);
          background: var(--color-bg);
          outline: none;
        }
      `}</style>
    </>
  );
}
