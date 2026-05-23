import { useState, useRef, useCallback, type KeyboardEvent } from "react";
import { Icon } from "../common/Icon";
import { TemplatePicker } from "../common/TemplatePicker";
import type { ExecutionStatus } from "../../types/workflow";

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
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const handleSend = useCallback(() => {
    const trimmed = text.trim();
    if (!trimmed || disabled) return;
    onSend(trimmed);
    setText("");
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
    }
  }, [text, disabled, onSend]);

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

  const hasContent = text.trim().length > 0;

  return (
    <div className="input-area-wrapper" role="form" aria-label="消息输入">
      <div className="input-container-wrapper" style={{ position: "relative" }}>
        <div className={`input-container ${hasContent ? "has-content" : ""}`}>
          <button className="input-btn" title="附加文件" aria-label="附加文件">
            <Icon name="attach" />
          </button>

          <textarea
            ref={textareaRef}
            className="input-textarea"
            rows={1}
            placeholder="输入指令，让Agent帮你处理文档..."
            aria-label="消息输入框"
            value={text}
            onChange={(e) => { setText(e.target.value); handleInput(); }}
            onKeyDown={handleKeyDown}
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
                aria-disabled={disabled || !text.trim()}
                onClick={handleSend}
                disabled={disabled || !text.trim()}
              >
                <Icon name="send" />
              </button>
            )}
          </div>
        </div>

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
        .input-container {
          display: flex;
          align-items: center;
          gap: 6px;
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-lg);
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
