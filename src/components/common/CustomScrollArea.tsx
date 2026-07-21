import { useRef, useState, useEffect, useCallback, type ReactNode, type UIEvent } from 'react';

const THUMB_HEIGHT = 40;

interface CustomScrollAreaProps {
  children: ReactNode;
  className?: string;
  onScroll?: (e: UIEvent<HTMLDivElement>) => void;
  /** 外部传入的 ref，指向内部可滚动容器 */
  scrollRef?: React.RefObject<HTMLDivElement | null>;
  /** 传递给内部可滚动容器的额外属性 */
  contentAttrs?: React.HTMLAttributes<HTMLDivElement>;
}

export function CustomScrollArea({
  children,
  className = '',
  onScroll,
  scrollRef: externalRef,
  contentAttrs,
}: CustomScrollAreaProps) {
  const internalRef = useRef<HTMLDivElement>(null);
  const scrollRef = externalRef ?? internalRef;
  const [showScrollbar, setShowScrollbar] = useState(false);
  const [scrollProgress, setScrollProgress] = useState(0);
  const [clientHeight, setClientHeight] = useState(0);
  const [isDragging, setIsDragging] = useState(false);
  const isDraggingRef = useRef(false);
  const dragStartYRef = useRef(0);
  const dragStartScrollTopRef = useRef(0);

  const updateScrollbar = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    const { scrollTop, scrollHeight, clientHeight: ch } = el;
    const maxScroll = scrollHeight - ch;
    setClientHeight(ch);
    setShowScrollbar(scrollHeight > ch);
    setScrollProgress(maxScroll > 0 ? scrollTop / maxScroll : 0);
  }, [scrollRef]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    updateScrollbar();
    const observer = new ResizeObserver(() => updateScrollbar());
    observer.observe(el);
    observer.observe(el.firstElementChild as Element);
    return () => observer.disconnect();
  }, [updateScrollbar, scrollRef]);

  const handleScroll = useCallback((e: UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    const { scrollTop, scrollHeight, clientHeight } = el;
    const maxScroll = scrollHeight - clientHeight;
    setShowScrollbar(scrollHeight > clientHeight);
    setScrollProgress(maxScroll > 0 ? scrollTop / maxScroll : 0);
    onScroll?.(e);
  }, [onScroll]);

  const handleThumbMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const el = scrollRef.current;
    if (!el) return;
    isDraggingRef.current = true;
    dragStartYRef.current = e.clientY;
    dragStartScrollTopRef.current = el.scrollTop;
    setIsDragging(true);
  }, [scrollRef]);

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      if (!isDraggingRef.current) return;
      const el = scrollRef.current;
      if (!el) return;
      const { clientHeight, scrollHeight } = el;
      const deltaY = e.clientY - dragStartYRef.current;
      const trackHeight = clientHeight - THUMB_HEIGHT;
      const ratioDelta = trackHeight > 0 ? deltaY / trackHeight : 0;
      const maxScroll = scrollHeight - clientHeight;
      el.scrollTop = Math.max(0, Math.min(maxScroll, dragStartScrollTopRef.current + ratioDelta * maxScroll));
    };

    const handleMouseUp = () => {
      isDraggingRef.current = false;
      setIsDragging(false);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging, scrollRef]);

  const thumbTop = clientHeight > THUMB_HEIGHT ? scrollProgress * (clientHeight - THUMB_HEIGHT) : 0;

  return (
    <div className={`custom-scroll-area ${className}`}>
      <div
        ref={scrollRef}
        className="custom-scroll-area-content"
        onScroll={handleScroll}
        {...contentAttrs}
      >
        {children}
      </div>
      {showScrollbar && (
        <div className="custom-scroll-track">
          <div
            className={`custom-scroll-thumb${isDragging ? ' dragging' : ''}`}
            style={{
              height: THUMB_HEIGHT,
              transform: `translateY(${thumbTop}px)`,
            }}
            onMouseDown={handleThumbMouseDown}
          />
        </div>
      )}
    </div>
  );
}
