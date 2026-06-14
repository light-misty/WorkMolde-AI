import { useState } from "react";
import type { WorkflowNode, ToolNodeData } from "../../types";
import { useTranslation } from 'react-i18next';
import { Icon } from "../common/Icon";

interface ToolNodeProps {
  node: WorkflowNode<"tool">;
}

export function ToolNode({ node }: ToolNodeProps) {
  const { t } = useTranslation();
  const data = node.data as ToolNodeData;
  const hasError = data.success === false;
  // 判断工具是否正在执行中
  const isRunning = node.status === "running";
  // 判断是否为代码解释器工具
  const isCodeInterpreter = data.toolName === "code_interpreter_handler";
  const [errorExpanded, setErrorExpanded] = useState(false);

  // 代码解释器错误：截断显示，可展开
  const errorText = data.error || "";
  const shouldTruncateError = isCodeInterpreter && errorText.length > 150;
  const displayError = shouldTruncateError && !errorExpanded
    ? errorText.slice(0, 150) + "..."
    : errorText;

  return (
    <div className={`wf-node animate-node-in${isRunning ? " wf-tool-running" : ""}`}>
      <div className={`wf-node-dot${isRunning ? " wf-tool-dot-running" : " bg-bg-sub text-text-secondary"}`}>
        {isRunning ? (
          // 执行中：显示旋转加载图标
          <svg className="wf-tool-spinner" viewBox="0 0 24 24" fill="none">
            <circle className="wf-tool-spinner-track" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" />
            <path className="wf-tool-spinner-arc" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
          </svg>
        ) : hasError ? (
          // 执行失败：显示错误图标
          <Icon name="error" size={12} />
        ) : (
          // 执行完成：显示工具图标
          <Icon name="tool" size={12} />
        )}
      </div>

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
            {isCodeInterpreter ? t('toolNode.codeExecutionFailed') + ": " : ""}
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

      <style>{`
        .wf-error-expand-btn {
          font-size: 10px;
          color: var(--color-accent);
          background: none;
          border: none;
          cursor: pointer;
          padding: 0 2px;
          margin-left: 4px;
        }
        .wf-error-expand-btn:hover {
          text-decoration: underline;
        }
      `}</style>
    </div>
  );
}
