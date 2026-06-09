import { useEffect, useState, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "../common/Icon";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import * as Diff from "diff";
import { PdfCanvasViewer } from "./PdfCanvasViewer";

interface DiffData {
  oldContent: string;
  newContent: string;
}

interface PreviewOverlayProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  content?: string;
  fileType?: string;
  diffData?: DiffData | null;
  // PDF 文件的 base64 编码数据，用于 pdfjs-dist 渲染
  pdfBase64Data?: string | null;
}

export function PreviewOverlay({
  open,
  onClose,
  title = "",
  content = "",
  fileType,
  diffData = null,
  pdfBase64Data = null,
}: PreviewOverlayProps) {
  const { t } = useTranslation();
  const [showDiff, setShowDiff] = useState(false);

  useEffect(() => {
    if (!open) return;
    setShowDiff(false);
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 bg-black/30 z-[200] flex items-center justify-center animate-fade-in"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div
        className="w-4/5 max-w-[960px] h-[85vh] bg-bg rounded-[var(--radius-lg)] shadow-lg flex flex-col overflow-hidden animate-slide-up"
        onClick={(e) => e.stopPropagation()}
      >
        {/* 顶部栏 */}
        <div className="flex items-center px-5 py-3 border-b border-border gap-3 flex-shrink-0">
          <span className="font-semibold text-[14px] flex-1 truncate">{title}</span>
          <div className="flex gap-[6px] items-center">
            {diffData && (
              <button
                className="px-[10px] py-1 rounded-[var(--radius-sm)] text-[11px] font-medium bg-bg-sub text-text-secondary hover:bg-bg-hover transition-all"
                onClick={() => setShowDiff(!showDiff)}
              >
                {showDiff ? t("preview.documentPreview") : t("preview.diffCompare")}
              </button>
            )}
            <button
              className="w-[30px] h-[30px] flex items-center justify-center rounded-[var(--radius-sm)] transition-colors text-text-secondary hover:bg-bg-sub"
              onClick={onClose}
            >
              <Icon name="close" size={18} />
            </button>
          </div>
        </div>

        {/* 内容区 */}
        {showDiff && diffData ? (
          <DiffView oldContent={diffData.oldContent} newContent={diffData.newContent} />
        ) : fileType?.toLowerCase() === "pdf" && pdfBase64Data ? (
          // PDF 真实渲染模式：PdfCanvasViewer 自带滚动和工具栏，不需要外层滚动包裹
          // 必须设置 flex flex-col，否则 PdfCanvasViewer 的 flex-1 不生效，导致高度为0
          <div className="flex-1 overflow-hidden flex flex-col">
            <ContentRenderer content={content} fileType={fileType} pdfBase64Data={pdfBase64Data} />
          </div>
        ) : (
          <div className="flex-1 overflow-y-auto">
            <ContentRenderer content={content} fileType={fileType} pdfBase64Data={pdfBase64Data} />
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * 根据 fileType 选择对应的渲染方式
 */
function ContentRenderer({ content, fileType, pdfBase64Data }: { content: string; fileType?: string; pdfBase64Data?: string | null }) {
  const { t } = useTranslation();
  const normalizedType = fileType?.toLowerCase()?.trim() ?? "";

  // PDF 真实渲染预览：使用 pdfjs-dist Canvas 渲染
  if (normalizedType === "pdf" && pdfBase64Data) {
    return <PdfCanvasViewer base64Data={pdfBase64Data} />;
  }

  // Markdown 渲染
  if (normalizedType === "md" || normalizedType === "markdown") {
    return <MarkdownPreview content={content} />;
  }

  // Excel 表格渲染
  if (normalizedType === "xlsx") {
    return <ExcelTableRenderer content={content} />;
  }

  // Word / PPT / PDF 结构化渲染（PDF 无 base64 数据时降级为文本预览）
  if (normalizedType === "docx" || normalizedType === "pptx" || normalizedType === "pdf") {
    return <DocumentStructureRenderer content={content} fileType={normalizedType} />;
  }

  // 其他格式：纯文本显示
  return (
    <div className="px-10 py-8 leading-[1.8] text-text-secondary text-[14px] whitespace-pre-wrap">
      {content || (
        <div className="flex items-center justify-center h-full text-text-tertiary">
          {t("preview.noContent")}
        </div>
      )}
    </div>
  );
}

/**
 * Markdown 预览组件
 * 使用 react-markdown + remark-gfm + rehype-highlight 渲染
 */
function MarkdownPreview({ content }: { content: string }) {
  const { t } = useTranslation();
  if (!content) {
    return (
      <div className="flex items-center justify-center h-full text-text-tertiary px-10 py-8">
        {t("preview.noContent")}
      </div>
    );
  }

  return (
    <div className="markdown-preview px-10 py-8 leading-[1.8] text-text-secondary text-[14px]">
      <Markdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
        {content}
      </Markdown>
    </div>
  );
}

/**
 * Excel 表格渲染组件
 * 解析 JSON 格式的 Excel 数据并渲染为 HTML 表格
 * 数据格式: { sheets: { Sheet1: { data: [[...], [...]], row_count: N, col_count: M } }, sheet_names: ["Sheet1"] }
 */
function ExcelTableRenderer({ content }: { content: string }) {
  const { t } = useTranslation();
  // 尝试解析 JSON 数据
  const parsed = useMemo(() => {
    if (!content) return null;
    try {
      const data = JSON.parse(content);
      // 校验是否为 Excel 数据格式
      if (data && typeof data === "object" && data.sheets && typeof data.sheets === "object") {
        return data;
      }
      return null;
    } catch {
      return null;
    }
  }, [content]);

  // 解析失败时回退到纯文本显示
  if (!parsed) {
    return (
      <div className="px-10 py-8 leading-[1.8] text-text-secondary text-[14px] whitespace-pre-wrap">
        {content || (
          <div className="flex items-center justify-center h-full text-text-tertiary">
            {t("preview.noContent")}
          </div>
        )}
      </div>
    );
  }

  const sheetNames: string[] = parsed.sheet_names ?? Object.keys(parsed.sheets);

  return (
    <div className="px-6 py-6">
      {/* 工作表标签栏 */}
      {sheetNames.length > 1 && (
        <div className="flex gap-1 mb-4 border-b border-border pb-0">
          {sheetNames.map((name) => (
            <span
              key={name}
              className="px-3 py-1.5 text-[12px] font-medium text-text-secondary bg-bg-sub rounded-t-[var(--radius-sm)] border border-border border-b-0 -mb-px"
            >
              {name}
            </span>
          ))}
        </div>
      )}

      {/* 逐工作表渲染表格 */}
      {sheetNames.map((sheetName) => {
        const sheet = parsed.sheets[sheetName];
        if (!sheet || !Array.isArray(sheet.data)) return null;

        const rows: string[][] = sheet.data;
        if (rows.length === 0) return null;

        // 第一行作为表头
        const headerRow = rows[0];
        const bodyRows = rows.slice(1);

        return (
          <div key={sheetName} className="mb-6">
            {/* 多工作表时显示工作表名称 */}
            {sheetNames.length > 1 && (
              <div className="text-[13px] font-semibold text-text-primary mb-2">
                {sheetName}
                <span className="ml-2 text-[11px] font-normal text-text-tertiary">
                  {t("preview.rowXCol", { rows: sheet.row_count ?? bodyRows.length, cols: sheet.col_count ?? headerRow.length })}
                </span>
              </div>
            )}
            <div className="overflow-x-auto border border-border rounded-[var(--radius-sm)]">
              <table className="w-full border-collapse text-[13px]">
                <thead>
                  <tr className="bg-bg-sub">
                    {headerRow.map((cell, colIdx) => (
                      <th
                        key={colIdx}
                        className="px-3 py-2 text-left font-semibold text-text-primary border-b border-border whitespace-nowrap"
                      >
                        {cell ?? ""}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {bodyRows.map((row, rowIdx) => (
                    <tr
                      key={rowIdx}
                      className={rowIdx % 2 === 1 ? "bg-bg-sub/50" : ""}
                    >
                      {headerRow.map((_, colIdx) => (
                        <td
                          key={colIdx}
                          className="px-3 py-2 text-text-secondary border-b border-border-light whitespace-nowrap"
                        >
                          {row[colIdx] ?? ""}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        );
      })}
    </div>
  );
}

/**
 * 结构化文档渲染组件
 * 用于 Word / PPT / PDF 的 JSON 格式化预览
 */
function DocumentStructureRenderer({ content, fileType }: { content: string; fileType: string }) {
  const { t } = useTranslation();
  // 尝试解析 JSON 数据
  const parsed = useMemo(() => {
    if (!content) return null;
    try {
      const data = JSON.parse(content);
      if (data && typeof data === "object") {
        return data;
      }
      return null;
    } catch {
      return null;
    }
  }, [content]);

  // 解析失败时回退到纯文本显示
  if (!parsed) {
    return (
      <div className="px-10 py-8 leading-[1.8] text-text-secondary text-[14px] whitespace-pre-wrap">
        {content || (
          <div className="flex items-center justify-center h-full text-text-tertiary">
            {t("preview.noContent")}
          </div>
        )}
      </div>
    );
  }

  if (fileType === "docx") {
    return <WordDocumentView data={parsed} />;
  }

  if (fileType === "pptx") {
    return <PptDocumentView data={parsed} />;
  }

  if (fileType === "pdf") {
    return <PdfDocumentView data={parsed} />;
  }

  // 未知结构化格式，回退纯文本
  return (
    <div className="px-10 py-8 leading-[1.8] text-text-secondary text-[14px] whitespace-pre-wrap">
      {content}
    </div>
  );
}

/**
 * Word 文档结构化视图
 * 数据格式: { paragraphs: [{text, style}], tables: [[[...]]], properties: {...} }
 */
function WordDocumentView({ data }: { data: Record<string, unknown> }) {
  const { t } = useTranslation();
  const paragraphs = Array.isArray(data.paragraphs) ? data.paragraphs : [];
  const tables = Array.isArray(data.tables) ? data.tables : [];
  const properties = data.properties && typeof data.properties === "object" ? data.properties as Record<string, unknown> : null;

  return (
    <div className="px-10 py-8">
      {/* 文档属性 */}
      {properties && Object.keys(properties).length > 0 && (
        <div className="mb-6 p-4 bg-bg-sub rounded-[var(--radius-sm)] border border-border-light">
          <div className="text-[12px] font-semibold text-text-primary mb-2">{t("preview.documentProperties")}</div>
          <div className="grid grid-cols-2 gap-x-6 gap-y-1 text-[12px]">
            {Object.entries(properties).map(([key, value]) => (
              <div key={key}>
                <span className="text-text-tertiary">{key}: </span>
                <span className="text-text-secondary">{String(value ?? "")}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 段落内容 */}
      {paragraphs.length > 0 && (
        <div className="space-y-1">
          {paragraphs.map((p: Record<string, unknown>, idx: number) => {
            const text = String(p.text ?? "");
            const style = String(p.style ?? "").toLowerCase();

            // 根据样式渲染标题层级
            if (style.includes("heading 1") || style.includes("title")) {
              return (
                <h1 key={idx} className="text-[20px] font-bold text-text-primary mt-6 mb-2">
                  {text}
                </h1>
              );
            }
            if (style.includes("heading 2")) {
              return (
                <h2 key={idx} className="text-[17px] font-bold text-text-primary mt-5 mb-1.5">
                  {text}
                </h2>
              );
            }
            if (style.includes("heading 3")) {
              return (
                <h3 key={idx} className="text-[15px] font-semibold text-text-primary mt-4 mb-1">
                  {text}
                </h3>
              );
            }
            if (style.includes("heading")) {
              return (
                <h4 key={idx} className="text-[14px] font-semibold text-text-primary mt-3 mb-1">
                  {text}
                </h4>
              );
            }

            // 空段落保留间距
            if (!text.trim()) {
              return <div key={idx} className="h-3" />;
            }

            // 普通段落
            return (
              <p key={idx} className="text-[14px] text-text-secondary leading-[1.8]">
                {text}
              </p>
            );
          })}
        </div>
      )}

      {/* 表格内容 */}
      {tables.length > 0 && (
        <div className="mt-6 space-y-4">
          {tables.map((table: unknown[][], tableIdx: number) => {
            if (!Array.isArray(table) || table.length === 0) return null;
            // 第一行作为表头
            const headerRow = table[0];
            const bodyRows = table.slice(1);
            return (
              <div key={tableIdx} className="overflow-x-auto border border-border rounded-[var(--radius-sm)]">
                <table className="w-full border-collapse text-[13px]">
                  <thead>
                    <tr className="bg-bg-sub">
                      {headerRow.map((cell: unknown, colIdx: number) => (
                        <th key={colIdx} className="px-3 py-2 text-left font-semibold text-text-primary border-b border-border whitespace-nowrap">
                          {String(cell ?? "")}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {bodyRows.map((row: unknown[], rowIdx: number) => (
                      <tr key={rowIdx} className={rowIdx % 2 === 1 ? "bg-bg-sub/50" : ""}>
                        {headerRow.map((_: unknown, colIdx: number) => (
                          <td key={colIdx} className="px-3 py-2 text-text-secondary border-b border-border-light whitespace-nowrap">
                            {String(row[colIdx] ?? "")}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            );
          })}
        </div>
      )}

      {/* 无内容提示 */}
      {paragraphs.length === 0 && tables.length === 0 && !properties && (
        <div className="flex items-center justify-center h-32 text-text-tertiary text-[14px]">
          {t("preview.emptyDocument")}
        </div>
      )}
    </div>
  );
}

/**
 * PPT 文档结构化视图
 * 数据格式: { slides: [{ shapes: [{ name, text }] }], slide_count: N }
 */
function PptDocumentView({ data }: { data: Record<string, unknown> }) {
  const { t } = useTranslation();
  const slides = Array.isArray(data.slides) ? data.slides : [];
  const slideCount = typeof data.slide_count === "number" ? data.slide_count : slides.length;

  return (
    <div className="px-10 py-8">
      {/* 幻灯片统计 */}
      <div className="text-[12px] text-text-tertiary mb-4">
        {t("preview.slideCount", { count: slideCount })}
      </div>

      {/* 逐幻灯片渲染 */}
      <div className="space-y-4">
        {slides.map((slide: Record<string, unknown>, slideIdx: number) => {
          const shapes = Array.isArray(slide.shapes) ? slide.shapes : [];
          return (
            <div
              key={slideIdx}
              className="border border-border rounded-[var(--radius-md)] overflow-hidden"
            >
              {/* 幻灯片标题栏 */}
              <div className="px-4 py-2 bg-bg-sub border-b border-border flex items-center gap-2">
                <span className="w-5 h-5 rounded-full bg-accent/10 text-accent text-[11px] font-semibold flex items-center justify-center">
                  {slideIdx + 1}
                </span>
                <span className="text-[12px] font-medium text-text-primary">
                  {t("preview.slideNumber", { number: slideIdx + 1 })}
                </span>
              </div>

              {/* 幻灯片内容 */}
              <div className="px-5 py-4 space-y-2">
                {shapes.length > 0 ? (
                  shapes.map((shape: Record<string, unknown>, shapeIdx: number) => {
                    const name = String(shape.name ?? "");
                    const text = String(shape.text ?? "");

                    // 无文本的形状跳过
                    if (!text.trim()) return null;

                    // 标题形状特殊样式
                    const isTitle = name.toLowerCase().includes("title");
                    return (
                      <div key={shapeIdx}>
                        {isTitle ? (
                          <div className="text-[16px] font-semibold text-text-primary">
                            {text}
                          </div>
                        ) : (
                          <div className="text-[14px] text-text-secondary leading-[1.8]">
                            {text}
                          </div>
                        )}
                      </div>
                    );
                  })
                ) : (
                  <div className="text-[12px] text-text-tertiary">{t("preview.emptySlide")}</div>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {/* 无内容提示 */}
      {slides.length === 0 && (
        <div className="flex items-center justify-center h-32 text-text-tertiary text-[14px]">
          {t("preview.emptyDocument")}
        </div>
      )}
    </div>
  );
}

/**
 * PDF 文档结构化视图
 * 数据格式: { pages: [{ page_number, text }], page_count: N }
 */
function PdfDocumentView({ data }: { data: Record<string, unknown> }) {
  const { t } = useTranslation();
  const pages = Array.isArray(data.pages) ? data.pages : [];
  const pageCount = typeof data.page_count === "number" ? data.page_count : pages.length;

  return (
    <div className="px-10 py-8">
      {/* 页面统计 */}
      <div className="text-[12px] text-text-tertiary mb-4">
        {t("preview.pageCount", { count: pageCount })}
      </div>

      {/* 逐页渲染 */}
      <div className="space-y-4">
        {pages.map((page: Record<string, unknown>, pageIdx: number) => {
          const pageNumber = typeof page.page_number === "number" ? page.page_number : pageIdx + 1;
          const text = String(page.text ?? "");

          return (
            <div
              key={pageIdx}
              className="border border-border rounded-[var(--radius-md)] overflow-hidden"
            >
              {/* 页码标题栏 */}
              <div className="px-4 py-2 bg-bg-sub border-b border-border flex items-center gap-2">
                <span className="w-5 h-5 rounded-full bg-purple/10 text-purple text-[11px] font-semibold flex items-center justify-center">
                  {pageNumber}
                </span>
                <span className="text-[12px] font-medium text-text-primary">
                  {t("preview.pageNumber", { number: pageNumber })}
                </span>
              </div>

              {/* 页面文本内容 */}
              <div className="px-5 py-4 text-[14px] text-text-secondary leading-[1.8] whitespace-pre-wrap">
                {text.trim() || (
                  <span className="text-text-tertiary">{t("preview.emptyPage")}</span>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {/* 无内容提示 */}
      {pages.length === 0 && (
        <div className="flex items-center justify-center h-32 text-text-tertiary text-[14px]">
          {t("preview.emptyDocument")}
        </div>
      )}
    </div>
  );
}

interface DiffLine {
  type: "added" | "removed" | "unchanged";
  oldLineNum?: number;
  newLineNum?: number;
  content: string;
}

/** 使用 diff 算法计算行级差异，构建 DiffLine 数组 */
function computeDiffLines(oldContent: string, newContent: string): DiffLine[] {
  const changes = Diff.diffLines(oldContent, newContent);
  const result: DiffLine[] = [];
  let oldLineNum = 0;
  let newLineNum = 0;

  for (const change of changes) {
    if (!change.value) continue;
    // 移除末尾换行符后按行拆分
    const lines = change.value.replace(/\n$/, "").split("\n");

    if (!change.added && !change.removed) {
      // 未变化的行，左右两侧行号同步递增
      for (const line of lines) {
        oldLineNum++;
        newLineNum++;
        result.push({ type: "unchanged", oldLineNum, newLineNum, content: line });
      }
    } else if (change.removed) {
      // 删除的行，仅左侧有行号
      for (const line of lines) {
        oldLineNum++;
        result.push({ type: "removed", oldLineNum, content: line });
      }
    } else if (change.added) {
      // 新增的行，仅右侧有行号
      for (const line of lines) {
        newLineNum++;
        result.push({ type: "added", newLineNum, content: line });
      }
    }
  }

  return result;
}

function DiffView({ oldContent, newContent }: { oldContent: string; newContent: string }) {
  const { t } = useTranslation();
  const diffLines = useMemo(() => computeDiffLines(oldContent, newContent), [oldContent, newContent]);
  const leftRef = useRef<HTMLDivElement>(null);
  const rightRef = useRef<HTMLDivElement>(null);
  // 同步滚动锁，防止循环触发
  const syncing = useRef(false);
  // 视图模式：并排对比(side-by-side) 或 内联对比(inline)
  const [viewMode, setViewMode] = useState<"side-by-side" | "inline">("side-by-side");

  // 同步滚动处理，使用 requestAnimationFrame 防止循环触发
  const handleScroll = (source: "left" | "right") => {
    if (syncing.current) return;
    syncing.current = true;
    const sourceEl = source === "left" ? leftRef.current : rightRef.current;
    const targetEl = source === "left" ? rightRef.current : leftRef.current;
    if (sourceEl && targetEl) {
      targetEl.scrollTop = sourceEl.scrollTop;
    }
    requestAnimationFrame(() => { syncing.current = false; });
  };

  // 差异统计
  const stats = useMemo(() => {
    let added = 0;
    let removed = 0;
    for (const line of diffLines) {
      if (line.type === "added") added++;
      if (line.type === "removed") removed++;
    }
    return { added, removed };
  }, [diffLines]);

  return (
    <div className="flex flex-col flex-1 overflow-hidden">
      {/* 差异统计栏 + 视图模式切换 */}
      <div className="flex items-center gap-4 px-5 py-2 border-b border-border bg-bg-sub text-[12px] flex-shrink-0">
        <span className="text-text-secondary">{t("preview.diffStats")}</span>
        <span className="text-success font-medium">+{stats.added} {t("preview.added")}</span>
        <span className="text-error font-medium">-{stats.removed} {t("preview.removed")}</span>
        {/* 视图模式切换按钮 */}
        <div className="ml-auto flex items-center gap-1">
          <button
            className={`diff-mode-btn ${viewMode === "side-by-side" ? "diff-mode-btn-active" : ""}`}
            onClick={() => setViewMode("side-by-side")}
            title={t("preview.sideBySideCompare")}
          >
            {t("preview.sideBySide")}
          </button>
          <button
            className={`diff-mode-btn ${viewMode === "inline" ? "diff-mode-btn-active" : ""}`}
            onClick={() => setViewMode("inline")}
            title={t("preview.inlineCompare")}
          >
            {t("preview.inline")}
          </button>
        </div>
      </div>

      {/* 根据视图模式渲染不同内容 */}
      {viewMode === "inline" ? (
        /* 内联对比视图：所有行在一个面板中显示 */
        <div className="flex-1 overflow-y-auto px-5 py-5 font-mono text-[12px] leading-[1.8]">
          {diffLines.map((line, i) => {
            const isAdded = line.type === "added";
            const isRemoved = line.type === "removed";
            return (
              <div
                key={i}
                className={
                  isAdded ? "bg-success-light text-success" :
                  isRemoved ? "bg-error-light text-error" : ""
                }
              >
                {/* 删除行显示旧行号，新增行显示新行号，未修改行显示行号 */}
                <span className="diff-ln">{line.oldLineNum ?? line.newLineNum ?? ""}</span>
                <span className={`diff-marker ${isAdded ? "diff-marker-added" : ""} ${isRemoved ? "diff-marker-removed" : ""}`}>
                  {isAdded ? "+" : isRemoved ? "-" : " "}
                </span>
                {line.content}
              </div>
            );
          })}
        </div>
      ) : (
      /* 并排对比面板 */
      <div className="flex flex-1 overflow-hidden">
        {/* 左侧：修改前（显示 removed + unchanged 行） */}
        <div
          ref={leftRef}
          className="flex-1 overflow-y-auto px-5 py-5 font-mono text-[12px] leading-[1.8] bg-[var(--color-bg-sub)] border-r border-border"
          onScroll={() => handleScroll("left")}
        >
          <div className="px-3 py-2 bg-bg-sub font-sans font-semibold text-[12px] mb-3 sticky top-0 z-10">{t("preview.before")}</div>
          {diffLines.map((line, i) => {
            // 新增行在左侧显示为空占位，保持与右侧对齐
            if (line.type === "added") {
              return (
                <div key={i} className="diff-line-placeholder">
                  <span className="diff-ln"></span>
                  <span className="diff-marker"> </span>
                </div>
              );
            }
            const isRemoved = line.type === "removed";
            return (
              <div key={i} className={isRemoved ? "bg-error-light text-error" : ""}>
                <span className="diff-ln">{line.oldLineNum ?? ""}</span>
                <span className={`diff-marker ${isRemoved ? "diff-marker-removed" : ""}`}>{isRemoved ? "-" : " "}</span>
                {line.content}
              </div>
            );
          })}
        </div>

        {/* 右侧：修改后（显示 added + unchanged 行） */}
        <div
          ref={rightRef}
          className="flex-1 overflow-y-auto px-5 py-5 font-mono text-[12px] leading-[1.8] bg-bg"
          onScroll={() => handleScroll("right")}
        >
          <div className="px-3 py-2 bg-bg-sub font-sans font-semibold text-[12px] mb-3 sticky top-0 z-10">{t("preview.after")}</div>
          {diffLines.map((line, i) => {
            // 删除行在右侧显示为空占位，保持与左侧对齐
            if (line.type === "removed") {
              return (
                <div key={i} className="diff-line-placeholder">
                  <span className="diff-ln"></span>
                  <span className="diff-marker"> </span>
                </div>
              );
            }
            const isAdded = line.type === "added";
            return (
              <div key={i} className={isAdded ? "bg-success-light text-success" : ""}>
                <span className="diff-ln">{line.newLineNum ?? ""}</span>
                <span className={`diff-marker ${isAdded ? "diff-marker-added" : ""}`}>{isAdded ? "+" : " "}</span>
                {line.content}
              </div>
            );
          })}
        </div>
      </div>
      )}

      <style>{`
        .diff-ln {
          display: inline-block;
          width: 36px;
          color: var(--color-text-tertiary);
          text-align: right;
          margin-right: 12px;
          user-select: none;
        }
        .diff-marker {
          display: inline-block;
          width: 12px;
          user-select: none;
          font-weight: 600;
        }
        .diff-marker-removed {
          color: var(--color-error);
        }
        .diff-marker-added {
          color: var(--color-success);
        }
        .diff-line-placeholder {
          background: var(--color-bg-sub);
          min-height: 1.8em;
        }
        /* 视图模式切换按钮样式 */
        .diff-mode-btn {
          padding: 2px 8px;
          border-radius: var(--radius-sm);
          border: 1px solid var(--color-border);
          background: transparent;
          color: var(--color-text-tertiary);
          cursor: pointer;
          font-size: 11px;
          transition: all 0.15s;
        }
        .diff-mode-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-secondary);
        }
        .diff-mode-btn-active {
          background: var(--color-accent-lighter);
          color: var(--color-accent);
          border-color: var(--color-accent);
        }
      `}</style>
    </div>
  );
}
