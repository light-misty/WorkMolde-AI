import type { ReactNode } from "react";

interface MainLayoutProps {
  mainArea: ReactNode;
  sidebar: ReactNode;
  sidebarVisible?: boolean;
}

export function MainLayout({ mainArea, sidebar, sidebarVisible = true }: MainLayoutProps) {
  return (
    <div className="flex flex-1 overflow-hidden bg-bg-sub">
      {/* 主界面区 - 白色圆角卡片 */}
      <div className="flex-1 flex flex-col min-w-0 p-0">
        <div className="flex-1 flex flex-col bg-bg rounded-xl border-[0.5px] border-border overflow-hidden">
          {mainArea}
        </div>
      </div>

      {/* 右侧栏 */}
      {sidebarVisible && (
        <div className="sb-container">
          <div className="sb-scroll">
            {sidebar}
          </div>
        </div>
      )}

      <style>{`
        .sb-container {
          width: 300px;
          flex-shrink: 0;
          display: flex;
          flex-direction: column;
          background: var(--color-bg-sub);
          overflow: hidden;
          position: relative;
          transition: width 0.2s ease, opacity 0.2s ease;
        }
        .sb-scroll {
          flex: 1;
          overflow-y: auto;
          overflow-x: hidden;
          mask-image: linear-gradient(to bottom, black 85%, transparent 100%);
          -webkit-mask-image: linear-gradient(to bottom, black 85%, transparent 100%);
        }
        @media (max-width: 900px) {
          .sb-container {
            width: 240px !important;
          }
        }
      `}</style>
    </div>
  );
}
