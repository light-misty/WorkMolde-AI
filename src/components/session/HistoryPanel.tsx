import { useEffect } from "react";
import { Icon } from "../common/Icon";
import { useSessionStore } from "../../stores/useSessionStore";

interface HistoryPanelProps {
  open: boolean;
  onClose: () => void;
  onSwitchSession: (sessionId: string) => void;
}

export function HistoryPanel({ open, onClose, onSwitchSession }: HistoryPanelProps) {
  const { sessions, currentSessionId, loadSessions } = useSessionStore();

  // 面板打开时刷新会话列表，确保显示最新数据（包括通过 Agent 新创建的会话）
  useEffect(() => {
    if (open) {
      loadSessions();
    }
  }, [open, loadSessions]);

  return (
    <>
      {open && (
        <div
          className="fixed inset-0 z-[140]"
          onClick={onClose}
        />
      )}

      <div
        className={`fixed top-[52px] left-0 w-[280px] bottom-0 bg-bg border-r border-border z-[150] flex flex-col transition-transform duration-250 ${
          open ? "translate-x-0" : "-translate-x-full"
        }`}
      >
        <div className="px-4 py-4 border-b border-border flex items-center justify-between">
          <h3 className="text-[14px] font-semibold">历史会话</h3>
          <button
            className="w-[28px] h-[28px] flex items-center justify-center rounded-[var(--radius-sm)] transition-colors duration-150 text-text-secondary"
            onClick={onClose}
          >
            <Icon name="close" size={16} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-2">
          {sessions.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full text-text-tertiary text-[13px] px-4 text-center">
              <Icon name="history" size={24} className="mb-3 opacity-50" />
              <p>暂无历史会话</p>
            </div>
          ) : (
            sessions.map((s) => (
              <div
                key={s.id}
                className={`px-3 py-[10px] rounded-[var(--radius-sm)] cursor-pointer transition-colors duration-150 mb-[2px] ${
                  s.id === currentSessionId ? "bg-accent-light" : "hover:bg-bg-sub"
                }`}
                onClick={() => { onSwitchSession(s.id); onClose(); }}
              >
                <div className={`text-[13px] font-medium mb-1 ${
                  s.id === currentSessionId ? "text-accent" : "text-text-primary"
                }`}>
                  {s.title}
                </div>
                <div className="text-[11px] text-text-tertiary flex gap-2">
                  <span>{new Date(s.updatedAt).toLocaleDateString("zh-CN", { month: "numeric", day: "numeric" })}</span>
                  <span>{s.status}</span>
                </div>
              </div>
            ))
          )}
        </div>
      </div>
    </>
  );
}
