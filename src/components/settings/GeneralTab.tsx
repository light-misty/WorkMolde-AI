import { useState, useEffect } from "react";
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from "../../stores/useSettingsStore";
import { useSessionStore } from "../../stores/useSessionStore";
import { useToastStore } from "../../stores/useToastStore";
import * as tauriCmd from "../../services/tauri";
import { getVersion } from "@tauri-apps/api/app";

export function GeneralTab() {
  const { t } = useTranslation();
  const { settings, updateSettings } = useSettingsStore();
  const { clearAllSessions } = useSessionStore();
  const addToast = useToastStore((s) => s.addToast);
  const removeToast = useToastStore((s) => s.removeToast);
  const [clearConfirm, setClearConfirm] = useState(false);
  // 日志路径信息
  const [logPathInfo, setLogPathInfo] = useState<{ logSource: string } | null>(null);
  // 更新相关状态
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [currentVersion, setCurrentVersion] = useState<string>("");
  const [updateCheckResult, setUpdateCheckResult] = useState<"upToDate" | "available" | "error" | null>(null);

  // 获取当前版本号
  useState(() => {
    getVersion().then((v) => setCurrentVersion(v)).catch(() => setCurrentVersion("0.1.0"));
  });

  // 获取日志路径信息
  useEffect(() => {
    tauriCmd.getLogPath().then(setLogPathInfo).catch(() => setLogPathInfo(null));
  }, []);

  return (
    <div>
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">{t('settings.general.sectionTitle')}</span>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">{t('settings.general.authorName')}</div>
            <div className="setting-desc">{t('settings.general.authorNameDesc')}</div>
          </div>
          <input
            className="setting-input"
            value={settings.general.authorName}
            onChange={(e) => updateSettings({ general: { authorName: e.target.value } })}
          />
        </div>

        {/* 作者邮箱 */}
        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">{t('settings.general.authorEmail')}</div>
            <div className="setting-desc">{t('settings.general.authorEmailDesc')}</div>
          </div>
          <input
            className="setting-input"
            value={settings.general.authorEmail}
            onChange={(e) => updateSettings({ general: { authorEmail: e.target.value } })}
          />
        </div>

        {/* 作者公司/组织 */}
        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">{t('settings.general.authorCompany')}</div>
            <div className="setting-desc">{t('settings.general.authorCompanyDesc')}</div>
          </div>
          <input
            className="setting-input"
            value={settings.general.authorCompany}
            onChange={(e) => updateSettings({ general: { authorCompany: e.target.value } })}
          />
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">{t('settings.general.confirmLevel')}</div>
            <div className="setting-desc">{t('settings.general.confirmLevelDesc')}</div>
          </div>
          <select
            className="setting-select"
            value={settings.general.confirmationLevel}
            onChange={(e) => updateSettings({ general: { confirmationLevel: e.target.value as typeof settings.general.confirmationLevel } })}
          >
            <option value="always">{t('settings.general.confirmAlways')}</option>
            <option value="editOnly">{t('settings.general.confirmEditOnly')}</option>
            <option value="never">{t('settings.general.confirmNever')}</option>
          </select>
        </div>
      </div>

      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">{t('settings.general.versionSnapshot')}</span>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">{t('settings.general.retentionPolicy')}</div>
          </div>
          <select
            className="setting-select"
            value={settings.versionSnapshot.retentionPolicy}
            onChange={(e) => updateSettings({ versionSnapshot: { retentionPolicy: e.target.value as typeof settings.versionSnapshot.retentionPolicy } })}
          >
            <option value="byCount">{t('settings.general.byCount', { count: settings.versionSnapshot.maxCount })}</option>
            <option value="byDays">{t('settings.general.byDays', { days: settings.versionSnapshot.maxDays })}</option>
            <option value="both">{t('settings.general.both')}</option>
          </select>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">{t('settings.general.appDataDir')}</div>
            <div className="setting-desc">{t('settings.general.appDataDirDesc')}</div>
          </div>
          <span className="setting-path">%APPDATA%/DocAgent</span>
        </div>
      </div>

      {/* 数据管理 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">{t('settings.general.dataManagement')}</span>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">{t('settings.general.exportErrorLog')}</div>
            <div className="setting-desc">
              {t('settings.general.exportErrorLogDesc')}
              {logPathInfo && (
                <span className="log-path-hint">{t('settings.general.logSourcePath', { path: logPathInfo.logSource })}</span>
              )}
            </div>
          </div>
          <button
            className="dm-btn"
            onClick={async () => {
              if (!logPathInfo?.logSource) return;
              try {
                await tauriCmd.openDirectory(logPathInfo.logSource);
              } catch (error) {
                console.error("[GeneralTab] 打开日志目录失败:", error);
                addToast("error", `${t('settings.general.openDirFail')}: ${error instanceof Error ? error.message : String(error)}`);
              }
            }}
          >
            {t('settings.general.openLogDir')}
          </button>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">{t('settings.general.clearSessionData')}</div>
            <div className="setting-desc">{t('settings.general.clearSessionDataDesc')}</div>
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
                {t('settings.general.confirmClear')}
              </button>
              <button
                className="dm-btn"
                onClick={() => setClearConfirm(false)}
              >
                {t('settings.general.cancel')}
              </button>
            </div>
          ) : (
            <button
              className="dm-btn dm-btn-danger"
              onClick={() => setClearConfirm(true)}
            >
              {t('settings.general.clear')}
            </button>
          )}
        </div>
      </div>

      {/* 更新设置 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">{t('settings.general.updateSettings')}</span>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">{t('settings.general.autoCheckUpdate')}</div>
            <div className="setting-desc">{t('settings.general.autoCheckUpdateDesc')}</div>
          </div>
          <label className="setting-toggle">
            <input
              type="checkbox"
              checked={settings.update.autoCheck}
              onChange={(e) => updateSettings({ update: { autoCheck: e.target.checked } })}
            />
            <span className="setting-toggle-slider" />
          </label>
        </div>

        <div className="setting-row">
          <div className="setting-info">
            <div className="setting-label">{t('settings.general.manualCheckUpdate')}</div>
            <div className="setting-desc">
              {updateCheckResult === "upToDate" && t('settings.general.upToDate')}
              {updateCheckResult === "available" && t('settings.general.updateAvailable')}
              {updateCheckResult === "error" && t('settings.general.checkFailed')}
              {!updateCheckResult && t('settings.general.currentVersion', { version: currentVersion })}
            </div>
          </div>
          <button
            className="dm-btn"
            disabled={checkingUpdate}
            onClick={async () => {
              setCheckingUpdate(true);
              setUpdateCheckResult(null);
              const toastId = addToast("info", t('update.checking'));
              try {
                const result = await tauriCmd.checkUpdate();
                removeToast(toastId);
                if (result) {
                  setUpdateCheckResult("available");
                  addToast("success", t('update.newVersionFound', { version: result.version }));
                } else {
                  setUpdateCheckResult("upToDate");
                  addToast("success", t('settings.general.upToDate'));
                }
              } catch (err) {
                removeToast(toastId);
                console.error("[GeneralTab] 检查更新失败:", err);
                setUpdateCheckResult("error");
                // 提取具体错误信息，帮助用户排查问题
                const errMsg = err instanceof Error ? err.message : String(err);
                addToast("error", t('update.checkFailedWithError', { error: errMsg }));
              } finally {
                setCheckingUpdate(false);
              }
            }}
          >
            {checkingUpdate ? t('settings.general.checking') : t('settings.general.checkUpdate')}
          </button>
        </div>
      </div>

      {/* 关于 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">{t('settings.general.about')}</span>
        </div>

        <div className="about-card">
          <div className="about-name">DocAgent</div>
          <div className="about-version">v{currentVersion || "0.1.0"}</div>
          <div className="about-desc">
            {t('settings.general.aboutDesc')}
          </div>
          <div className="about-meta">
            <div className="about-meta-row">
              <span className="about-meta-label">{t('settings.general.techStack')}</span>
              <span className="about-meta-value">{t('app.techStack')}</span>
            </div>
            <div className="about-meta-row">
              <span className="about-meta-label">{t('settings.general.engineVersion')}</span>
              <span className="about-meta-value">{t('app.engineVersion')}</span>
            </div>
          </div>
        </div>
      </div>

      <style>{`
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
        .setting-toggle {
          position: relative;
          display: inline-block;
          width: 36px;
          height: 20px;
          flex-shrink: 0;
          cursor: pointer;
        }
        .setting-toggle input {
          opacity: 0;
          width: 0;
          height: 0;
          position: absolute;
        }
        .setting-toggle-slider {
          position: absolute;
          inset: 0;
          background: var(--color-border-strong);
          border-radius: 10px;
          transition: all 0.2s;
        }
        .setting-toggle-slider::before {
          content: '';
          position: absolute;
          width: 16px;
          height: 16px;
          left: 2px;
          top: 2px;
          background: #fff;
          border-radius: 50%;
          transition: all 0.2s;
        }
        .setting-toggle input:checked + .setting-toggle-slider {
          background: var(--color-accent);
        }
        .setting-toggle input:checked + .setting-toggle-slider::before {
          transform: translateX(16px);
        }
        .setting-toggle input:focus-visible + .setting-toggle-slider {
          outline: 2px solid var(--color-accent);
          outline-offset: 2px;
        }
      `}</style>
    </div>
  );
}

