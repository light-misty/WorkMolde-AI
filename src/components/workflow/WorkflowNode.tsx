import type { WorkflowNode, WorkflowNodeType } from "../../types";
import { useWorkflowStore } from "../../stores/useWorkflowStore";
import { UserNode } from "./UserNode";
import { ThinkingNode } from "./ThinkingNode";
import { ContentNode } from "./ContentNode";
import { ToolNode } from "./ToolNode";
import { ConfirmNode } from "./ConfirmNode";
import { ErrorNode } from "./ErrorNode";

interface WorkflowNodeRendererProps {
  node: WorkflowNode;
  onRetry?: () => void;
}

export function WorkflowNodeRenderer({ node, onRetry }: WorkflowNodeRendererProps) {
  const { toggleNode } = useWorkflowStore();
  const nt = node.type as WorkflowNodeType;

  switch (nt) {
    case "user":
      return <UserNode node={node as WorkflowNode<"user">} />;
    case "thinking":
      return <ThinkingNode node={node as WorkflowNode<"thinking">} />;
    case "content":
      return <ContentNode node={node as WorkflowNode<"content">} />;
    case "tool":
      return <ToolNode node={node as WorkflowNode<"tool">} />;
    case "confirm":
      return <ConfirmNode node={node as WorkflowNode<"confirm">} />;
    case "error":
      return <ErrorNode node={node as WorkflowNode<"error">} onToggle={() => toggleNode(node.id)} onRetry={onRetry} />;
    default:
      return null;
  }
}
