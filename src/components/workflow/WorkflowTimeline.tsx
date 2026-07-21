import { useEffect, useRef, useCallback, useMemo, useState } from "react";
import { useTranslation } from 'react-i18next';
import { useWorkflowStore, nodeRefsMap } from "../../stores/useWorkflowStore";
import { useAgentModeStore } from "../../stores/useAgentModeStore";
import type { AgentMode } from "../../stores/useAgentModeStore";
import { Icon, type IconName } from "../common/Icon";
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

  return (
    <span className="typewriter-text">
      <span aria-hidden="true" className="typewriter-text-placeholder">{text}</span>
      <span className="typewriter-text-visible">{displayedText}</span>
    </span>
  );
}

/**
 * 简单的 throttle 实现
 * 在 delay 时间内只执行一次，最后一次调用会被延迟执行以确保尾部触发
 */
function throttle<T extends (...args: any[]) => void>(fn: T, delay: number): T {
  let lastCall = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;
  return ((...args: any[]) => {
    const now = Date.now();
    const remaining = delay - (now - lastCall);
    if (remaining <= 0) {
      if (timer) { clearTimeout(timer); timer = null; }
      lastCall = now;
      fn(...args);
    } else if (!timer) {
      timer = setTimeout(() => {
        lastCall = Date.now();
        timer = null;
        fn(...args);
      }, remaining);
    }
  }) as T;
}

/**
 * 工作流时间线组件（原生滚动版）
 * 使用 CSS content-visibility: auto 优化长列表渲染性能，
 * 避免虚拟滚动带来的 DOM 不完整和滚动卡顿问题。
 * 支持流式输出即时滚动和新增节点平滑滚动。
 */
export function WorkflowTimeline({ onRetryError, typewriterVisible = false }: WorkflowTimelineProps) {
  const { t } = useTranslation();
  const { nodes, registerNodeRef, unregisterNodeRef } = useWorkflowStore();
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

  // 计算当前可见节点：找到最接近容器视口中心的节点并更新 store
  const computeCurrentVisibleNode = useCallback(() => {
    // 直接读取模块级 nodeRefsMap（避免订阅 store state）
    const refs = nodeRefsMap;
    if (refs.size === 0) return;

    const container = scrollRef.current;
    if (!container) return;

    const containerRect = container.getBoundingClientRect();
    const centerY = containerRect.top + containerRect.height / 2;

    let closestNodeId: string | null = null;
    let closestDistance = Infinity;

    refs.forEach((el, nodeId) => {
      const rect = el.getBoundingClientRect();
      const nodeCenterY = rect.top + rect.height / 2;
      const distance = Math.abs(nodeCenterY - centerY);
      if (distance < closestDistance) {
        closestDistance = distance;
        closestNodeId = nodeId;
      }
    });

    if (closestNodeId) {
      const current = useWorkflowStore.getState().currentVisibleNodeId;
      if (current !== closestNodeId) {
        useWorkflowStore.getState().setCurrentVisibleNodeId(closestNodeId);
      }
    }
  }, []);

  // throttle 包裹，避免滚动时频繁计算
  const throttledCompute = useMemo(
    () => throttle(computeCurrentVisibleNode, 100),
    [computeCurrentVisibleNode]
  );

  // 检测用户是否手动上滚，决定是否自动跟随
  const handleScroll = useCallback(() => {
    // 更新当前可见节点（throttle 内部控制频率）
    throttledCompute();
    if (isProgrammaticScrollRef.current) return;
    const el = scrollRef.current;
    if (!el) return;
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    autoScrollRef.current = distanceFromBottom < 50;
  }, [throttledCompute]);

  // 自动滚动到底部：根据场景选择合适的滚动行为
  useEffect(() => {
    const prevLength = prevNodesLengthRef.current;
    prevNodesLengthRef.current = nodes.length;

    if (nodes.length > 0) {
      cancelAnimationFrame(rafIdRef.current);
      rafIdRef.current = requestAnimationFrame(() => {
        if (!scrollRef.current) return;
        const el = scrollRef.current;
        // 元素隐藏时（display: none）跳过滚动，避免 scrollHeight=0 导致 scrollTop 被重置
        if (el.offsetParent === null && el.getClientRects().length === 0) return;

        isProgrammaticScrollRef.current = true;

        const isSessionSwitch = nodes.length < prevLength || Math.abs(nodes.length - prevLength) > 1;
        const isNewNode = !isSessionSwitch && nodes.length > prevLength;
        const isNewAgentNode = isNewNode && nodes[nodes.length - 1]?.type !== "user";
        const isStreaming = streamingContentKey > 0;

        if (isSessionSwitch) {
          autoScrollRef.current = true;
          el.scrollTop = el.scrollHeight;
          // content-visibility: auto 导致 scrollHeight 初始不准，持续重试直到底部稳定
          let retries = 0;
          const retryScroll = () => {
            if (retries >= 30) { isProgrammaticScrollRef.current = false; return; }
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
        } else if (autoScrollRef.current || isNewNode) {
          if (isNewNode) autoScrollRef.current = true;
          if (isStreaming || isNewAgentNode) {
            el.scrollTo({ top: el.scrollHeight, behavior: "auto" });
          } else {
            el.scrollTo({ top: el.scrollHeight, behavior: "smooth" });
          }
          // content-visibility: auto 对新节点高度估算不准，重试确保到底
          if (isNewAgentNode) {
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
  }, [nodes.length, streamingContentKey]);

  // 节点数量变化时延迟计算一次当前可见节点，确保 DOM 已更新
  useEffect(() => {
    if (nodes.length === 0) return;
    const timer = setTimeout(() => {
      computeCurrentVisibleNode();
    }, 100);
    return () => clearTimeout(timer);
  }, [nodes.length, computeCurrentVisibleNode]);

  if (nodes.length === 0) {
    return (
      <EmptySessionTitle typewriterVisible={typewriterVisible} />
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
        <WorkflowNodeRenderer
          key={node.id}
          node={node}
          onRetry={onRetryError}
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
  );
}

/**
 * 空会话标题配置：根据当前 Agent 模式选择对应的图标和文案
 * - plan: 铅笔图标（plan-mode） + "Plan with Samoyed Work"
 * - build: 代码括号图标 </>（code-brackets） + "Code with Samoyed Work"
 * - document: 书本图标（book） + "Work with Samoyed Work"（原始样式）
 */
const EMPTY_MODE_CONFIG: Record<AgentMode, { icon: IconName; textKey: string }> = {
  plan: { icon: 'plan-mode', textKey: 'workflow.startNewSessionPlan' },
  build: { icon: 'code-brackets', textKey: 'workflow.startNewSessionCode' },
  document: { icon: 'book', textKey: 'workflow.startNewSession' },
};

/**
 * 空会话标题组件
 * 根据当前 Agent 模式显示对应图标和文字。
 * 图标直接显示（无动画），文字保持打字机效果（TypewriterText）。
 * 模式切换时 TypewriterText 通过 text prop 变化自动重启打字动画。
 */
function EmptySessionTitle({ typewriterVisible }: { typewriterVisible: boolean }) {
  const { t } = useTranslation();
  const mode = useAgentModeStore((s) => s.mode);
  const { icon, textKey } = EMPTY_MODE_CONFIG[mode];
  const titleText = t(textKey);

  return (
    <div className="wf-empty" role="status" aria-label={t('workflow.emptySession')}>
      <h3
        className={`wf-empty-title wf-empty-main-title wf-empty-main-title-with-icon wf-empty-mode-title wf-empty-mode-${mode}`}
      >
        <Icon name={icon} size={42} className="wf-empty-book-icon wf-empty-mode-icon" />
        {typewriterVisible ? (
          <TypewriterText text={titleText} />
        ) : (
          titleText
        )}
      </h3>
    </div>
  );
}
