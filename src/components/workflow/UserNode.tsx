import type { WorkflowNode, UserNodeData } from "../../types";
import { Icon } from "../common/Icon";
import { formatSize } from "../../utils/format";

interface UserNodeProps {
  node: WorkflowNode<"user">;
}

export function UserNode({ node }: UserNodeProps) {
  const data = node.data as UserNodeData;
  const hasAttachments = data.attachments && data.attachments.length > 0;

  return (
    <div className="wf-node animate-node-in">
      <div className="wf-node-card">
        <div className="wf-node-body">
          <div className="wf-user-text">{data.content}</div>
          {hasAttachments && (
            <div className="wf-user-attachments">
              {data.attachments.map((att) => (
                <span key={att.id} className="wf-attachment-tag" title={att.name}>
                  <Icon name={att.mimeType.startsWith("image/") ? "image" : "file"} size={10} />
                  <span className="wf-attachment-name">{att.name}</span>
                  <span className="wf-attachment-size">{formatSize(att.size)}</span>
                </span>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
