import type { WorkflowNode, ErrorNodeData } from "../../types";
import { useTranslation } from 'react-i18next';
import { Icon } from "../common/Icon";

interface ErrorNodeProps {
  node: WorkflowNode<"error">;
  onToggle: () => void;
  onRetry?: () => void;
}

export function ErrorNode({ node, onRetry }: ErrorNodeProps) {
  const { t } = useTranslation();
  const data = node.data as ErrorNodeData;

  return (
    <div className="wf-node animate-node-in">
      <div className="wf-error-flat">
        <div className="wf-error-message">{data.message}</div>
        <details className="wf-error-details">
          <summary>{t('errorNode.errorDetails')}</summary>
          <div className="wf-error-detail-content">
            <div>{t('errorNode.errorCode')}: E{data.code}</div>
            <div>{t('errorNode.module')}: {data.module}</div>
          </div>
        </details>
        {data.recoverable && onRetry && (
          <button className="wf-error-retry-btn" onClick={onRetry}>
            <Icon name="refresh" size={14} />
            {t('errorNode.retry')}
          </button>
        )}
      </div>
    </div>
  );
}
