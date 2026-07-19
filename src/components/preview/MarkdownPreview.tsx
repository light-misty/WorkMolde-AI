import { useState, useRef, useCallback } from "react";
import { useTranslation } from 'react-i18next';
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";

interface MarkdownPreviewProps {
  content: string;
  className?: string;
}

// react-markdown 传递给自定义组件的额外属性
type MdExtraProps = { node?: unknown; siblingCount?: number };

// 从 className 中提取语言标识
function extractLanguage(className: string | undefined): string {
  if (!className) return "";
  const match = className.match(/language-(\S+)/);
  return match ? match[1] : "";
}

// 代码块组件（带复制按钮和语言标签）
function CodeBlock({
  children,
  node: _node,
  siblingCount: _sc,
  ...rest
}: React.HTMLAttributes<HTMLPreElement> & MdExtraProps) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  const ref = useRef<HTMLPreElement>(null);

  // 从子 code 元素的 className 中提取语言
  let language = "";
  const childElements = Array.isArray(children) ? children : children ? [children] : [];
  for (const child of childElements) {
    if (child && typeof child === "object" && "props" in child) {
      language = extractLanguage(
        (child.props as { className?: string }).className
      );
      if (language) break;
    }
  }

  const handleCopy = useCallback(async () => {
    const text = ref.current?.textContent || "";
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, []);

  return (
    <div className="md-code-block">
      <div className="md-code-header">
        <span className="md-code-lang">{language || "text"}</span>
        <button className="md-code-copy" onClick={handleCopy}>
          {copied ? t('common.copied') : t('common.copy')}
        </button>
      </div>
      <pre ref={ref} className="md-code-content" {...rest}>
        {children}
      </pre>
    </div>
  );
}

export function MarkdownPreview({
  content,
  className = "",
}: MarkdownPreviewProps) {
  return (
    <>
      <div className={`markdown-preview ${className}`}>
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          rehypePlugins={[rehypeHighlight]}
          components={{
            // 代码块：包装为带标题栏的容器
            pre: CodeBlock,

            // 表格：添加外层滚动容器和样式类
            table: ({
              node: _node,
              siblingCount: _sc,
              children,
              ...rest
            }: React.TableHTMLAttributes<HTMLTableElement> & MdExtraProps) => (
              <div className="md-table-wrap">
                <table className="md-table" {...rest}>
                  {children}
                </table>
              </div>
            ),

            // 链接：在新窗口打开
            a: ({
              node: _node,
              siblingCount: _sc,
              href,
              children,
              ...rest
            }: React.AnchorHTMLAttributes<HTMLAnchorElement> & MdExtraProps) => (
              <a
                href={href}
                target="_blank"
                rel="noopener noreferrer"
                {...rest}
              >
                {children}
              </a>
            ),

            // 图片：限制最大宽度
            img: ({
              node: _node,
              siblingCount: _sc,
              ...rest
            }: React.ImgHTMLAttributes<HTMLImageElement> & MdExtraProps) => (
              <img className="md-image" {...rest} />
            ),
          }}
        >
          {content}
        </ReactMarkdown>
      </div>
      <style>{markdownStyles}</style>
    </>
  );
}

// Markdown 预览样式
const markdownStyles = `
/* ===== 基础排版 ===== */
.markdown-preview {
  color: var(--color-text-secondary);
  font-size: 14px;
  line-height: 1.8;
}

/* 标题 */
.markdown-preview h1,
.markdown-preview h2,
.markdown-preview h3,
.markdown-preview h4,
.markdown-preview h5,
.markdown-preview h6 {
  color: var(--color-text-primary);
  font-weight: 600;
  margin-top: 1.4em;
  margin-bottom: 0.6em;
  line-height: 1.4;
}

.markdown-preview h1 {
  font-size: 1.75em;
  padding-bottom: 0.3em;
  border-bottom: 1px solid var(--color-border);
}

.markdown-preview h2 {
  font-size: 1.4em;
  padding-bottom: 0.25em;
  border-bottom: 1px solid var(--color-border-light);
}

.markdown-preview h3 { font-size: 1.2em; }
.markdown-preview h4 { font-size: 1.05em; }
.markdown-preview h5 { font-size: 1em; }
.markdown-preview h6 { font-size: 0.95em; color: var(--color-text-secondary); }

.markdown-preview h1:first-child,
.markdown-preview h2:first-child,
.markdown-preview h3:first-child {
  margin-top: 0;
}

/* 段落 */
.markdown-preview p {
  margin-bottom: 0.8em;
}

.markdown-preview p:last-child {
  margin-bottom: 0;
}

/* 列表 */
.markdown-preview ul,
.markdown-preview ol {
  padding-left: 1.8em;
  margin-bottom: 0.8em;
}

.markdown-preview ul {
  list-style-type: disc;
}

.markdown-preview ol {
  list-style-type: decimal;
}

.markdown-preview li {
  margin-bottom: 0.25em;
}

.markdown-preview li > ul,
.markdown-preview li > ol {
  margin-bottom: 0;
  margin-top: 0.25em;
}

/* GFM 任务列表 */
.markdown-preview ul:has(> li > input[type="checkbox"]) {
  list-style: none;
  padding-left: 0.5em;
}

.markdown-preview input[type="checkbox"] {
  margin-right: 0.5em;
  vertical-align: middle;
  accent-color: var(--color-accent);
}

/* 引用 */
.markdown-preview blockquote {
  border-left: 3px solid var(--color-accent);
  padding: 0.4em 1em;
  margin: 0.8em 0;
  color: var(--color-text-tertiary);
  background: var(--color-bg-sub);
  border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
}

.markdown-preview blockquote p:last-child {
  margin-bottom: 0;
}

/* 行内代码 */
.markdown-preview code {
  font-family: var(--font-mono);
  font-size: 0.875em;
  padding: 2px 6px;
  background: var(--color-bg-sub);
  border-radius: 3px;
  color: var(--color-accent);
}

/* ===== 代码块 ===== */
.md-code-block {
  margin: 0.8em 0;
  border-radius: var(--radius-md);
  overflow: hidden;
  border: 1px solid var(--color-border);
}

.md-code-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 12px;
  background: var(--color-bg-sub);
  border-bottom: 1px solid var(--color-border);
}

.md-code-lang {
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--color-text-tertiary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.md-code-copy {
  font-size: 11px;
  color: var(--color-text-tertiary);
  padding: 2px 8px;
  border-radius: 3px;
  transition: all 0.15s;
}

.md-code-copy:hover {
  color: var(--color-text-secondary);
  background: var(--color-bg-hover);
}

.md-code-content {
  margin: 0 !important;
  padding: 14px 16px !important;
  background: var(--color-bg-elevated) !important;
  overflow-x: auto;
  font-family: var(--font-mono);
  font-size: 13px;
  line-height: 1.6;
  color: var(--color-text-secondary);
}

/* 代码块中的 code 覆盖行内代码样式 */
.md-code-content code {
  font-family: var(--font-mono);
  font-size: 13px;
  background: none !important;
  padding: 0 !important;
  border-radius: 0 !important;
  color: inherit;
}

/* ===== 表格 ===== */
.md-table-wrap {
  overflow-x: auto;
  margin: 0.8em 0;
}

.md-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.md-table th {
  background: var(--color-bg-sub);
  font-weight: 600;
  text-align: left;
  padding: 8px 12px;
  border: 1px solid var(--color-border);
  color: var(--color-text-primary);
}

.md-table td {
  padding: 8px 12px;
  border: 1px solid var(--color-border);
}

.md-table tr:nth-child(even) td {
  background: var(--color-bg-sub);
}

/* ===== 其他元素 ===== */

/* 链接 */
.markdown-preview a {
  color: var(--color-accent);
  text-decoration: none;
}

.markdown-preview a:hover {
  text-decoration: underline;
}

/* 图片 */
.md-image {
  max-width: 100%;
  height: auto;
  border-radius: var(--radius-sm);
  margin: 0.5em 0;
}

/* 分隔线 */
.markdown-preview hr {
  border: none;
  border-top: 1px solid var(--color-border);
  margin: 1.5em 0;
}

/* 删除线 */
.markdown-preview del {
  color: var(--color-text-tertiary);
}

/* ===== highlight.js 语法高亮 - 深色主题 ===== */
.md-code-content .hljs-keyword,
.md-code-content .hljs-selector-tag,
.md-code-content .hljs-literal { color: #569cd6; }

.md-code-content .hljs-string,
.md-code-content .hljs-doctag,
.md-code-content .hljs-template-tag,
.md-code-content .hljs-template-variable { color: #ce9178; }

.md-code-content .hljs-number,
.md-code-content .hljs-built_in { color: #b5cea8; }

.md-code-content .hljs-comment,
.md-code-content .hljs-quote { color: #6a9955; font-style: italic; }

.md-code-content .hljs-function .hljs-title,
.md-code-content .hljs-title.function_ { color: #dcdcaa; }

.md-code-content .hljs-class .hljs-title,
.md-code-content .hljs-title.class_ { color: #4ec9b0; }

.md-code-content .hljs-variable,
.md-code-content .hljs-attr { color: #9cdcfe; }

.md-code-content .hljs-type,
.md-code-content .hljs-params { color: #4ec9b0; }

.md-code-content .hljs-meta { color: #569cd6; }

.md-code-content .hljs-tag { color: #569cd6; }

.md-code-content .hljs-name { color: #569cd6; }

.md-code-content .hljs-attribute { color: #9cdcfe; }

.md-code-content .hljs-symbol,
.md-code-content .hljs-bullet { color: #d7ba7d; }

.md-code-content .hljs-addition { color: #b5cea8; background: rgba(181, 206, 168, 0.1); }

.md-code-content .hljs-deletion { color: #ce9178; background: rgba(206, 145, 120, 0.1); }

.md-code-content .hljs-emphasis { font-style: italic; }

.md-code-content .hljs-strong { font-weight: 600; }

.md-code-content .hljs-regexp { color: #d16969; }

.md-code-content .hljs-property { color: #9cdcfe; }

.md-code-content .hljs-section { color: #4ec9b0; }
`;
