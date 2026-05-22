import type { ReactNode } from "react";

interface MainAreaProps {
  workflow: ReactNode;
  inputArea: ReactNode;
}

export function MainArea({ workflow, inputArea }: MainAreaProps) {
  return (
    <>
      {/* 工作流区域：滚动由 WorkflowTimeline 内部虚拟滚动容器管理 */}
      <div className="workflow-area flex-1">
        {workflow}
      </div>

      {/* 输入框 */}
      {inputArea}
    </>
  );
}
