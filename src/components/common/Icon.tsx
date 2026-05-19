import type { SVGProps } from "react";

type IconName =
  | "user" | "thinking" | "tool" | "result" | "reply" | "error"
  | "chevron-down" | "chevron-up" | "chevron-left" | "chevron-right"
  | "history" | "plus" | "settings" | "send" | "attach" | "template"
  | "file" | "doc" | "xlsx" | "ppt" | "pdf" | "folder"
  | "search" | "close" | "warning" | "check" | "dot"
  | "code" | "menu" | "minimize" | "maximize" | "unmaximize";

interface IconProps extends SVGProps<SVGSVGElement> {
  name: IconName;
  size?: number;
}

const paths: Record<IconName, React.JSX.Element> = {
  // 用户
  user: (
    <g key="user">
      <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
      <circle cx="12" cy="7" r="4" />
    </g>
  ),
  // 思考
  thinking: (
    <g key="thinking">
      <path d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 1 1 7.072 0l-.548.547A3.374 3.374 0 0 0 14 18.469V19a2 2 0 1 1-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
    </g>
  ),
  // 工具
  tool: (
    <g key="tool">
      <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" />
    </g>
  ),
  // 结果
  result: (
    <g key="result">
      <polyline points="20 6 9 17 4 12" />
    </g>
  ),
  // 回复
  reply: (
    <g key="reply">
      <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
    </g>
  ),
  // 错误
  error: (
    <g key="error">
      <line x1="18" y1="6" x2="6" y2="18" />
      <line x1="6" y1="6" x2="18" y2="18" />
    </g>
  ),
  // 下箭头
  "chevron-down": (
    <g key="chevron-down">
      <path d="M6 9l6 6 6-6" />
    </g>
  ),
  "chevron-up": (
    <g key="chevron-up">
      <path d="M18 15l-6-6-6 6" />
    </g>
  ),
  "chevron-left": (
    <g key="chevron-left">
      <path d="M15 18l-6-6 6-6" />
    </g>
  ),
  "chevron-right": (
    <g key="chevron-right">
      <path d="M9 18l6-6-6-6" />
    </g>
  ),
  // 历史
  history: (
    <g key="history">
      <circle cx="12" cy="12" r="10" />
      <polyline points="12 6 12 12 16 14" />
    </g>
  ),
  // 加号
  plus: (
    <g key="plus">
      <line x1="12" y1="5" x2="12" y2="19" />
      <line x1="5" y1="12" x2="19" y2="12" />
    </g>
  ),
  // 设置
  settings: (
    <g key="settings">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </g>
  ),
  // 发送
  send: (
    <g key="send">
      <line x1="22" y1="2" x2="11" y2="13" />
      <polygon points="22 2 15 22 11 13 2 9 22 2" />
    </g>
  ),
  // 附件
  attach: (
    <g key="attach">
      <path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48" />
    </g>
  ),
  // 模板
  template: (
    <g key="template">
      <rect x="3" y="3" width="18" height="18" rx="2" />
      <path d="M3 9h18" />
      <path d="M9 21V9" />
    </g>
  ),
  // 文件
  file: (
    <g key="file">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <polyline points="14 2 14 8 20 8" />
    </g>
  ),
  // doc
  doc: (
    <g key="doc">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <polyline points="14 2 14 8 20 8" />
      <line x1="16" y1="13" x2="8" y2="13" />
      <line x1="16" y1="17" x2="8" y2="17" />
      <polyline points="10 9 9 9 8 9" />
    </g>
  ),
  // xlsx
  xlsx: (
    <g key="xlsx">
      <rect x="3" y="3" width="18" height="18" rx="2" />
      <line x1="3" y1="9" x2="21" y2="9" />
      <line x1="3" y1="15" x2="21" y2="15" />
      <line x1="9" y1="3" x2="9" y2="21" />
      <line x1="15" y1="3" x2="15" y2="21" />
    </g>
  ),
  // ppt
  ppt: (
    <g key="ppt">
      <rect x="2" y="4" width="20" height="16" rx="2" />
      <line x1="2" y1="10" x2="22" y2="10" />
    </g>
  ),
  // pdf
  pdf: (
    <g key="pdf">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <polyline points="14 2 14 8 20 8" />
      <circle cx="12" cy="15" r="2" />
      <path d="M12 7v4" />
    </g>
  ),
  // 文件夹
  folder: (
    <g key="folder">
      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
    </g>
  ),
  // 搜索
  search: (
    <g key="search">
      <circle cx="11" cy="11" r="8" />
      <line x1="21" y1="21" x2="16.65" y2="16.65" />
    </g>
  ),
  // 关闭
  close: (
    <g key="close">
      <line x1="18" y1="6" x2="6" y2="18" />
      <line x1="6" y1="6" x2="18" y2="18" />
    </g>
  ),
  // 警告
  warning: (
    <g key="warning">
      <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
    </g>
  ),
  // 勾选
  check: (
    <g key="check">
      <polyline points="20 6 9 17 4 12" />
    </g>
  ),
  // 圆点
  dot: (
    <g key="dot">
      <circle cx="12" cy="12" r="2" />
    </g>
  ),
  // 代码
  code: (
    <g key="code">
      <polyline points="16 18 22 12 16 6" />
      <polyline points="8 6 2 12 8 18" />
    </g>
  ),
  // 菜单
  menu: (
    <g key="menu">
      <line x1="4" y1="6" x2="20" y2="6" />
      <line x1="4" y1="12" x2="20" y2="12" />
      <line x1="4" y1="18" x2="20" y2="18" />
    </g>
  ),
  // 最小化
  minimize: (
    <g key="minimize">
      <line x1="5" y1="12" x2="19" y2="12" />
    </g>
  ),
  // 最大化
  maximize: (
    <g key="maximize">
      <rect x="4" y="4" width="16" height="16" rx="2" />
    </g>
  ),
  // 还原（取消最大化）
  unmaximize: (
    <g key="unmaximize">
      <rect x="3" y="7" width="12" height="12" rx="1" />
      <path d="M7 7V5a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-2" />
    </g>
  ),
};

export function Icon({ name, size = 18, className = "", ...props }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={name === "dot" ? 0 : name === "check" || name === "result" ? 2.5 : 2}
      width={size}
      height={size}
      className={className}
      {...props}
    >
      {paths[name]}
    </svg>
  );
}
