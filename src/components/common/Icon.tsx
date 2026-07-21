import type { SVGProps } from "react";

export type IconName =
  | "user" | "thinking" | "tool" | "error"
  | "chevron-down" | "chevron-up" | "chevron-left" | "chevron-right"
  | "chevron-diagonal-out" | "chevron-diagonal-in"
  | "history" | "plus" | "minus" | "settings" | "send" | "attach" | "template"
  | "file" | "doc" | "xlsx" | "ppt" | "pdf" | "folder"
  | "search" | "close" | "warning" | "check" | "check-circle" | "dot"
  | "code" | "menu" | "minimize" | "maximize" | "unmaximize"
  | "refresh" | "edit" | "trash" | "stop" | "back"
  | "copy" | "eye" | "folder-plus" | "file-plus" | "external-link"
  | "chart" | "clock" | "git-compare" | "git-branch" | "undo"
  | "theme" | "moon" | "keyboard" | "info" | "image" | "book" | "more-vertical"
  // 空会话标题专用：规划模式清单图标、构建模式双尖括号图标
  | "plan-mode" | "code-brackets"
  // GitHub logo
  | "github";

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
  // 斜对角双向直角(朝外): 右上角直角顶点在右上 + 左下角直角顶点在左下, 表示可收缩
  // 两个 L 形分别紧贴右上角和左下角, 中间留出对角空白带, 避免构成矩形视觉
  "chevron-diagonal-out": (
    <g key="chevron-diagonal-out">
      {/* 右上角 L: 拐角在右上 (19,4), 仅占据右上角 6x6 区域 */}
      <path d="M13 4 L19 4 L19 10" />
      {/* 左下角 L: 拐角在左下 (5,20), 仅占据左下角 6x6 区域 */}
      <path d="M11 20 L5 20 L5 14" />
    </g>
  ),
  // 斜对角双向直角(朝内): 右上角直角顶点在左下 + 左下角直角顶点在右上, 表示可展开
  // 两个 L 形分别紧贴右上角和左下角, 中间留出对角空白带, 避免重叠
  "chevron-diagonal-in": (
    <g key="chevron-diagonal-in">
      {/* 右上角 L: 拐角在右上 (19,4), 水平向左再垂直向上, 仅占据右上角 6x6 区域 */}
      <path d="M19 10 L13 10 L13 4" />
      {/* 左下角 L: 拐角在左下 (5,20), 水平向右再垂直向下, 仅占据左下角 6x6 区域 */}
      <path d="M5 14 L11 14 L11 20" />
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
  // 减号
  minus: (
    <g key="minus">
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
      <line x1="12" y1="20" x2="12" y2="4" />
      <polyline points="6 10 12 4 18 10" />
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
      <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
      <line x1="12" y1="8" x2="12" y2="14" />
      <circle cx="12" cy="17" r="1.2" fill="currentColor" stroke="none" />
    </g>
  ),
  // 勾选
  check: (
    <g key="check">
      <polyline points="20 6 9 17 4 12" />
    </g>
  ),
  // 勾选圆圈
  "check-circle": (
    <g key="check-circle">
      <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
      <polyline points="22 4 12 14.01 9 11.01" />
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
  // 刷新
  refresh: (
    <g key="refresh">
      <polyline points="23 4 23 10 17 10" />
      <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" />
    </g>
  ),
  // 编辑
  edit: (
    <g key="edit">
      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
    </g>
  ),
  // 删除
  trash: (
    <g key="trash">
      <polyline points="3 6 5 6 21 6" />
      <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
    </g>
  ),
  // 停止
  stop: (
    <g key="stop">
      <rect x="6" y="6" width="12" height="12" rx="1" />
    </g>
  ),
  // 返回
  back: (
    <g key="back">
      <line x1="19" y1="12" x2="5" y2="12" />
      <polyline points="12 19 5 12 12 5" />
    </g>
  ),
  // 复制
  copy: (
    <g key="copy">
      <rect x="9" y="9" width="13" height="13" rx="2" />
      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
    </g>
  ),
  // 预览/查看
  eye: (
    <g key="eye">
      <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
      <circle cx="12" cy="12" r="3" />
    </g>
  ),
  // 新建文件夹
  "folder-plus": (
    <g key="folder-plus">
      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
      <line x1="12" y1="11" x2="12" y2="17" />
      <line x1="9" y1="14" x2="15" y2="14" />
    </g>
  ),
  // 新建文件
  "file-plus": (
    <g key="file-plus">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <polyline points="14 2 14 8 20 8" />
      <line x1="12" y1="18" x2="12" y2="12" />
      <line x1="9" y1="15" x2="15" y2="15" />
    </g>
  ),
  // 外部链接/在资源管理器中显示
  "external-link": (
    <g key="external-link">
      <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
      <polyline points="15 3 21 3 21 9" />
      <line x1="10" y1="14" x2="21" y2="3" />
    </g>
  ),
  // 图表/统计
  chart: (
    <g key="chart">
      <line x1="18" y1="20" x2="18" y2="10" />
      <line x1="12" y1="20" x2="12" y2="4" />
      <line x1="6" y1="20" x2="6" y2="14" />
    </g>
  ),
  // 时钟
  clock: (
    <g key="clock">
      <circle cx="12" cy="12" r="10" />
      <polyline points="12 6 12 12 16 14" />
    </g>
  ),
  // 版本对比
  "git-compare": (
    <g key="git-compare">
      <circle cx="18" cy="18" r="3" />
      <circle cx="6" cy="6" r="3" />
      <path d="M13 6h3a2 2 0 0 1 2 2v7" />
      <path d="M11 18H8a2 2 0 0 1-2-2V9" />
    </g>
  ),
  // 分支图标（用于分支组指示）
  "git-branch": (
    <g key="git-branch">
      <line x1="6" y1="3" x2="6" y2="15" />
      <circle cx="18" cy="6" r="3" />
      <circle cx="6" cy="18" r="3" />
      <path d="M18 9a9 9 0 0 1-9 9" />
    </g>
  ),
  // 撤销/回滚
  undo: (
    <g key="undo">
      <polyline points="1 4 1 10 7 10" />
      <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />
    </g>
  ),
  // 月亮/夜间模式
  moon: (
    <g key="moon">
      <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
    </g>
  ),
  // 主题/外观
  theme: (
    <g key="theme">
      <circle cx="12" cy="12" r="5" />
      <line x1="12" y1="1" x2="12" y2="3" />
      <line x1="12" y1="21" x2="12" y2="23" />
      <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
      <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
      <line x1="1" y1="12" x2="3" y2="12" />
      <line x1="21" y1="12" x2="23" y2="12" />
      <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
      <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
    </g>
  ),
  // 键盘
  keyboard: (
    <g key="keyboard">
      <rect x="2" y="6" width="20" height="12" rx="2" />
      <circle cx="7" cy="10" r="1" fill="currentColor" stroke="none" />
      <circle cx="12" cy="10" r="1" fill="currentColor" stroke="none" />
      <circle cx="17" cy="10" r="1" fill="currentColor" stroke="none" />
      <line x1="8" y1="14" x2="16" y2="14" />
    </g>
  ),
  // 信息/帮助
  info: (
    <g key="info" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="10" />
      <path d="M12 8v4" />
      <path d="M12 16h.01" />
    </g>
  ),
  // 图片
  image: (
    <g key="image">
      <rect x="3" y="3" width="18" height="18" rx="2" />
      <circle cx="8.5" cy="8.5" r="1.5" />
      <polyline points="21 15 16 10 5 21" />
    </g>
  ),
  // 书本（展开的书）
  book: (
    <g key="book">
      <path d="M3 19C5 18 7 17.5 9 17.5C11 17.5 12 18 12 19V6C12 5 11 4.5 9 4.5C7 4.5 5 5 3 6V19Z" />
      <path d="M21 19C19 18 17 17.5 15 17.5C13 17.5 12 18 12 19V6C12 5 13 4.5 15 4.5C17 4.5 19 5 21 6V19Z" />
    </g>
  ),
  // 更多（垂直三点）
  "more-vertical": (
    <g key="more-vertical">
      <circle cx="12" cy="5" r="2" fill="currentColor" stroke="none" />
      <circle cx="12" cy="12" r="2" fill="currentColor" stroke="none" />
      <circle cx="12" cy="19" r="2" fill="currentColor" stroke="none" />
    </g>
  ),
  // 规划模式图标：铅笔，末端方形带橡皮擦线条，加长笔身 + 笔芯
  "plan-mode": (
    <g key="plan-mode">
      {/* 铅笔主体：末端方形（无圆角），底部尖头 */}
      <path d="M15 4 L19 8 L9 18 L4 19 L5 14 Z" />
      {/* 橡皮擦分隔线 */}
      <line x1="12" y1="7" x2="16" y2="11" />
    </g>
  ),
  // GitHub logo
  github: (
    <g key="github" strokeLinecap="round" strokeLinejoin="round">
      <path d="M15 22v-4a4.8 4.8 0 0 0-1-3.5c3 0 6-2 6-5.5.08-1.25-.27-2.48-1-3.5.28-1.15.28-2.35 0-3.5 0 0-1 0-3 1.5-2.64-.5-5.36-.5-8 0C6 2 5 2 5 2c-.3 1.15-.3 2.35 0 3.5A5.403 5.403 0 0 0 4 9c0 3.5 3 5.5 6 5.5-.39.49-.68 1.05-.85 1.65-.17.6-.22 1.23-.15 1.85v4" />
      <path d="M9 18c-4.51 2-5-2-7-2" />
    </g>
  ),
  // 构建模式图标：左尖括号 + 中间左斜杠 + 右尖括号，斜杠两侧与尖括号之间各留一空格距离 </>
  "code-brackets": (
    <g key="code-brackets">
      {/* 左尖括号 < */}
      <polyline points="7 6 1 12 7 18" />
      {/* 右尖括号 > */}
      <polyline points="17 6 23 12 17 18" />
      {/* 中间左斜杠 \，与两侧尖括号各留约 1 单位空格距离 */}
      <line x1="15" y1="4" x2="9" y2="20" />
    </g>
  ),
};

export function Icon({ name, size = 18, className = "", ...props }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={name === "dot" ? 0 : name === "check" ? 2.5 : 2}
      width={size}
      height={size}
      className={className}
      {...props}
    >
      {paths[name]}
    </svg>
  );
}
