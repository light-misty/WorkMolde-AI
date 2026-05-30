// 快捷键列表
const shortcutList = [
  { keys: "Ctrl+N", desc: "新建会话" },
  { keys: "Ctrl+W", desc: "关闭当前会话" },
  { keys: "Ctrl+Enter", desc: "发送消息" },
  { keys: "Shift+Enter", desc: "输入框换行" },
  { keys: "Ctrl+B", desc: "切换侧边栏" },
  { keys: "Ctrl+/", desc: "快速提示" },
  { keys: "Escape", desc: "关闭弹窗/取消操作" },
  { keys: "Ctrl+,", desc: "打开设置" },
];

// 常见问题
const faqList = [
  {
    q: "如何配置 LLM Provider?",
    a: "在设置 > LLM 配置中，点击\"添加 Provider\"按钮，选择 Provider 类型（OpenAI/Anthropic/Gemini/Ollama/自定义），填写 API 地址、API Key 和模型名称即可。",
  },
  {
    q: "支持哪些文档格式?",
    a: "支持 Word(.docx)、Excel(.xlsx)、PowerPoint(.pptx)、PDF(.pdf) 和 Markdown(.md) 格式的生成、读取和修改，以及这些格式之间的相互转换。",
  },
  {
    q: "如何创建自定义 Skill?",
    a: "在设置 > Skills 管理中，点击\"添加自定义 Skill\"按钮。自定义 Skill 本质是 Prompt 模板，支持 {{param_name}} 占位符，Agent 调用时参数会自动替换。",
  },
  {
    q: "Agent 执行高风险操作时如何确认?",
    a: "Agent 执行删除、覆盖等高风险操作时会弹出确认对话框，您可以选择批准或拒绝。确认级别可在设置 > 通用设置中调整（全部需确认/仅编辑确认/全部自动确认）。",
  },
  {
    q: "如何查看文档版本历史?",
    a: "在文件树中右键点击文件选择\"版本历史\"，或在文档预览面板中点击版本历史按钮。每次文档修改前会自动创建版本快照，支持回滚到任意历史版本。",
  },
  {
    q: "如何切换工作区?",
    a: "点击顶部栏的工作区选择器可切换工作区，或在设置 > 工作区管理中添加、切换和管理工作区。每个工作区的文件和配置相互独立。",
  },
  {
    q: "如何使用 Prompt 模板?",
    a: "在设置 > Prompt 模板中可创建和管理常用 Prompt 模板，支持变量占位符。在输入框中可通过模板选择器快速插入模板内容。",
  },
];

// 内置 Skill 列表
const builtinSkills = [
  { name: "docx_skill", desc: "Word 文档操作（生成/读取/修改/转换/分析）" },
  { name: "xlsx_skill", desc: "Excel 文档操作（生成/读取/修改/转换/分析）" },
  { name: "pptx_skill", desc: "PPT 文档操作（生成/读取/修改/转换/分析）" },
  { name: "pdf_skill", desc: "PDF 文档操作（生成/读取/修改/转换/分析）" },
];

export function HelpTab() {

  return (
    <div>
      {/* 快速入门 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">快速入门</span>
        </div>
        <div className="help-card">
          <div className="help-step">
            <span className="help-step-num">1</span>
            <div className="help-step-content">
              <div className="help-step-title">配置 LLM Provider</div>
              <div className="help-step-desc">在设置中添加 OpenAI/Anthropic/Gemini 等 API 配置</div>
            </div>
          </div>
          <div className="help-step">
            <span className="help-step-num">2</span>
            <div className="help-step-content">
              <div className="help-step-title">选择工作区</div>
              <div className="help-step-desc">指定文档存放的目录，Agent 将在此目录下操作文件</div>
            </div>
          </div>
          <div className="help-step">
            <span className="help-step-num">3</span>
            <div className="help-step-content">
              <div className="help-step-title">开始对话</div>
              <div className="help-step-desc">在输入框中描述您的需求，Agent 会自动选择合适的 Skill 完成任务</div>
            </div>
          </div>
        </div>
      </div>

      {/* 快捷键 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">快捷键</span>
        </div>
        <div className="help-shortcut-list">
          {shortcutList.map((item) => (
            <div key={item.keys} className="help-shortcut-row">
              <span className="help-shortcut-desc">{item.desc}</span>
              <kbd className="help-shortcut-key">{item.keys}</kbd>
            </div>
          ))}
        </div>
      </div>

      {/* 内置 Skill 列表 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">内置 Skill 列表</span>
        </div>
        <div className="help-skill-list">
          {builtinSkills.map((skill) => (
            <div key={skill.name} className="help-skill-row">
              <span className="help-skill-name">{skill.name}</span>
              <span className="help-skill-desc">{skill.desc}</span>
            </div>
          ))}
        </div>
      </div>

      {/* 常见问题 */}
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">常见问题</span>
        </div>
        <div className="help-faq-list">
          {faqList.map((item, i) => (
            <div key={i} className="help-faq-item">
              <div className="help-faq-q">{item.q}</div>
              <div className="help-faq-a">{item.a}</div>
            </div>
          ))}
        </div>
      </div>

      <style>{`
        .help-card {
          padding: 16px;
          background: var(--color-bg-sub);
          border-radius: var(--radius-md);
          border: 1px solid var(--color-border-light);
          display: flex;
          flex-direction: column;
          gap: 12px;
        }
        .help-step {
          display: flex;
          align-items: flex-start;
          gap: 12px;
        }
        .help-step-num {
          width: 24px;
          height: 24px;
          border-radius: 50%;
          background: var(--color-accent);
          color: #fff;
          display: flex;
          align-items: center;
          justify-content: center;
          font-size: 12px;
          font-weight: 700;
          flex-shrink: 0;
        }
        .help-step-content {
          flex: 1;
          min-width: 0;
        }
        .help-step-title {
          font-size: 13px;
          font-weight: 600;
          color: var(--color-text-primary);
        }
        .help-step-desc {
          font-size: 12px;
          color: var(--color-text-tertiary);
          margin-top: 2px;
        }
        .help-shortcut-list {
          display: flex;
          flex-direction: column;
        }
        .help-shortcut-row {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 8px 12px;
          border-bottom: 1px solid var(--color-border-light);
        }
        .help-shortcut-row:last-child {
          border-bottom: none;
        }
        .help-shortcut-desc {
          font-size: 13px;
          color: var(--color-text-primary);
        }
        .help-shortcut-key {
          padding: 2px 8px;
          border-radius: var(--radius-sm);
          background: var(--color-bg-sub);
          border: 1px solid var(--color-border);
          font-size: 11px;
          font-family: var(--font-mono);
          color: var(--color-text-secondary);
        }
        .help-skill-list {
          display: flex;
          flex-direction: column;
        }
        .help-skill-row {
          display: flex;
          align-items: center;
          padding: 8px 12px;
          border-bottom: 1px solid var(--color-border-light);
          gap: 12px;
        }
        .help-skill-row:last-child {
          border-bottom: none;
        }
        .help-skill-name {
          font-size: 12px;
          font-family: var(--font-mono);
          color: var(--color-accent);
          min-width: 160px;
        }
        .help-skill-desc {
          font-size: 12px;
          color: var(--color-text-tertiary);
        }
        .help-faq-list {
          display: flex;
          flex-direction: column;
          gap: 12px;
        }
        .help-faq-item {
          padding: 12px;
          background: var(--color-bg-sub);
          border-radius: var(--radius-sm);
          border: 1px solid var(--color-border-light);
        }
        .help-faq-q {
          font-size: 13px;
          font-weight: 600;
          color: var(--color-text-primary);
          margin-bottom: 4px;
        }
        .help-faq-a {
          font-size: 12px;
          color: var(--color-text-tertiary);
          line-height: 1.6;
        }
      `}</style>
    </div>
  );
}
