import { useState, useRef, useCallback, type KeyboardEvent, type DragEvent, type ClipboardEvent } from "react";
import { Icon } from "../common/Icon";
import { TemplatePicker } from "../common/TemplatePicker";
import type { ExecutionStatus } from "../../types/workflow";
import type { AttachmentMeta } from "../../types/session";
import { useAttachmentStore, inferAttachmentType, SUPPORTED_ATTACHMENT_MIME_TYPES, MAX_IMAGE_SIZE, MAX_TEXT_SIZE, MAX_DOCUMENT_SIZE, MAX_ATTACHMENT_COUNT, hasImageAttachments } from "../../stores/useAttachmentStore";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { formatSize } from "../../utils/format";

interface InputAreaProps {
  onSend: (text: string) => void;
  disabled?: boolean;
  // Agent 执行状态
  executionStatus?: ExecutionStatus;
  onStop?: () => void;
}

export function InputArea({ onSend, disabled = false, executionStatus = "idle", onStop }: InputAreaProps) {
  const [text, setText] = useState("");
  const [pickerOpen, setPickerOpen] = useState(false);
  const [isDragOver, setIsDragOver] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const attachments = useAttachmentStore((s) => s.attachments);
  const addAttachment = useAttachmentStore((s) => s.addAttachment);
  const removeAttachment = useAttachmentStore((s) => s.removeAttachment);
  const clearAttachments = useAttachmentStore((s) => s.clearAttachments);

  // 检查当前 Provider 是否支持视觉
  const providers = useSettingsStore((s) => s.llmProviders);
  const currentProvider = providers.find((p) => p.isDefault) || providers[0];
  const supportsVision = currentProvider?.supportsVision ?? false;
  const showVisionWarning = hasImageAttachments(attachments) && !supportsVision;

  const handleSend = useCallback(() => {
    const trimmed = text.trim();
    if ((!trimmed && attachments.length === 0) || disabled) return;
    onSend(trimmed || "[附件]");
    setText("");
    clearAttachments();
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
    }
  }, [text, disabled, onSend, attachments.length, clearAttachments]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
      // Ctrl+/ 快捷键打开模板选择器
      if (e.ctrlKey && e.key === "/") {
        e.preventDefault();
        setPickerOpen((prev) => !prev);
      }
      // Escape 关闭模板选择器
      if (e.key === "Escape" && pickerOpen) {
        e.preventDefault();
        setPickerOpen(false);
      }
    },
    [handleSend, pickerOpen]
  );

  const handleInput = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 120) + "px";
  }, []);

  // 模板插入回调
  const handleTemplateInsert = useCallback((templateText: string) => {
    setText(templateText);
    setPickerOpen(false);
    // 聚焦输入框
    setTimeout(() => textareaRef.current?.focus(), 50);
    // 调整高度
    setTimeout(() => {
      if (textareaRef.current) {
        textareaRef.current.style.height = "auto";
        textareaRef.current.style.height = Math.min(textareaRef.current.scrollHeight, 120) + "px";
      }
    }, 60);
  }, []);

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

  return (
    <div className="input-area-wrapper" role="form" aria-label="消息输入">
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
                  aria-label={`移除 ${att.name}`}
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
            <span>当前模型不支持图片理解，图片内容将无法被识别。建议用文字描述图片内容。</span>
          </div>
        )}

        <div
          className={`input-container ${hasContent ? "has-content" : ""} ${isDragOver ? "drag-over" : ""}`}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
        >
          <button className="input-btn" title="附加文件" aria-label="附加文件" onClick={handleFileSelect}>
            <Icon name="attach" />
          </button>
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
            placeholder="输入指令，让Agent帮你处理文档..."
            aria-label="消息输入框"
            value={text}
            onChange={(e) => { setText(e.target.value); handleInput(); }}
            onKeyDown={handleKeyDown}
            onPaste={handlePaste}
            disabled={disabled}
          />

          <div className="input-actions-right">
            <button
              className={`input-btn ${pickerOpen ? "input-btn-active" : ""}`}
              title="Prompt模板 (Ctrl+/)"
              aria-label="Prompt模板"
              aria-expanded={pickerOpen}
              onClick={() => setPickerOpen(!pickerOpen)}
            >
              <Icon name="template" />
            </button>
            {executionStatus === "running" && onStop ? (
              <button
                className="stop-btn"
                title="停止执行"
                aria-label="停止执行"
                onClick={onStop}
              >
                <Icon name="stop" />
              </button>
            ) : executionStatus === "stopping" ? (
              <button
                className="stop-btn stop-btn-loading"
                title="正在停止..."
                disabled
              >
                <span className="loading-spinner"></span>
              </button>
            ) : (
              <button
                className={`send-btn ${hasContent && !disabled ? "send-btn-active" : ""}`}
                title="发送"
                aria-label="发送消息"
                aria-disabled={disabled || !hasContent}
                onClick={handleSend}
                disabled={disabled || !hasContent}
              >
                <Icon name="send" />
              </button>
            )}
          </div>
        </div>

        {/* 拖拽覆盖层 */}
        {isDragOver && (
          <div className="drag-overlay">
            <Icon name="attach" />
            <span>释放以添加附件</span>
          </div>
        )}

        {/* 模板选择器 */}
        <TemplatePicker
          open={pickerOpen}
          onClose={() => setPickerOpen(false)}
          onInsert={handleTemplateInsert}
        />
      </div>

      <div className="shortcut-hints" aria-hidden="true">
        <span>
          <kbd className="kbd">Enter</kbd> 发送
        </span>
        <span>
          <kbd className="kbd">Shift + Enter</kbd> 换行
        </span>
        <span>
          <kbd className="kbd">Ctrl + /</kbd> 模板
        </span>
        <span>
          <kbd className="kbd">Ctrl + V</kbd> 粘贴图片
        </span>
      </div>

      <style>{`
        .input-area-wrapper {
          padding: 10px 24px 14px;
          background: var(--color-bg);
          flex-shrink: 0;
        }
        @media (max-width: 768px) {
          .input-area-wrapper {
            padding: 8px 16px 12px;
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
          align-items: center;
          gap: 6px;
          border: 1px solid var(--color-border-light);
          border-radius: 9px;
          padding: 6px 10px 6px 12px;
          transition: all 0.2s;
          background: var(--color-bg);
          box-shadow: var(--shadow-xs);
        }
        .input-container:focus-within {
          border-color: var(--color-accent);
          box-shadow: 0 0 0 3px var(--color-accent-lighter), var(--shadow-sm);
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
          flex: 1;
          resize: none;
          min-height: 20px;
          max-height: 100px;
          line-height: 1.5;
          font-size: 13px;
          padding: 2px 4px;
          outline: none;
        }
        .input-textarea:focus-visible {
          outline: none;
        }
        .input-textarea::placeholder {
          color: var(--color-text-quaternary);
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
        .shortcut-hints {
          font-size: 10px;
          color: var(--color-text-quaternary);
          margin-top: 6px;
          padding-left: 4px;
          display: flex;
          align-items: center;
          gap: 12px;
        }
        .kbd {
          font-family: var(--font-mono);
          font-size: 9px;
          padding: 1px 4px;
          background: var(--color-bg-sub);
          border: 1px solid var(--color-border-light);
          border-radius: 2px;
          color: var(--color-text-tertiary);
        }
      `}</style>
    </div>
  );
}
