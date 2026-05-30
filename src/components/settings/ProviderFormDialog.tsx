import { useState, useEffect } from "react";
import type { ProviderInfo, LLMProviderType, ConnectionResult } from "../../types";
import * as tauriCmd from "../../services/tauri";

interface ProviderFormDialogProps {
  mode: "add" | "edit";
  provider?: ProviderInfo | null;
  onClose: () => void;
  onSaved: () => void;
}

const providerTypeOptions: { value: LLMProviderType; label: string; defaultBase: string }[] = [
  { value: "openai", label: "OpenAI", defaultBase: "https://api.openai.com/v1" },
  { value: "anthropic", label: "Anthropic", defaultBase: "https://api.anthropic.com" },
  { value: "gemini", label: "Google Gemini", defaultBase: "https://generativelanguage.googleapis.com/v1beta" },
  { value: "ollama", label: "Ollama", defaultBase: "http://localhost:11434/v1" },
  { value: "custom", label: "自定义", defaultBase: "" },
];

export function ProviderFormDialog({ mode, provider, onClose, onSaved }: ProviderFormDialogProps) {
  const [name, setName] = useState(provider?.name ?? "");
  const [providerType, setProviderType] = useState<LLMProviderType>(provider?.providerType ?? "openai");
  const [apiBase, setApiBase] = useState(provider?.apiBase ?? "https://api.openai.com/v1");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState(provider?.model ?? "");
  const [contextWindow, setContextWindow] = useState<string>(
    provider?.contextWindow ? String(provider.contextWindow) : ""
  );
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<ConnectionResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const option = providerTypeOptions.find((o) => o.value === providerType);
    if (option && mode === "add") {
      setApiBase(option.defaultBase);
    }
  }, [providerType, mode]);

  const handleSave = async () => {
    if (!name.trim()) { setError("请输入 Provider 名称"); return; }
    if (!apiBase.trim()) { setError("请输入 API Base URL"); return; }
    if (!model.trim()) { setError("请输入模型名称"); return; }
    if (mode === "add" && !apiKey.trim()) { setError("请输入 API Key"); return; }

    setSaving(true);
    setError(null);
    try {
      const config = {
        name: name.trim(),
        providerType,
        apiBase: apiBase.trim(),
        apiKey: apiKey.trim(),
        model: model.trim(),
        contextWindow: contextWindow.trim() ? Number(contextWindow) || undefined : undefined,
      };
      if (mode === "add") {
        await tauriCmd.addProvider(config);
      } else if (provider) {
        await tauriCmd.updateProvider(provider.id, config);
      }
      onSaved();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : typeof err === "string" ? err : "保存失败";
      setError(msg);
    } finally {
      setSaving(false);
    }
  };

  const handleTest = async () => {
    // 验证必要参数（添加和编辑模式通用）
    if (!apiBase.trim()) {
      setError("请输入 API Base URL");
      return;
    }
    if (!model.trim()) {
      setError("请输入模型名称");
      return;
    }
    // 添加模式下 API Key 必填；编辑模式下可留空，后端会从已保存 Provider 查找
    if (mode === "add" && !apiKey.trim()) {
      setError("请输入 API Key");
      return;
    }

    setTesting(true);
    setTestResult(null);
    setError(null);
    try {
      // 始终使用 testConnectionWithConfig 传递当前表单值
      // 编辑模式下传入 providerId，后端在 API Key 为空时自动从已保存 Provider 查找
      const config = {
        name: name.trim(),
        providerType,
        apiBase: apiBase.trim(),
        apiKey: apiKey.trim(),
        model: model.trim(),
        contextWindow: contextWindow.trim() ? Number(contextWindow) || undefined : undefined,
      };
      const providerId = mode === "edit" ? provider?.id : undefined;
      const result = await tauriCmd.testConnectionWithConfig(config, providerId);
      setTestResult(result);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : typeof err === "string" ? err : "连接测试失败";
      setTestResult({ success: false, latencyMs: 0, errorMessage: msg, error: msg });
    } finally {
      setTesting(false);
    }
  };

  return (
    <div
      className="fixed inset-0 bg-overlay z-[400] flex items-center justify-center animate-fade-in"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div
        className="dialog-modal"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="dialog-header">
          <h3 className="dialog-title">
            {mode === "add" ? "添加 LLM Provider" : "编辑 LLM Provider"}
          </h3>
          <button className="dialog-close-btn" onClick={onClose}>x</button>
        </div>

        <div className="dialog-body">
          <div className="form-group">
            <label className="form-label">Provider 名称</label>
            <input
              className="form-input"
              placeholder="例如：我的 GPT-4o"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>

          <div className="form-group">
            <label className="form-label">Provider 类型</label>
            <select
              className="form-select"
              value={providerType}
              onChange={(e) => setProviderType(e.target.value as LLMProviderType)}
            >
              {providerTypeOptions.map((opt) => (
                <option key={opt.value} value={opt.value}>{opt.label}</option>
              ))}
            </select>
          </div>

          <div className="form-group">
            <label className="form-label">API Base URL</label>
            <input
              className="form-input form-input-mono"
              placeholder="https://api.openai.com/v1"
              value={apiBase}
              onChange={(e) => setApiBase(e.target.value)}
            />
          </div>

          <div className="form-group">
            <label className="form-label">
              API Key{mode === "edit" ? "（留空则保持不变）" : ""}
            </label>
            <input
              type="password"
              className="form-input form-input-mono"
              placeholder={mode === "edit" ? "sk-..." : "sk-..."}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
            />
          </div>

          <div className="form-group">
            <label className="form-label">模型名称</label>
            <input
              className="form-input form-input-mono"
              placeholder="例如：gpt-4o、claude-3-5-sonnet、gemini-1.5-pro"
              value={model}
              onChange={(e) => setModel(e.target.value)}
            />
          </div>

          <div className="form-group">
            <label className="form-label">
              上下文窗口大小 (tokens)
              <span className="form-label-hint">留空则自动推断</span>
            </label>
            <input
              className="form-input form-input-mono"
              type="number"
              placeholder="例如：128000、200000、1000000"
              value={contextWindow}
              onChange={(e) => setContextWindow(e.target.value)}
              min="4096"
            />
          </div>

          {testResult && (
            <div className={`test-result ${testResult.success ? "test-success" : "test-error"}`}>
              {testResult.success ? (
                <span>连接成功 - 延迟: {testResult.latencyMs}ms{testResult.model ? ` | 模型: ${testResult.model}` : ""}</span>
              ) : (
                <span>连接失败: {testResult.errorMessage || testResult.error || "未知错误"}</span>
              )}
            </div>
          )}

          {error && (
            <div className="test-result test-error">{error}</div>
          )}
        </div>

        <div className="dialog-footer">
          <button
            className="dialog-btn dialog-btn-ghost mr-auto"
            onClick={handleTest}
            disabled={testing}
          >
            {testing ? (
              <span className="test-loading">
                <span className="test-spinner"></span>
                测试中
              </span>
            ) : "测试连接"}
          </button>
          <button className="dialog-btn dialog-btn-ghost" onClick={onClose}>取消</button>
          <button className="dialog-btn dialog-btn-primary" onClick={handleSave} disabled={saving}>
            {saving ? "保存中..." : "保存"}
          </button>
        </div>
      </div>

      <style>{`
        .dialog-modal {
          width: 520px;
          background: var(--color-bg-elevated);
          border-radius: var(--radius-xl);
          box-shadow: var(--shadow-xl);
          display: flex;
          flex-direction: column;
          overflow: hidden;
          animation: scaleIn 0.2s ease;
        }
        .dialog-header {
          padding: 18px 24px;
          border-bottom: 1px solid var(--color-border-light);
          display: flex;
          align-items: center;
          gap: 12px;
          flex-shrink: 0;
        }
        .dialog-title {
          font-size: 15px;
          font-weight: 700;
          color: var(--color-text-primary);
          flex: 1;
        }
        .dialog-close-btn {
          width: 28px;
          height: 28px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-sm);
          color: var(--color-text-secondary);
          transition: all 0.15s;
          font-size: 16px;
        }
        .dialog-close-btn:hover {
          background: var(--color-bg-sub);
          color: var(--color-text-primary);
        }
        .dialog-body {
          flex: 1;
          overflow-y: auto;
          padding: 20px 24px;
          display: flex;
          flex-direction: column;
          gap: 16px;
        }
        .form-group {
          display: flex;
          flex-direction: column;
          gap: 6px;
        }
        .form-label {
          font-size: 12px;
          font-weight: 500;
          color: var(--color-text-secondary);
          display: flex;
          align-items: center;
          gap: 6px;
        }
        .form-label-hint {
          font-size: 11px;
          font-weight: 400;
          color: var(--color-text-quaternary);
        }
        .form-input {
          padding: 8px 12px;
          border: 1px solid var(--color-border);
          border-radius: var(--radius-sm);
          font-size: 13px;
          transition: all 0.2s;
          background: var(--color-bg);
          color: var(--color-text-primary);
        }
        .form-input:focus {
          border-color: var(--color-accent);
          box-shadow: 0 0 0 2px var(--color-accent-lighter);
          outline: none;
        }
        .form-input-mono {
          font-family: var(--font-mono);
        }
        .form-select {
          padding: 8px 12px;
          border: 1px solid var(--color-border);
          border-radius: var(--radius-sm);
          font-size: 13px;
          background: var(--color-bg);
          color: var(--color-text-primary);
          cursor: pointer;
          transition: all 0.2s;
        }
        .form-select:focus {
          border-color: var(--color-accent);
          box-shadow: 0 0 0 2px var(--color-accent-lighter);
          outline: none;
        }
        .test-result {
          padding: 8px 12px;
          border-radius: var(--radius-sm);
          font-size: 12px;
        }
        .test-success {
          background: var(--color-success-light);
          color: var(--color-success);
          border: 1px solid rgba(52, 199, 36, 0.3);
        }
        .test-error {
          background: var(--color-error-light);
          color: var(--color-error);
          border: 1px solid rgba(245, 74, 69, 0.3);
        }
        .dialog-footer {
          padding: 16px 24px;
          border-top: 1px solid var(--color-border-light);
          display: flex;
          align-items: center;
          gap: 8px;
          flex-shrink: 0;
        }
        .dialog-btn {
          padding: 6px 16px;
          border-radius: var(--radius-sm);
          font-size: 12px;
          font-weight: 500;
          border: none;
          cursor: pointer;
          transition: all 0.15s;
        }
        .dialog-btn-primary {
          background: var(--color-accent);
          color: white;
        }
        .dialog-btn-primary:hover:not(:disabled) {
          background: var(--color-accent-hover);
        }
        .dialog-btn-primary:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }
        .dialog-btn-ghost {
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
        }
        .dialog-btn-ghost:hover {
          background: var(--color-bg-hover);
        }
        .dialog-btn-ghost:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }
        .test-loading {
          display: inline-flex;
          align-items: center;
          gap: 4px;
        }
        .test-spinner {
          width: 10px;
          height: 10px;
          border: 2px solid var(--color-text-quaternary);
          border-top-color: var(--color-text-secondary);
          border-radius: 50%;
          animation: spin 0.8s linear infinite;
        }
        @keyframes spin {
          to { transform: rotate(360deg); }
        }
      `}</style>
    </div>
  );
}
