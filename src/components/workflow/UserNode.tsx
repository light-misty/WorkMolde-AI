import { useState, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { WorkflowNode, UserNodeData } from "../../types";
import { Icon } from "../common/Icon";
import { formatSize } from "../../utils/format";
import { useWorkflowStore } from "../../stores/useWorkflowStore";
import { useSessionStore } from "../../stores/useSessionStore";
import * as tauriCmd from "../../services/tauri";

interface UserNodeProps {
  node: WorkflowNode<"user">;
  hideCopy?: boolean;
}

export function UserNode({ node, hideCopy }: UserNodeProps) {
  const { t } = useTranslation();
  const data = node.data as UserNodeData;
  const hasAttachments = data.attachments && data.attachments.length > 0;
  const [copied, setCopied] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  // 创建分支原位编辑模式状态
  const [isEditing, setIsEditing] = useState(false);
  const [editContent, setEditContent] = useState("");

  const executionStatus = useWorkflowStore((s) => s.executionStatus);
  const isAgentRunning = executionStatus === "running";

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

  const handleDelete = async () => {
    const sessionId = useSessionStore.getState().currentSessionId;
    if (!sessionId) return;

    const { nodes, clearSessionCache, loadContextUsage } = useWorkflowStore.getState();

    const currentIdx = nodes.findIndex((n) => n.id === node.id);
    if (currentIdx === -1) return;

    let endIdx = nodes.length;
    for (let i = currentIdx + 1; i < nodes.length; i++) {
      if (nodes[i].type === "user") {
        endIdx = i;
        break;
      }
    }

    // 方案一：从节点数据中收集 messageId（会话从后端加载时已填充）
    const messageIdSet = new Set<string>();
    for (const n of nodes.slice(currentIdx, endIdx)) {
      const mid = (n.data as unknown as Record<string, unknown>).messageId as string | undefined;
      if (mid) messageIdSet.add(mid);
    }

    // 方案二：节点是在实时对话中创建的（无 messageId），从后端拉取消息列表来定位
    if (messageIdSet.size === 0) {
      try {
        const detail = await tauriCmd.getSession(sessionId);
        const msgs = detail.messages;
        // 统计当前用户节点是第几条用户消息
        let userMsgIndex = 0;
        for (let i = 0; i < currentIdx; i++) {
          if (nodes[i].type === "user") userMsgIndex++;
        }
        // 在消息列表中找到对应的用户消息
        let msgStartIdx = -1;
        let userSeen = -1;
        for (let i = 0; i < msgs.length; i++) {
          if (msgs[i].role === "user") {
            userSeen++;
            if (userSeen === userMsgIndex) {
              msgStartIdx = i;
              break;
            }
          }
        }
        if (msgStartIdx === -1) return;
        // 收集从该用户消息到下一用户消息之间的所有消息 ID
        for (let i = msgStartIdx; i < msgs.length; i++) {
          messageIdSet.add(msgs[i].id);
          if (i > msgStartIdx && msgs[i].role === "user") {
            // 不包含下一条用户消息本身
            messageIdSet.delete(msgs[i].id);
            break;
          }
        }
      } catch (err) {
        console.error("[UserNode] 获取会话消息失败:", err);
        return;
      }
    }

    const messageIds = Array.from(messageIdSet);
    if (messageIds.length === 0) return;

    try {
      await tauriCmd.deleteSessionMessages(sessionId, messageIds);

      useWorkflowStore.setState({
        nodes: nodes.filter((_, i) => i < currentIdx || i >= endIdx),
      });

      clearSessionCache(sessionId);
      loadContextUsage(sessionId);
    } catch (err) {
      console.error("[UserNode] 删除消息失败:", err);
    }
  };

  // 进入原位编辑模式：用当前消息内容预填 textarea
  const handleStartEdit = () => {
    setEditContent(data.content);
    setIsEditing(true);
  };

  // 取消编辑：退出编辑模式并清空编辑内容
  const handleCancelEdit = () => {
    setIsEditing(false);
    setEditContent("");
  };

  // 确认创建分支：调用后端 create_branch 复制前缀消息+切换活跃分支，
  // 然后刷新 workflow 节点显示新分支，最后通过 pendingBranchSend 触发 App.tsx 发送新分支消息
  const handleConfirmCreateBranch = async () => {
    const sessionId = useSessionStore.getState().currentSessionId;
    if (!sessionId) return;

    const trimmedContent = editContent.trim();
    if (!trimmedContent) {
      // 空内容不允许创建分支
      return;
    }

    // 获取要分叉的 user 消息 ID
    // 实时对话中通过 addNode 创建的 user 节点没有 messageId，需要从后端按位置匹配查找
    let forkMessageId = data.messageId;
    if (!forkMessageId) {
      try {
        const { nodes } = useWorkflowStore.getState();
        // 统计当前节点之前（含自身）的 user 节点数量，得到 1-based 索引
        const userMsgIndex = nodes.slice(0, nodes.findIndex((n) => n.id === node.id) + 1)
          .filter((n) => n.type === "user").length;
        if (userMsgIndex === 0) return;

        // 从后端获取消息列表，找到对应位置的 user 消息 id
        const detail = await tauriCmd.getSession(sessionId);
        const userMessages = detail.messages.filter((m) => m.role === "user");
        forkMessageId = userMessages[userMsgIndex - 1]?.id;
        if (!forkMessageId) return;
      } catch (err) {
        console.error("[UserNode] 获取会话消息失败:", err);
        return;
      }
    }

    try {
      // 1. 调用后端创建分支（复制前缀消息+设置活跃分支）
      //    新 user 消息由后续 startAgent 创建，并通过 branchGroupId 设置 branch_group_id
      const branchResult = await tauriCmd.createBranch(sessionId, forkMessageId);

      // 2. 退出编辑模式
      setIsEditing(false);
      setEditContent("");

      // 3. 刷新 workflow 节点：从后端重新加载当前活跃分支的消息
      const [branchGroups, detail] = await Promise.all([
        tauriCmd.listBranchGroups(sessionId),
        tauriCmd.getSession(sessionId),
      ]);
      useWorkflowStore.getState().loadFromMessages(
        detail.messages,
        branchGroups,
        detail.activeBranchId,
      );

      // 4. 清空 workflow 缓存（分支已切换，旧缓存失效）
      useWorkflowStore.getState().clearSessionCache(sessionId);

      // 5. 通过 pendingBranchSend 触发 App.tsx 的 handleSend 流程
      //    不能直接调用 tauriCmd.startAgent，否则会绕过 useAgent.sendMessage，
      //    导致 deepThinkingContentRef 等 ref 未重置（思考过程追加问题），
      //    且不会创建 user 节点（用户消息不显示问题）。
      //    App.tsx 监听 pendingBranchSend 后会复用 handleSend 完整流程：
      //    resetRefs + addNode("user") + setExecutionStatus + sendMessage(branchGroupId)
      useWorkflowStore.getState().setPendingBranchSend({
        content: trimmedContent,
        branchGroupId: branchResult.branchGroupId,
      });
    } catch (err) {
      console.error("[UserNode] 创建分支失败:", err);
    }
  };

  // 编辑模式键盘事件：Ctrl/Cmd+Enter 触发创建分支，Esc 取消
  const handleEditKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      void handleConfirmCreateBranch();
    }
    if (e.key === "Escape") {
      e.preventDefault();
      handleCancelEdit();
    }
  };

  // 切换分支：根据方向（-1 上一个 / 1 下一个）在分支组内循环切换
  const handleSwitchBranch = async (direction: 1 | -1) => {
    if (!data.branchGroupId || !data.branchId) return;

    // 从 useWorkflowStore 获取分支组信息
    const branchGroups = useWorkflowStore.getState().branchGroups;
    const group = branchGroups.find((g) => g.branchGroupId === data.branchGroupId);
    if (!group || group.branches.length === 0) return;

    // 找到当前分支在组内的位置
    const currentIdx = group.branches.findIndex((b) => b.branchId === data.branchId);
    if (currentIdx === -1) return;

    // 计算下一个分支（循环切换）
    const total = group.branches.length;
    const nextIdx = (currentIdx + direction + total) % total;
    const nextBranchId = group.branches[nextIdx].branchId;

    // 调用 useSessionStore.switchBranch 切换分支
    try {
      await useSessionStore.getState().switchBranch(nextBranchId);
    } catch (err) {
      console.error("[UserNode] 切换分支失败:", err);
    }
  };

  const showActions = !hideCopy;

  return (
    <div className="wf-node wf-user-node">
      <div className="wf-user-msg-wrapper">
        <div className="wf-node-card">
          <div className="wf-node-body">
            {isEditing ? (
              <textarea
                className="wf-user-edit-textarea"
                value={editContent}
                onChange={(e) => setEditContent(e.target.value)}
                onKeyDown={handleEditKeyDown}
                autoFocus
                rows={3}
              />
            ) : (
              <div className="wf-user-text">{data.content}</div>
            )}
            {hasAttachments && !isEditing && (
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
        {showActions && (
          isEditing ? (
            <div className="wf-edit-actions-row">
              <button
                className="wf-edit-confirm-button"
                onClick={() => void handleConfirmCreateBranch()}
                title={t('workflow.confirmCreateBranch')}
              >
                <Icon name="check" size={12} />
              </button>
              <button
                className="wf-edit-cancel-button"
                onClick={handleCancelEdit}
                title={t('workflow.cancelCreateBranch')}
              >
                <Icon name="close" size={12} />
              </button>
            </div>
          ) : (
            <div className="wf-user-bottom-row">
              {/* 分支切换器：仅当存在分支组且分支数 > 1 时显示，位于创建分支按钮左边 */}
              {data.branchGroupId && data.branchTotal && data.branchTotal > 1 && (
                <div className="wf-branch-switcher">
                  <button
                    className="wf-branch-arrow"
                    onClick={() => handleSwitchBranch(-1)}
                    disabled={isAgentRunning}
                    title={t('workflow.previousBranch')}
                  >
                    <Icon name="chevron-left" size={12} />
                  </button>
                  <span className="wf-branch-counter">
                    {data.branchIndex}/{data.branchTotal}
                  </span>
                  <button
                    className="wf-branch-arrow"
                    onClick={() => handleSwitchBranch(1)}
                    disabled={isAgentRunning}
                    title={t('workflow.nextBranch')}
                  >
                    <Icon name="chevron-right" size={12} />
                  </button>
                </div>
              )}
              <div className={`wf-msg-actions-row${copied ? " wf-copy-visible" : ""}`}>
                {!isAgentRunning && (
                  <button
                    className="wf-branch-button"
                    onClick={handleStartEdit}
                    title={t('workflow.createBranch')}
                  >
                    <Icon name="git-compare" size={12} />
                  </button>
                )}
                {!isAgentRunning && (
                  <button
                    className="wf-delete-button"
                    onClick={() => setShowDeleteConfirm(true)}
                    title={t('workflow.deleteMessage')}
                  >
                    <Icon name="trash" size={12} />
                  </button>
                )}
                <div className="wf-msg-copy-btn">
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
              </div>
            </div>
          )
        )}
      </div>

      {showDeleteConfirm && createPortal(
        <div className="wf-del-overlay" onClick={() => setShowDeleteConfirm(false)}>
          <div className="wf-del-dialog" onClick={(e) => e.stopPropagation()}>
            <div className="wf-del-header">
              <span className="wf-del-icon">
                <Icon name="warning" size={18} />
              </span>
              <span className="wf-del-title">{t('deleteConfirm.title')}</span>
            </div>
            <div className="wf-del-body">
              <p className="wf-del-message">{t('workflow.deleteMessageConfirm')}</p>
            </div>
            <div className="wf-del-footer">
              <button
                className="wf-del-btn wf-del-btn-danger"
                onClick={() => {
                  setShowDeleteConfirm(false);
                  handleDelete();
                }}
              >
                {t('common.delete')}
              </button>
              <button
                className="wf-del-btn wf-del-btn-cancel"
                onClick={() => setShowDeleteConfirm(false)}
              >
                {t('deleteConfirm.cancel')}
              </button>
            </div>
          </div>
        </div>,
        document.body
      )}
    </div>
  );
}
