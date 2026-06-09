import { useEffect, useRef, useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "../common/Icon";
import * as pdfjsLib from "pdfjs-dist";

// 配置 pdfjs-dist Worker：使用 Vite 的 new URL() 语法引入 worker 文件
pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
  "pdfjs-dist/build/pdf.worker.min.mjs",
  import.meta.url
).toString();

interface PdfCanvasViewerProps {
  base64Data: string;
}

// 缩放级别选项
const SCALE_STEP = 0.25;
const SCALE_MIN = 0.5;
const SCALE_MAX = 3.0;

export function PdfCanvasViewer({ base64Data }: PdfCanvasViewerProps) {
  const { t } = useTranslation();
  const containerRef = useRef<HTMLDivElement>(null);
  const scrollAreaRef = useRef<HTMLDivElement>(null);
  const pdfDocRef = useRef<pdfjsLib.PDFDocumentProxy | null>(null);
  const [totalPages, setTotalPages] = useState(0);
  const [scale, setScale] = useState(1.2);
  const [scaleMode, setScaleMode] = useState<"custom" | "fitWidth" | "fitPage">("custom");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [currentPage, setCurrentPage] = useState(1);
  // 记录已渲染的页面，避免重复渲染
  const renderedPagesRef = useRef<Set<number>>(new Set());
  // 渲染任务引用，用于取消进行中的渲染
  const renderTasksRef = useRef<Map<number, pdfjsLib.RenderTask>>(new Map());
  // 页面 canvas 引用
  const pageCanvasRefs = useRef<Map<number, HTMLCanvasElement>>(new Map());
  // 页面容器引用（用于 IntersectionObserver）
  const pageContainerRefs = useRef<Map<number, HTMLDivElement>>(new Map());
  // 观察器引用
  const observerRef = useRef<IntersectionObserver | null>(null);

  // 渲染单页 PDF 到 canvas
  const renderPage = useCallback(async (pageNum: number) => {
    const pdf = pdfDocRef.current;
    if (!pdf) return;

    const canvas = pageCanvasRefs.current.get(pageNum);
    if (!canvas) return;

    // 如果该页已有渲染任务，先取消
    const existingTask = renderTasksRef.current.get(pageNum);
    if (existingTask) {
      try { existingTask.cancel(); } catch { /* 忽略 */ }
    }

    try {
      const page = await pdf.getPage(pageNum);
      const viewport = page.getViewport({ scale });

      // 设置 canvas 尺寸：考虑设备像素比以确保高清渲染
      const outputScale = window.devicePixelRatio || 1;
      canvas.width = Math.floor(viewport.width * outputScale);
      canvas.height = Math.floor(viewport.height * outputScale);
      canvas.style.width = `${Math.floor(viewport.width)}px`;
      canvas.style.height = `${Math.floor(viewport.height)}px`;

      // v4 API：必须传 canvasContext，手动处理 devicePixelRatio 缩放
      const context = canvas.getContext("2d");
      if (!context) return;

      // 先重置变换矩阵，再应用设备像素比缩放
      context.setTransform(outputScale, 0, 0, outputScale, 0, 0);

      const renderTask = page.render({
        canvasContext: context,
        viewport,
      });

      renderTasksRef.current.set(pageNum, renderTask);

      await renderTask.promise;
      renderedPagesRef.current.add(pageNum);
      renderTasksRef.current.delete(pageNum);
    } catch (err: unknown) {
      // 取消渲染不算错误
      if (err instanceof Error && err.name === "RenderingCancelledException") {
        return;
      }
      console.error(`[PdfCanvasViewer] 渲染第 ${pageNum} 页失败:`, err);
    }
  }, [scale]);

  // 加载 PDF 文档
  useEffect(() => {
    let cancelled = false;

    const loadPdf = async () => {
      try {
        setLoading(true);
        setError(null);
        renderedPagesRef.current.clear();
        renderTasksRef.current.clear();
        pageCanvasRefs.current.clear();
        pageContainerRefs.current.clear();

        // base64 字符串 -> Uint8Array
        const binaryString = atob(base64Data);
        const bytes = new Uint8Array(binaryString.length);
        for (let i = 0; i < binaryString.length; i++) {
          bytes[i] = binaryString.charCodeAt(i);
        }

        const pdf = await pdfjsLib.getDocument({ data: bytes }).promise;
        if (cancelled) {
          pdf.destroy();
          return;
        }

        pdfDocRef.current = pdf;
        setTotalPages(pdf.numPages);
        setCurrentPage(1);
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : t("preview.pdfLoadFailed"));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };

    loadPdf();

    return () => {
      cancelled = true;
      // 取消所有进行中的渲染任务
      renderTasksRef.current.forEach((task) => {
        try { task.cancel(); } catch { /* 忽略取消错误 */ }
      });
      pdfDocRef.current?.destroy();
      pdfDocRef.current = null;
    };
  }, [base64Data, t]);

  // PDF 加载完成后，渲染可见页面
  // 使用 IntersectionObserver 实现懒渲染 + 直接渲染兜底
  useEffect(() => {
    if (!pdfDocRef.current || loading) return;

    // 清空已渲染记录（缩放变化时需要重新渲染）
    renderedPagesRef.current.clear();

    // 取消所有进行中的渲染任务
    renderTasksRef.current.forEach((task) => {
      try { task.cancel(); } catch { /* 忽略 */ }
    });
    renderTasksRef.current.clear();

    const scrollArea = scrollAreaRef.current;
    if (!scrollArea) return;

    // 创建 IntersectionObserver，检测页面是否进入可视区域
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          const pageNum = Number(entry.target.getAttribute("data-page-num"));
          if (!pageNum) return;

          if (entry.isIntersecting) {
            renderPage(pageNum);
          }
        });
      },
      {
        root: scrollArea,
        rootMargin: "300px 0px",
        threshold: 0,
      }
    );

    observerRef.current = observer;

    // 观察所有页面容器
    pageContainerRefs.current.forEach((el) => {
      observer.observe(el);
    });

    // 兜底：延迟后直接渲染前几页，防止 IntersectionObserver 未触发
    const fallbackTimer = setTimeout(() => {
      const pagesToRender = Math.min(totalPages, 3);
      for (let i = 1; i <= pagesToRender; i++) {
        if (!renderedPagesRef.current.has(i)) {
          renderPage(i);
        }
      }
    }, 500);

    return () => {
      clearTimeout(fallbackTimer);
      observer.disconnect();
      observerRef.current = null;
    };
  }, [totalPages, scale, loading, renderPage]);

  // 监听滚动位置，更新当前页码
  useEffect(() => {
    const scrollArea = scrollAreaRef.current;
    if (!scrollArea || !pdfDocRef.current) return;

    const handleScroll = () => {
      const scrollTop = scrollArea.scrollTop;
      const containerTop = scrollArea.getBoundingClientRect().top;
      const centerY = scrollTop + containerTop + scrollArea.clientHeight / 3;

      // 找到位于视口中心位置的页面
      let foundPage = 1;
      pageContainerRefs.current.forEach((el, pageNum) => {
        const rect = el.getBoundingClientRect();
        const elTop = rect.top + scrollTop - containerTop;
        const elBottom = elTop + rect.height;
        if (centerY >= elTop && centerY <= elBottom) {
          foundPage = pageNum;
        }
      });

      setCurrentPage(foundPage);
    };

    scrollArea.addEventListener("scroll", handleScroll, { passive: true });
    return () => scrollArea.removeEventListener("scroll", handleScroll);
  }, [loading]);

  // 缩放控制
  const handleZoomIn = useCallback(() => {
    setScaleMode("custom");
    setScale((s) => Math.min(s + SCALE_STEP, SCALE_MAX));
  }, []);

  const handleZoomOut = useCallback(() => {
    setScaleMode("custom");
    setScale((s) => Math.max(s - SCALE_STEP, SCALE_MIN));
  }, []);

  // 适合宽度：根据容器宽度计算缩放比例
  const handleFitWidth = useCallback(async () => {
    const pdf = pdfDocRef.current;
    const scrollArea = scrollAreaRef.current;
    if (!pdf || !scrollArea) return;

    try {
      const page = await pdf.getPage(1);
      const viewport = page.getViewport({ scale: 1.0 });
      const containerWidth = scrollArea.clientWidth - 48;
      const newScale = containerWidth / viewport.width;
      setScale(Math.max(SCALE_MIN, Math.min(newScale, SCALE_MAX)));
      setScaleMode("fitWidth");
    } catch { /* 忽略 */ }
  }, []);

  // 适合整页：根据容器高度计算缩放比例
  const handleFitPage = useCallback(async () => {
    const pdf = pdfDocRef.current;
    const scrollArea = scrollAreaRef.current;
    if (!pdf || !scrollArea) return;

    try {
      const page = await pdf.getPage(1);
      const viewport = page.getViewport({ scale: 1.0 });
      const containerWidth = scrollArea.clientWidth - 48;
      const containerHeight = scrollArea.clientHeight - 48;
      const scaleW = containerWidth / viewport.width;
      const scaleH = containerHeight / viewport.height;
      const newScale = Math.min(scaleW, scaleH);
      setScale(Math.max(SCALE_MIN, Math.min(newScale, SCALE_MAX)));
      setScaleMode("fitPage");
    } catch { /* 忽略 */ }
  }, []);

  // 跳转到指定页码
  const handleGoToPage = useCallback((pageNum: number) => {
    const el = pageContainerRefs.current.get(pageNum);
    if (el && scrollAreaRef.current) {
      el.scrollIntoView({ behavior: "smooth", block: "start" });
      setCurrentPage(pageNum);
    }
  }, []);

  // 上一页
  const handlePrevPage = useCallback(() => {
    if (currentPage > 1) {
      handleGoToPage(currentPage - 1);
    }
  }, [currentPage, handleGoToPage]);

  // 下一页
  const handleNextPage = useCallback(() => {
    if (currentPage < totalPages) {
      handleGoToPage(currentPage + 1);
    }
  }, [currentPage, totalPages, handleGoToPage]);

  // 注册页面 canvas 和容器引用
  const registerPageCanvas = useCallback((pageNum: number, canvas: HTMLCanvasElement | null) => {
    if (canvas) {
      pageCanvasRefs.current.set(pageNum, canvas);
      // 如果 observer 已存在，且该页尚未被观察，立即观察
      if (observerRef.current) {
        const container = pageContainerRefs.current.get(pageNum);
        if (container) {
          observerRef.current.observe(container);
        }
      }
    } else {
      pageCanvasRefs.current.delete(pageNum);
    }
  }, []);

  const registerPageContainer = useCallback((pageNum: number, el: HTMLDivElement | null) => {
    if (el) {
      pageContainerRefs.current.set(pageNum, el);
    } else {
      pageContainerRefs.current.delete(pageNum);
    }
  }, []);

  // 加载状态
  if (loading) {
    return (
      <div className="flex items-center justify-center flex-1 min-h-0 text-text-tertiary text-[14px]">
        <svg className="animate-spin w-5 h-5 mr-2" viewBox="0 0 24 24" fill="none">
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
        </svg>
        {t("preview.loadingPdf")}
      </div>
    );
  }

  // 错误状态
  if (error) {
    return (
      <div className="flex flex-col items-center justify-center flex-1 min-h-0 text-text-tertiary text-[14px] gap-3">
        <div className="w-12 h-12 rounded-[var(--radius-md)] bg-error/10 flex items-center justify-center text-error text-[20px]">
          !
        </div>
        <div>{t("preview.pdfLoadFailed")}</div>
        <div className="text-[12px] text-text-tertiary max-w-[400px] text-center">{error}</div>
      </div>
    );
  }

  return (
    <div className="flex flex-col flex-1 min-h-0">
      {/* 工具栏 */}
      <div className="flex items-center gap-2 px-5 py-2 border-b border-border bg-bg-sub flex-shrink-0">
        {/* 页码导航 */}
        <button
          className="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] text-text-secondary hover:bg-bg-hover transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
          onClick={handlePrevPage}
          disabled={currentPage <= 1}
        >
          <Icon name="chevron-left" size={14} />
        </button>
        <span className="text-[12px] text-text-secondary tabular-nums min-w-[60px] text-center">
          {currentPage} / {totalPages}
        </span>
        <button
          className="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] text-text-secondary hover:bg-bg-hover transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
          onClick={handleNextPage}
          disabled={currentPage >= totalPages}
        >
          <Icon name="chevron-right" size={14} />
        </button>

        <div className="w-px h-4 bg-border mx-1" />

        {/* 缩放控制 */}
        <button
          className="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] text-text-secondary hover:bg-bg-hover transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
          onClick={handleZoomOut}
          disabled={scale <= SCALE_MIN}
        >
          <Icon name="minus" size={14} />
        </button>
        <span className="text-[12px] text-text-secondary tabular-nums min-w-[40px] text-center">
          {Math.round(scale * 100)}%
        </span>
        <button
          className="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] text-text-secondary hover:bg-bg-hover transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
          onClick={handleZoomIn}
          disabled={scale >= SCALE_MAX}
        >
          <Icon name="plus" size={14} />
        </button>

        <div className="w-px h-4 bg-border mx-1" />

        {/* 适应模式 */}
        <button
          className={`px-2 h-7 flex items-center justify-center rounded-[var(--radius-sm)] text-[11px] font-medium transition-colors ${scaleMode === "fitWidth" ? "bg-accent/10 text-accent" : "text-text-secondary hover:bg-bg-hover"}`}
          onClick={handleFitWidth}
        >
          {t("preview.fitWidth")}
        </button>
        <button
          className={`px-2 h-7 flex items-center justify-center rounded-[var(--radius-sm)] text-[11px] font-medium transition-colors ${scaleMode === "fitPage" ? "bg-accent/10 text-accent" : "text-text-secondary hover:bg-bg-hover"}`}
          onClick={handleFitPage}
        >
          {t("preview.fitPage")}
        </button>
      </div>

      {/* PDF 内容区 */}
      <div
        ref={scrollAreaRef}
        className="flex-1 overflow-y-auto bg-bg-sub min-h-0"
      >
        <div ref={containerRef} className="py-4 px-6 flex flex-col items-center gap-4">
          {totalPages > 0 && Array.from({ length: totalPages }, (_, i) => {
            const pageNum = i + 1;
            return (
              <div
                key={pageNum}
                ref={(el) => registerPageContainer(pageNum, el)}
                data-page-num={pageNum}
                className="pdf-page-wrapper bg-white shadow-md rounded-[var(--radius-sm)] overflow-hidden"
              >
                <canvas
                  ref={(el) => registerPageCanvas(pageNum, el)}
                />
              </div>
            );
          })}
        </div>
      </div>

      <style>{`
        .pdf-page-wrapper {
          line-height: 0;
        }
        .pdf-page-wrapper canvas {
          display: block;
        }
      `}</style>
    </div>
  );
}
