import type { ReactNode } from "react";
import type { WorkflowNode, WorkflowNodeType } from "../../types";
import { useWorkflowStore } from "../../stores/useWorkflowStore";
import { UserNode } from "./UserNode";
import { ThinkingNode } from "./ThinkingNode";
import { ContentNode } from "./ContentNode";
import { ToolNode } from "./ToolNode";
import { ConfirmNode } from "./ConfirmNode";
import { ErrorNode } from "./ErrorNode";
import { CompactionNode } from "./CompactionNode";
import { SubAgentNode } from "./SubAgentNode";
import { QuestionNode } from "./QuestionNode";

interface WorkflowNodeRendererProps {
  node: WorkflowNode;
  onRetry?: () => void;
  hideCopy?: boolean;
  /** 节点根元素 ref 回调，用于注册到外部 ref 注册表（如 useWorkflowStore.nodeRefs） */
  nodeRef?: (el: HTMLElement | null) => void;
}

export function WorkflowNodeRenderer({ node, onRetry, hideCopy, nodeRef }: WorkflowNodeRendererProps) {
  const { toggleNode } = useWorkflowStore();
  // 跳转后高亮节点 ID（jumpToNode 设置，1.5 秒后清空）
  const highlightNodeId = useWorkflowStore((s) => s.highlightNodeId);
  const nt = node.type as WorkflowNodeType;

  let content: ReactNode;
  switch (nt) {
    case "user":
      content = <UserNode node={node as WorkflowNode<"user">} hideCopy={hideCopy} />;
      break;
    case "thinking":
      content = <ThinkingNode node={node as WorkflowNode<"thinking">} />;
      break;
    case "content":
      content = <ContentNode node={node as WorkflowNode<"content">} hideCopy={hideCopy} />;
      break;
    case "tool":
      content = <ToolNode node={node as WorkflowNode<"tool">} />;
      break;
    case "confirm":
      content = <ConfirmNode node={node as WorkflowNode<"confirm">} />;
      break;
    case "error":
      content = <ErrorNode node={node as WorkflowNode<"error">} onToggle={() => toggleNode(node.id)} onRetry={onRetry} />;
      break;
    case "compaction":
      content = <CompactionNode node={node as WorkflowNode<"compaction">} />;
      break;
    case "sub_agent":
      content = <SubAgentNode node={node as WorkflowNode<"sub_agent">} />;
      break;
    case "question":
      content = <QuestionNode node={node as WorkflowNode<"question">} />;
      break;
    default:
      content = null;
  }

  // 传入 nodeRef 时包裹一个 div 作为根元素应用 ref；不传时保持原有渲染结果
  // 高亮 class 加在该外层 div 上（jumpToNode 仅对已注册 ref 的节点生效，故无 nodeRef 分支无需处理）
  const highlightClass = highlightNodeId === node.id ? "wf-node-highlight" : "";
  if (nodeRef) {
    return <div ref={nodeRef} className={highlightClass}>{content}</div>;
  }
  return content;
}
