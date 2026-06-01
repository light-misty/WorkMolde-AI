import { useSettingsStore } from "../../stores/useSettingsStore";

export function SkillsTab() {
  const { skills, tools } = useSettingsStore();

  // 内置 Skill 列表
  const builtinSkills = skills.filter((s) => s.isBuiltin);

  return (
    <div>
      {/* 内置 Tools */}
      <div className="section-header">
        <span className="section-title">内置 Tools</span>
        <span className="section-badge">{tools.length}</span>
      </div>

      <div className="skills-list">
        {tools.map((t) => (
          <div key={t.id} className="skill-item">
            <div className="skill-item-info">
              <div className="skill-name-row">
                <span className="skill-name">{t.name}</span>
                <span className="skill-tool-badge">Tool</span>
              </div>
              <div className="skill-desc">{t.description}</div>
            </div>
            <div className="tool-always-on">
              始终启用
            </div>
          </div>
        ))}
      </div>

      {/* 内置 Skills */}
      <div className="section-header" style={{ marginTop: 24 }}>
        <span className="section-title">内置 Skills</span>
        <span className="section-badge">{builtinSkills.length}</span>
      </div>

      <div className="skills-list">
        {builtinSkills.map((s) => (
          <div key={s.id} className="skill-item">
            <div className="skill-item-info">
              <div className="skill-name-row">
                <span className="skill-name">{s.name}</span>
                <span className="skill-skill-badge">Skill</span>
              </div>
              <div className="skill-desc">{s.description}</div>
            </div>
            <div className="tool-always-on">
              始终启用
            </div>
          </div>
        ))}
      </div>

      <style>{`
        .skills-list {
          display: flex;
          flex-direction: column;
          margin-bottom: 24px;
        }
        .skill-item {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 10px 12px;
          border-bottom: 1px solid var(--color-border-light);
          transition: background 0.15s;
        }
        .skill-item:hover {
          background: var(--color-accent-bg);
        }
        .skill-item:last-child {
          border-bottom: none;
        }
        .skill-item-info {
          flex: 1;
          min-width: 0;
        }
        .skill-name {
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-primary);
        }
        .skill-name-row {
          display: flex;
          align-items: center;
          gap: 6px;
        }
        .skill-tool-badge {
          font-size: 10px;
          font-weight: 500;
          padding: 1px 6px;
          border-radius: 4px;
          background: var(--color-accent-bg);
          color: var(--color-accent);
        }
        .skill-skill-badge {
          font-size: 10px;
          font-weight: 500;
          padding: 1px 6px;
          border-radius: 4px;
          background: var(--color-purple-light);
          color: var(--color-purple);
        }
        .tool-always-on {
          font-size: 11px;
          color: var(--color-text-quaternary);
          flex-shrink: 0;
        }
        .skill-desc {
          font-size: 11px;
          color: var(--color-text-quaternary);
          margin-top: 2px;
        }
      `}</style>
    </div>
  );
}
