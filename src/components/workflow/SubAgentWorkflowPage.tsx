import { useEffect, useRef, useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { useWorkflowStore } from "../../stores/useWorkflowStore";
import { Icon } from "../common/Icon";
import { WorkflowNodeRenderer } from "./WorkflowNode";
import { listSubAgentMessages } from "../../services/tauri";
import { CustomScrollArea } from "../common/CustomScrollArea";

interface SubAgentWorkflowPageProps {
  agentId: string;
}

/**
 * 子 Agent 工作流详情页
 * 展示指定子 Agent 的完整工作流时间线
 * - 顶部栏：返回按钮 + 标题
 * - 主体：复用 WorkflowNodeRenderer 渲染 subAgentNodes
 */
export function SubAgentWorkflowPage({ agentId }: SubAgentWorkflowPageProps) {
  const { t } = useTranslation();
  const subAgentNodes = useWorkflowStore((s) => s.subAgentNodes);
  const clearSubAgentWorkflow = useWorkflowStore((s) => s.clearSubAgentWorkflow);
  const loadSubAgentMessages = useWorkflowStore((s) => s.loadSubAgentMessages);
  const registerNodeRef = useWorkflowStore((s) => s.registerNodeRef);
  const unregisterNodeRef = useWorkflowStore((s) => s.unregisterNodeRef);
  const scrollRef = useRef<HTMLDivElement>(null);
  // 本地 loading 状态：控制初次加载
  const [localLoading, setLocalLoading] = useState(true);

  // 追随滚动相关状态
  // 追踪是否应自动滚动（用户未手动上滚时自动跟随）
  const autoScrollRef = useRef(true);
  // 用于取消前一次 requestAnimationFrame，避免同一帧内重复滚动
  const rafIdRef = useRef<number>(0);
  // 标记当前滚动是否由程序触发，避免 onScroll 误判为用户手动上滚
  const isProgrammaticScrollRef = useRef(false);
  // 追踪上一次节点数量，用于判断是会话切换还是增量更新
  const prevNodesLengthRef = useRef(subAgentNodes.length);

  // 计算流式内容变化标识：当流式节点的文本内容增长时，此值变化
  const streamingContentKey = subAgentNodes.reduce((acc, node) => {
    if (node.type === "content" || node.type === "thinking") {
      const d = node.data as { content: string; isStreaming?: boolean };
      if (node.status === "running" || d.isStreaming) return acc + d.content.length;
    }
    return acc;
  }, 0);

  // 检测用户是否手动上滚，决定是否自动跟随
  const handleScroll = useCallback(() => {
    if (isProgrammaticScrollRef.current) return;
    const el = scrollRef.current;
    if (!el) return;
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    autoScrollRef.current = distanceFromBottom < 50;
  }, []);

  // 加载子 Agent 消息：agentId 变化时重新加载
  useEffect(() => {
    let cancelled = false;
    setLocalLoading(true);

    listSubAgentMessages(agentId)
      .then((messages) => {
        if (cancelled) return;
        loadSubAgentMessages(messages);
        setLocalLoading(false);
      })
      .catch((err) => {
        if (cancelled) return;
        console.error("Failed to load sub agent messages:", err);
        loadSubAgentMessages([]);
        setLocalLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [agentId, loadSubAgentMessages]);

  // 自动滚动到底部：根据场景选择合适的滚动行为
  useEffect(() => {
    const prevLength = prevNodesLengthRef.current;
    prevNodesLengthRef.current = subAgentNodes.length;

    if (subAgentNodes.length > 0) {
      cancelAnimationFrame(rafIdRef.current);
      rafIdRef.current = requestAnimationFrame(() => {
        if (!scrollRef.current) return;

        isProgrammaticScrollRef.current = true;
        const el = scrollRef.current;

        const isSessionSwitch = subAgentNodes.length < prevLength || Math.abs(subAgentNodes.length - prevLength) > 1;
        const isNewNode = !isSessionSwitch && subAgentNodes.length > prevLength;
        const isStreaming = streamingContentKey > 0;

        if (isSessionSwitch) {
          autoScrollRef.current = true;
          el.scrollTop = el.scrollHeight;
          requestAnimationFrame(() => { isProgrammaticScrollRef.current = false; });
        } else if (autoScrollRef.current || isNewNode) {
          if (isNewNode) autoScrollRef.current = true;
          if (isStreaming || isNewNode) {
            el.scrollTo({ top: el.scrollHeight, behavior: "auto" });
          } else {
            el.scrollTo({ top: el.scrollHeight, behavior: "smooth" });
          }
          if (isNewNode) {
            let retries = 0;
            const retryScroll = () => {
              if (retries >= 5) { isProgrammaticScrollRef.current = false; return; }
              retries++;
              requestAnimationFrame(() => {
                if (el.scrollHeight - el.scrollTop - el.clientHeight >= 2) {
                  el.scrollTo({ top: el.scrollHeight, behavior: "auto" });
                  retryScroll();
                } else {
                  isProgrammaticScrollRef.current = false;
                }
              });
            };
            requestAnimationFrame(retryScroll);
          } else {
            requestAnimationFrame(() => { isProgrammaticScrollRef.current = false; });
          }
        } else {
          isProgrammaticScrollRef.current = false;
        }
      });
    }
    return () => cancelAnimationFrame(rafIdRef.current);
  }, [subAgentNodes.length, streamingContentKey]);

  // 返回主工作流
  const handleBack = () => {
    clearSubAgentWorkflow();
  };

  return (
    <div className="subagent-page">
      {/* 顶部栏：返回按钮 + 标题 */}
      <div className="subagent-header">
        <button
          className="subagent-back-btn"
          onClick={handleBack}
          title={t("subAgentWorkflow.back")}
        >
          <Icon name="back" size={16} />
          <span>{t("subAgentWorkflow.back")}</span>
        </button>
        <h3 className="subagent-title">{t("subAgentWorkflow.title")}</h3>
      </div>

      {/* 主体：子 Agent 工作流时间线 */}
      <div className="subagent-body">
        {localLoading ? (
          <div className="subagent-status">{t("subAgentWorkflow.loading")}</div>
        ) : subAgentNodes.length === 0 ? (
          <div className="subagent-status">{t("subAgentWorkflow.empty")}</div>
        ) : (
          <CustomScrollArea
            className="workflow-scroll-container"
            scrollRef={scrollRef}
            onScroll={handleScroll}
          >
            <div className="workflow-scroll-padding">
              {subAgentNodes.map((node) => (
                <WorkflowNodeRenderer
                  key={node.id}
                  node={node}
                  hideCopy
                  nodeRef={(el) => {
                    if (el) {
                      registerNodeRef(node.id, el);
                    } else {
                      unregisterNodeRef(node.id);
                    }
                  }}
                />
              ))}
            </div>
          </CustomScrollArea>
        )}
      </div>

      <style>{`
        .subagent-page {
          display: flex;
          flex-direction: column;
          height: 100%;
          min-height: 0;
        }
        .subagent-header {
          display: flex;
          align-items: center;
          gap: 12px;
          padding: 8px 16px;
          border-bottom: 1px solid var(--color-border, #e5e6eb);
          flex-shrink: 0;
          height: 44px;
        }
        .subagent-back-btn {
          display: flex;
          align-items: center;
          gap: 4px;
          padding: 4px 8px;
          background: transparent;
          border: none;
          color: var(--color-text-secondary, #646a73);
          cursor: pointer;
          font-size: 13px;
          border-radius: 4px;
          transition: background 0.15s, color 0.15s;
        }
        .subagent-back-btn:hover {
          background: var(--color-bg-hover, #f0f1f5);
          color: var(--color-text-primary, #1f2329);
        }
        .subagent-title {
          font-size: 14px;
          font-weight: 500;
          color: var(--color-text-primary, #1f2329);
          margin: 0;
        }
        .subagent-body {
          flex: 1;
          min-height: 0;
          overflow: hidden;
          display: flex;
          flex-direction: column;
        }
        .subagent-status {
          display: flex;
          align-items: center;
          justify-content: center;
          flex: 1;
          color: var(--color-text-tertiary, #8f959e);
          font-size: 13px;
        }
        /* 子Agent工作流中父Agent指令消息框：横跨整个页面 */
        .subagent-body .wf-user-msg-wrapper {
          width: 100%;
          max-width: 100%;
        }
        .subagent-body .wf-user-node .wf-node-card {
          width: 100%;
          max-width: 100%;
        }
        .subagent-body .wf-msg-copy-btn {
          align-self: flex-end;
        }
      `}</style>
    </div>
  );
}
