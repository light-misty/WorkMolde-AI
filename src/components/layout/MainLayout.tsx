import type { ReactNode } from "react";

interface MainLayoutProps {
  mainArea: ReactNode;
  sidebar: ReactNode;
}

export function MainLayout({ mainArea, sidebar }: MainLayoutProps) {
  return (
    <div className="flex flex-1 overflow-hidden">
      {/* 主界面区 */}
      <div className="flex-1 flex flex-col min-w-0 border-r border-border">
        {mainArea}
      </div>

      {/* 右侧栏 */}
      <div className="sb-container">
        <div className="sb-scroll">
          {sidebar}
        </div>
      </div>

      <style>{`
        .sb-container {
          width: 300px;
          flex-shrink: 0;
          display: flex;
          flex-direction: column;
          background: var(--color-bg-sub);
          overflow: hidden;
          position: relative;
        }
        .sb-scroll {
          flex: 1;
          overflow-y: auto;
          overflow-x: hidden;
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
