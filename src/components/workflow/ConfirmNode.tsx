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
  const [feedback, setFeedback] = useState("");

  return (
    <div className="wf-node">
      <div className="wf-confirm-flat">
        <div className="wf-confirm-title">
          <Icon name="warning" size={14} />
          {data.title}
        </div>
        <div className="wf-confirm-desc">{data.description}</div>

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
