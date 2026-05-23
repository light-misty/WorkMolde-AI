import { useEffect, useState, useMemo } from "react";
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  PieChart,
  Pie,
  Cell,
  Legend,
} from "recharts";
import { getTokenUsageTrend, getTokenProviderUsage, getTokenUsageOverview } from "../../services/tauri";
import { formatTokens } from "../../utils/format";
import type { DailyUsageItem, ProviderUsageItem, TokenUsageOverview } from "../../types";

const PIE_COLORS = [
  "var(--color-accent)",
  "var(--color-purple)",
  "#10b981",
  "#f59e0b",
  "#ef4444",
  "#06b6d4",
  "#8b5cf6",
  "#ec4899",
];

type TrendRange = 7 | 14 | 30;

export function TokenUsageTab() {
  const [overview, setOverview] = useState<TokenUsageOverview | null>(null);
  const [trend, setTrend] = useState<DailyUsageItem[]>([]);
  const [providerUsage, setProviderUsage] = useState<ProviderUsageItem[]>([]);
  const [trendRange, setTrendRange] = useState<TrendRange>(14);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadData();
  }, []);

  useEffect(() => {
    loadTrend(trendRange);
  }, [trendRange]);

  async function loadData() {
    setLoading(true);
    try {
      const [overviewData, providerData] = await Promise.all([
        getTokenUsageOverview(),
        getTokenProviderUsage(),
      ]);
      setOverview(overviewData);
      setProviderUsage(providerData);
    } catch (error) {
      console.error("[TokenUsageTab] 加载数据失败:", error);
    } finally {
      setLoading(false);
    }
  }

  async function loadTrend(days: number) {
    try {
      const trendData = await getTokenUsageTrend(undefined, days);
      setTrend(trendData);
    } catch (error) {
      console.error("[TokenUsageTab] 加载趋势数据失败:", error);
    }
  }

  // 趋势图表数据：合并输入/输出为堆叠柱状图
  const trendChartData = useMemo(() => {
    return trend.map((item) => ({
      date: item.date.slice(5),
      输入: item.inputTokens,
      输出: item.outputTokens,
    }));
  }, [trend]);

  // Provider 分布饼图数据
  const providerPieData = useMemo(() => {
    return providerUsage.map((item) => ({
      name: `${item.provider} / ${item.model}`,
      value: item.inputTokens + item.outputTokens,
    }));
  }, [providerUsage]);

  if (loading) {
    return (
      <div className="tu-loading">
        <div className="tu-loading-spinner" />
        <span>加载统计数据...</span>
      </div>
    );
  }

  return (
    <div>
      {/* 用量概览卡片 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">用量概览</span>
        </div>
        <div className="tu-overview-grid">
          <OverviewCard label="累计总量" value={overview ? formatTokens(overview.totalInput + overview.totalOutput) : "0"} sub={`输入 ${formatTokens(overview?.totalInput ?? 0)} / 输出 ${formatTokens(overview?.totalOutput ?? 0)}`} />
          <OverviewCard label="今日用量" value={overview ? formatTokens(overview.todayInput + overview.todayOutput) : "0"} sub={`输入 ${formatTokens(overview?.todayInput ?? 0)} / 输出 ${formatTokens(overview?.todayOutput ?? 0)}`} accent />
          <OverviewCard label="本月用量" value={overview ? formatTokens(overview.monthInput + overview.monthOutput) : "0"} sub={`输入 ${formatTokens(overview?.monthInput ?? 0)} / 输出 ${formatTokens(overview?.monthOutput ?? 0)}`} />
        </div>
      </div>

      {/* 每日用量趋势图 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">每日用量趋势</span>
          <div className="tu-range-switcher">
            {([7, 14, 30] as TrendRange[]).map((d) => (
              <button
                key={d}
                className={`tu-range-btn ${trendRange === d ? "active" : ""}`}
                onClick={() => setTrendRange(d)}
              >
                {d}天
              </button>
            ))}
          </div>
        </div>
        <div className="tu-chart-container">
          {trendChartData.length > 0 ? (
            <ResponsiveContainer width="100%" height={220}>
              <BarChart data={trendChartData} margin={{ top: 8, right: 8, left: -10, bottom: 0 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border-light)" />
                <XAxis
                  dataKey="date"
                  tick={{ fontSize: 11, fill: "var(--color-text-tertiary)" }}
                  axisLine={{ stroke: "var(--color-border-light)" }}
                  tickLine={false}
                />
                <YAxis
                  tick={{ fontSize: 11, fill: "var(--color-text-tertiary)" }}
                  axisLine={false}
                  tickLine={false}
                  tickFormatter={(v: number) => v >= 1000 ? `${(v / 1000).toFixed(0)}k` : String(v)}
                />
                <Tooltip
                  contentStyle={{
                    background: "var(--color-bg-elevated)",
                    border: "1px solid var(--color-border)",
                    borderRadius: "var(--radius-sm)",
                    fontSize: 12,
                    color: "var(--color-text-primary)",
                  }}
                  formatter={(value: unknown, name: unknown) => [formatTokens(Number(value)), String(name)]}
                />
                <Bar dataKey="输入" stackId="tokens" fill="var(--color-accent)" radius={[0, 0, 0, 0]} />
                <Bar dataKey="输出" stackId="tokens" fill="var(--color-purple)" radius={[3, 3, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          ) : (
            <div className="tu-empty">暂无用量数据</div>
          )}
        </div>
      </div>

      {/* Provider 分布 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">Provider 分布</span>
        </div>
        <div className="tu-chart-container">
          {providerPieData.length > 0 ? (
            <ResponsiveContainer width="100%" height={240}>
              <PieChart>
                <Pie
                  data={providerPieData}
                  cx="50%"
                  cy="50%"
                  innerRadius={50}
                  outerRadius={80}
                  paddingAngle={2}
                  dataKey="value"
                  label={({ name, percent }: { name?: string; percent?: number }) =>
                    `${name ?? ""} ${((percent ?? 0) * 100).toFixed(0)}%`
                  }
                  labelLine={{ stroke: "var(--color-text-tertiary)", strokeWidth: 1 }}
                >
                  {providerPieData.map((_, index) => (
                    <Cell key={`cell-${index}`} fill={PIE_COLORS[index % PIE_COLORS.length]} />
                  ))}
                </Pie>
                <Tooltip
                  contentStyle={{
                    background: "var(--color-bg-elevated)",
                    border: "1px solid var(--color-border)",
                    borderRadius: "var(--radius-sm)",
                    fontSize: 12,
                    color: "var(--color-text-primary)",
                  }}
                  formatter={(value: unknown) => formatTokens(Number(value))}
                />
                <Legend
                  wrapperStyle={{ fontSize: 12, color: "var(--color-text-secondary)" }}
                />
              </PieChart>
            </ResponsiveContainer>
          ) : (
            <div className="tu-empty">暂无 Provider 数据</div>
          )}
        </div>
      </div>

      {/* Provider 明细表 */}
      {providerUsage.length > 0 && (
        <div className="settings-section">
          <div className="section-header">
            <span className="section-title">Provider 明细</span>
          </div>
          <div className="tu-provider-table">
            <table>
              <thead>
                <tr>
                  <th>Provider</th>
                  <th>Model</th>
                  <th>输入 Tokens</th>
                  <th>输出 Tokens</th>
                  <th>合计</th>
                </tr>
              </thead>
              <tbody>
                {providerUsage.map((item, idx) => (
                  <tr key={idx}>
                    <td>{item.provider}</td>
                    <td className="tu-mono">{item.model}</td>
                    <td className="tu-mono">{formatTokens(item.inputTokens)}</td>
                    <td className="tu-mono">{formatTokens(item.outputTokens)}</td>
                    <td className="tu-mono tu-total">{formatTokens(item.inputTokens + item.outputTokens)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      <style>{`
        .tu-loading {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          padding: 60px 0;
          gap: 12px;
          color: var(--color-text-tertiary);
          font-size: 13px;
        }
        .tu-loading-spinner {
          width: 24px;
          height: 24px;
          border: 2px solid var(--color-border);
          border-top-color: var(--color-accent);
          border-radius: 50%;
          animation: tu-spin 0.8s linear infinite;
        }
        @keyframes tu-spin {
          to { transform: rotate(360deg); }
        }
        .tu-overview-grid {
          display: grid;
          grid-template-columns: repeat(3, 1fr);
          gap: 12px;
        }
        .tu-overview-card {
          padding: 16px;
          background: var(--color-bg-sub);
          border-radius: var(--radius-md);
          border: 1px solid var(--color-border-light);
          transition: border-color 0.2s;
        }
        .tu-overview-card:hover {
          border-color: var(--color-border);
        }
        .tu-overview-card.accent {
          border-color: var(--color-accent);
          background: var(--color-accent-lighter, rgba(59, 130, 246, 0.06));
        }
        .tu-overview-label {
          font-size: 11px;
          font-weight: 500;
          color: var(--color-text-quaternary);
          text-transform: uppercase;
          letter-spacing: 0.3px;
          margin-bottom: 6px;
        }
        .tu-overview-value {
          font-size: 20px;
          font-weight: 700;
          color: var(--color-text-primary);
          font-family: var(--font-mono);
          letter-spacing: -0.5px;
        }
        .tu-overview-sub {
          font-size: 11px;
          color: var(--color-text-tertiary);
          margin-top: 4px;
        }
        .tu-range-switcher {
          display: flex;
          gap: 0;
          border: 1px solid var(--color-border);
          border-radius: var(--radius-sm);
          overflow: hidden;
          margin-left: auto;
        }
        .tu-range-btn {
          padding: 4px 12px;
          font-size: 11px;
          font-weight: 500;
          color: var(--color-text-secondary);
          background: var(--color-bg);
          border: none;
          border-right: 1px solid var(--color-border);
          cursor: pointer;
          transition: all 0.15s;
        }
        .tu-range-btn:last-child {
          border-right: none;
        }
        .tu-range-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
        }
        .tu-range-btn.active {
          background: var(--color-accent);
          color: #fff;
        }
        .tu-chart-container {
          margin-top: 8px;
          min-height: 100px;
        }
        .tu-empty {
          display: flex;
          align-items: center;
          justify-content: center;
          height: 120px;
          color: var(--color-text-tertiary);
          font-size: 13px;
        }
        .tu-provider-table {
          overflow-x: auto;
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-sm);
        }
        .tu-provider-table table {
          width: 100%;
          border-collapse: collapse;
          font-size: 12px;
        }
        .tu-provider-table th {
          padding: 8px 12px;
          text-align: left;
          font-weight: 600;
          color: var(--color-text-primary);
          background: var(--color-bg-sub);
          border-bottom: 1px solid var(--color-border-light);
          white-space: nowrap;
        }
        .tu-provider-table td {
          padding: 8px 12px;
          color: var(--color-text-secondary);
          border-bottom: 1px solid var(--color-border-light);
          white-space: nowrap;
        }
        .tu-provider-table tr:last-child td {
          border-bottom: none;
        }
        .tu-provider-table tr:hover td {
          background: var(--color-bg-sub);
        }
        .tu-mono {
          font-family: var(--font-mono);
        }
        .tu-total {
          font-weight: 600;
          color: var(--color-text-primary);
        }
      `}</style>
    </div>
  );
}

/** 概览卡片子组件 */
function OverviewCard({ label, value, sub, accent }: { label: string; value: string; sub: string; accent?: boolean }) {
  return (
    <div className={`tu-overview-card ${accent ? "accent" : ""}`}>
      <div className="tu-overview-label">{label}</div>
      <div className="tu-overview-value">{value}</div>
      <div className="tu-overview-sub">{sub}</div>
    </div>
  );
}
