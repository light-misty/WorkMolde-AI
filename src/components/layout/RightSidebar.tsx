import { useState } from "react";
import { useTranslation } from "react-i18next";
import { AgentInfoSection } from "../sidebar/AgentInfoSection";
import { FileTreeSection } from "../sidebar/FileTreeSection";
import { SessionListSection } from "../sidebar/SessionListSection";
import { Icon } from "../common/Icon";
import { useWorkspaceStore } from "../../stores/useWorkspaceStore";

interface RightSidebarProps {
  /** 文件预览回调 */
  onOpenPreview: (filePath: string, fileName: string) => void;
  /** 版本历史回调 */
  onOpenVersionHistory: (filePath: string, fileName: string) => void;
  /** 切换会话（父组件需同步切换工作区） */
  onSwitchSession: (sessionId: string, workspaceId?: string) => void;
  /** 为指定工作区新建会话 */
  onCreateSession: (workspaceId: string) => void;
  /** 切换工作区并准备展示文件树 */
  onShowFiles: (workspaceId: string) => void;
  /** 删除当前会话后清理工作流 */
  onDeleteCurrentSession: (nextSessionId: string | null) => void;
}

type RightSidebarView = "sessions" | "files";

/**
 * 右侧栏容器
 * 在「会话列表」与「工作区文件」两种视图之间切换。
 */
export function RightSidebar({
  onOpenPreview,
  onOpenVersionHistory,
  onSwitchSession,
  onCreateSession,
  onShowFiles,
  onDeleteCurrentSession,
}: RightSidebarProps) {
  const { t } = useTranslation();
  const [view, setView] = useState<RightSidebarView>("sessions");
  const { workspaces, currentWorkspaceId } = useWorkspaceStore();

  const currentWorkspace = workspaces.find((w) => w.id === currentWorkspaceId);

  const handleShowFiles = (workspaceId: string) => {
    // 通知父组件切换活动工作区，保证文件树加载正确路径
    onShowFiles(workspaceId);
    setView("files");
  };

  const handleBackToSessions = () => {
    setView("sessions");
  };

  return (
    <div className="right-sidebar">
      {view === "files" ? (
        <>
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
        </>
      ) : (
        <>
          {/* Agent 信息区置于会话列表上方，默认收缩，可点击展开 */}
          <AgentInfoSection />
          <SessionListSection
            onSwitchSession={onSwitchSession}
            onCreateSession={onCreateSession}
            onShowFiles={handleShowFiles}
            onDeleteCurrentSession={onDeleteCurrentSession}
          />
        </>
      )}

      <style>{`
        .right-sidebar {
          display: flex;
          flex-direction: column;
          height: 100%;
          width: 100%;
          overflow: hidden;
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
          font-size: 12px;
          font-weight: 500;
          color: var(--color-text-secondary);
          background: transparent;
          border: none;
          cursor: pointer;
          transition: all 0.15s;
        }
        .file-tree-back-btn:hover {
          background: var(--color-accent-bg);
          color: var(--color-accent);
        }
        .file-tree-workspace-name {
          font-size: 12px;
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
      `}</style>
    </div>
  );
}
