import type { MouseEvent } from "react";
import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { DeleteConfirmDialog } from "../common/DeleteConfirmDialog";
import { Icon } from "../common/Icon";
import { useSessionStore } from "../../stores/useSessionStore";
import { useWorkspaceStore } from "../../stores/useWorkspaceStore";
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

  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState("");
  const editInputRef = useRef<HTMLInputElement>(null);

  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const [deleteConfirmTitle, setDeleteConfirmTitle] = useState("");

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
    const current = workspaces.find((w) => w.id === currentWorkspaceId);

    // 当前工作区排在最前（即使没有会话也显示，便于用户新建会话）
    if (current) {
      result.push({ workspace: current, sessions: map.get(current.id) || [] });
    }

    // 其余工作区按名称排序展示（即使没有会话也显示）
    workspaces
      .filter((w) => w.id !== currentWorkspaceId)
      .sort((a, b) => a.name.localeCompare(b.name))
      .forEach((w) => {
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
    setTimeout(() => editInputRef.current?.focus(), 0);
  };

  const confirmRename = async () => {
    if (editingId && editingTitle.trim()) {
      await updateSessionTitle(editingId, editingTitle.trim());
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

    await deleteSession(deleteConfirmId);

    if (isDeletingCurrent) {
      onDeleteCurrentSession(nextSessionId || null);
    }

    setDeleteConfirmId(null);
    setDeleteConfirmTitle("");
  };

  const formatDate = (dateStr: string) => {
    try {
      return new Date(dateStr).toLocaleDateString("zh-CN", { month: "numeric", day: "numeric" });
    } catch {
      return "";
    }
  };

  return (
    <>
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

      <div className="session-list-section">
        <div className="session-list-header">
          <span className="session-list-title">{t("sessionList.title")}</span>
        </div>

        <div className="session-list-body">
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
                            <div className="session-item-left">
                              <Icon name="file" size={13} className="session-item-icon" />
                              <div className="session-item-content">
                                <div className="session-item-title" title={s.title}>
                                  {s.title}
                                </div>
                                <div className="session-item-meta">
                                  <span>{formatDate(s.updatedAt)}</span>
                                  <span className="session-message-count">{s.messageCount} 条</span>
                                </div>
                              </div>
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
          border-bottom: 1px solid var(--color-border-light);
        }
        .session-list-header {
          padding: 10px 16px;
          display: flex;
          align-items: center;
          justify-content: space-between;
        }
        .session-list-title {
          font-size: 11px;
          font-weight: 600;
          color: var(--color-text-secondary);
          letter-spacing: 0.6px;
          text-transform: uppercase;
        }
        .session-list-body {
          flex: 1;
          overflow-y: auto;
          padding: 0 8px 8px;
        }
        .session-list-empty {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          padding: 32px 12px;
          gap: 10px;
          color: var(--color-text-quaternary);
          font-size: 12px;
          text-align: center;
        }
        .workspace-group {
          margin-bottom: 2px;
        }
        .workspace-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 7px 8px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          user-select: none;
          transition: background 0.15s;
        }
        .workspace-header:hover {
          background: var(--color-accent-bg);
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
          font-size: 13px;
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
          padding-left: 12px;
        }
        .session-item {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 6px;
          padding: 7px 8px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          margin-bottom: 1px;
          border: 1px solid transparent;
          transition: all 0.15s;
        }
        .session-item:hover {
          background: var(--color-accent-bg);
          border-color: var(--color-accent-light);
        }
        .session-item.active {
          background: var(--color-accent-light);
          border-color: var(--color-accent-light);
        }
        .session-item-left {
          display: flex;
          align-items: center;
          gap: 6px;
          min-width: 0;
          flex: 1;
        }
        .session-item-icon {
          color: var(--color-text-tertiary);
          flex-shrink: 0;
        }
        .session-item-content {
          min-width: 0;
          flex: 1;
        }
        .session-item-title {
          font-size: 12px;
          font-weight: 500;
          color: var(--color-text-primary);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
          margin-bottom: 2px;
        }
        .session-item.active .session-item-title {
          color: var(--color-accent);
        }
        .session-item-meta {
          font-size: 10px;
          color: var(--color-text-quaternary);
          display: flex;
          gap: 6px;
          align-items: center;
        }
        .session-message-count {
          padding: 0 4px;
          border-radius: 3px;
          background: var(--color-bg);
          font-size: 9px;
        }
        .session-item.active .session-message-count {
          background: var(--color-accent-light);
          color: var(--color-accent);
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
          font-size: 12px;
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
