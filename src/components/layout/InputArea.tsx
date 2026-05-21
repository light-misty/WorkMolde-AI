import { useState, useRef, useCallback, type KeyboardEvent } from "react";
import { Icon } from "../common/Icon";

interface InputAreaProps {
  onSend: (text: string) => void;
  disabled?: boolean;
  templateLabel?: string;
  onToggleTemplate?: () => void;
}

export function InputArea({ onSend, disabled = false, templateLabel, onToggleTemplate }: InputAreaProps) {
  const [text, setText] = useState("");
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
    },
    [handleSend]
  );

  const handleInput = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 120) + "px";
  }, []);

  const hasContent = text.trim().length > 0;

  return (
    <div className="input-area-wrapper">
      <div className={`input-container ${hasContent ? "has-content" : ""}`}>
        <button className="input-btn" title="附加文件">
          <Icon name="attach" />
        </button>

        <textarea
          ref={textareaRef}
          className="input-textarea"
          rows={1}
          placeholder="输入指令，让Agent帮你处理文档..."
          value={text}
          onChange={(e) => { setText(e.target.value); handleInput(); }}
          onKeyDown={handleKeyDown}
          disabled={disabled}
        />

        <div className="input-actions-right">
          {templateLabel && (
            <button className="template-badge" title={templateLabel}>
              <Icon name="template" size={12} />
              <span>{templateLabel}</span>
            </button>
          )}
          {onToggleTemplate && (
            <button className="input-btn" title="Prompt模板" onClick={onToggleTemplate}>
              <Icon name="template" />
            </button>
          )}
          <button
            className={`send-btn ${hasContent && !disabled ? "send-btn-active" : ""}`}
            title="发送"
            onClick={handleSend}
            disabled={disabled || !text.trim()}
          >
            <Icon name="send" />
          </button>
        </div>
      </div>

      <div className="shortcut-hints">
        <span>
          <kbd className="kbd">Enter</kbd> 发送
        </span>
        <span>
          <kbd className="kbd">Shift + Enter</kbd> 换行
        </span>
        <span>
          <kbd className="kbd">Ctrl + N</kbd> 新建会话
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
          box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
        }
        .input-container:focus-within {
          border-color: var(--color-accent);
          box-shadow: 0 0 0 3px var(--color-accent-lighter), 0 2px 8px rgba(0, 0, 0, 0.06);
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
        .template-badge {
          display: inline-flex;
          align-items: center;
          gap: 3px;
          padding: 2px 6px;
          background: var(--color-accent-light);
          border-radius: var(--radius-xs);
          font-size: 10px;
          color: var(--color-accent);
          font-weight: 500;
          border: none;
          cursor: default;
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
