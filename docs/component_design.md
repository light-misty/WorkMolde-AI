# DocAgent AI 文档处理桌面应用 - 前端组件设计文档

> 技术栈：React 19 + TypeScript 5 + Zustand 5 + Tailwind CSS 4 + Shadcn/ui
> 文档版本：0.1.6
> 最后更新：2026-06-14

---

## 目录

1. [组件树结构](#1-组件树结构)
2. [Zustand Store 定义](#2-zustand-store-定义)
3. [自定义 Hooks](#3-自定义-hooks)
4. [组件间通信方式](#4-组件间通信方式)
5. [类型定义汇总](#5-类型定义汇总)

---

## 1. 组件树结构

```
App
├── ErrorBoundary                      // 全局错误边界
├── ThemeProvider                       // 主题 Provider
├── TopBar                             // 顶部导航栏
│   ├── WindowControls                 // 窗口控制按钮组（最小化/最大化/关闭）
│   ├── WorkspaceSelector              // 工作区选择器
│   ├── HistoryButton                  // 历史会话按钮
│   ├── NewSessionButton               // 新建会话按钮
│   └── SettingsButton                 // 设置按钮
├── NetworkStatusBanner                // 网络状态横幅（断网提示）
├── MainLayout                         // 主布局容器
│   ├── MainArea                       // 主内容区域
│   │   ├── WorkflowTimeline           // 工作流时间线
│   │   │   ├── WorkflowNode           // 多态节点容器
│   │   │   │   ├── UserNode           // 用户消息节点
│   │   │   │   ├── ThinkingNode       // 思考过程节点
│   │   │   │   ├── ContentNode        // 回复内容节点
│   │   │   │   ├── ToolNode           // 工具调用节点
│   │   │   │   ├── ResultNode         // 执行结果节点
│   │   │   │   ├── ConfirmNode        // 确认请求节点
│   │   │   │   └── ErrorNode          // 错误节点
│   │   │   └── IterationGroup         // Agent 执行轮次分组
│   │   └── InputArea                  // 输入区域
│   │       └── SendButton             // 发送按钮
│   └── Sidebar                        // 右侧栏
│       ├── FileTreeSection            // 文件树区域
│       ├── AgentInfoSection           // Agent 信息区域
├── PreviewOverlay  (lazy)             // 预览覆盖层
│   ├── MarkdownPreview                // Markdown 预览（react-markdown+remark-gfm+rehype-hilite）
│   ├── PdfCanvasViewer                // PDF 预览（pdfjs-dist Canvas 渲染）
│   ├── WordDocumentView               // Word 结构化渲染
│   ├── ExcelTableRenderer             // Excel 表格渲染
│   ├── PptDocumentView                // PPT 结构化渲染
│   ├── VersionHistoryPanel            // 版本快照历史面板
│   │   └── DiffView                   // 差异对比视图
│   └── TextPreview                    // 纯文本预览
├── SettingsDialog  (lazy)             // 设置弹窗
│   ├── LLMConfig                      // Provider 管理（含 ProviderFormDialog）
│   ├── WorkspaceTab                   // 工作区管理（含 AddWorkspaceDialog）
│   ├── HandlersTab                    // Handler/Tool 信息展示
│   ├── TemplatesTab                   // 模板管理（含 TemplateEditDialog）
│   ├── AppearanceTab                  // 外观设置
│   ├── ShortcutsTab                   // 快捷键设置
│   ├── GeneralTab                     // 通用设置
│   └── HelpTab                        // 帮助信息
├── ToastContainer                     // Toast 通知容器
├── UpdateNotification  (lazy)         // 更新通知
└── DeleteConfirmDialog                // 删除确认对话框
```

---

## 2. Zustand Store 定义

### 2.1 useWorkflowStore

管理工作流节点状态和 Agent 执行状态。

```typescript
interface WorkflowState {
  nodes: WorkflowNode[];
  currentNodeId: string | null;
  executionStatus: 'idle' | 'running' | 'paused' | 'completed' | 'failed' | 'cancelled';
  error: string | null;
  autoScroll: boolean;
  iterationGroups: number;

  addNode: (node: WorkflowNode) => void;
  updateNode: (id: string, updates: Partial<WorkflowNode>) => void;
  removeNode: (id: string) => void;
  clearNodes: () => void;
  setExecutionStatus: (status: ExecutionStatus) => void;
  setError: (error: string | null) => void;
  setAutoScroll: (autoScroll: boolean) => void;
  setIterationGroups: (groups: number) => void;
}
```

### 2.2 useSessionStore

```typescript
interface SessionState {
  currentSessionId: string | null;
  sessions: Session[];
  isLoading: boolean;
  error: string | null;

  createSession: (params?: CreateSessionParams) => Promise<Session>;
  switchSession: (sessionId: string) => Promise<SessionDetail | null>;
  deleteSession: (sessionId: string) => Promise<void>;
  updateSessionTitle: (sessionId: string, title: string) => Promise<void>;
  loadSessions: () => Promise<void>;
  clearAllSessions: () => Promise<number>;
  getCurrentSession: () => Session | undefined;
}
```

### 2.3 useWorkspaceStore

```typescript
interface WorkspaceState {
  currentWorkspaceId: string | null;
  workspaces: WorkspaceInfo[];
  isLoading: boolean;
  error: string | null;

  addWorkspace: (path: string, name?: string) => Promise<WorkspaceInfo>;
  removeWorkspace: (workspaceId: string) => Promise<void>;
  switchWorkspace: (workspaceId: string) => Promise<void>;
  loadWorkspaces: () => Promise<void>;
  getCurrentWorkspace: () => WorkspaceInfo | undefined;
}
```

### 2.4 useSettingsStore

```typescript
interface SettingsState {
  settings: AppSettings | null;
  isSettingsOpen: boolean;
  activeSettingsTab: string;

  loadSettings: () => Promise<void>;
  updateSettings: (settings: Partial<AppSettings>) => Promise<void>;
  openSettings: (tab?: string) => void;
  closeSettings: () => void;
}
```

### 2.5 useFileTreeStore

```typescript
interface FileTreeState {
  treeData: FileNode[];
  expandedKeys: string[];
  selectedKeys: string[];
  searchKeyword: string;
  isLoading: boolean;
  error: string | null;

  loadTree: (workspaceId: string, path?: string) => Promise<void>;
  toggleNode: (key: string) => void;
  selectNode: (key: string) => void;
  setSearchKeyword: (keyword: string) => void;
  refresh: () => Promise<void>;
}
```

### 2.6 useAttachmentStore

```typescript
interface AttachmentState {
  attachments: AttachmentMeta[];
  addAttachment: (attachment: AttachmentMeta) => void;
  removeAttachment: (index: number) => void;
  clearAttachments: () => void;
}
```

### 2.7 useToastStore

```typescript
interface ToastState {
  toasts: Toast[];
  addToast: (toast: Omit<Toast, 'id'>) => void;
  removeToast: (id: string) => void;
}

interface Toast {
  id: string;
  type: 'error' | 'success' | 'warning';
  message: string;
}
```

### 2.8 useNetworkStore

```typescript
interface NetworkState {
  status: 'online' | 'offline';
  setStatus: (status: 'online' | 'offline') => void;
}
```

---

## 3. 自定义 Hooks

### 3.1 useAgent

Agent 交互核心 Hook，负责消息发送、事件监听和执行控制。

```typescript
interface UseAgentReturn {
  isExecuting: boolean;
  currentStatus: ExecutionStatus;
  error: string | null;

  sendMessage: (prompt: string, options?: AgentOptions) => Promise<void>;
  stopAgent: () => Promise<void>;
  confirmOperation: (operationId: string, approved: boolean, feedback?: string) => Promise<void>;
  getContextUsage: (sessionId: string) => Promise<ContextUsageInfo | null>;
}

function useAgent(sessionId: string | null): UseAgentReturn;
```

监听事件：`agent:thinking`, `agent:deep_thinking`, `agent:content`, `agent:tool_call`, `agent:tool_result`, `agent:confirm`, `agent:context_update`, `agent:done`, `agent:error`, `agent:stopped`

---

## 4. 组件间通信方式

| 通信方式 | 适用场景 |
|---------|---------|
| Zustand Store | 跨组件全局状态共享 |
| Props 传递 | 父子组件直接通信 |
| Tauri Events | Rust 后端 → 前端（Agent 流式事件、系统事件） |
| Tauri Invoke | 前端 → Rust 后端（命令调用） |

### 典型通信流程

**用户发送消息**：
```
InputArea.onSend → useAgent.sendMessage → invoke("start_agent")
  → Rust AgentExecutor → LLM API → Tauri Events → useWorkflowStore → UI
```

**切换工作区**：
```
WorkspaceSelector → useWorkspaceStore.switchWorkspace
  → useFileTreeStore.loadTree
  → useSessionStore.loadSessions
```

---

## 5. 类型定义汇总

```typescript
// 工作流节点类型
type WorkflowNodeType = 'user' | 'thinking' | 'content' | 'tool' | 'result' | 'confirm' | 'error';
type NodeStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

// 执行状态
type ExecutionStatus = 'idle' | 'running' | 'completed' | 'failed' | 'cancelled';

// 确认级别
type ConfirmationLevel = 'Always' | 'EditOnly' | 'Never';

// 主题模式
type ThemeMode = 'light' | 'dark' | 'system';

// 通知类型
type ToastType = 'error' | 'success' | 'warning';
```
