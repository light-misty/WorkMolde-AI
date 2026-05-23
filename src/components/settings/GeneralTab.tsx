import { useState } from "react";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { useSessionStore } from "../../stores/useSessionStore";
import { useToastStore } from "../../stores/useToastStore";
import type { AppSettings } from "../../types";
import * as tauriCmd from "../../services/tauri";

export function GeneralTab() {
  const { settings, updateSettings } = useSettingsStore();
  const { clearAllSessions } = useSessionStore();
  const addToast = useToastStore((s) => s.addToast);
  const [clearConfirm, setClearConfirm] = useState(false);
  const [exportingLog, setExportingLog] = useState(false);

  return (
    <div>
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">基本设置</span>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">作者名（全局默认）</div>
            <div className="setting-desc">生成文档时自动填充的作者元数据</div>
          </div>
          <input
            className="setting-input"
            value={settings.general.authorName}
            onChange={(e) => updateSettings({ general: { authorName: e.target.value } })}
          />
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">操作确认级别</div>
            <div className="setting-desc">Agent执行文件操作时的确认策略</div>
          </div>
          <select
            className="setting-select"
            value={settings.general.confirmationLevel}
            onChange={(e) => updateSettings({ general: { confirmationLevel: e.target.value as typeof settings.general.confirmationLevel } })}
          >
            <option value="always">全部需确认</option>
            <option value="editOnly">仅编辑操作确认</option>
            <option value="never">全部自动确认</option>
          </select>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">语言</div>
          </div>
          <select
            className="setting-select"
            value={settings.general.language}
            onChange={(e) => updateSettings({ general: { language: e.target.value } })}
          >
            <option value="zh-CN">简体中文</option>
            <option value="en-US">English</option>
          </select>
        </div>
      </div>

      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">Token 预算</span>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">日预算上限</div>
            <div className="setting-desc">超出时触发提醒</div>
          </div>
          <input
            className="setting-input setting-input-narrow"
            placeholder="不限制"
            value={settings.tokenBudget.dailyLimit || ""}
            onChange={(e) => updateSettings({ tokenBudget: { dailyLimit: Number(e.target.value) || 0 } })}
          />
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">月预算上限</div>
          </div>
          <input
            className="setting-input setting-input-narrow"
            placeholder="不限制"
            value={settings.tokenBudget.monthlyLimit || ""}
            onChange={(e) => updateSettings({ tokenBudget: { monthlyLimit: Number(e.target.value) || 0 } })}
          />
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">超出预算行为</div>
          </div>
          <select
            className="setting-select"
            value={settings.tokenBudget.exceedAction}
            onChange={(e) => updateSettings({ tokenBudget: { exceedAction: e.target.value as typeof settings.tokenBudget.exceedAction } })}
          >
            <option value="warn">仅提醒</option>
            <option value="block">暂停Agent</option>
            <option value="fallback">切换到更便宜的模型</option>
          </select>
        </div>
      </div>

      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">版本快照</span>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">保留策略</div>
          </div>
          <select
            className="setting-select"
            value={settings.versionSnapshot.retentionPolicy}
            onChange={(e) => updateSettings({ versionSnapshot: { retentionPolicy: e.target.value as typeof settings.versionSnapshot.retentionPolicy } })}
          >
            <option value="byCount">按数量（最近{settings.versionSnapshot.maxCount}个）</option>
            <option value="byDays">按时间（最近{settings.versionSnapshot.maxDays}天）</option>
            <option value="both">两者都满足</option>
          </select>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">应用数据目录</div>
            <div className="setting-desc">快照和配置的存储位置</div>
          </div>
          <span className="setting-path">%APPDATA%/DocAgent</span>
        </div>
      </div>

      {/* 数据管理 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">数据管理</span>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">导出配置</div>
            <div className="setting-desc">将应用设置和 LLM 配置导出为 JSON 文件</div>
          </div>
          <button
            className="dm-btn"
            onClick={() => handleExportSettings(settings)}
          >
            导出
          </button>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">导出错误日志</div>
            <div className="setting-desc">将应用运行日志导出为文本文件，便于排查问题</div>
          </div>
          <button
            className="dm-btn"
            disabled={exportingLog}
            onClick={async () => {
              setExportingLog(true);
              try {
                const logContent = await tauriCmd.getErrorLog();
                const blob = new Blob([logContent], { type: "text/plain" });
                const url = URL.createObjectURL(blob);
                const a = document.createElement("a");
                a.href = url;
                a.download = `docagent-error-log-${new Date().toISOString().slice(0, 10)}.txt`;
                a.click();
                URL.revokeObjectURL(url);
                addToast("success", "错误日志导出成功");
              } catch (error) {
                console.error("[GeneralTab] 导出错误日志失败:", error);
                addToast("error", `导出失败: ${error instanceof Error ? error.message : String(error)}`);
              } finally {
                setExportingLog(false);
              }
            }}
          >
            {exportingLog ? "导出中..." : "导出"}
          </button>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">清除会话数据</div>
            <div className="setting-desc">删除所有会话记录和消息，此操作不可撤销</div>
          </div>
          {clearConfirm ? (
            <div className="dm-confirm-group">
              <button
                className="dm-btn dm-btn-danger"
                onClick={async () => {
                  try {
                    await clearAllSessions();
                  } catch (error) {
                    console.error("[GeneralTab] 清除会话数据失败:", error);
                  }
                  setClearConfirm(false);
                }}
              >
                确认清除
              </button>
              <button
                className="dm-btn"
                onClick={() => setClearConfirm(false)}
              >
                取消
              </button>
            </div>
          ) : (
            <button
              className="dm-btn dm-btn-danger"
              onClick={() => setClearConfirm(true)}
            >
              清除
            </button>
          )}
        </div>
      </div>

      {/* 关于 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">关于</span>
        </div>

        <div className="about-card">
          <div className="about-name">DocAgent</div>
          <div className="about-version">v0.1.0</div>
          <div className="about-desc">
            基于 AI Agent 的智能文档处理桌面应用
          </div>
          <div className="about-meta">
            <div className="about-meta-row">
              <span className="about-meta-label">技术栈</span>
              <span className="about-meta-value">Tauri 2 + React + Rust + Python</span>
            </div>
            <div className="about-meta-row">
              <span className="about-meta-label">引擎版本</span>
              <span className="about-meta-value">Python 3.12+ Sidecar</span>
            </div>
          </div>
        </div>
      </div>

      <style>{`
        .settings-section {
          margin-bottom: 24px;
        }
        .settings-section:last-child {
          margin-bottom: 0;
        }
        .section-header {
          display: flex;
          align-items: center;
          gap: 8px;
          margin-bottom: 16px;
        }
        .section-title {
          font-size: 13px;
          font-weight: 600;
          color: var(--color-text-secondary);
          text-transform: uppercase;
          letter-spacing: 0.3px;
        }
        .setting-row {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 10px 12px;
          border-bottom: 1px solid var(--color-border-light);
          gap: 16px;
        }
        .setting-row:last-child {
          border-bottom: none;
        }
        .setting-info {
          flex: 1;
          min-width: 0;
        }
        .setting-label {
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-primary);
        }
        .setting-desc {
          font-size: 11px;
          color: var(--color-text-quaternary);
          margin-top: 2px;
        }
        .setting-input {
          padding: 6px 10px;
          border: 1px solid var(--color-border);
          border-radius: var(--radius-sm);
          font-size: 13px;
          transition: all 0.2s;
          background: var(--color-bg);
          color: var(--color-text-primary);
        }
        .setting-input:focus {
          border-color: var(--color-accent);
          box-shadow: 0 0 0 2px var(--color-accent-lighter);
          outline: none;
        }
        .setting-input-narrow {
          width: 120px;
        }
        .setting-select {
          padding: 6px 10px;
          border: 1px solid var(--color-border);
          border-radius: var(--radius-sm);
          font-size: 13px;
          background: var(--color-bg);
          color: var(--color-text-primary);
          cursor: pointer;
          transition: all 0.2s;
        }
        .setting-select:focus {
          border-color: var(--color-accent);
          box-shadow: 0 0 0 2px var(--color-accent-lighter);
          outline: none;
        }
        .setting-path {
          font-size: 12px;
          color: var(--color-text-quaternary);
          font-family: var(--font-mono);
          flex-shrink: 0;
        }
        .dm-btn {
          padding: 5px 14px;
          font-size: 12px;
          font-weight: 500;
          border-radius: var(--radius-sm);
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
          cursor: pointer;
          transition: all 0.15s;
          flex-shrink: 0;
          border: 1px solid var(--color-border);
        }
        .dm-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
        }
        .dm-btn-danger {
          background: var(--color-error-bg);
          color: var(--color-error);
          border-color: var(--color-error);
        }
        .dm-btn-danger:hover {
          background: var(--color-error);
          color: #fff;
        }
        .dm-confirm-group {
          display: flex;
          gap: 6px;
          flex-shrink: 0;
        }
        .about-card {
          padding: 20px;
          background: var(--color-bg-sub);
          border-radius: var(--radius-md);
          border: 1px solid var(--color-border-light);
        }
        .about-name {
          font-size: 18px;
          font-weight: 700;
          color: var(--color-text-primary);
        }
        .about-version {
          font-size: 12px;
          font-family: var(--font-mono);
          color: var(--color-text-tertiary);
          margin-top: 4px;
        }
        .about-desc {
          font-size: 13px;
          color: var(--color-text-secondary);
          margin-top: 8px;
        }
        .about-meta {
          margin-top: 16px;
          display: flex;
          flex-direction: column;
          gap: 6px;
        }
        .about-meta-row {
          display: flex;
          align-items: center;
          gap: 12px;
        }
        .about-meta-label {
          font-size: 12px;
          color: var(--color-text-quaternary);
          min-width: 60px;
        }
        .about-meta-value {
          font-size: 12px;
          color: var(--color-text-secondary);
        }
      `}</style>
    </div>
  );
}

/**
 * 导出设置到 JSON 文件
 */
function handleExportSettings(settings: AppSettings) {
  const json = JSON.stringify(settings, null, 2);
  const blob = new Blob([json], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `docagent-settings-${new Date().toISOString().slice(0, 10)}.json`;
  a.click();
  URL.revokeObjectURL(url);
}
