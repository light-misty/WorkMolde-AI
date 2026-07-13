import { useEffect, useState } from "react";
import { useWorkspaceStore } from "../../stores/useWorkspaceStore";
import { getWorkspaceGitStatus } from "../../services/tauri";
import type { GitStatus } from "../../types";

const gitStatusCache = new Map<string, GitStatus>();

interface WorkspaceGitStatusProps {
  pageLevel?: boolean;
}

export function WorkspaceGitStatus({ pageLevel = false }: WorkspaceGitStatusProps) {
  const { currentWorkspaceId, workspaces } = useWorkspaceStore();
  const [gitStatus, setGitStatus] = useState<GitStatus | null>(null);
  const currentWs = workspaces.find((w) => w.id === currentWorkspaceId);

  useEffect(() => {
    if (!currentWs?.path) {
      setGitStatus(null);
      return;
    }

    const path = currentWs.path;

    if (gitStatusCache.has(path)) {
      setGitStatus(gitStatusCache.get(path)!);
    }

    let cancelled = false;

    const fetchStatus = () => {
      getWorkspaceGitStatus(path)
        .then((status) => {
          gitStatusCache.set(path, status);
          if (!cancelled) setGitStatus(status);
        })
        .catch(() => {
          if (!cancelled) setGitStatus(null);
        });
    };

    fetchStatus();
    const pollId = setInterval(fetchStatus, 3000);

    return () => {
      cancelled = true;
      clearInterval(pollId);
    };
  }, [currentWs?.path]);

  if (!currentWs) return null;

  const label = gitStatus?.isGitRepo && gitStatus.branchName
    ? `${currentWs.path}:${gitStatus.branchName}`
    : currentWs.path;

  return (
    <div className={pageLevel ? "ws-git-status-page" : "ws-git-status-inline"}>
      <span className="ws-git-status-text">{label}</span>
      <style>{`
        .ws-git-status-text {
          font-size: 11px;
          color: var(--color-text-quaternary);
          font-family: var(--font-mono);
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
          user-select: none;
        }
        .ws-git-status-page {
          position: absolute;
          bottom: 8px;
          right: 24px;
          max-width: 50%;
          pointer-events: none;
        }
        .ws-git-status-inline {
          display: flex;
          align-items: center;
          min-width: 0;
          max-width: 240px;
        }
      `}</style>
    </div>
  );
}
