import { useEffect, useRef, useCallback } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useWorkflowStore } from "../../stores/useWorkflowStore";
import { WorkflowNodeRenderer } from "./WorkflowNode";
import { Icon, type IconName } from "../common/Icon";

interface WorkflowTimelineProps {
  /** 错误节点重试回调 */
  onRetryError?: () => void;
}

/**
 * 工作流时间线组件（虚拟滚动版）
 * 使用 @tanstack/react-virtual 实现虚拟滚动，仅渲染可视区域内的节点
 * 支持动态高度测量和自动滚动到底部
 */
export function WorkflowTimeline({ onRetryError }: WorkflowTimelineProps) {
  const { nodes } = useWorkflowStore();
  const scrollRef = useRef<HTMLDivElement>(null);
  // 追踪是否应自动滚动（用户未手动上滚时自动跟随）
  const autoScrollRef = useRef(true);

  // 创建虚拟化器，使用动态高度测量
  const virtualizer = useVirtualizer({
    count: nodes.length,
    getScrollElement: () => scrollRef.current,
    // 预估节点高度，用于首次渲染前的布局计算
    estimateSize: (index) => {
      const node = nodes[index];
      if (!node) return 80;
      switch (node.type) {
        case "user": return 60;
        case "thinking": return 120;
        case "tool": return 150;
        case "result": return 100;
        case "reply": return 200;
        case "confirm": return 120;
        default: return 80;
      }
    },
    // 启用动态测量，当节点内容变化时自动重新计算高度
    measureElement: (el) => el?.getBoundingClientRect().height ?? 0,
    // 过扫描量：在可视区域外额外渲染的节点数，减少快速滚动时的空白
    overscan: 5,
  });

  // 检测用户是否手动上滚，决定是否自动跟随
  const handleScroll = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    // 距离底部 50px 以内视为"在底部"，保持自动滚动
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    autoScrollRef.current = distanceFromBottom < 50;
  }, []);

  // 新增节点时自动滚动到底部
  useEffect(() => {
    if (autoScrollRef.current && nodes.length > 0) {
      // 使用 requestAnimationFrame 确保 DOM 更新后再滚动
      requestAnimationFrame(() => {
        virtualizer.scrollToIndex(nodes.length - 1, {
          align: "end",
          behavior: "smooth",
        });
      });
    }
  }, [nodes.length, virtualizer]);

  // 空状态：展示引导性的快速开始提示
  if (nodes.length === 0) {
    const quickStarts: { icon: IconName; text: string }[] = [
      { icon: "doc", text: "生成一份Word文档" },
      { icon: "xlsx", text: "创建Excel表格" },
      { icon: "ppt", text: "制作PPT演示" },
      { icon: "pdf", text: "转换文档格式" },
    ];

    return (
      <div className="wf-empty" role="status" aria-label="空会话">
        <h3 className="wf-empty-title">开始新会话</h3>
        <p className="wf-empty-desc">
          在下方输入指令，Agent 将协助你处理文档
        </p>
        <div className="wf-empty-quick">
          {quickStarts.map((item) => (
            <div key={item.text} className="wf-empty-quick-item">
              <Icon name={item.icon} size={14} />
              <span>{item.text}</span>
            </div>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      className="workflow-scroll-container"
      onScroll={handleScroll}
      role="log"
      aria-label="工作流时间线"
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
                // 绝对定位元素不尊重父元素的 padding，需要手动补偿
                // 使 .wf-node 从 timeline 的内容区域开始，图标和竖线才能正确对齐
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
