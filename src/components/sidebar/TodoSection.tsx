import { SidebarSection } from "../layout/Sidebar";
import { Icon } from "../common/Icon";

interface TodoItem {
  id: string;
  text: string;
  done: boolean;
  active: boolean;
}

interface TodoSectionProps {
  items?: TodoItem[];
}

export function TodoSection({ items }: TodoSectionProps) {
  const todoItems = items ?? [];

  if (todoItems.length === 0) {
    return (
      <SidebarSection title="任务进度">
        <div className="td-empty" role="status">
          <Icon name="check-circle" size={20} className="td-empty-icon" />
          <span className="td-empty-text">暂无任务</span>
        </div>
        <style>{`
          .td-empty {
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            gap: 6px;
            padding: 20px 16px;
          }
          .td-empty-icon {
            opacity: 0.4;
          }
          .td-empty-text {
            font-size: 12px;
            color: var(--color-text-quaternary);
          }
        `}</style>
      </SidebarSection>
    );
  }

  const doneCount = todoItems.filter((i) => i.done).length;
  const totalCount = todoItems.length;
  const progress = totalCount > 0 ? (doneCount / totalCount) * 100 : 0;

  return (
    <SidebarSection title="任务进度">
      {/* 进度摘要 */}
      <div className="td-summary">
        <div className="td-progress-track" role="progressbar" aria-valuenow={doneCount} aria-valuemin={0} aria-valuemax={totalCount} aria-label="任务进度">
          <div
            className="td-progress-fill"
            style={{ width: `${progress}%` }}
          />
        </div>
        <span className="td-progress-label">
          {doneCount}/{totalCount} 已完成
        </span>
      </div>

      {/* 任务列表 */}
      <div className="td-list" role="list">
        {todoItems.map((item, index) => (
          <div
            key={item.id}
            className={`td-item ${
              item.done ? "done" :
              item.active ? "active" :
              "pending"
            }`}
            role="listitem"
          >
            {/* 连接线 */}
            {index > 0 && <div className="td-connector" />}

            {/* 状态指示器 */}
            <span
              className={`td-indicator ${
                item.done
                  ? "ind-done"
                  : item.active
                  ? "ind-active"
                  : "ind-pending"
              }`}
            >
              {item.done && (
                <Icon name="check" size={10} strokeWidth={3} />
              )}
              {item.active && (
                <span className="td-active-pulse" />
              )}
            </span>

            {/* 任务文本 */}
            <span className={`td-text ${item.done ? "td-text-done" : ""}`}>
              {item.text}
            </span>
          </div>
        ))}
      </div>

      <style>{`
        .td-summary {
          display: flex;
          align-items: center;
          gap: 10px;
          margin-bottom: 10px;
        }
        .td-progress-track {
          flex: 1;
          height: 4px;
          background: var(--color-border-light);
          border-radius: 2px;
          overflow: hidden;
        }
        .td-progress-fill {
          height: 100%;
          border-radius: 2px;
          background: linear-gradient(90deg, var(--color-accent), var(--color-success));
          transition: width 0.5s cubic-bezier(0.4, 0, 0.2, 1);
        }
        .td-progress-label {
          font-size: 11px;
          font-weight: 500;
          color: var(--color-text-quaternary);
          white-space: nowrap;
          font-family: var(--font-mono);
        }
        .td-list {
          display: flex;
          flex-direction: column;
          position: relative;
        }
        .td-item {
          display: flex;
          align-items: center;
          gap: 10px;
          padding: 6px 8px;
          border-radius: var(--radius-sm);
          position: relative;
          transition: background 0.15s;
        }
        .td-item:hover {
          background: var(--color-accent-bg);
        }
        .td-item.done {
          color: var(--color-text-quaternary);
        }
        .td-item.active {
          color: var(--color-accent);
        }
        .td-item.pending {
          color: var(--color-text-secondary);
        }
        .td-connector {
          position: absolute;
          left: 16px;
          top: -4px;
          width: 1px;
          height: 8px;
          background: var(--color-border-light);
        }
        .td-indicator {
          width: 16px;
          height: 16px;
          border-radius: 50%;
          display: flex;
          align-items: center;
          justify-content: center;
          flex-shrink: 0;
          transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
        }
        .ind-done {
          background: var(--color-success);
          color: white;
          box-shadow: 0 1px 3px rgba(52, 199, 36, 0.3);
        }
        .ind-active {
          border: 2px solid var(--color-accent);
          background: var(--color-accent-light);
          box-shadow: 0 0 0 3px rgba(51, 112, 255, 0.1);
        }
        .ind-pending {
          border: 1.5px solid var(--color-border-strong);
          background: var(--color-bg);
        }
        .td-active-pulse {
          width: 6px;
          height: 6px;
          border-radius: 50%;
          background: var(--color-accent);
          animation: tdPulse 1.5s ease-in-out infinite;
        }
        .td-text {
          flex: 1;
          font-size: 12px;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
          line-height: 1.4;
        }
        .td-item.active .td-text {
          font-weight: 600;
        }
        .td-text-done {
          text-decoration: line-through;
          text-decoration-color: var(--color-text-quaternary);
        }
        @keyframes tdPulse {
          0%, 100% { opacity: 1; transform: scale(1); }
          50% { opacity: 0.5; transform: scale(0.85); }
        }
      `}</style>
    </SidebarSection>
  );
}
