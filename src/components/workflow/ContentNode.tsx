import type { WorkflowNode, ContentNodeData } from "../../types";
import { MarkdownPreview } from "../preview/MarkdownPreview";

interface ContentNodeProps {
  node: WorkflowNode<"content">;
}

export function ContentNode({ node }: ContentNodeProps) {
  const data = node.data as ContentNodeData;
  // 判断是否处于流式输出状态
  const isStreaming = data.isStreaming || node.status === "running";

  return (
    <div className="wf-node animate-node-in">
      <div className="wf-content-dot" />
      <div className="wf-content-text-wrapper">
        <MarkdownPreview
          content={data.content}
          className={`wf-content-markdown${isStreaming ? " streaming" : ""}`}
        />
      </div>
      <style>{`
        .wf-content-dot {
          width: 6px;
          height: 6px;
          border-radius: 50%;
          background: var(--color-text-quaternary);
          flex-shrink: 0;
          margin-top: 7px;
        }
        .wf-content-text-wrapper {
          min-width: 0;
          flex: 1;
        }
        .wf-content-markdown {
          color: var(--color-text-primary);
          word-break: break-word;
          line-height: 1.6;
        }
        .wf-content-markdown p:last-child {
          margin-bottom: 0;
        }
        .wf-content-markdown h1:first-child,
        .wf-content-markdown h2:first-child,
        .wf-content-markdown h3:first-child {
          margin-top: 0;
        }
        .wf-content-markdown.streaming > :last-child::after {
          content: "";
          display: inline-block;
          width: 2px;
          height: 16px;
          background: var(--color-accent);
          margin-left: 2px;
          vertical-align: middle;
          animation: wf-cursor-blink 1s step-end infinite;
        }
        @keyframes wf-cursor-blink {
          0%, 100% { opacity: 1; }
          50% { opacity: 0; }
        }
      `}</style>
    </div>
  );
}
