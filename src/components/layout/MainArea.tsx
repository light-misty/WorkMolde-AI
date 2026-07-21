import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useWorkflowStore } from "../../stores/useWorkflowStore";
import { WorkflowRightSidebar } from "../workflow/WorkflowRightSidebar";
import { Icon } from "../common/Icon";

interface MainAreaProps {
  workflow: ReactNode;
  inputArea: ReactNode;
  /** 是否为空会话状态：空会话时工作流与输入框整体垂直居中 */
  isEmpty?: boolean;
}

export function MainArea({ workflow, inputArea, isEmpty = false }: MainAreaProps) {
  const { t } = useTranslation();
  // 右侧边栏可见性：仅在非空会话且开关开启时显示
  const rightSidebarVisible = useWorkflowStore((s) => s.rightSidebarVisible);
  const setRightSidebarVisible = useWorkflowStore((s) => s.setRightSidebarVisible);
  // 非空会话时始终渲染右侧边栏（通过 collapsed class + 动画控制显隐，避免条件渲染导致动画失效）
  const showRightSidebar = !isEmpty;
  // 右侧边栏收起时，显示浮动展开按钮
  const showToggleButton = !isEmpty && !rightSidebarVisible;
  // 右侧边栏收起时，给 workflow-area 添加预留区域 class，避免消息框与浮动按钮重叠
  const workflowAreaClass = `workflow-area ${isEmpty ? "" : "flex-1"}${!isEmpty && !rightSidebarVisible ? " workflow-area-reserved" : ""}`;
  // 右侧边栏展开时，给 main-area 添加 class，让 InputArea 也跟随收缩宽度（通过 CSS 选择器）
  const mainAreaClass = `main-area ${isEmpty ? "main-area-empty" : ""}${!isEmpty && rightSidebarVisible ? " main-area-sidebar-expanded" : ""}`;

  return (
    <div className={mainAreaClass}>
      {/* 工作流区域：滚动由 WorkflowTimeline 内部虚拟滚动容器管理 */}
      <div className={workflowAreaClass}>
        {workflow}
        {showRightSidebar && <WorkflowRightSidebar collapsed={!rightSidebarVisible} />}
        {/* 右侧边栏收起时的浮动展开按钮（无边框，仅图标） */}
        {showToggleButton && (
          <button
            className="workflow-right-sidebar-toggle"
            onClick={() => setRightSidebarVisible(true)}
            title={t('workflow.showBranchGraph')}
          >
            <Icon name="git-branch" size={14} />
          </button>
        )}
      </div>

      {/* 输入框 */}
      {inputArea}

      <style>{`
        .main-area {
          display: flex;
          flex-direction: column;
          flex: 1;
          min-height: 0;
        }
        .main-area-empty {
          justify-content: center;
          align-items: center;
        }
        .main-area-empty .workflow-area {
          flex: 0 0 auto;
          width: 100%;
          /* 空会话时恢复 block 布局，避免 flex row 导致 EmptySessionTitle 错位 */
          display: block;
          position: static;
        }
        /* min-height: 0 必需：允许 workflow-area 在 flex 列布局中缩小到小于内容高度，
           否则 WorkflowTimeline 的虚拟滚动容器会撑大 workflow-area，
           导致 overflow-y: auto 失效并挤压输入框 */
        .workflow-area {
          min-height: 0;
          /* flex 横向布局：让 workflow 内容与右侧分支导航并排 */
          display: flex;
          flex-direction: row;
          /* 为浮动展开按钮提供定位上下文 */
          position: relative;
        }
        /* 右侧边栏收起时，为浮动按钮预留空间，避免消息框与按钮重叠 */
        .workflow-area-reserved > .workflow-scroll-container {
          padding-right: 48px;
        }
        .workflow-right-sidebar-toggle {
          position: absolute;
          top: 12px;
          right: 12px;
          z-index: 10;
          width: 28px;
          height: 28px;
          display: flex;
          align-items: center;
          justify-content: center;
          /* 去除边框，仅保留图标 */
          border: none;
          background: transparent;
          color: var(--color-text-secondary);
          cursor: pointer;
          transition: color 0.15s;
        }
        .workflow-right-sidebar-toggle:hover {
          color: var(--color-text-primary);
        }
        /* 右侧边栏展开时，InputArea 也跟随收缩宽度（侧边栏 240px 宽度 + 1px 左边框） */
        .main-area-sidebar-expanded > .input-area-wrapper {
          padding-right: 265px;
          transition: padding-right 0.3s ease;
        }
        .input-area-wrapper {
          transition: padding-right 0.3s ease;
          /* 为边框线伪元素提供定位上下文 */
          position: relative;
        }
        /* 输入框区域的右侧竖线：与侧边栏左边框对齐，使框线延伸到输入框区域 */
        .input-area-wrapper::before {
          content: '';
          position: absolute;
          top: 0;
          bottom: 0;
          right: 240px;
          width: 1px;
          background: var(--color-border);
          pointer-events: none;
          opacity: 0;
          transition: opacity 0.3s ease;
        }
        /* 侧边栏展开时显示竖线，与侧边栏 border-left-color 过渡同步 */
        .main-area-sidebar-expanded > .input-area-wrapper::before {
          opacity: 1;
        }
      `}</style>
    </div>
  );
}
