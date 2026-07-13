import { useTranslation } from 'react-i18next';
import { useState, useRef, useCallback, useEffect, useLayoutEffect, type KeyboardEvent as ReactKeyboardEvent, type DragEvent, type ClipboardEvent } from "react";
import { Icon } from "../common/Icon";
import { ProviderSelector } from "../common/ProviderSelector";
import { WorkspaceSelector } from "./WorkspaceSelector";
import { WorkspaceGitStatus } from "./WorkspaceGitStatus";
import type { ExecutionStatus } from "../../types/workflow";
import type { AttachmentMeta } from "../../types/session";
import { useAttachmentStore, inferAttachmentType, SUPPORTED_ATTACHMENT_MIME_TYPES, MAX_IMAGE_SIZE, MAX_TEXT_SIZE, MAX_DOCUMENT_SIZE, MAX_ATTACHMENT_COUNT, hasImageAttachments } from "../../stores/useAttachmentStore";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { useSessionStore } from "../../stores/useSessionStore";
import { useWorkspaceStore } from "../../stores/useWorkspaceStore";
import { useAgentModeStore } from "../../stores/useAgentModeStore";
import { formatSize, matchesShortcut } from "../../utils/format";
import { switchAgentMode } from "../../services/tauri";
import type { PromptTemplate } from "../../types";

interface InputAreaProps {
  onSend: (text: string) => void;
  disabled?: boolean;
  // Agent 执行状态
  executionStatus?: ExecutionStatus;
  onStop?: () => void;
  /** 是否为居中布局（空会话状态）：居中时限制最大宽度 */
  centered?: boolean;
}

export function InputArea({ onSend, disabled = false, executionStatus = "idle", onStop, centered = false }: InputAreaProps) {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const [isDragOver, setIsDragOver] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  // 保存模板插入时的 focus/height 定时器，组件卸载时清理
  const templateFocusTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const templateHeightTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const attachments = useAttachmentStore((s) => s.attachments);
  const addAttachment = useAttachmentStore((s) => s.addAttachment);
  const removeAttachment = useAttachmentStore((s) => s.removeAttachment);
  const clearAttachments = useAttachmentStore((s) => s.clearAttachments);

  // 检查当前生效的 Provider 是否支持视觉：优先使用用户为当前会话选择的 Provider
  const { llmProviders, preferredProviderId } = useSettingsStore();
  const currentProvider = llmProviders.find((p) => p.id === preferredProviderId)
    || llmProviders[0];
  const supportsVision = currentProvider?.supportsVision ?? false;
  const showVisionWarning = hasImageAttachments(attachments) && !supportsVision;

  const { currentWorkspaceId, workspaces } = useWorkspaceStore();
  const hasWorkspace = workspaces.length > 0 && currentWorkspaceId !== null;
  const hasProvider = llmProviders.length > 0;
  const configReady = !centered || (hasWorkspace && hasProvider);

  const currentSessionId = useSessionStore((s) => s.currentSessionId);
  // 自动聚焦：初始挂载 + 会话切换时
  useEffect(() => {
    textareaRef.current?.focus();
  }, []);
  useEffect(() => {
    textareaRef.current?.focus();
  }, [currentSessionId]);

  // 组件卸载时清理模板插入相关的定时器
  useEffect(() => {
    return () => {
      if (templateFocusTimerRef.current !== null) clearTimeout(templateFocusTimerRef.current);
      if (templateHeightTimerRef.current !== null) clearTimeout(templateHeightTimerRef.current);
    };
  }, []);

  // 从设置中读取快捷键配置
  const sendMessageShortcut = useSettingsStore((s) => s.settings.shortcuts.sendMessage);

  const templates = useSettingsStore((s) => s.templates);
  const openSettings = useSettingsStore((s) => s.openSettings);
  const pendingInsertTemplate = useSettingsStore((s) => s.pendingInsertTemplate);
  const setPendingInsertTemplate = useSettingsStore((s) => s.setPendingInsertTemplate);

  const handleSend = useCallback(() => {
    const trimmed = text.trim();
    if ((!trimmed && attachments.length === 0) || disabled || !configReady) return;
    onSend(trimmed || t('inputArea.attachment'));
    setText("");
    clearAttachments();
  }, [text, disabled, onSend, attachments.length, clearAttachments, configReady]);

  const handleKeyDown = useCallback(
    (e: ReactKeyboardEvent<HTMLTextAreaElement>) => {
      // 发送消息快捷键（从设置中读取）
      if (matchesShortcut(e, sendMessageShortcut)) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend, sendMessageShortcut]
  );

  // 输入框最大高度
  const MAX_TEXTAREA_HEIGHT = 240;

  // 自动调整高度的核心函数
  const adjustTextareaHeight = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, MAX_TEXTAREA_HEIGHT) + "px";
  }, []);

  // 每次 text 更新后（React 提交 DOM 后）重新调整高度，确保高度始终正确
  useLayoutEffect(() => {
    adjustTextareaHeight();
  }, [text, adjustTextareaHeight]);

  const handleInput = useCallback(() => {
    adjustTextareaHeight();
  }, [adjustTextareaHeight]);

  // 模板插入回调
  const handleTemplateInsert = useCallback((templateText: string) => {
    setText(templateText);
    // 聚焦输入框（保存定时器，组件卸载时清理）
    templateFocusTimerRef.current = setTimeout(() => textareaRef.current?.focus(), 50);
    // 调整高度（保存定时器，组件卸载时清理）
    templateHeightTimerRef.current = setTimeout(adjustTextareaHeight, 60);
  }, []);

  // 监听来自设置的待插入模板文本（由 TemplatesTab 的"使用"按钮触发）
  useEffect(() => {
    if (pendingInsertTemplate !== null) {
      handleTemplateInsert(pendingInsertTemplate);
      setPendingInsertTemplate(null);
    }
  }, [pendingInsertTemplate, handleTemplateInsert, setPendingInsertTemplate]);

  // 处理文件选择
  const handleFileSelect = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  const handleFileChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files) return;
    processFiles(Array.from(files));
    // 重置 input，允许重复选择同一文件
    e.target.value = "";
  }, []);

  // 将文件读取为 base64
  const readFileAsBase64 = (file: File): Promise<string> => {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => {
        const result = reader.result as string;
        // 去除 data:xxx;base64, 前缀
        const base64 = result.split(",")[1] || result;
        resolve(base64);
      };
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(file);
    });
  };

  // 处理文件列表，添加到附件 store
  const processFiles = useCallback(async (files: File[]) => {
    for (const file of files) {
      // 检查附件数量上限
      const currentCount = useAttachmentStore.getState().attachments.length;
      if (currentCount >= MAX_ATTACHMENT_COUNT) {
        console.warn(`附件数量已达上限 (${MAX_ATTACHMENT_COUNT} 个)`);
        break;
      }
      // 检查 MIME 类型
      if (!SUPPORTED_ATTACHMENT_MIME_TYPES.includes(file.type) && !file.type.startsWith("image/")) {
        console.warn(`不支持的文件类型: ${file.type} (${file.name})`);
        continue;
      }
      // 检查文件大小（区分图片/文档/文本类型）
      const attType = inferAttachmentType(file.type);
      const sizeLimit = attType === "image" ? MAX_IMAGE_SIZE : attType === "document" ? MAX_DOCUMENT_SIZE : MAX_TEXT_SIZE;
      if (file.size > sizeLimit) {
        console.warn(`文件过大: ${file.name} (${formatSize(file.size)})`);
        continue;
      }
      // 读取文件内容为 base64
      let base64Data: string | undefined;
      try {
        base64Data = await readFileAsBase64(file);
      } catch (err) {
        console.warn(`读取文件失败: ${file.name}`, err);
        continue;
      }
      const attachment: AttachmentMeta = {
        name: file.name,
        mimeType: file.type || "application/octet-stream",
        size: file.size,
        type: attType,
        data: base64Data,
      };
      addAttachment(attachment);
    }
  }, [addAttachment]);

  // 拖拽处理
  const handleDragOver = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(true);
  }, []);

  const handleDragLeave = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);
  }, []);

  const handleDrop = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);
    const files = Array.from(e.dataTransfer.files);
    if (files.length > 0) {
      void processFiles(files);
    }
  }, [processFiles]);

  // 粘贴处理
  const handlePaste = useCallback((e: ClipboardEvent<HTMLTextAreaElement>) => {
    const items = e.clipboardData?.items;
    if (!items) return;
    const imageFiles: File[] = [];
    for (const item of Array.from(items)) {
      if (item.type.startsWith("image/")) {
        const file = item.getAsFile();
        if (file) {
          imageFiles.push(file);
        }
      }
    }
    if (imageFiles.length > 0) {
      // 不阻止默认文本粘贴，只处理图片
      void processFiles(imageFiles);
    }
  }, [processFiles]);

  const hasContent = text.trim().length > 0 || attachments.length > 0;

  const configTip = centered && !configReady
    ? (!hasWorkspace && !hasProvider
        ? t('inputArea.configReminder.noWorkspaceAndProvider')
        : !hasWorkspace
          ? t('inputArea.configReminder.noWorkspace')
          : t('inputArea.configReminder.noProvider'))
    : "";

  return (
    <div className={`input-area-wrapper ${centered ? "input-area-wrapper-centered" : ""}`} role="form" aria-label={t('inputArea.messageInput')}>
      <div className="input-container-wrapper" style={{ position: "relative" }}>
        {/* 附件预览条 */}
        {attachments.length > 0 && (
          <div className="attachment-preview-bar">
            {attachments.map((att, idx) => (
              <div key={idx} className="attachment-chip" title={att.name}>
                <Icon name={att.type === "image" ? "image" : "file"} />
                <span className="attachment-name">{att.name}</span>
                <span className="attachment-size">{formatSize(att.size)}</span>
                <button
                  className="attachment-remove"
                  onClick={() => removeAttachment(idx)}
                  aria-label={t('inputArea.removeAttachment', { name: att.name })}
                >
                  <Icon name="close" />
                </button>
              </div>
            ))}
          </div>
        )}

        {/* 视觉不支持警告 */}
        {showVisionWarning && (
          <div className="vision-warning-bar">
            <Icon name="info" />
            <span>{t('inputArea.visionWarning')}</span>
          </div>
        )}

        <div
          className={`input-container ${hasContent ? "has-content" : ""} ${isDragOver ? "drag-over" : ""}`}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
        >
          <input
            ref={fileInputRef}
            type="file"
            multiple
            accept={SUPPORTED_ATTACHMENT_MIME_TYPES.join(",")}
            style={{ display: "none" }}
            onChange={handleFileChange}
          />

          <textarea
            ref={textareaRef}
            className="input-textarea"
            rows={1}
            placeholder={t('inputArea.placeholder')}
            aria-label={t('inputArea.messageInputBox')}
            value={text}
            onChange={(e) => { setText(e.target.value); handleInput(); }}
            onKeyDown={handleKeyDown}
            onPaste={handlePaste}
            disabled={disabled}
          />

          <div className="input-inner-bottom">
            <div className="input-inner-left">
              {centered ? <WorkspaceSelector /> : <WorkspaceGitStatus />}
            </div>
            <div className="input-inner-right">
              <ModeSelector dropdownUp={!centered} />
              <ProviderSelector dropdownUp={!centered} />
              <div className="input-actions-right">
                <button className="input-btn" title={t('inputArea.attachFile')} aria-label={t('inputArea.attachFile')} onClick={handleFileSelect}>
                  <Icon name="attach" />
                </button>
                {executionStatus === "running" && onStop ? (
                  <button
                    className="stop-btn"
                    title={t('inputArea.stopExecution')}
                    aria-label={t('inputArea.stopExecution')}
                    onClick={onStop}
                  >
                    <Icon name="stop" />
                  </button>
                ) : executionStatus === "stopping" ? (
                  <button
                    className="stop-btn stop-btn-loading"
                    title={t('inputArea.stopping')}
                    disabled
                  >
                    <span className="loading-spinner"></span>
                  </button>
                ) : (
                  <button
                    className={`send-btn ${hasContent && !disabled && configReady ? "send-btn-active" : ""}`}
                    title={configTip || t('inputArea.send')}
                    aria-label={t('inputArea.sendMessage')}
                    aria-disabled={disabled || !hasContent || !configReady}
                    onClick={handleSend}
                    disabled={disabled || !hasContent || !configReady}
                  >
                    <Icon name="send" />
                  </button>
                )}
              </div>
            </div>
          </div>
        </div>

        {/* 拖拽覆盖层 */}
        {isDragOver && (
          <div className="drag-overlay">
            <Icon name="attach" />
            <span>{t('inputArea.dropToAdd')}</span>
          </div>
        )}

        {/* 模板卡片（空会话状态） */}
        {centered && (
          <TemplateCards
            templates={templates}
            onInsert={handleTemplateInsert}
            onOpenSettings={() => openSettings("template")}
          />
        )}
      </div>

      <style>{`
        .input-area-wrapper {
          padding: 10px 24px 14px;
          background: var(--color-bg);
          flex-shrink: 0;
          width: 100%;
        }
        .input-area-wrapper-centered {
          max-width: 760px;
          margin: 0 auto;
        }
        .input-area-wrapper-centered .input-textarea {
          min-height: 60px;
        }
        @media (max-width: 768px) {
          .input-area-wrapper {
            padding: 8px 16px 12px;
          }
          .input-area-wrapper-centered {
            max-width: 100%;
          }
        }
        .attachment-preview-bar {
          display: flex;
          flex-wrap: wrap;
          gap: 6px;
          padding: 6px 10px;
          margin-bottom: 4px;
          background: var(--color-bg-sub);
          border: 1px solid var(--color-border-light);
          border-radius: 9px 9px 0 0;
          border-bottom: none;
        }
        .attachment-chip {
          display: inline-flex;
          align-items: center;
          gap: 4px;
          padding: 3px 8px;
          background: var(--color-bg);
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-sm);
          font-size: 11px;
          color: var(--color-text-secondary);
          max-width: 200px;
        }
        .attachment-name {
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
          max-width: 100px;
        }
        .attachment-size {
          color: var(--color-text-quaternary);
          font-size: 10px;
          flex-shrink: 0;
        }
        .attachment-remove {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 16px;
          height: 16px;
          border-radius: 50%;
          color: var(--color-text-quaternary);
          flex-shrink: 0;
          font-size: 10px;
        }
        .attachment-remove:hover {
          color: var(--color-error);
          background: var(--color-error-light);
        }
        .vision-warning-bar {
          display: flex;
          align-items: center;
          gap: 6px;
          padding: 6px 10px;
          margin-bottom: 4px;
          background: var(--color-warning-light, #fef3cd);
          border: 1px solid var(--color-warning, #ffc107);
          border-radius: 6px;
          font-size: 11px;
          color: var(--color-warning-dark, #856404);
        }
        .input-container {
          display: flex;
          flex-direction: column;
          border: 1px solid var(--color-border-strong);
          border-radius: 12px;
          padding: 10px 14px 8px 14px;
          transition: all 0.2s;
          background: var(--color-bg);
          box-shadow: var(--shadow-xs);
        }
        .input-container:focus-within {
          border-color: color-mix(in srgb, var(--color-border-strong), black 10%);
        }
        .input-container.has-content {
          border-color: var(--color-accent);
        }
        .input-container.drag-over {
          border-color: var(--color-accent);
          background: var(--color-accent-light);
          box-shadow: 0 0 0 3px var(--color-accent-lighter);
        }
        .attachment-preview-bar + .input-container {
          border-radius: 0 0 9px 9px;
        }
        .input-btn {
          width: 28px;
          height: 28px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-sm);
          color: var(--color-text-quaternary);
          transition: all 0.15s;
          flex-shrink: 0;
        }
        .input-btn:hover {
          color: var(--color-text-secondary);
          background: var(--color-bg-sub);
        }
        .input-btn-active {
          color: var(--color-accent);
          background: var(--color-accent-light);
        }
        .input-textarea {
          resize: none;
          box-sizing: border-box;
          min-height: 30px;
          max-height: 240px;
          line-height: 1.5;
          font-size: 14px;
          padding: 2px 0 2px 4px;
          outline: none;
          align-self: stretch;
          overflow-y: auto;
          scrollbar-width: thin;
          scrollbar-color: var(--color-border-strong) transparent;
        }
        .input-textarea:focus-visible {
          outline: none;
        }
        .input-textarea::placeholder {
          color: var(--color-text-quaternary);
        }
        .input-textarea::-webkit-scrollbar {
          width: 4px;
        }
        .input-textarea::-webkit-scrollbar-track {
          background: transparent;
        }
        .input-textarea::-webkit-scrollbar-thumb {
          background: var(--color-border-strong);
          border-radius: 2px;
        }
        .input-textarea::-webkit-scrollbar-thumb:hover {
          background: var(--color-text-quaternary);
        }
        .input-actions-right {
          display: flex;
          align-items: center;
          gap: 4px;
          flex-shrink: 0;
        }
        .send-btn {
          width: 30px;
          height: 30px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-sm);
          color: var(--color-text-quaternary);
          background: var(--color-bg-sub);
          transition: all 0.2s;
          flex-shrink: 0;
        }
        .send-btn:hover:not(:disabled) {
          background: var(--color-bg-hover);
          color: var(--color-text-secondary);
        }
        .send-btn.send-btn-active {
          background: var(--color-accent);
          color: white;
        }
        .send-btn.send-btn-active:hover {
          background: var(--color-accent-hover);
        }
        .send-btn:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }
        .stop-btn {
          width: 30px;
          height: 30px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-sm);
          color: white;
          background: var(--color-error);
          transition: all 0.2s;
          flex-shrink: 0;
        }
        .stop-btn:hover {
          background: var(--color-error);
          filter: brightness(0.9);
        }
        .stop-btn-loading {
          background: var(--color-text-quaternary);
          cursor: wait;
        }
        .loading-spinner {
          width: 14px;
          height: 14px;
          border: 2px solid rgba(255, 255, 255, 0.3);
          border-top-color: white;
          border-radius: 50%;
          animation: spin 0.8s linear infinite;
        }
        @keyframes spin {
          to { transform: rotate(360deg); }
        }
        .drag-overlay {
          position: absolute;
          inset: 0;
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          gap: 8px;
          background: var(--color-accent-light);
          border: 2px dashed var(--color-accent);
          border-radius: 9px;
          color: var(--color-accent);
          font-size: 14px;
          font-weight: 500;
          z-index: 10;
          pointer-events: none;
        }
        .input-inner-bottom {
          display: flex;
          align-items: center;
          justify-content: space-between;
          margin-top: 4px;
          min-height: 28px;
        }
        .input-inner-left {
          display: flex;
          align-items: center;
          flex-shrink: 0;
          min-width: 0;
        }
        .input-inner-right {
          display: flex;
          align-items: center;
          gap: 2px;
          flex-shrink: 0;
        }
        .template-cards-section {
          margin-top: 16px;
        }
        .template-cards-grid {
          display: grid;
          grid-template-columns: repeat(4, 1fr);
          max-width: 520px;
          margin: 0 auto;
          gap: 8px;
        }
        .template-cards-section .template-card {
          display: flex;
          flex-direction: column;
          align-items: center;
          padding: 12px;
          border: 1px solid var(--color-border-light);
          border-radius: 12px;
          background: var(--color-bg);
          cursor: pointer;
          transition: all 0.15s;
          text-align: center;
        }
        .template-cards-section .template-card:hover {
          border-color: var(--color-border);
          background: var(--color-bg-sub);
          box-shadow: var(--shadow-sm);
        }
        .template-cards-section .template-card-name {
          font-size: 12px;
          font-weight: 600;
          color: var(--color-text-primary);
        }
        .template-cards-section .template-card-more {
          display: flex;
          align-items: center;
          justify-content: center;
          border: 1px dashed var(--color-border-light);
          border-radius: 12px;
          background: transparent;
          cursor: pointer;
          transition: all 0.15s;
          padding: 12px;
        }
        .template-cards-section .template-card-more:hover {
          color: var(--color-accent);
          background: var(--color-accent-light);
        }
        .template-cards-section .template-card-more-text {
          font-size: 12px;
          font-weight: 600;
          color: var(--color-text-primary);
        }
        .template-cards-section .template-card-more:hover .template-card-more-text {
          color: var(--color-accent);
        }
      `}</style>
    </div>
  );
}

/**
 * Agent 模式切换下拉框（Plan/Build/Document）
 * 在已存在会话页面的输入框中为上拉列表（dropdownUp=true），
 * 空会话页面为下拉列表（dropdownUp=false）。
 */
function ModeSelector({ dropdownUp = false }: { dropdownUp?: boolean }) {
  const { t } = useTranslation();
  const mode = useAgentModeStore((s) => s.mode);
  const setMode = useAgentModeStore((s) => s.setMode);
  const currentSessionId = useSessionStore((s) => s.currentSessionId);
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // 模式切换：立即更新前端状态（乐观更新），异步通知后端
  // 当前会话为空时仅更新前端状态，不调用后端
  const handleSwitch = async (newMode: 'plan' | 'build' | 'document') => {
    if (newMode === mode) return;
    setMode(newMode);
    setOpen(false);
    if (currentSessionId) {
      try {
        await switchAgentMode(currentSessionId, newMode);
      } catch (err) {
        console.error('切换 Agent 模式失败:', err);
      }
    }
  };

  // 点击外部关闭下拉框
  const handleClickOutside = useCallback((e: MouseEvent) => {
    if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
      setOpen(false);
    }
  }, []);

  // 按 Escape 关闭下拉框
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === "Escape") {
      setOpen(false);
    }
  }, []);

  useEffect(() => {
    if (open) {
      const timer = setTimeout(() => {
        document.addEventListener("mousedown", handleClickOutside);
        document.addEventListener("keydown", handleKeyDown);
      }, 0);
      return () => {
        clearTimeout(timer);
        document.removeEventListener("mousedown", handleClickOutside);
        document.removeEventListener("keydown", handleKeyDown);
      };
    }
  }, [open, handleClickOutside, handleKeyDown]);

  // 三种模式定义：图标、标签、描述
  const modes: Array<{ key: 'plan' | 'build' | 'document'; label: string; icon: 'edit' | 'code' | 'file'; desc: string }> = [
    { key: 'plan', label: t('agentMode.plan'), icon: 'edit', desc: t('agentMode.planMode') },
    { key: 'build', label: t('agentMode.build'), icon: 'code', desc: t('agentMode.buildMode') },
    { key: 'document', label: t('agentMode.document'), icon: 'file', desc: t('agentMode.documentMode') },
  ];

  const currentMode = modes.find((m) => m.key === mode) ?? modes[1];

  return (
    <div ref={containerRef} className="mode-selector-container">
      <div
        role="button"
        aria-label={t('agentMode.switchGroup')}
        tabIndex={0}
        className={`mode-selector-trigger ${open ? "mode-selector-trigger-active" : ""}`}
        onClick={() => setOpen((prev) => !prev)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setOpen((prev) => !prev);
          }
        }}
      >
        <Icon name={currentMode.icon} size={14} />
        <span className="mode-selector-label">{currentMode.label}</span>
        <Icon name={open ? "chevron-up" : "chevron-down"} size={12} />
      </div>

      {open && (
        <div className={`mode-selector-dropdown ${dropdownUp ? "mode-selector-dropdown-up" : ""}`}>
          <div className="mode-selector-list">
            {modes.map((m) => (
              <div
                key={m.key}
                className={`mode-selector-item mode-${m.key} ${m.key === mode ? "mode-selector-item-active" : ""}`}
                onClick={() => handleSwitch(m.key)}
                role="option"
                aria-selected={m.key === mode}
              >
                <div className="mode-selector-item-icon">
                  <Icon name={m.icon} size={16} />
                </div>
                <div className="mode-selector-item-info">
                  <span className="mode-selector-item-name">{m.label}</span>
                  <span className="mode-selector-item-desc">{m.desc}</span>
                </div>
                {m.key === mode && (
                  <Icon name="check" size={14} />
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      <style>{`
        .mode-selector-container {
          position: relative;
        }
        .mode-selector-trigger {
          display: flex;
          align-items: center;
          gap: 4px;
          padding: 4px 8px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          transition: background 0.15s;
          font-size: 12px;
          font-weight: 500;
          color: var(--color-text-secondary);
          white-space: nowrap;
          user-select: none;
        }
        .mode-selector-trigger:hover {
          background: var(--color-bg-sub);
        }
        .mode-selector-trigger-active {
          background: var(--color-bg-sub);
          color: var(--color-text-primary);
        }
        .mode-selector-label {
          overflow: hidden;
          text-overflow: ellipsis;
        }
        .mode-selector-dropdown {
          position: absolute;
          right: 0;
          top: calc(100% + 6px);
          min-width: 200px;
          max-width: 260px;
          background: var(--color-bg-elevated);
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-md);
          box-shadow: var(--shadow-lg);
          z-index: 200;
          animation: mode-dropdown-in 0.15s ease-out;
          overflow: hidden;
        }
        .mode-selector-dropdown-up {
          top: auto;
          bottom: calc(100% + 6px);
          animation-name: mode-dropdown-in-up;
        }
        @keyframes mode-dropdown-in {
          from { opacity: 0; transform: scale(0.96) translateY(4px); }
          to { opacity: 1; transform: scale(1) translateY(0); }
        }
        @keyframes mode-dropdown-in-up {
          from { opacity: 0; transform: scale(0.96) translateY(-4px); }
          to { opacity: 1; transform: scale(1) translateY(0); }
        }
        .mode-selector-list {
          display: flex;
          flex-direction: column;
          gap: 1px;
          padding: 4px;
        }
        .mode-selector-item {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 6px 10px;
          border-radius: var(--radius-sm);
          cursor: pointer;
        }
        .mode-selector-item:hover {
          background: var(--color-bg-hover);
        }
        .mode-selector-item-active {
          background: var(--color-bg-sub);
        }
        .mode-selector-item-icon {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 20px;
          height: 20px;
          border-radius: var(--radius-sm);
          flex-shrink: 0;
          color: var(--color-text-tertiary);
          transition: color 0.15s;
        }
        .mode-plan:hover .mode-selector-item-icon {
          color: var(--color-warning, #f59e0b);
        }
        .mode-build:hover .mode-selector-item-icon {
          color: #3b82f6;
        }
        .mode-document:hover .mode-selector-item-icon {
          color: var(--color-success, #10b981);
        }
        .mode-selector-item-active.mode-plan .mode-selector-item-icon {
          color: var(--color-warning, #f59e0b);
        }
        .mode-selector-item-active.mode-build .mode-selector-item-icon {
          color: #3b82f6;
        }
        .mode-selector-item-active.mode-document .mode-selector-item-icon {
          color: var(--color-success, #10b981);
        }
        .mode-selector-item-info {
          display: flex;
          flex-direction: column;
          gap: 1px;
          min-width: 0;
          flex: 1;
        }
        .mode-selector-item-name {
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-primary);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .mode-selector-item-desc {
          font-size: 11px;
          color: var(--color-text-quaternary);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
      `}</style>
    </div>
  );
}

function TemplateCards({ templates, onInsert, onOpenSettings }: {
  templates: PromptTemplate[];
  onInsert: (text: string) => void;
  onOpenSettings: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="template-cards-section">
      <div className="template-cards-grid">
        {templates.filter((tpl) => tpl.isBuiltin).slice(0, 3).map((tpl) => (
          <button
            key={tpl.id}
            className="template-card"
            onClick={() => onInsert(t(`settings.templates.builtinItems.${tpl.id}.content`, { defaultValue: tpl.content }))}
          >
            <span className="template-card-name">{t(`settings.templates.builtinItems.${tpl.id}.name`, { defaultValue: tpl.name })}</span>
          </button>
        ))}
        <button className="template-card-more" onClick={onOpenSettings}>
          <span className="template-card-more-text">{t("inputArea.templateCards.moreTemplates")}</span>
        </button>
      </div>
    </div>
  );
}
