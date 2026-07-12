import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../common/Button";
import { Icon } from "../common/Icon";
import { useToastStore } from "../../stores/useToastStore";
import { lspGetStatus, lspRestartServer, lspStopAll } from "../../services/tauri";
import type { LspServerInfo, LspServerStatus } from "../../types";

/** 将 LSP 状态映射为翻译键后缀 */
function statusKey(status: LspServerStatus): string {
  switch (status) {
    case "ready":
      // ready 视为运行中
      return "statusRunning";
    case "terminated":
      // terminated 视为已停止
      return "statusStopped";
    case "starting":
      return "statusStarting";
    case "error":
      return "statusError";
    case "stopped":
    default:
      return "statusStopped";
  }
}

/** 状态对应的 CSS 颜色类名 */
function statusColorClass(status: LspServerStatus): string {
  switch (status) {
    case "ready":
      return "lsp-status-running";
    case "starting":
      return "lsp-status-starting";
    case "error":
      return "lsp-status-error";
    case "stopped":
    case "terminated":
    default:
      return "lsp-status-stopped";
  }
}

/** 格式化时间戳为可读时间 */
function formatTime(ts: number): string {
  if (!ts) return "-";
  return new Date(ts).toLocaleString();
}

export function LspStatusPanel() {
  const { t } = useTranslation();
  const addToast = useToastStore((s) => s.addToast);

  const [servers, setServers] = useState<LspServerInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // 正在重启的语言集合，用于禁用对应按钮
  const [restartingLangs, setRestartingLangs] = useState<Set<string>>(new Set());
  const [stoppingAll, setStoppingAll] = useState(false);

  /** 加载 LSP 服务器状态 */
  const loadStatus = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await lspGetStatus();
      setServers(list);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      addToast("error", `${t("settings.lsp.loadError")}: ${msg}`);
    } finally {
      setLoading(false);
    }
  }, [addToast, t]);

  useEffect(() => {
    loadStatus();
  }, [loadStatus]);

  /** 重启单个服务器 */
  const handleRestart = useCallback(async (language: string) => {
    setRestartingLangs((prev) => {
      const next = new Set(prev);
      next.add(language);
      return next;
    });
    try {
      await lspRestartServer(language);
      addToast("success", t("settings.lsp.restartSuccess"));
      await loadStatus();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      addToast("error", `${t("settings.lsp.restartError")}: ${msg}`);
    } finally {
      setRestartingLangs((prev) => {
        const next = new Set(prev);
        next.delete(language);
        return next;
      });
    }
  }, [addToast, loadStatus, t]);

  /** 停止所有服务器 */
  const handleStopAll = useCallback(async () => {
    setStoppingAll(true);
    try {
      await lspStopAll();
      addToast("success", t("settings.lsp.stopAllSuccess"));
      await loadStatus();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      addToast("error", `${t("settings.lsp.stopAllError")}: ${msg}`);
    } finally {
      setStoppingAll(false);
    }
  }, [addToast, loadStatus, t]);

  return (
    <div>
      {/* 标题区 */}
      <div className="section-header">
        <span className="section-title">{t("settings.lsp.title")}</span>
        <span className="section-badge lsp-experimental-badge">{t("settings.lsp.experimental")}</span>
      </div>
      <div className="lsp-description">{t("settings.lsp.description")}</div>

      {/* 操作按钮区 */}
      <div className="lsp-actions">
        <Button
          variant="ghost"
          size="sm"
          onClick={loadStatus}
          disabled={loading}
          className="lsp-action-btn"
        >
          <Icon name="refresh" size={14} />
          <span>{t("settings.lsp.refresh")}</span>
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={handleStopAll}
          disabled={stoppingAll || loading || servers.length === 0}
          className="lsp-action-btn"
        >
          <Icon name="stop" size={14} />
          <span>{t("settings.lsp.stopAll")}</span>
        </Button>
      </div>

      {/* 错误提示 */}
      {error && (
        <div className="lsp-error-banner">{t("settings.lsp.loadError")}: {error}</div>
      )}

      {/* 服务器列表 */}
      {servers.length === 0 ? (
        <div className="lsp-empty">{t("settings.lsp.noServers")}</div>
      ) : (
        <div className="lsp-table">
          <div className="lsp-table-header">
            <div className="lsp-col-language">{t("settings.lsp.language")}</div>
            <div className="lsp-col-status">{t("settings.lsp.status")}</div>
            <div className="lsp-col-server">{t("settings.lsp.serverName")}</div>
            <div className="lsp-col-workspace">{t("settings.lsp.workspaceRoot")}</div>
            <div className="lsp-col-started">{t("settings.lsp.startedAt")}</div>
            <div className="lsp-col-action"></div>
          </div>
          {servers.map((srv) => {
            const restarting = restartingLangs.has(srv.language);
            return (
              <div key={srv.language} className="lsp-table-row">
                <div className="lsp-col-language lsp-language-cell">{srv.language}</div>
                <div className="lsp-col-status">
                  <span className={`lsp-status-badge ${statusColorClass(srv.status)}`}>
                    {t(`settings.lsp.${statusKey(srv.status)}`)}
                  </span>
                </div>
                <div className="lsp-col-server lsp-mono-text">
                  {srv.serverName ? `${srv.serverName}${srv.serverVersion ? `@${srv.serverVersion}` : ""}` : "-"}
                </div>
                <div className="lsp-col-workspace lsp-mono-text" title={srv.workspaceRoot}>
                  {srv.workspaceRoot}
                </div>
                <div className="lsp-col-started lsp-mono-text">{formatTime(srv.startedAt)}</div>
                <div className="lsp-col-action">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleRestart(srv.language)}
                    disabled={restarting || loading}
                  >
                    <Icon name="refresh" size={14} />
                    <span style={{ marginLeft: 4 }}>{t("settings.lsp.restart")}</span>
                  </Button>
                </div>
                {srv.status === "error" && srv.error && (
                  <div className="lsp-error-detail lsp-mono-text">{srv.error}</div>
                )}
              </div>
            );
          })}
        </div>
      )}

      <style>{`
        .lsp-experimental-badge {
          background: var(--color-purple-light);
          color: var(--color-purple);
        }
        .lsp-description {
          font-size: 12px;
          color: var(--color-text-quaternary);
          margin-bottom: 16px;
        }
        .lsp-actions {
          display: flex;
          gap: 8px;
          margin-bottom: 16px;
        }
        .lsp-action-btn {
          display: inline-flex;
          align-items: center;
          gap: 4px;
        }
        .lsp-error-banner {
          padding: 8px 12px;
          margin-bottom: 12px;
          border-radius: var(--radius-sm);
          background: var(--color-error-bg);
          color: var(--color-error);
          font-size: 12px;
        }
        .lsp-empty {
          padding: 32px 12px;
          text-align: center;
          font-size: 12px;
          color: var(--color-text-quaternary);
        }
        .lsp-table {
          display: flex;
          flex-direction: column;
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-sm);
          overflow: hidden;
        }
        .lsp-table-header {
          display: grid;
          grid-template-columns: 120px 100px 160px 1fr 160px 120px;
          gap: 8px;
          padding: 8px 12px;
          background: var(--color-bg-sub);
          font-size: 11px;
          font-weight: 600;
          color: var(--color-text-secondary);
          text-transform: uppercase;
          letter-spacing: 0.3px;
        }
        .lsp-table-row {
          display: grid;
          grid-template-columns: 120px 100px 160px 1fr 160px 120px;
          gap: 8px;
          padding: 10px 12px;
          border-top: 1px solid var(--color-border-light);
          font-size: 12px;
          color: var(--color-text-primary);
          position: relative;
          transition: background 0.15s;
        }
        .lsp-table-row:hover {
          background: var(--color-accent-bg);
        }
        .lsp-language-cell {
          font-weight: 500;
        }
        .lsp-mono-text {
          font-family: monospace;
          font-size: 11px;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .lsp-status-badge {
          display: inline-block;
          padding: 2px 8px;
          border-radius: 10px;
          font-size: 10px;
          font-weight: 500;
        }
        .lsp-status-running {
          background: var(--color-success-bg);
          color: var(--color-success);
        }
        .lsp-status-starting {
          background: var(--color-warning-bg);
          color: var(--color-warning);
        }
        .lsp-status-error {
          background: var(--color-error-bg);
          color: var(--color-error);
        }
        .lsp-status-stopped {
          background: var(--color-bg-hover);
          color: var(--color-text-quaternary);
        }
        .lsp-error-detail {
          grid-column: 1 / -1;
          margin-top: 6px;
          padding: 6px 8px;
          border-radius: var(--radius-sm);
          background: var(--color-error-bg);
          color: var(--color-error);
          font-size: 11px;
          word-break: break-all;
        }
      `}</style>
    </div>
  );
}
