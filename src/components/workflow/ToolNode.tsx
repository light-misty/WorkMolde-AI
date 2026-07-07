import { useState } from "react";
import type { WorkflowNode, ToolNodeData } from "../../types";
import { useTranslation } from 'react-i18next';

interface ToolNodeProps {
  node: WorkflowNode<"tool">;
}

export function ToolNode({ node }: ToolNodeProps) {
  const { t } = useTranslation();
  const data = node.data as ToolNodeData;
  const hasError = data.success === false;
  // 判断工具是否正在执行中
  const isRunning = node.status === "running";
  const [errorExpanded, setErrorExpanded] = useState(false);

  // 错误文本：截断显示
  const errorText = data.error || "";
  const shouldTruncateError = errorText.length > 150;
  const displayError = shouldTruncateError && !errorExpanded
    ? errorText.slice(0, 150) + "..."
    : errorText;

  return (
    <div className={`wf-node${isRunning ? " wf-tool-running" : ""}`}>
      <div className="wf-tool-content">
        {/* 工具名称和简要描述 */}
        <div className="wf-tool-brief">
          <span className="font-mono">{data.toolName}</span>
          <span> · </span>
          <span>{data.briefDescription}</span>
          {isRunning && (
            <span className="wf-tool-status-running">{t('toolNode.executing')}</span>
          )}
          {hasError && data.error && (
            <span className="wf-tool-error">
              {" — "}
              {displayError}
              {shouldTruncateError && (
                <button
                  className="wf-error-expand-btn"
                  onClick={(e) => {
                    e.stopPropagation();
                    setErrorExpanded(!errorExpanded);
                  }}
                >
                  {errorExpanded ? t('toolNode.collapseError') : t('toolNode.expandError')}
                </button>
              )}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
