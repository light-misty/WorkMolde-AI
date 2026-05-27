import { useEffect, useState, useRef } from "react";
import { Icon } from "../common/Icon";
import { useSessionStore } from "../../stores/useSessionStore";

interface HistoryPanelProps {
  open: boolean;
  onClose: () => void;
  onSwitchSession: (sessionId: string) => void;
  // 删除当前会话后的回调，用于清空工作流或切换到其他会话
  onDeleteCurrentSession?: (nextSessionId: string | null) => void;
}

export function HistoryPanel({ open, onClose, onSwitchSession, onDeleteCurrentSession }: HistoryPanelProps) {
  const { sessions, currentSessionId, loadSessions, deleteSession, updateSessionTitle } = useSessionStore();

  // 正在编辑的会话ID
  const [editingId, setEditingId] = useState<string | null>(null);
  // 编辑中的标题文本
  const [editingTitle, setEditingTitle] = useState("");
  const editInputRef = useRef<HTMLInputElement>(null);

  // 删除确认弹窗状态
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const [deleteConfirmTitle, setDeleteConfirmTitle] = useState("");

  useEffect(() => {
    if (open) {
      loadSessions();
    }
  }, [open, loadSessions]);

  // 当进入编辑模式时，自动聚焦输入框
  useEffect(() => {
    if (editingId && editInputRef.current) {
      editInputRef.current.focus();
      editInputRef.current.select();
    }
  }, [editingId]);

  // 开始重命名
  const handleStartRename = (sessionId: string, currentTitle: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setEditingId(sessionId);
    setEditingTitle(currentTitle);
  };

  // 确认重命名
  const handleConfirmRename = async () => {
    if (editingId && editingTitle.trim()) {
      await updateSessionTitle(editingId, editingTitle.trim());
    }
    setEditingId(null);
    setEditingTitle("");
  };

  // 取消重命名
  const handleCancelRename = () => {
    setEditingId(null);
    setEditingTitle("");
  };

  // 重命名输入框键盘事件
  const handleRenameKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      handleConfirmRename();
    } else if (e.key === "Escape") {
      handleCancelRename();
    }
  };

  // 点击删除按钮，显示确认弹窗
  const handleDeleteClick = (sessionId: string, sessionTitle: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setDeleteConfirmId(sessionId);
    setDeleteConfirmTitle(sessionTitle);
  };

  // 确认删除
  const handleConfirmDelete = async () => {
    if (deleteConfirmId) {
      // 判断是否删除的是当前会话
      const isDeletingCurrent = deleteConfirmId === currentSessionId;
      
      // 计算删除后的下一个会话ID（在删除前计算，因为删除后列表会变化）
      const currentIndex = sessions.findIndex(s => s.id === deleteConfirmId);
      const nextSessionId = sessions.length > 1
        ? (sessions[currentIndex + 1]?.id || sessions[currentIndex - 1]?.id || null)
        : null;
      
      await deleteSession(deleteConfirmId);
      
      // 如果删除的是当前会话，通知父组件处理工作流更新
      if (isDeletingCurrent && onDeleteCurrentSession) {
        onDeleteCurrentSession(nextSessionId);
      }
      
      setDeleteConfirmId(null);
      setDeleteConfirmTitle("");
    }
  };

  // 取消删除
  const handleCancelDelete = () => {
    setDeleteConfirmId(null);
    setDeleteConfirmTitle("");
  };

  return (
    <>
      {/* 删除确认弹窗 */}
      {deleteConfirmId && (
        <div className="delete-confirm-overlay" onClick={handleCancelDelete} role="dialog" aria-label="确认删除" aria-modal="true">
          <div className="delete-confirm-dialog" onClick={(e) => e.stopPropagation()}>
            <div className="delete-confirm-icon">
              <Icon name="warning" size={24} />
            </div>
            <div className="delete-confirm-title">确认删除会话</div>
            <div className="delete-confirm-desc">
              确定要删除会话 "{deleteConfirmTitle}" 吗？此操作无法撤销。
            </div>
            <div className="delete-confirm-actions">
              <button className="btn btn-ghost" onClick={handleCancelDelete}>
                取消
              </button>
              <button className="btn btn-danger" onClick={handleConfirmDelete}>
                删除
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 从右侧滑入，完全覆盖右侧栏 */}
      <div
        className={`history-panel-container fixed top-[52px] right-0 w-[300px] bottom-0 z-[150] flex flex-col transition-transform duration-300 ease-out ${
          open ? "translate-x-0" : "translate-x-full"
        }`}
        role="complementary"
        aria-label="历史会话"
      >
        <div className="history-header">
          <h3 className="history-title">历史会话</h3>
          <button
            className="history-close-btn"
            onClick={onClose}
            aria-label="关闭历史面板"
          >
            <Icon name="close" size={16} />
          </button>
        </div>

        <div className="history-list" role="list" aria-label="会话列表">
          {sessions.length === 0 ? (
            <div className="history-empty" role="status">
              <Icon name="history" size={32} className="opacity-30" />
              <p>暂无历史会话</p>
            </div>
          ) : (
            sessions.map((s) => (
              <div
                key={s.id}
                className={`history-item ${s.id === currentSessionId ? "active" : ""}`}
                role="listitem"
                aria-selected={s.id === currentSessionId}
                onClick={() => {
                  if (editingId === s.id) return;
                  onSwitchSession(s.id);
                  onClose();
                }}
              >
                {/* 编辑模式：显示输入框 */}
                {editingId === s.id ? (
                  <div className="history-item-edit">
                    <input
                      ref={editInputRef}
                      className="history-edit-input"
                      value={editingTitle}
                      onChange={(e) => setEditingTitle(e.target.value)}
                      onKeyDown={handleRenameKeyDown}
                      onBlur={handleConfirmRename}
                    />
                  </div>
                ) : (
                  <>
                    <div className="history-item-content">
                      <div className={`history-item-title ${s.id === currentSessionId ? "text-accent" : ""}`}>
                        {s.title}
                      </div>
                      <div className="history-item-meta">
                        <span>{new Date(s.updatedAt).toLocaleDateString("zh-CN", { month: "numeric", day: "numeric" })}</span>
                        <span className="history-status">{s.status}</span>
                      </div>
                    </div>
                    {/* 操作按钮：编辑和删除 */}
                    <div className="history-item-actions">
                      <button
                        className="history-action-btn"
                        title="重命名"
                        aria-label="重命名会话"
                        onClick={(e) => handleStartRename(s.id, s.title, e)}
                      >
                        <Icon name="edit" size={14} />
                      </button>
                      <button
                        className="history-action-btn history-action-btn-danger"
                        title="删除会话"
                        aria-label="删除会话"
                        onClick={(e) => handleDeleteClick(s.id, s.title, e)}
                      >
                        <Icon name="trash" size={14} />
                      </button>
                    </div>
                  </>
                )}
              </div>
            ))
          )}
        </div>

        <style>{`
          .history-panel-container {
            background: var(--color-bg-sub);
          }
          .history-header {
            padding: 16px;
            border-bottom: 1px solid var(--color-border-light);
            display: flex;
            align-items: center;
            justify-content: space-between;
          }
          .history-title {
            font-size: 14px;
            font-weight: 600;
            color: var(--color-text-primary);
          }
          .history-close-btn {
            width: 28px;
            height: 28px;
            display: flex;
            align-items: center;
            justify-content: center;
            border-radius: var(--radius-sm);
            color: var(--color-text-secondary);
            transition: all 0.15s;
          }
          .history-close-btn:hover {
            background: var(--color-bg-hover);
            color: var(--color-text-primary);
          }
          .history-list {
            flex: 1;
            overflow-y: auto;
            padding: 8px;
          }
          .history-empty {
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            height: 100%;
            gap: 12px;
            color: var(--color-text-quaternary);
            font-size: 13px;
          }
          .history-item {
            padding: 10px 12px;
            border-radius: var(--radius-sm);
            cursor: pointer;
            transition: all 0.15s;
            margin-bottom: 2px;
            border: 1px solid transparent;
            display: flex;
            align-items: flex-start;
            justify-content: space-between;
            gap: 8px;
          }
          .history-item:hover {
            background: var(--color-accent-bg);
            border-color: var(--color-accent-light);
          }
          .history-item.active {
            background: var(--color-accent-light);
            border-color: var(--color-accent-light);
          }
          .history-item-content {
            flex: 1;
            min-width: 0;
          }
          .history-item-title {
            font-size: 13px;
            font-weight: 500;
            margin-bottom: 4px;
            color: var(--color-text-primary);
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
          }
          .history-item-meta {
            font-size: 11px;
            color: var(--color-text-quaternary);
            display: flex;
            gap: 8px;
            align-items: center;
          }
          .history-status {
            padding: 1px 6px;
            border-radius: 3px;
            background: var(--color-bg);
            font-size: 10px;
            font-weight: 500;
          }
          .history-item.active .history-status {
            background: var(--color-accent-light);
            color: var(--color-accent);
          }
          /* 操作按钮区域 */
          .history-item-actions {
            display: flex;
            align-items: center;
            gap: 2px;
            opacity: 0;
            transition: opacity 0.15s;
            flex-shrink: 0;
            margin-top: 1px;
          }
          .history-item:hover .history-item-actions {
            opacity: 1;
          }
          .history-action-btn {
            width: 26px;
            height: 26px;
            display: flex;
            align-items: center;
            justify-content: center;
            border-radius: var(--radius-sm);
            color: var(--color-text-tertiary);
            transition: all 0.15s;
          }
          .history-action-btn:hover {
            background: var(--color-bg-hover);
            color: var(--color-text-primary);
          }
          .history-action-btn-danger:hover {
            background: var(--color-error-bg);
            color: var(--color-error);
          }
          /* 编辑模式输入框 */
          .history-item-edit {
            flex: 1;
          }
          .history-edit-input {
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
          /* 删除确认弹窗 */
          .delete-confirm-overlay {
            position: fixed;
            inset: 0;
            z-index: 200;
            display: flex;
            align-items: center;
            justify-content: center;
            background: var(--color-overlay);
          }
          .delete-confirm-dialog {
            background: var(--color-bg);
            border-radius: var(--radius-lg);
            padding: 24px;
            width: 360px;
            max-width: 90vw;
            box-shadow: var(--shadow-lg);
            animation: scaleIn 0.2s ease;
          }
          .delete-confirm-icon {
            width: 48px;
            height: 48px;
            border-radius: 50%;
            background: var(--color-error-bg);
            display: flex;
            align-items: center;
            justify-content: center;
            margin: 0 auto 16px;
            color: var(--color-error);
          }
          .delete-confirm-title {
            font-size: 16px;
            font-weight: 600;
            color: var(--color-text-primary);
            text-align: center;
            margin-bottom: 8px;
          }
          .delete-confirm-desc {
            font-size: 13px;
            color: var(--color-text-secondary);
            text-align: center;
            line-height: 1.5;
            margin-bottom: 20px;
          }
          .delete-confirm-actions {
            display: flex;
            gap: 12px;
            justify-content: center;
          }
        `}</style>
      </div>
    </>
  );
}
