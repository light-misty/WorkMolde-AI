import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useWorkflowStore } from '../../stores/useWorkflowStore';
import { useSessionStore } from '../../stores/useSessionStore';
import { listAllBranchUserMessages } from '../../services/tauri';
import { Icon } from '../common/Icon';
import { CustomScrollArea } from '../common/CustomScrollArea';
import type { UserNodeData } from '../../types/workflow';
import type { BranchUserMessage } from '../../types/session';

interface WorkflowRightSidebarProps {
  /** 是否处于收起状态（由父组件控制，用于触发滑入/滑出动画） */
  collapsed?: boolean;
}

/**
 * 工作流右侧边栏：分支导航
 * - 展示 user 节点列表
 * - 分支组指示按钮：对属于多分支组的 user 节点显示分支数量，点击展开/折叠
 * - 分支切换：展开后显示分支列表，点击切换活跃分支；Agent 运行时禁用
 * - 搜索功能：点击搜索图标显示输入框，按关键词过滤用户消息
 * - 滑入/滑出动画：外层控制 width，内层控制 transform，避免内容被压缩
 */
export function WorkflowRightSidebar({ collapsed = false }: WorkflowRightSidebarProps) {
  const { t } = useTranslation();
  const nodes = useWorkflowStore((s) => s.nodes);
  const currentVisibleNodeId = useWorkflowStore((s) => s.currentVisibleNodeId);
  const setRightSidebarVisible = useWorkflowStore((s) => s.setRightSidebarVisible);
  // 跳转到指定节点（滚动+高亮）
  const jumpToNode = useWorkflowStore((s) => s.jumpToNode);
  // 通过 messageId 跳转（用于跨分支搜索结果）
  const jumpToMessage = useWorkflowStore((s) => s.jumpToMessage);
  // 执行状态：Agent 运行时禁用分支切换
  const executionStatus = useWorkflowStore((s) => s.executionStatus);
  // 分支组信息：用于展开后渲染分支列表
  const branchGroups = useWorkflowStore((s) => s.branchGroups);
  // 当前活跃分支 ID：用于高亮当前分支、判断跨分支跳转
  const activeBranchId = useWorkflowStore((s) => s.activeBranchId);

  // 本地 state 管理各分支组的展开/折叠状态（按 branchGroupId 隔离）
  const [expandedGroupIds, setExpandedGroupIds] = useState<Set<string>>(new Set());
  // 搜索状态：是否显示搜索输入框
  const [isSearching, setIsSearching] = useState(false);
  // 搜索关键词
  const [searchQuery, setSearchQuery] = useState('');
  // 全分支用户消息列表（进入搜索模式时加载）
  const [allBranchMessages, setAllBranchMessages] = useState<BranchUserMessage[]>([]);
  // 全分支消息加载状态
  const [isLoadingMessages, setIsLoadingMessages] = useState(false);

  // 进入搜索模式时加载所有分支的 user 消息
  useEffect(() => {
    if (!isSearching) return;
    const sessionId = useSessionStore.getState().currentSessionId;
    if (!sessionId) return;
    let cancelled = false;
    setIsLoadingMessages(true);
    listAllBranchUserMessages(sessionId)
      .then((messages) => {
        if (!cancelled) {
          setAllBranchMessages(messages);
        }
      })
      .catch((err) => {
        console.error('[WorkflowRightSidebar] 加载全分支用户消息失败:', err);
      })
      .finally(() => {
        if (!cancelled) setIsLoadingMessages(false);
      });
    return () => {
      cancelled = true;
    };
  }, [isSearching]);

  // 切换指定分支组的展开/折叠状态
  const toggleGroup = (groupId: string) => {
    setExpandedGroupIds((prev) => {
      const next = new Set(prev);
      if (next.has(groupId)) {
        next.delete(groupId);
      } else {
        next.add(groupId);
      }
      return next;
    });
  };

  // 切换到指定分支：Agent 运行时禁用
  const handleSwitchToBranch = async (branchId: string) => {
    if (executionStatus === 'running') return;
    try {
      await useSessionStore.getState().switchBranch(branchId);
    } catch (err) {
      console.error('[WorkflowRightSidebar] 切换分支失败:', err);
    }
  };

  // 关闭搜索：清空关键词并隐藏搜索框
  const handleCloseSearch = () => {
    setIsSearching(false);
    setSearchQuery('');
    setAllBranchMessages([]);
  };

  // 点击搜索结果项跳转：当前分支直接跳转，其他分支先切换再跳转
  const handleSearchResultClick = async (message: BranchUserMessage) => {
    if (executionStatus === 'running') return;
    // 当前分支：直接通过 messageId 跳转
    if (message.branchId === activeBranchId) {
      jumpToMessage(message.messageId);
      return;
    }
    // 其他分支：先切换分支，等待工作流重渲染后通过 messageId 跳转
    try {
      await useSessionStore.getState().switchBranch(message.branchId);
      // 切换分支后工作流重渲染需要时间，用 requestAnimationFrame 延迟跳转
      // 需要多次重试以确保节点已注册到 nodeRefsMap
      const tryJump = (attempts: number) => {
        if (jumpToMessage(message.messageId)) return;
        if (attempts > 0) {
          requestAnimationFrame(() => tryJump(attempts - 1));
        }
      };
      // 最多重试 30 帧（约 500ms）
      setTimeout(() => tryJump(30), 100);
    } catch (err) {
      console.error('[WorkflowRightSidebar] 跨分支跳转失败:', err);
    }
  };

  // 当前分支的 user 节点（从工作流节点列表过滤）
  const currentUserNodes = nodes.filter((n) => n.type === 'user');

  // 搜索模式：构建合并显示列表
  // - 当前分支节点：从 nodes 过滤（保留分支组指示等交互）
  // - 其他分支节点：从 allBranchMessages 过滤（仅显示匹配项）
  // - 匹配项用深色背景标记
  const searchResultsFromOtherBranches = searchQuery
    ? allBranchMessages.filter(
        (m) =>
          m.branchId !== activeBranchId &&
          m.content.toLowerCase().includes(searchQuery.toLowerCase()),
      )
    : [];

  // 当前分支是否包含匹配项（仅用于"无搜索结果"提示判断）
  const hasCurrentBranchMatches = searchQuery
    ? currentUserNodes.some((n) => {
        const data = n.data as UserNodeData;
        return (data.content || '').toLowerCase().includes(searchQuery.toLowerCase());
      })
    : false;

  return (
    <div className={`workflow-right-sidebar${collapsed ? ' collapsed' : ''}`}>
      {/* 内层容器：固定宽度，配合外层 width 动画实现滑入/滑出效果 */}
      <div className="workflow-right-sidebar-inner">
        <div className="branch-graph-header">
          {isSearching ? (
            // 搜索模式：显示搜索输入框 + 关闭按钮
            <div className="branch-graph-search-row">
              <Icon name="search" size={12} className="branch-graph-search-icon" />
              <input
                type="text"
                className="branch-graph-search-input"
                placeholder={t('workflow.searchUserMessages')}
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                autoFocus
              />
              <button
                className="branch-graph-close-btn"
                onClick={handleCloseSearch}
                title={t('common.cancel')}
              >
                <Icon name="close" size={14} />
              </button>
            </div>
          ) : (
            // 默认模式：显示标题 + 搜索按钮 + 收起按钮
            <>
              <span className="branch-graph-title">{t('workflow.branchGraph')}</span>
              <div className="branch-graph-header-actions">
                <button
                  className="branch-graph-close-btn"
                  onClick={() => setIsSearching(true)}
                  title={t('workflow.searchUserMessages')}
                >
                  <Icon name="search" size={14} />
                </button>
                <button
                  className="branch-graph-close-btn"
                  onClick={() => setRightSidebarVisible(false)}
                  title={t('workflow.hideBranchGraph')}
                >
                  <Icon name="close" size={14} />
                </button>
              </div>
            </>
          )}
        </div>
        <CustomScrollArea className="branch-graph-content">
          {currentUserNodes.length === 0 && !isSearching ? (
            <div className="branch-graph-empty">{t('workflow.emptyWorkflow')}</div>
          ) : isLoadingMessages && isSearching ? (
            <div className="branch-graph-empty">{t('common.loading') }</div>
          ) : (
            <div className="branch-graph-padding">
              {/* 当前分支的 user 节点列表（搜索时不清空，仅作为分支导航展示） */}
              {currentUserNodes.map((node, index) => {
                const data = node.data as UserNodeData;
                const content = data.content || '';
                const summary = content.length > 40 ? content.slice(0, 40) + '...' : (content || t('workflow.attachmentMessage'));
                const isActive = currentVisibleNodeId === node.id;
                // 判断该 user 节点是否属于多分支组（branchTotal > 1）
                const hasBranchGroup = !!(data.branchGroupId && data.branchTotal && data.branchTotal > 1);
                // 当前分支组是否处于展开状态
                const isExpanded = hasBranchGroup && expandedGroupIds.has(data.branchGroupId!);
                // 获取当前分支组信息
                const group = hasBranchGroup ? branchGroups.find((g) => g.branchGroupId === data.branchGroupId) : undefined;
                // 按 sortOrder 升序排序分支列表
                const sortedBranches = group?.branches.slice().sort((a, b) => a.sortOrder - b.sortOrder) || [];
                // 节点 class：active（当前可见）
                const nodeClass = `branch-graph-node${isActive ? ' active' : ''}`;
                return (
                  <div
                    key={node.id}
                    className={nodeClass}
                    data-node-id={node.id}
                    onClick={() => jumpToNode(node.id)}
                  >
                    <span className="branch-graph-node-index">{index + 1}</span>
                    <span className="branch-graph-node-content">{summary}</span>
                    {/* 分支组指示按钮：点击切换展开/折叠，阻止冒泡避免触发节点跳转 */}
                    {hasBranchGroup && (
                      <div
                        className="branch-group-indicator"
                        onClick={(e) => {
                          e.stopPropagation();
                          toggleGroup(data.branchGroupId!);
                        }}
                      >
                        <Icon name="git-branch" size={11} />
                        <span>{t('workflow.branchesCount', { count: data.branchTotal })}</span>
                      </div>
                    )}
                    {/* 展开时显示分支列表，点击切换活跃分支；当前活跃分支高亮，非活跃分支标记 disabled */}
                    {isExpanded && sortedBranches.length > 0 && (
                      <div className="branch-group-list">
                        {sortedBranches.map((b) => {
                          const isCurrent = b.branchId === activeBranchId;
                          return (
                            <div
                              key={b.branchId}
                              className={`branch-group-item${isCurrent ? ' active' : ' disabled'}`}
                              onClick={(e) => {
                                e.stopPropagation();
                                void handleSwitchToBranch(b.branchId);
                              }}
                            >
                              {b.name}
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>
                );
              })}

              {/* 搜索模式下，其他分支的匹配项追加在列表末尾，标识来源分支名 */}
              {searchQuery && searchResultsFromOtherBranches.length > 0 && (
                <>
                  <div className="branch-graph-separator" />
                  {searchResultsFromOtherBranches.map((msg) => {
                    // 截取前 40 字符作为摘要
                    const summary = msg.content.length > 40 ? msg.content.slice(0, 40) + '...' : msg.content;
                    // 查找来源分支名称
                    const sourceBranch = branchGroups
                      .flatMap((g) => g.branches)
                      .find((b) => b.branchId === msg.branchId);
                    const branchName = sourceBranch?.name || msg.branchId.slice(-8);
                    return (
                      <div
                        key={msg.messageId}
                        className="branch-graph-node other-branch"
                        onClick={() => void handleSearchResultClick(msg)}
                      >
                        <span className="branch-graph-node-index">
                          <Icon name="git-branch" size={11} />
                        </span>
                        <span className="branch-graph-node-content">{summary}</span>
                        <span className="branch-graph-node-branch-tag">{branchName}</span>
                      </div>
                    );
                  })}
                </>
              )}

              {/* 搜索模式下无任何匹配项时显示提示 */}
              {searchQuery &&
                !hasCurrentBranchMatches &&
                searchResultsFromOtherBranches.length === 0 && (
                  <div className="branch-graph-empty">{t('workflow.noSearchResults')}</div>
                )}
            </div>
          )}
        </CustomScrollArea>
      </div>
    </div>
  );
}
