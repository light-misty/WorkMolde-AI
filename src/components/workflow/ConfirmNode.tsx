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
  const permissionHandler = useWorkflowStore((s) => s.permissionHandler);
  const isPending = data.confirmed === null && node.status === "running";
  const [feedback, setFeedback] = useState("");

  // Phase 2: 优先使用 permissionHandler（三态权限系统），回退到 confirmHandler（旧版二态）
  const usePermissionFlow = !!permissionHandler;

  // 风险等级颜色映射：critical=红、high=橙、medium=蓝、normal=灰
  const riskLevelColor = (level?: string): string => {
    switch (level) {
      case 'critical': return 'var(--color-error, #ef4444)';
      case 'high': return 'var(--color-warning, #f59e0b)';
      case 'medium': return 'var(--color-accent, #3b82f6)';
      default: return 'var(--color-text-tertiary, #6b7280)';
    }
  };

  // 权限回复结果文本：区分 once/always/reject 三态
  const getResultText = (): string => {
    if (data.permissionResponse === 'once') return t('permission.onceAllowed');
    if (data.permissionResponse === 'always') return t('permission.alwaysAllowed');
    if (data.confirmed === false) {
      return data.feedback
        ? `${t('permission.rejected')}: ${data.feedback}`
        : t('permission.rejected');
    }
    return t('confirmNode.userConfirmed');
  };

  return (
    <div className="wf-node">
      <div className="wf-confirm-flat">
        <div className="wf-confirm-title">
          <Icon name="warning" size={14} style={{ color: riskLevelColor(data.riskLevel) }} />
          {data.title}
          {data.riskLevel && (
            <span
              className="wf-confirm-risk-badge"
              style={{
                color: riskLevelColor(data.riskLevel),
                borderColor: riskLevelColor(data.riskLevel),
              }}
            >
              {data.riskLevel}
            </span>
          )}
        </div>
        <div className="wf-confirm-desc">{data.description}</div>

        {isPending ? (
          <div className="wf-confirm-actions">
            {usePermissionFlow ? (
              // Phase 2: 三态权限审批按钮（仅本次允许/永久允许/拒绝）
              <div className="wf-permission-buttons">
                <button
                  className="wf-perm-btn wf-perm-once"
                  onClick={async (e) => {
                    e.stopPropagation();
                    await permissionHandler?.('once');
                  }}
                >
                  {t('permission.once')}
                </button>
                <button
                  className="wf-perm-btn wf-perm-always"
                  onClick={async (e) => {
                    e.stopPropagation();
                    await permissionHandler?.('always');
                  }}
                >
                  {t('permission.always')}
                </button>
                <button
                  className="wf-perm-btn wf-perm-reject"
                  onClick={async (e) => {
                    e.stopPropagation();
                    await permissionHandler?.('reject', feedback || undefined);
                  }}
                >
                  {t('permission.reject')}
                </button>
              </div>
            ) : (
              // 旧版二态确认按钮（向后兼容，permissionHandler 不存在时使用）
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
            )}
            <div className="wf-confirm-feedback">
              <textarea
                className="wf-confirm-feedback-input"
                placeholder={t('permission.feedbackPlaceholder')}
                value={feedback}
                onChange={(e) => setFeedback(e.target.value)}
                rows={2}
              />
            </div>
          </div>
        ) : (
          <div className={`wf-confirm-result ${data.confirmed ? "confirmed" : "cancelled"}`}>
            {getResultText()}
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
        .wf-permission-buttons {
          display: flex;
          gap: 6px;
          margin-bottom: 8px;
          flex-wrap: wrap;
        }
        .wf-perm-btn {
          padding: 4px 12px;
          min-height: 28px;
          border-radius: var(--radius-sm, 6px);
          font-size: 12px;
          font-weight: 500;
          cursor: pointer;
          transition: all 0.15s;
          border: 1px solid transparent;
          white-space: nowrap;
        }
        .wf-perm-once {
          background: var(--color-accent, #3b82f6);
          color: white;
          border-color: var(--color-accent, #3b82f6);
        }
        .wf-perm-once:hover {
          filter: brightness(0.9);
        }
        .wf-perm-always {
          background: var(--color-success, #10b981);
          color: white;
          border-color: var(--color-success, #10b981);
        }
        .wf-perm-always:hover {
          filter: brightness(0.9);
        }
        .wf-perm-reject {
          background: transparent;
          color: var(--color-error, #ef4444);
          border-color: var(--color-error, #ef4444);
        }
        .wf-perm-reject:hover {
          background: var(--color-error-light, #fee2e2);
        }
        .wf-confirm-risk-badge {
          display: inline-block;
          padding: 1px 6px;
          margin-left: 6px;
          font-size: 10px;
          font-weight: 600;
          text-transform: uppercase;
          border: 1px solid;
          border-radius: 3px;
          line-height: 1.4;
        }
        .wf-confirm-feedback-input {
          width: 100%;
          max-width: 200px;
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
