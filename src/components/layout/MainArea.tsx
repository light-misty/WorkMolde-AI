import type { ReactNode } from "react";

interface MainAreaProps {
  workflow: ReactNode;
  inputArea: ReactNode;
  /** 是否为空会话状态：空会话时工作流与输入框整体垂直居中 */
  isEmpty?: boolean;
}

export function MainArea({ workflow, inputArea, isEmpty = false }: MainAreaProps) {
  return (
    <div className={`main-area ${isEmpty ? "main-area-empty" : ""}`}>
      {/* 工作流区域：滚动由 WorkflowTimeline 内部虚拟滚动容器管理 */}
      <div className={`workflow-area ${isEmpty ? "" : "flex-1"}`}>
        {workflow}
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
        }
        /* min-height: 0 必需：允许 workflow-area 在 flex 列布局中缩小到小于内容高度，
           否则 WorkflowTimeline 的虚拟滚动容器会撑大 workflow-area，
           导致 overflow-y: auto 失效并挤压输入框 */
        .workflow-area {
          min-height: 0;
        }
      `}</style>
    </div>
  );
}
