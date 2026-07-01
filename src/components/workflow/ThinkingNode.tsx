import { useState, useEffect } from "react";
import type { WorkflowNode, ThinkingNodeData } from "../../types";
import { Icon } from "../common/Icon";

interface ThinkingNodeProps {
  node: WorkflowNode<"thinking">;
}

export function ThinkingNode({ node }: ThinkingNodeProps) {
  const data = node.data as ThinkingNodeData;
  const isStreaming = data.isStreaming || node.status === "running";
  const [expanded, setExpanded] = useState(isStreaming);

  useEffect(() => {
    if (isStreaming) {
      setExpanded(true);
    } else if (node.status === "completed") {
      setExpanded(false);
    }
  }, [isStreaming, node.status]);

  return (
    <div className="wf-node animate-node-in">
      <div className="wf-thinking-block">
        <div
          className="wf-thinking-toggle"
          onClick={() => setExpanded((prev) => !prev)}
        >
          <Icon
            name={expanded ? "chevron-down" : "chevron-right"}
            size={12}
          />
          {/* 思考链标签始终使用Thinking文字表示，不跟随i18n切换 */}
          <span>Thinking...</span>
        </div>

        {expanded && (
          <div className="wf-thinking-content">
            {data.content.split("\n\n").filter((p) => p.trim()).map((paragraph, index) => (
              <p key={index} className="wf-thinking-paragraph">
                {paragraph}
              </p>
            ))}
            {isStreaming && <span className="cursor-blink" />}
          </div>
        )}
      </div>
    </div>
  );
}
