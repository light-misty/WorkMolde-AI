import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { check, type Update, type DownloadEvent } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { useWorkflowStore } from "../../stores/useWorkflowStore";
import { useUpdateStore } from "../../stores/useUpdateStore";

interface UpdateNotificationProps {
  open: boolean;
  onClose: () => void;
}

// 更新状态机：idle → checking → available → downloading → downloaded → installing/restarting
// downloaded 状态下显示"立即重启"和"稍后重启"按钮
type UpdateState = "idle" | "checking" | "available" | "downloading" | "downloaded" | "installing" | "restarting" | "error";

export function UpdateNotification({ open, onClose }: UpdateNotificationProps) {
  const { t } = useTranslation();
  const [state, setState] = useState<UpdateState>("idle");
  const [updateInfo, setUpdateInfo] = useState<Update | null>(null);
  const [errorMessage, setErrorMessage] = useState("");
  // 下载进度
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [downloadedBytes, setDownloadedBytes] = useState(0);
  const [totalBytes, setTotalBytes] = useState(0);

  // 从 workflow store 获取执行状态，用于判断智能体是否正在工作
  const executionStatus = useWorkflowStore((s) => s.executionStatus);
  // 智能体是否正在工作（running 或 stopping 状态）
  const isAgentWorking = executionStatus === "running" || executionStatus === "stopping";

  // 从 update store 获取方法，用于保存待安装的更新
  const setPendingUpdate = useUpdateStore((s) => s.setPendingUpdate);

  // 关闭通知：如果在 downloaded 状态关闭，自动保存待安装更新（等同于"稍后重启"）
  const handleClose = useCallback(() => {
    if (state === "downloaded" && updateInfo) {
      setPendingUpdate(updateInfo);
    }
    onClose();
  }, [state, updateInfo, setPendingUpdate, onClose]);

  // 重置所有状态
  const resetState = useCallback(() => {
    setState("idle");
    setUpdateInfo(null);
    setErrorMessage("");
    setDownloadProgress(0);
    setDownloadedBytes(0);
    setTotalBytes(0);
  }, []);

  // 关闭时重置状态
  useEffect(() => {
    if (!open) {
      resetState();
    }
  }, [open, resetState]);

  // 检查更新
  const handleCheck = useCallback(async () => {
    setState("checking");
    setErrorMessage("");
    try {
      const result = await check();
      if (result) {
        setUpdateInfo(result);
        setState("available");
      } else {
        // 没有可用更新，直接关闭通知
        setState("idle");
        handleClose();
      }
    } catch (err) {
      console.error("[UpdateNotification] 检查更新失败:", err);
      setErrorMessage(err instanceof Error ? err.message : String(err));
      setState("error");
    }
  }, [onClose, handleClose]);

  // 当组件打开时自动检查
  useEffect(() => {
    if (open && state === "idle") {
      handleCheck();
    }
  }, [open, state, handleCheck]);

  // 仅下载更新（不安装），下载完成后提示用户选择重启时机
  const handleDownload = useCallback(async () => {
    if (!updateInfo) return;

    setState("downloading");
    setDownloadProgress(0);
    setDownloadedBytes(0);
    setTotalBytes(0);

    try {
      await updateInfo.download((event: DownloadEvent) => {
        switch (event.event) {
          case "Started":
            // 下载开始，记录总大小
            if (event.data.contentLength) {
              setTotalBytes(event.data.contentLength);
            }
            break;
          case "Progress": {
            // 下载进度更新
            setDownloadedBytes((prev) => prev + event.data.chunkLength);
            break;
          }
          case "Finished":
            // 下载完成，进入已下载状态，等待用户选择重启时机
            setState("downloaded");
            break;
        }
      });
    } catch (err) {
      console.error("[UpdateNotification] 下载更新失败:", err);
      setErrorMessage(err instanceof Error ? err.message : String(err));
      setState("error");
    }
  }, [updateInfo]);

  // 立即重启：先安装已下载的更新，然后重启应用
  const handleRestartNow = useCallback(async () => {
    if (!updateInfo) return;
    // 智能体正在工作时禁止重启
    if (isAgentWorking) return;

    setState("installing");
    try {
      await updateInfo.install();
      setState("restarting");
      await relaunch();
    } catch (err) {
      console.error("[UpdateNotification] 安装更新/重启失败:", err);
      setErrorMessage(err instanceof Error ? err.message : String(err));
      setState("error");
    }
  }, [updateInfo, isAgentWorking]);

  // 稍后重启：将更新保存到全局 store，在下次关闭程序时安装
  const handleRestartLater = useCallback(() => {
    if (updateInfo) {
      // 保存更新引用到全局 store，App.tsx 会在窗口关闭时调用 install()
      setPendingUpdate(updateInfo);
    }
    onClose();
  }, [updateInfo, setPendingUpdate, onClose]);

  // 重新尝试
  const handleRetry = useCallback(() => {
    if (state === "error" && updateInfo) {
      // 如果之前已经获取到更新信息，直接重新下载
      handleDownload();
    } else {
      // 否则重新检查
      handleCheck();
    }
  }, [state, updateInfo, handleDownload, handleCheck]);

  // 格式化文件大小
  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return "0 B";
    const units = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
  };

  // 计算下载进度百分比
  useEffect(() => {
    if (totalBytes > 0) {
      setDownloadProgress(Math.round((downloadedBytes / totalBytes) * 100));
    }
  }, [downloadedBytes, totalBytes]);

  if (!open) return null;

  return (
    <div className="update-notification">
      {/* 关闭按钮：下载中和安装中不允许关闭 */}
      {state !== "downloading" && state !== "installing" && state !== "restarting" && (
        <button className="update-close" onClick={handleClose}>
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
            <path d="M2 2L12 12M12 2L2 12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
        </button>
      )}

      {/* 检查中 */}
      {state === "checking" && (
        <div className="update-body">
          <div className="update-icon update-icon-loading">
            <svg className="update-spin" width="20" height="20" viewBox="0 0 24 24" fill="none">
              <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="2.5" opacity="0.25" />
              <path d="M4 12a8 8 0 018-8" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
            </svg>
          </div>
          <div className="update-title">{t("update.checking")}</div>
        </div>
      )}

      {/* 发现新版本 */}
      {state === "available" && updateInfo && (
        <div className="update-body">
          <div className="update-title">{t("update.newVersionFound", { version: updateInfo.version })}</div>
          <div className="update-version-info">
            {t("update.currentVersionLabel", { version: updateInfo.currentVersion })}
          </div>
          {updateInfo.body && (
            <div className="update-changelog">
              {updateInfo.body}
            </div>
          )}
          <div className="update-actions">
            <button className="update-btn update-btn-primary" onClick={handleDownload}>
              {t("update.updateNow")}
            </button>
            <button className="update-btn update-btn-ghost" onClick={handleClose}>
              {t("update.later")}
            </button>
          </div>
        </div>
      )}

      {/* 下载中 */}
      {state === "downloading" && (
        <div className="update-body">
          <div className="update-title">{t("update.downloadingUpdate")}</div>
          <div className="update-progress-bar">
            <div
              className="update-progress-fill"
              style={{ width: `${downloadProgress}%` }}
            />
          </div>
          <div className="update-progress-info">
            <span>{downloadProgress}%</span>
            {totalBytes > 0 && (
              <span>{formatBytes(downloadedBytes)} / {formatBytes(totalBytes)}</span>
            )}
          </div>
        </div>
      )}

      {/* 下载完成，等待用户选择重启时机 */}
      {state === "downloaded" && updateInfo && (
        <div className="update-body">
          <div className="update-title">{t("update.installComplete")}</div>
          <div className="update-desc">{t("update.needRestart")}</div>
          {/* 智能体正在工作时显示提示 */}
          {isAgentWorking && (
            <div className="update-agent-warning">
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <path d="M7 1L13 12H1L7 1Z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
                <path d="M7 5.5V8" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
                <circle cx="7" cy="10" r="0.5" fill="currentColor" />
              </svg>
              <span>{t("update.agentRunning")}</span>
            </div>
          )}
          <div className="update-actions">
            <button
              className={`update-btn update-btn-primary ${isAgentWorking ? "update-btn-disabled" : ""}`}
              onClick={handleRestartNow}
              disabled={isAgentWorking}
              title={isAgentWorking ? t("update.agentRunningDesc") : undefined}
            >
              {t("update.restartNow")}
            </button>
            <button className="update-btn update-btn-ghost" onClick={handleRestartLater}>
              {t("update.restartLater")}
            </button>
          </div>
          <div className="update-later-hint">{t("update.restartLaterDesc")}</div>
        </div>
      )}

      {/* 安装中 */}
      {state === "installing" && (
        <div className="update-body">
          <div className="update-icon update-icon-loading">
            <svg className="update-spin" width="20" height="20" viewBox="0 0 24 24" fill="none">
              <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="2.5" opacity="0.25" />
              <path d="M4 12a8 8 0 018-8" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
            </svg>
          </div>
          <div className="update-title">{t("update.installingUpdate")}</div>
          <div className="update-desc">{t("update.installCompleteRestart")}</div>
        </div>
      )}

      {/* 重启中 */}
      {state === "restarting" && (
        <div className="update-body">
          <div className="update-icon update-icon-loading">
            <svg className="update-spin" width="20" height="20" viewBox="0 0 24 24" fill="none">
              <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="2.5" opacity="0.25" />
              <path d="M4 12a8 8 0 018-8" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
            </svg>
          </div>
          <div className="update-title">{t("update.installingUpdate")}</div>
        </div>
      )}

      {/* 错误状态 */}
      {state === "error" && (
        <div className="update-body">
          <div className="update-icon update-icon-error">
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
              <circle cx="10" cy="10" r="9" stroke="currentColor" strokeWidth="1.5" />
              <path d="M7 7L13 13M13 7L7 13" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            </svg>
          </div>
          <div className="update-title">{t("update.updateFailed")}</div>
          <div className="update-error-msg">{errorMessage}</div>
          <div className="update-actions">
            <button className="update-btn update-btn-primary" onClick={handleRetry}>
              {t("common.retry")}
            </button>
            <button className="update-btn update-btn-ghost" onClick={handleClose}>
              {t("common.close")}
            </button>
          </div>
        </div>
      )}

      <style>{`
        .update-notification {
          position: fixed;
          bottom: 20px;
          right: 20px;
          width: 360px;
          z-index: 9998;
          background: var(--color-bg-elevated);
          border: 1px solid var(--color-border);
          border-radius: var(--radius-lg);
          box-shadow: var(--shadow-xl);
          animation: updateSlideUp 0.3s ease forwards;
          overflow: hidden;
        }

        .update-body {
          padding: 20px;
          display: flex;
          flex-direction: column;
          gap: 10px;
        }

        .update-close {
          position: absolute;
          top: 12px;
          right: 12px;
          width: 24px;
          height: 24px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: 4px;
          color: var(--color-text-quaternary);
          transition: all 0.15s;
          z-index: 1;
        }

        .update-close:hover {
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
        }

        .update-icon {
          flex-shrink: 0;
          display: flex;
          align-items: center;
          justify-content: center;
        }

        .update-icon-loading {
          color: var(--color-accent);
        }

        .update-icon-success {
          color: var(--color-success);
        }

        .update-icon-error {
          color: var(--color-error);
        }

        .update-spin {
          animation: spin 1s linear infinite;
        }

        .update-title {
          font-size: 14px;
          font-weight: 600;
          color: var(--color-text-primary);
        }

        .update-desc {
          font-size: 12px;
          color: var(--color-text-tertiary);
        }

        .update-version-info {
          font-size: 12px;
          color: var(--color-text-tertiary);
          font-family: var(--font-mono);
        }

        .update-changelog {
          font-size: 12px;
          line-height: 1.6;
          color: var(--color-text-secondary);
          max-height: 120px;
          overflow-y: auto;
          padding: 8px 10px;
          background: var(--color-bg-sub);
          border-radius: var(--radius-sm);
          border: 1px solid var(--color-border-light);
          white-space: pre-wrap;
          word-break: break-word;
        }

        .update-progress-bar {
          width: 100%;
          height: 6px;
          background: var(--color-bg-sub);
          border-radius: 3px;
          overflow: hidden;
        }

        .update-progress-fill {
          height: 100%;
          background: var(--color-accent);
          border-radius: 3px;
          transition: width 0.2s ease;
        }

        .update-progress-info {
          display: flex;
          justify-content: space-between;
          font-size: 11px;
          color: var(--color-text-tertiary);
          font-family: var(--font-mono);
        }

        .update-error-msg {
          font-size: 12px;
          color: var(--color-error);
          line-height: 1.5;
          word-break: break-word;
        }

        /* 智能体工作警告提示 */
        .update-agent-warning {
          display: flex;
          align-items: center;
          gap: 6px;
          padding: 8px 10px;
          background: var(--color-warning-bg, rgba(250, 173, 20, 0.1));
          border-radius: var(--radius-sm);
          font-size: 12px;
          color: var(--color-warning, #faad14);
          line-height: 1.4;
        }

        .update-agent-warning svg {
          flex-shrink: 0;
          color: var(--color-warning, #faad14);
        }

        /* 稍后重启提示文字 */
        .update-later-hint {
          font-size: 11px;
          color: var(--color-text-quaternary);
          line-height: 1.4;
        }

        .update-actions {
          display: flex;
          gap: 8px;
          margin-top: 4px;
        }

        .update-btn {
          padding: 6px 16px;
          font-size: 12px;
          font-weight: 500;
          border-radius: var(--radius-sm);
          transition: all 0.15s;
          cursor: pointer;
        }

        .update-btn-primary {
          background: var(--color-accent);
          color: #fff;
        }

        .update-btn-primary:hover {
          background: var(--color-accent-hover);
        }

        /* 禁用状态的立即重启按钮 */
        .update-btn-disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .update-btn-disabled:hover {
          background: var(--color-accent);
        }

        .update-btn-ghost {
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
          border: 1px solid var(--color-border);
        }

        .update-btn-ghost:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
        }

        @keyframes updateSlideUp {
          from {
            opacity: 0;
            transform: translateY(20px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
        }
      `}</style>
    </div>
  );
}
