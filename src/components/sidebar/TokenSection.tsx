import { SidebarSection } from "../layout/Sidebar";
import { useTokenStore } from "../../stores/useTokenStore";
import { formatTokens } from "../../utils/format";

export function TokenSection() {
  const { sessionTokens, inputTokens, outputTokens, dailyTotal, monthlyTotal, dailyBudget, monthlyBudget } = useTokenStore();

  const dailyBudgetPercent = dailyBudget > 0 ? (dailyTotal / dailyBudget) * 100 : 0;
  const monthlyBudgetPercent = monthlyBudget > 0 ? (monthlyTotal / monthlyBudget) * 100 : 0;

  return (
    <SidebarSection title="Token 统计">
      <div className="tk-grid">
        {/* 本次会话 */}
        <div className="tk-field">
          <span className="tk-field-label">本次会话</span>
          <span className="tk-field-value tk-value-lg">{formatTokens(sessionTokens)}</span>
        </div>
        <div className="tk-breakdown">
          <div className="tk-breakdown-item">
            <span className="tk-breakdown-dot tk-dot-input" />
            <span className="tk-breakdown-label">输入</span>
            <span className="tk-breakdown-value tk-val-input">{formatTokens(inputTokens)}</span>
          </div>
          <div className="tk-breakdown-item">
            <span className="tk-breakdown-dot tk-dot-output" />
            <span className="tk-breakdown-label">输出</span>
            <span className="tk-breakdown-value tk-val-output">{formatTokens(outputTokens)}</span>
          </div>
        </div>

        {/* 分隔线 */}
        <div className="tk-divider" />

        {/* 累计统计 */}
        <div className="tk-field">
          <span className="tk-field-label">今日累计</span>
          <span className="tk-field-value">{formatTokens(dailyTotal)}</span>
        </div>
        {dailyBudget > 0 && (
          <div className="tk-mini-bar-track">
            <div
              className={`tk-mini-bar-fill ${
                dailyBudgetPercent > 100 ? "bar-exceeded" : dailyBudgetPercent > 80 ? "bar-warning" : ""
              }`}
              style={{ width: `${Math.min(dailyBudgetPercent, 100)}%` }}
            />
          </div>
        )}

        <div className="tk-field">
          <span className="tk-field-label">本月累计</span>
          <span className="tk-field-value">{formatTokens(monthlyTotal)}</span>
        </div>
        {monthlyBudget > 0 && (
          <div className="tk-mini-bar-track">
            <div
              className={`tk-mini-bar-fill ${
                monthlyBudgetPercent > 100 ? "bar-exceeded" : monthlyBudgetPercent > 80 ? "bar-warning" : ""
              }`}
              style={{ width: `${Math.min(monthlyBudgetPercent, 100)}%` }}
            />
          </div>
        )}

        {/* 分隔线 */}
        <div className="tk-divider" />

        {/* 预算设置 */}
        <div className="tk-field">
          <span className="tk-field-label">日预算</span>
          <span className={`tk-field-value ${dailyBudget > 0 ? "" : "tk-value-unset"}`}>
            {dailyBudget > 0 ? formatTokens(dailyBudget) : "--"}
          </span>
        </div>
        <div className="tk-field">
          <span className="tk-field-label">月预算</span>
          <span className={`tk-field-value ${monthlyBudget > 0 ? "" : "tk-value-unset"}`}>
            {monthlyBudget > 0 ? formatTokens(monthlyBudget) : "--"}
          </span>
        </div>
      </div>

      <style>{`
        .tk-grid {
          display: flex;
          flex-direction: column;
          gap: 6px;
        }
        .tk-field {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 4px 0;
        }
        .tk-field-label {
          font-size: 12px;
          color: var(--color-text-quaternary);
        }
        .tk-field-value {
          font-family: var(--font-mono);
          font-size: 12px;
          font-weight: 500;
          color: var(--color-text-primary);
        }
        .tk-value-lg {
          font-size: 14px;
          font-weight: 600;
          letter-spacing: -0.2px;
        }
        .tk-value-unset {
          color: var(--color-text-quaternary);
        }
        .tk-breakdown {
          display: flex;
          gap: 16px;
          padding-left: 2px;
        }
        .tk-breakdown-item {
          display: flex;
          align-items: center;
          gap: 5px;
        }
        .tk-breakdown-dot {
          width: 6px;
          height: 6px;
          border-radius: 50%;
          flex-shrink: 0;
        }
        .tk-dot-input {
          background: var(--color-accent);
        }
        .tk-dot-output {
          background: var(--color-purple);
        }
        .tk-breakdown-label {
          font-size: 11px;
          color: var(--color-text-quaternary);
        }
        .tk-breakdown-value {
          font-family: var(--font-mono);
          font-size: 11px;
          font-weight: 500;
        }
        .tk-val-input {
          color: var(--color-accent);
        }
        .tk-val-output {
          color: var(--color-purple);
        }
        .tk-divider {
          height: 1px;
          background: var(--color-border-light);
          margin: 2px 0;
        }
        .tk-mini-bar-track {
          height: 2px;
          background: var(--color-border-light);
          border-radius: 1px;
          overflow: hidden;
        }
        .tk-mini-bar-fill {
          height: 100%;
          border-radius: 1px;
          background: var(--color-accent);
          transition: width 0.5s cubic-bezier(0.4, 0, 0.2, 1), background 0.3s;
        }
        .tk-mini-bar-fill.bar-warning {
          background: var(--color-warning);
        }
        .tk-mini-bar-fill.bar-exceeded {
          background: var(--color-error);
        }
      `}</style>
    </SidebarSection>
  );
}
