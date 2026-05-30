export function formatTime(ts: number): string {
  const d = new Date(ts);
  return d.toTimeString().slice(0, 8);
}

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function generateToolBrief(toolName: string, input: Record<string, unknown>): string {
  const f = (key: string) => String(input[key] ?? "");
  const actionMap: Record<string, string> = {
    generate: "生成",
    read: "读取",
    modify: "修改",
    convert: "转换",
    analyze: "分析",
  };
  const formatMap: Record<string, string> = {
    docx_skill: "Word",
    xlsx_skill: "Excel",
    pptx_skill: "PPT",
    pdf_skill: "PDF",
  };
  const action = actionMap[f("action")] || "";
  const format = formatMap[toolName] || "";
  switch (toolName) {
    case "docx_skill":
    case "xlsx_skill":
    case "pptx_skill":
    case "pdf_skill":
      return `${action} ${format} ${f("path") || "文档"}`;
    case "delete_file":
      return `删除 ${f("path") || "文件"}`;
    case "search_files":
      return `搜索 ${f("query") ? `"${f("query")}"` : "文件"}`;
    case "list_directory":
      return "列出目录";
    case "read_file":
      return `读取 ${f("path") || "文件"}`;
    case "write_text_file":
      return `写入 ${f("path") || "文件"}`;
    default:
      return toolName;
  }
}
