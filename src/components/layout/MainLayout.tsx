import type { ReactNode } from "react";

interface MainLayoutProps {
  mainArea: ReactNode;
  sidebar: ReactNode;
  sidebarVisible?: boolean;
}

export function MainLayout({ mainArea, sidebar, sidebarVisible = true }: MainLayoutProps) {
  return (
    <div className="flex flex-1 overflow-hidden bg-bg-sub" style={{ position: 'relative' }}>
      {/* 左侧栏：绝对定位不参与 flex 布局，始终保持在左侧保持内容完整宽度 */}
      <div className="sb-container">
        <div className="sb-scroll">
          {sidebar}
        </div>
      </div>

      {/* 主界面区：通过 margin-left 留出侧边栏空间，收缩时扩张覆盖侧边栏 */}
      <div className={`main-area-wrap${sidebarVisible ? '' : ' sb-collapsed'}`}>
        <div className={`flex-1 flex flex-col min-w-0 min-h-0 pr-3${!sidebarVisible ? ' pl-3' : ''}`}>
          <div className="flex-1 flex flex-col bg-bg rounded-md border-[0.5px] border-border overflow-hidden">
            {mainArea}
          </div>
        </div>
      </div>

      <style>{`
        .sb-container {
          position: absolute;
          left: 0;
          top: 0;
          bottom: 0;
          width: 260px;
          z-index: 1;
          display: flex;
          flex-direction: column;
          background: var(--color-bg-sub);
          overflow: hidden;
        }
        .sb-scroll {
          flex: 1;
          /* min-height: 0 必需：允许 flex 子元素缩小到小于内容高度，
             否则内容会撑大容器导致 overflow-y: auto 失效，
             切换会话时 scrollTop 无法自动调整，内容会向上移动甚至消失 */
          min-height: 0;
          overflow-y: auto;
          overflow-x: hidden;
          width: 260px;
        }
        .main-area-wrap {
          flex: 1;
          display: flex;
          flex-direction: column;
          min-width: 0;
          margin-left: 260px;
          position: relative;
          z-index: 2;
          padding-bottom: 12px;
          transition: margin-left 0.25s cubic-bezier(0.4, 0, 0.2, 1);
        }
        .main-area-wrap.sb-collapsed {
          margin-left: 0;
        }
        @media (max-width: 900px) {
          .sb-container { width: 200px; }
          .sb-scroll { width: 200px; }
          .main-area-wrap { margin-left: 200px; }
          .main-area-wrap.sb-collapsed { margin-left: 0; }
        }
      `}</style>
    </div>
  );
}
