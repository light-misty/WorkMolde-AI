import type { ReactNode } from "react";
import { useState } from "react";
import { Icon } from "../common/Icon";

interface SidebarSectionProps {
  title: string;
  defaultOpen?: boolean;
  children: ReactNode;
}

export function SidebarSection({ title, defaultOpen = true, children }: SidebarSectionProps) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div className="sb-section">
      <div
        className="sb-section-header"
        role="button"
        aria-expanded={open}
        aria-label={title}
        tabIndex={0}
        onClick={() => setOpen(!open)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setOpen(!open);
          }
        }}
      >
        <div className="sb-section-header-left">
          <span className="sb-section-accent" />
          <span className="sb-section-title">{title}</span>
        </div>
        <span
          className="sb-section-chevron"
          style={{ transform: open ? "rotate(0deg)" : "rotate(-90deg)" }}
        >
          <Icon name="chevron-down" size={14} />
        </span>
      </div>
      <div
        className="sb-section-body"
        role="region"
        aria-label={title}
        style={{
          maxHeight: open ? "2000px" : "0px",
          opacity: open ? 1 : 0,
        }}
      >
        <div className="sb-section-content">
          {children}
        </div>
      </div>

      <style>{`
        .sb-section {
          border-bottom: 1px solid var(--color-border-light);
        }
        .sb-section-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 10px 16px;
          cursor: pointer;
          user-select: none;
          transition: background 0.2s;
        }
        .sb-section-header:hover {
          background: var(--color-accent-bg);
        }
        .sb-section-header:hover .sb-section-accent {
          opacity: 1;
          transform: scaleY(1);
        }
        .sb-section-header-left {
          display: flex;
          align-items: center;
          gap: 8px;
        }
        .sb-section-accent {
          width: 3px;
          height: 14px;
          border-radius: 2px;
          background: var(--color-accent);
          opacity: 0.4;
          transform: scaleY(0.7);
          transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
        }
        .sb-section-title {
          font-size: 11px;
          font-weight: 600;
          color: var(--color-text-secondary);
          letter-spacing: 0.6px;
          text-transform: uppercase;
        }
        .sb-section-chevron {
          display: flex;
          align-items: center;
          justify-content: center;
          color: var(--color-text-quaternary);
          transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
        }
        .sb-section-header:hover .sb-section-chevron {
          color: var(--color-text-secondary);
        }
        .sb-section-body {
          overflow: hidden;
          transition: max-height 0.3s cubic-bezier(0.4, 0, 0.2, 1),
                      opacity 0.25s ease;
        }
        .sb-section-content {
          padding: 0 16px 12px;
        }
      `}</style>
    </div>
  );
}
