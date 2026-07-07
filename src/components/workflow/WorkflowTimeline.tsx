import { useEffect, useRef, useCallback, useState } from "react";
import { useTranslation } from 'react-i18next';
import { useWorkflowStore } from "../../stores/useWorkflowStore";
import { Icon } from "../common/Icon";
import { WorkflowNodeRenderer } from "./WorkflowNode";

interface WorkflowTimelineProps {
  /** 错误节点重试回调 */
  onRetryError?: () => void;
  /** 是否显示打字机效果 */
  typewriterVisible?: boolean;
}

function TypewriterText({ text }: { text: string }) {
  const [displayedText, setDisplayedText] = useState("");

  useEffect(() => {
    setDisplayedText("");
    let index = 0;
    const interval = setInterval(() => {
      if (index < text.length) {
        setDisplayedText(text.slice(0, index + 1));
        index++;
      } else {
        clearInterval(interval);
      }
    }, 15);
    return () => clearInterval(interval);
  }, [text]);

  return <span>{displayedText}</span>;
}

/**
 * 工作流时间线组件（原生滚动版）
 * 使用 CSS content-visibility: auto 优化长列表渲染性能，
 * 避免虚拟滚动带来的 DOM 不完整和滚动卡顿问题。
 * 支持流式输出即时滚动和新增节点平滑滚动。
 */
export function WorkflowTimeline({ onRetryError, typewriterVisible = false }: WorkflowTimelineProps) {
  const { t } = useTranslation();
  const { nodes } = useWorkflowStore();
  const scrollRef = useRef<HTMLDivElement>(null);
  // 追踪是否应自动滚动（用户未手动上滚时自动跟随）
  const autoScrollRef = useRef(true);
  // 用于取消前一次 requestAnimationFrame，避免同一帧内重复滚动
  const rafIdRef = useRef<number>(0);
  // 标记当前滚动是否由程序触发，避免 onScroll 误判为用户手动上滚
  const isProgrammaticScrollRef = useRef(false);
  // 追踪上一次节点数量，用于判断是会话切换还是增量更新
  const prevNodesLengthRef = useRef(nodes.length);

  // 计算流式内容变化标识：当流式节点的文本内容增长时，此值变化
  const streamingContentKey = nodes.reduce((acc, node) => {
    if (node.type === "content") {
      const d = node.data as { content: string; isStreaming?: boolean };
      if (node.status === "running" || d.isStreaming) return acc + d.content.length;
    }
    if (node.type === "thinking") {
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

  // 自动滚动到底部：根据场景选择合适的滚动行为
  useEffect(() => {
    const prevLength = prevNodesLengthRef.current;
    prevNodesLengthRef.current = nodes.length;

    if (nodes.length > 0) {
      cancelAnimationFrame(rafIdRef.current);
      rafIdRef.current = requestAnimationFrame(() => {
        if (!scrollRef.current) return;

        isProgrammaticScrollRef.current = true;
        const el = scrollRef.current;

        // 判断是否为会话切换：节点数减少或一次性增加超过1个
        // 节点数减少永远是会话切换（正常流程不会删除节点）
        // 节点数跳跃增加也是会话切换（正常发消息一次只加1个节点）
        const isSessionSwitch = nodes.length < prevLength || Math.abs(nodes.length - prevLength) > 1;
        // 用户发消息：新增了恰好一个用户节点，即使之前手动上滚也应跟随
        const isNewUserMessage = !isSessionSwitch && nodes.length > prevLength && nodes[nodes.length - 1]?.type === "user";
        const isStreaming = streamingContentKey > 0;

        if (isSessionSwitch) {
          // 会话切换：强制跳转到底部，忽略用户之前的上滚状态
          autoScrollRef.current = true;
          el.scrollTop = el.scrollHeight;

          // content-visibility: auto 导致懒加载，scrollHeight 初始不准，
          // 持续重试直到底部稳定
          let retries = 0;
          const MAX_RETRIES = 30;
          const retryScroll = () => {
            if (retries >= MAX_RETRIES) {
              isProgrammaticScrollRef.current = false;
              return;
            }
            retries++;
            const prevHeight = el.scrollHeight;
            el.scrollTop = el.scrollHeight;
            requestAnimationFrame(() => {
              if (el.scrollHeight !== prevHeight || el.scrollHeight - el.scrollTop - el.clientHeight >= 2) {
                retryScroll();
              } else {
                isProgrammaticScrollRef.current = false;
              }
            });
          };
          requestAnimationFrame(retryScroll);
        } else if (autoScrollRef.current || isNewUserMessage) {
          // 用户在底部时自动跟随，或用户发新消息时强制跟随
          if (isNewUserMessage) {
            autoScrollRef.current = true;
          }
          if (isStreaming) {
            el.scrollTo({ top: el.scrollHeight, behavior: "auto" });
          } else {
            el.scrollTo({ top: el.scrollHeight, behavior: "smooth" });
          }
          requestAnimationFrame(() => {
            isProgrammaticScrollRef.current = false;
          });
        } else {
          isProgrammaticScrollRef.current = false;
        }
      });
    }
    return () => cancelAnimationFrame(rafIdRef.current);
  }, [nodes.length, streamingContentKey]);

  if (nodes.length === 0) {
    return (
      <div className="wf-empty" role="status" aria-label={t('workflow.emptySession')}>
        <h3 className="wf-empty-title wf-empty-main-title wf-empty-main-title-with-icon">
          <Icon name="book" size={42} className="wf-empty-book-icon" />
          {typewriterVisible ? (
            <TypewriterText text={t('workflow.startNewSession')} />
          ) : (
            t('workflow.startNewSession')
          )}
        </h3>
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      className="workflow-scroll-container"
      onScroll={handleScroll}
      role="log"
      aria-label={t('workflow.timeline')}
      aria-live="polite"
    >
      {nodes.map((node) => (
        <WorkflowNodeRenderer key={node.id} node={node} onRetry={onRetryError} />
      ))}
    </div>
  );
}
