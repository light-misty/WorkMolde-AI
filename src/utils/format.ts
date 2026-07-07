// 快捷键组合解析结果
export interface ParsedShortcut {
  key: string;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

// 解析快捷键字符串（如 "Ctrl+Enter"、"Enter"、"Shift+Enter"）为结构化对象
export function parseShortcut(shortcut: string): ParsedShortcut {
  const parts = shortcut.split("+").map((p) => p.trim());
  return {
    ctrlKey: parts.includes("Ctrl"),
    shiftKey: parts.includes("Shift"),
    altKey: parts.includes("Alt"),
    key: parts[parts.length - 1] || "",
  };
}

// 判断键盘事件是否匹配快捷键组合
export function matchesShortcut(e: { key: string; ctrlKey: boolean; shiftKey: boolean; altKey: boolean }, shortcut: string): boolean {
  const parsed = parseShortcut(shortcut);
  return (
    e.key.toLowerCase() === parsed.key.toLowerCase() &&
    e.ctrlKey === parsed.ctrlKey &&
    e.shiftKey === parsed.shiftKey &&
    e.altKey === parsed.altKey
  );
}

// 根据 sendMessage 快捷键推导换行快捷键
// 如果发送是 Enter，则换行是 Shift+Enter；如果发送是 Ctrl+Enter，则换行是 Enter
export function deriveNewLineShortcut(sendShortcut: string): string {
  const parsed = parseShortcut(sendShortcut);
  if (parsed.key.toLowerCase() === "enter") {
    if (parsed.ctrlKey) {
      return "Enter";
    }
    if (parsed.shiftKey) {
      return "Enter";
    }
    return "Shift+Enter";
  }
  return "Enter";
}

export function formatTime(ts: number): string {
  const d = new Date(ts);
  return d.toTimeString().slice(0, 8);
}

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

// 直接导入 i18n 实例（i18n 在应用启动时已同步初始化，此处在运行时调用时 i18n 已就绪）
import i18n from '../i18n';

export function generateToolBrief(toolName: string, input: Record<string, unknown>): string {
  const f = (key: string) => String(input[key] ?? "");
  const actionMap: Record<string, string> = {
    read: i18n.t('toolBrief.read'),
    convert: i18n.t('toolBrief.convert'),
    analyze: i18n.t('toolBrief.analyze'),
  };
  const formatMap: Record<string, string> = {
    docx_handler: "Word",
    xlsx_handler: "Excel",
    pptx_handler: "PPT",
    pdf_handler: "PDF",
  };
  const action = actionMap[f("action")] || "";
  const format = formatMap[toolName] || "";
  switch (toolName) {
    case "docx_handler":
    case "xlsx_handler":
    case "pptx_handler":
    case "pdf_handler":
      // 流式阶段提前发射时参数可能为空，此时只显示格式名称
      if (action) {
        return `${action} ${format} ${f("path") || i18n.t('toolBrief.document')}`;
      }
      return `${format} ${f("path") || i18n.t('toolBrief.document')}`;
    case "delete_file":
      return `${i18n.t('toolBrief.delete')} ${f("path") || i18n.t('toolBrief.file')}`;
    case "search_files":
      return `${i18n.t('toolBrief.search')} ${f("query") ? `"${f("query")}"` : i18n.t('toolBrief.file')}`;
    case "list_directory":
      return i18n.t('toolBrief.listDirectory');
    case "read_file":
      return `${i18n.t('toolBrief.read')} ${f("path") || i18n.t('toolBrief.file')}`;
    case "write_text_file":
      return `${i18n.t('toolBrief.write')} ${f("path") || i18n.t('toolBrief.file')}`;
    default:
      return toolName;
  }
}
