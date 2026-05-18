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
  { value: "ollama", label: "Ollama", defaultBase: "http://localhost:11434/v1" },
  { value: "custom", label: "自定义", defaultBase: "" },
];

export function ProviderFormDialog({ mode, provider, onClose, onSaved }: ProviderFormDialogProps) {
  const [name, setName] = useState(provider?.name ?? "");
  const [providerType, setProviderType] = useState<LLMProviderType>(provider?.providerType ?? "openai");
  const [apiBase, setApiBase] = useState(provider?.apiBase ?? "https://api.openai.com/v1");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState(provider?.model ?? "");
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
    if (!provider && mode === "add") {
      setError("请先保存 Provider 后再测试连接");
      return;
    }
    const targetId = provider?.id;
    if (!targetId) return;

    setTesting(true);
    setTestResult(null);
    setError(null);
    try {
      const result = await tauriCmd.testConnection(targetId);
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
      className="fixed inset-0 bg-black/35 z-[400] flex items-center justify-center animate-fade-in"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div
        className="w-[520px] bg-bg rounded-[var(--radius-lg)] shadow-lg flex flex-col overflow-hidden animate-slide-up"
        onClick={(e) => e.stopPropagation()}
      >
        {/* 标题栏 */}
        <div className="flex items-center px-6 py-4 border-b border-border gap-3 flex-shrink-0">
          <h3 className="text-[15px] font-bold flex-1">
            {mode === "add" ? "添加 LLM Provider" : "编辑 LLM Provider"}
          </h3>
          <button
            className="w-[28px] h-[28px] flex items-center justify-center rounded-[var(--radius-sm)] transition-colors duration-150 text-text-secondary hover:bg-bg-sub"
            onClick={onClose}
          >
            x
          </button>
        </div>

        {/* 表单内容 */}
        <div className="flex-1 overflow-y-auto px-6 py-5 space-y-4">
          {/* Provider 名称 */}
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">Provider 名称</label>
            <input
              className="w-full px-3 py-2 border border-border rounded-[var(--radius-sm)] text-[13px] transition-colors focus:border-accent focus:outline-none"
              placeholder="例如：我的 GPT-4o"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>

          {/* Provider 类型 */}
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">Provider 类型</label>
            <select
              className="w-full px-3 py-2 border border-border rounded-[var(--radius-sm)] text-[13px] bg-bg cursor-pointer focus:border-accent focus:outline-none"
              value={providerType}
              onChange={(e) => setProviderType(e.target.value as LLMProviderType)}
            >
              {providerTypeOptions.map((opt) => (
                <option key={opt.value} value={opt.value}>{opt.label}</option>
              ))}
            </select>
          </div>

          {/* API Base URL */}
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">API Base URL</label>
            <input
              className="w-full px-3 py-2 border border-border rounded-[var(--radius-sm)] text-[13px] font-mono transition-colors focus:border-accent focus:outline-none"
              placeholder="https://api.openai.com/v1"
              value={apiBase}
              onChange={(e) => setApiBase(e.target.value)}
            />
          </div>

          {/* API Key */}
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">
              API Key{mode === "edit" ? "（留空则保持不变）" : ""}
            </label>
            <input
              type="password"
              className="w-full px-3 py-2 border border-border rounded-[var(--radius-sm)] text-[13px] font-mono transition-colors focus:border-accent focus:outline-none"
              placeholder={mode === "edit" ? "sk-..." : "sk-..."}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
            />
          </div>

          {/* 模型名称 */}
          <div>
            <label className="block text-[12px] font-medium text-text-secondary mb-1">模型名称</label>
            <input
              className="w-full px-3 py-2 border border-border rounded-[var(--radius-sm)] text-[13px] font-mono transition-colors focus:border-accent focus:outline-none"
              placeholder="例如：gpt-4o、claude-3-5-sonnet、gemini-1.5-pro"
              value={model}
              onChange={(e) => setModel(e.target.value)}
            />
          </div>

          {/* 连接测试结果 */}
          {testResult && (
            <div className={`px-3 py-2 rounded-[var(--radius-sm)] text-[12px] ${
              testResult.success
                ? "bg-green-50 text-green-700 border border-green-200"
                : "bg-red-50 text-red-700 border border-red-200"
            }`}>
              {testResult.success ? (
                <span>连接成功 - 延迟: {testResult.latencyMs}ms{testResult.model ? ` | 模型: ${testResult.model}` : ""}</span>
              ) : (
                <span>连接失败: {testResult.errorMessage || testResult.error || "未知错误"}</span>
              )}
            </div>
          )}

          {/* 错误信息 */}
          {error && (
            <div className="px-3 py-2 rounded-[var(--radius-sm)] text-[12px] bg-red-50 text-red-700 border border-red-200">
              {error}
            </div>
          )}
        </div>

        {/* 底部操作栏 */}
        <div className="flex items-center justify-end gap-2 px-6 py-4 border-t border-border flex-shrink-0">
          {mode === "edit" && provider && (
            <button
              className="px-3 py-[6px] rounded-[var(--radius-sm)] text-[12px] font-medium bg-bg-sub text-text-secondary hover:bg-bg-hover transition-all mr-auto"
              onClick={handleTest}
              disabled={testing}
            >
              {testing ? "测试中..." : "测试连接"}
            </button>
          )}
          <button
            className="px-4 py-[6px] rounded-[var(--radius-sm)] text-[12px] font-medium bg-bg-sub text-text-secondary hover:bg-bg-hover transition-all"
            onClick={onClose}
          >
            取消
          </button>
          <button
            className="px-4 py-[6px] rounded-[var(--radius-sm)] text-[12px] font-medium bg-accent text-white hover:bg-accent-hover transition-all disabled:opacity-50"
            onClick={handleSave}
            disabled={saving}
          >
            {saving ? "保存中..." : "保存"}
          </button>
        </div>
      </div>
    </div>
  );
}
