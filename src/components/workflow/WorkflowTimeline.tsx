import { useEffect, useRef, useCallback, useState } from "react";
import { useTranslation } from 'react-i18next';
import { useVirtualizer } from "@tanstack/react-virtual";
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
  const [isComplete, setIsComplete] = useState(false);

  useEffect(() => {
    setDisplayedText("");
    setIsComplete(false);
    let index = 0;
    const interval = setInterval(() => {
      if (index < text.length) {
        setDisplayedText(text.slice(0, index + 1));
        index++;
      } else {
        clearInterval(interval);
        setIsComplete(true);
      }
    }, 15);
    return () => clearInterval(interval);
  }, [text]);

  return (
    <span>
      <span>{displayedText}</span>
      <span className={`typewriter-cursor ${isComplete ? "hidden" : ""}`}>|</span>
    </span>
  );
}

/**
 * 工作流时间线组件（虚拟滚动版）
 * 使用 @tanstack/react-virtual 实现虚拟滚动，仅渲染可视区域内的节点
 * 支持动态高度测量和自动滚动
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

  // 计算流式内容变化标识：当流式节点的文本/代码内容增长时，此值变化
  // 用于在流式输出期间（nodes.length 不变但内容增长时）也触发自动滚动
  const streamingContentKey = nodes.reduce((acc, node) => {
    if (node.type === "content") {
      const d = node.data as { content: string; isStreaming?: boolean };
      if (node.status === "running" || d.isStreaming) return acc + d.content.length;
    }
    if (node.type === "thinking") {
      const d = node.data as { content: string; isStreaming?: boolean };
      if (node.status === "running" || d.isStreaming) return acc + d.content.length;
    }
    if (node.type === "tool") {
      const d = node.data as { streamingCode?: string; isCodeStreaming?: boolean };
      if (d.isCodeStreaming && d.streamingCode) return acc + d.streamingCode.length;
    }
    return acc;
  }, 0);

  // 创建虚拟化器，使用动态高度测量
  const virtualizer = useVirtualizer({
    count: nodes.length,
    getScrollElement: () => scrollRef.current,
    // 预估节点高度，用于首次渲染前的布局计算
    estimateSize: (index) => {
      const node = nodes[index];
      if (!node) return 60;
      switch (node.type) {
        case "user": return 60;
        case "thinking": return 80;
        case "content": return 120;
        case "tool": return 40;
        case "confirm": return 100;
        case "error": return 80;
        default: return 60;
      }
    },
    // 启用动态测量，当节点内容变化时自动重新计算高度
    measureElement: (el) => el?.getBoundingClientRect().height ?? 0,
    // 过扫描量：在可视区域外额外渲染的节点数，减少快速滚动时的空白
    overscan: 5,
  });

  // 检测用户是否手动上滚，决定是否自动跟随
  // 程序触发的滚动（isProgrammaticScrollRef）不纳入判断
  const handleScroll = useCallback(() => {
    if (isProgrammaticScrollRef.current) return;
    const el = scrollRef.current;
    if (!el) return;
    // 距离底部 50px 以内视为"在底部"，保持自动滚动
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    autoScrollRef.current = distanceFromBottom < 50;
  }, []);

  // 自动滚动到底部：新增节点或流式内容更新时触发
  useEffect(() => {
    if (nodes.length > 0) {
      // 取消前一次未执行的滚动请求，确保每帧最多滚动一次
      cancelAnimationFrame(rafIdRef.current);
      rafIdRef.current = requestAnimationFrame(() => {
        // 在回调内再次检查，防止用户在 rAF 等待期间手动上滚
        if (autoScrollRef.current) {
          // 标记为程序触发的滚动，防止 onScroll 回调误判
          isProgrammaticScrollRef.current = true;
          virtualizer.scrollToIndex(nodes.length - 1, {
            align: "end",
            // 流式输出时使用即时滚动以紧跟内容，新增节点时使用平滑滚动
            behavior: streamingContentKey > 0 ? "auto" : "smooth",
          });
          // 程序滚动后延迟重置标志，确保 onScroll 事件已处理完毕
          requestAnimationFrame(() => {
            isProgrammaticScrollRef.current = false;
          });
        }
      });
    }
    return () => cancelAnimationFrame(rafIdRef.current);
  }, [nodes.length, streamingContentKey, virtualizer]);

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
      <div
        className="workflow-timeline"
        style={{
          height: virtualizer.getTotalSize(),
          width: "100%",
          position: "relative",
        }}
      >
        {virtualizer.getVirtualItems().map((virtualItem) => {
          const node = nodes[virtualItem.index];
          if (!node) return null;

          return (
            <div
              key={node.id}
              data-index={virtualItem.index}
              ref={virtualizer.measureElement}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                zIndex: 3,
                // 绝对定位元素不尊重父元素的 padding，需要手动补偿
                // 使 .wf-node 从 timeline 的内容区域开始
                paddingLeft: "28px",
                // 使用 transform 定位，比 top 性能更好（避免 reflow）
                transform: `translateY(${virtualItem.start}px)`,
              }}
            >
              <WorkflowNodeRenderer node={node} onRetry={onRetryError} />
            </div>
          );
        })}
      </div>
    </div>
  );
}
