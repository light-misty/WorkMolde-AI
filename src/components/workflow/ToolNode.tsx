import { useState, useEffect, useRef, useCallback } from "react";
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
  // 判断是否为代码解释器工具
  const isCodeInterpreter = data.toolName === "code_interpreter_handler";
  const [errorExpanded, setErrorExpanded] = useState(false);

  // 代码预览展开/收缩状态
  // 初始展开：代码正在流式输出时展开，完成后收缩
  const [codeExpanded, setCodeExpanded] = useState(data.isCodeStreaming ?? false);
  const prevIsCodeStreamingRef = useRef<boolean | undefined>(undefined);

  // 代码内容：优先使用流式代码，回退到 input.code
  // 去掉开头多余的换行符（LLM 生成的 JSON 字符串值常以 \n 开头）
  const codeContent = (data.streamingCode
    || (data.input?.code as string | undefined)
    || "").replace(/^[\n\r]+/, '');
  const isCodeStreaming = data.isCodeStreaming ?? false;

  const [copied, setCopied] = useState(false);

  // 复制代码到剪贴板
  const handleCopyCode = async () => {
    try {
      await navigator.clipboard.writeText(codeContent);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = codeContent;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // 代码预览自动滚动：流式输出时跟随最新代码，用户手动上滚时暂停
  const codePreviewRef = useRef<HTMLPreElement>(null);
  const codeAutoScrollRef = useRef(true);
  // 标记当前滚动是否由程序触发，避免 onScroll 误判为用户手动上滚
  const isProgrammaticScrollRef = useRef(false);

  // 当代码流式输出结束时，自动收缩代码预览
  useEffect(() => {
    if (prevIsCodeStreamingRef.current === true && !data.isCodeStreaming) {
      setCodeExpanded(false);
    }
    // 当代码流式输出开始时，展开代码预览并重置自动滚动
    if (data.isCodeStreaming && prevIsCodeStreamingRef.current !== true) {
      setCodeExpanded(true);
      codeAutoScrollRef.current = true;
    }
    prevIsCodeStreamingRef.current = data.isCodeStreaming;
  }, [data.isCodeStreaming]);

  // 检测用户是否在代码预览框中手动上滚
  // 程序触发的滚动（isProgrammaticScrollRef）不纳入判断
  const handleCodeScroll = useCallback(() => {
    if (isProgrammaticScrollRef.current) return;
    const el = codePreviewRef.current;
    if (!el) return;
    // 距离底部 20px 以内视为"在底部"，保持自动滚动
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    codeAutoScrollRef.current = distanceFromBottom < 20;
  }, []);

  // 代码流式输出时自动滚动到底部
  useEffect(() => {
    if (isCodeStreaming && codeAutoScrollRef.current && codePreviewRef.current) {
      requestAnimationFrame(() => {
        if (codeAutoScrollRef.current && codePreviewRef.current) {
          // 标记为程序触发的滚动，防止 onScroll 回调误判
          isProgrammaticScrollRef.current = true;
          codePreviewRef.current.scrollTop = codePreviewRef.current.scrollHeight;
          // 程序滚动后延迟重置标志，确保 onScroll 事件已处理完毕
          requestAnimationFrame(() => {
            isProgrammaticScrollRef.current = false;
          });
        }
      });
    }
  }, [codeContent, isCodeStreaming]);

  // 代码解释器错误：移除原始代码部分（已在代码预览区域展示），余下文本截断显示
  const errorText = data.error || "";
  const cleanError = isCodeInterpreter
    ? errorText.split("\n\n原始代码:")[0].trim()
    : errorText;
  const shouldTruncateError = cleanError.length > 150;
  const displayError = shouldTruncateError && !errorExpanded
    ? cleanError.slice(0, 150) + "..."
    : cleanError;

  // 收缩状态下显示前几行代码（最多3行），而非仅用省略号
  const collapsedMaxLines = 3;
  const codeLines = codeContent.split('\n');
  const collapsedCodePreview = codeLines.length <= collapsedMaxLines
    ? codeContent
    : codeLines.slice(0, collapsedMaxLines).join('\n');

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

        {/* 代码预览区域（仅 code_interpreter_handler 显示） */}
        {isCodeInterpreter && codeContent && (
          <div className={`wf-code-preview ${codeExpanded ? "wf-code-preview-expanded" : "wf-code-preview-collapsed"}`}>
            <div className="wf-code-preview-header">
              <span className="wf-code-preview-label">
                {isCodeStreaming ? t('toolNode.writingCode') : t('toolNode.codePreview')}
              </span>
              {!isCodeStreaming && (
                <div className="wf-code-preview-header-actions">
                  <button
                    className="wf-code-preview-toggle"
                    onClick={(e) => {
                      e.stopPropagation();
                      setCodeExpanded(!codeExpanded);
                    }}
                  >
                    {codeExpanded ? t('toolNode.collapseCode') : t('toolNode.expandCode')}
                  </button>
                  <button
                    className="wf-code-preview-copy-btn"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleCopyCode();
                    }}
                  >
                    {copied ? t('common.copied') : t('common.copy')}
                  </button>
                </div>
              )}
            </div>
            {codeExpanded ? (
              <pre ref={codePreviewRef} className="wf-code-preview-content" onScroll={handleCodeScroll}>
                {codeContent}

              </pre>
            ) : (
              <div className="wf-code-preview-collapsed-text">
                {collapsedCodePreview}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
