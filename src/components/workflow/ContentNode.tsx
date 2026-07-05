import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { WorkflowNode, ContentNodeData } from "../../types";
import { MarkdownPreview } from "../preview/MarkdownPreview";
import { Icon } from "../common/Icon";
import { useWorkflowStore } from "../../stores/useWorkflowStore";

interface ContentNodeProps {
  node: WorkflowNode<"content">;
}

export function ContentNode({ node }: ContentNodeProps) {
  const { t } = useTranslation();
  const data = node.data as ContentNodeData;
  const isCompleted = node.status === "completed" && !data.isStreaming;
  const [copied, setCopied] = useState(false);

  // 判断当前 content 节点是否为其所在助手回复片段的最后一个节点（不仅是 content 类型）
  // 仅在每轮智能体回答的最末端显示复制按钮，避免在工具调用、思考、确认等中间节点前错误出现按钮
  // 确保工作流进行中、暂停、切换会话、重新进入会话等场景下，中间 content 节点不显示复制按钮
  const nodes = useWorkflowStore((state) => state.nodes);
  const isLastContentInTurn = (() => {
    const idx = nodes.findIndex((n) => n.id === node.id);
    if (idx === -1) return false;
    // 检查紧邻的下一个节点：若不存在或为 user 节点（下一轮开始），则当前 content 是该轮次末尾
    // 任何其他类型的节点（content/tool/thinking/confirm/error）都意味着当前 content 非末尾
    const nextNode = nodes[idx + 1];
    return !nextNode || nextNode.type === "user";
  })();
  // 检查当前节点所在轮次是否已全部完成（从当前节点到下一个 user 节点之间的所有节点均非 running 状态）
  // 替代旧的全局 executionStatus 判断，避免新对话执行时隐藏上一轮对话的复制按钮
  const isTurnCompleted = (() => {
    const idx = nodes.findIndex((n) => n.id === node.id);
    if (idx === -1) return false;
    for (let i = idx; i < nodes.length; i++) {
      if (nodes[i].type === "user") break;
      if (nodes[i].status === "running") return false;
    }
    return true;
  })();

  // 复制内容到剪贴板：优先使用现代 Clipboard API，失败时降级为 execCommand
  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(data.content);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = data.content;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="wf-node">
      <div className="wf-content-text-wrapper">
        <MarkdownPreview
          content={data.content}
          className="wf-content-markdown"
        />
        {isCompleted && isLastContentInTurn && isTurnCompleted && (
          <div className="wf-content-copy-btn">
            <button
              className="wf-copy-button"
              onClick={handleCopy}
              title={copied ? t('common.copied') : t('common.copy')}
            >
              {copied ? (
                <Icon name="check" size={12} />
              ) : (
                <Icon name="copy" size={12} />
              )}
            </button>
          </div>
        )}
      </div>
      <style>{`
        .wf-content-text-wrapper {
          min-width: 0;
          flex: 1;
          flex-direction: column;
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

        /* 工作流区域表格：小圆角容器，表头深色背景，body 行无背景 */
        .wf-content-markdown .md-table-wrap {
          border-radius: var(--radius-sm);
          overflow: hidden;
          border: 1px solid var(--color-border);
        }
        .wf-content-markdown .md-table {
          margin: 0;
        }
        /* 单元格只保留右、下边框作为内部分隔线，外边框由容器提供 */
        .wf-content-markdown .md-table th,
        .wf-content-markdown .md-table td {
          border-top: none;
          border-left: none;
        }
        .wf-content-markdown .md-table th:last-child,
        .wf-content-markdown .md-table td:last-child {
          border-right: none;
        }
        .wf-content-markdown .md-table tbody tr:last-child td {
          border-bottom: none;
        }
        /* 表头深色背景，body 行背景透明（覆盖 tr 和 td 两处斑马纹来源） */
        .wf-content-markdown .md-table thead th {
          background: var(--color-bg-hover) !important;
        }
        .wf-content-markdown .md-table tbody tr,
        .wf-content-markdown .md-table tbody tr td {
          background: transparent !important;
        }

      `}</style>
    </div>
  );
}
