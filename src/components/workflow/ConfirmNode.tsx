import { useState } from "react";
import type { WorkflowNode, ConfirmNodeData } from "../../types";
import { useTranslation } from 'react-i18next';
import { Icon } from "../common/Icon";
import { useWorkflowStore } from "../../stores/useWorkflowStore";

interface ConfirmNodeProps {
  node: WorkflowNode<"confirm">;
}

export function ConfirmNode({ node }: ConfirmNodeProps) {
  const { t } = useTranslation();
  const data = node.data as ConfirmNodeData;
  const confirmHandler = useWorkflowStore((s) => s.confirmHandler);
  const isPending = data.confirmed === null && node.status === "running";
  const [codeExpanded, setCodeExpanded] = useState(false);
  const [feedback, setFeedback] = useState("");

  // 代码预览：截断显示
  const codePreview = data.code
    ? codeExpanded
      ? data.code
      : data.code.length > 300
        ? data.code.slice(0, 300) + "..."
        : data.code
    : null;

  return (
    <div className="wf-node animate-node-in">
      <div className="wf-confirm-flat">
        <div className="wf-confirm-title">
          <Icon name="warning" size={14} />
          {data.title}
        </div>
        <div className="wf-confirm-desc">{data.description}</div>

        {/* 代码预览区域 */}
        {codePreview && (
          <div className="wf-confirm-code-section">
            <div className="wf-confirm-code-header">
              <span className="wf-confirm-code-label">{t('confirmNode.codePreview')}</span>
              {data.code && data.code.length > 300 && (
                <button
                  className="wf-confirm-code-toggle"
                  onClick={(e) => {
                    e.stopPropagation();
                    setCodeExpanded(!codeExpanded);
                  }}
                >
                  {codeExpanded ? t('confirmNode.collapseCode') : t('confirmNode.expandCode')}
                </button>
              )}
            </div>
            <pre className="wf-confirm-code">{codePreview}</pre>
          </div>
        )}

        {isPending ? (
          <div className="wf-confirm-actions">
            <div className="wf-confirm-buttons">
              <button
                className="wf-confirm-btn btn btn-danger"
                onClick={async (e) => {
                  e.stopPropagation();
                  await confirmHandler?.(true);
                }}
              >
                {data.confirmLabel}
              </button>
              <button
                className="wf-confirm-btn btn btn-ghost"
                onClick={async (e) => {
                  e.stopPropagation();
                  await confirmHandler?.(false, feedback || undefined);
                }}
              >
                {data.cancelLabel}
              </button>
            </div>
            <div className="wf-confirm-feedback">
              <textarea
                className="wf-confirm-feedback-input"
                placeholder={t('confirmNode.feedbackPlaceholder')}
                value={feedback}
                onChange={(e) => setFeedback(e.target.value)}
                rows={2}
              />
            </div>
          </div>
        ) : (
          <div className={`wf-confirm-result ${data.confirmed ? "confirmed" : "cancelled"}`}>
            {data.confirmed
              ? t('confirmNode.userConfirmed')
              : data.feedback
                ? `${t('confirmNode.userCancelled')}: ${data.feedback}`
                : t('confirmNode.userCancelled')}
          </div>
        )}
      </div>

      <style>{`
        .wf-confirm-code-section {
          margin-top: 8px;
          border: 1px solid var(--color-border-light);
          border-radius: 6px;
          overflow: hidden;
        }
        .wf-confirm-code-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 4px 8px;
          background: var(--color-bg-sub);
          border-bottom: 1px solid var(--color-border-light);
        }
        .wf-confirm-code-label {
          font-size: 11px;
          font-weight: 500;
          color: var(--color-text-tertiary);
        }
        .wf-confirm-code-toggle {
          font-size: 10px;
          color: var(--color-accent);
          background: none;
          border: none;
          cursor: pointer;
          padding: 0;
        }
        .wf-confirm-code-toggle:hover {
          text-decoration: underline;
        }
        .wf-confirm-code {
          margin: 0;
          padding: 8px;
          font-family: 'Cascadia Code', 'Fira Code', 'Consolas', monospace;
          font-size: 11px;
          line-height: 1.5;
          color: var(--color-text-secondary);
          background: var(--color-bg);
          max-height: 200px;
          overflow: auto;
          white-space: pre-wrap;
          word-break: break-all;
        }
        .wf-confirm-actions {
          flex-direction: column;
        }
        .wf-confirm-buttons {
          display: flex;
          gap: 8px;
          margin-bottom: 8px;
        }
        .wf-confirm-btn {
          padding: 4px 12px;
          min-height: 28px;
        }
        .wf-confirm-feedback-input {
          width: 100%;
          max-width: 160px;
          padding: 6px 8px;
          font-size: 12px;
          line-height: 1.4;
          border: 1px solid var(--color-border-light);
          border-radius: 6px;
          background: var(--color-bg);
          color: var(--color-text);
          resize: vertical;
          font-family: inherit;
          box-sizing: border-box;
        }
        .wf-confirm-feedback-input:focus {
          outline: none;
          border-color: var(--color-accent);
        }
        .wf-confirm-feedback-input::placeholder {
          color: var(--color-text-tertiary);
        }
      `}</style>
    </div>
  );
}
