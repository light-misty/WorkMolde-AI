# DocAgent AI 文档处理桌面应用 - 前端组件设计文档

> 技术栈：React 18 + TypeScript 5 + Zustand 4 + Tailwind CSS 3 + Shadcn/ui
> 文档版本：1.0.0
> 最后更新：2026-05-14

---

## 目录

1. [组件树结构](#1-组件树结构)
2. [组件详细设计](#2-组件详细设计)
3. [Zustand Store 定义](#3-zustand-store-定义)
4. [自定义 Hooks](#4-自定义-hooks)
5. [组件间通信方式](#5-组件间通信方式)
6. [类型定义汇总](#6-类型定义汇总)

---

## 1. 组件树结构

```
App
├── TopBar                          // 顶部导航栏
│   ├── WorkspaceSelector           // 工作区选择器
│   ├── StatusIndicator             // 状态指示器
│   └── ActionButtons               // 操作按钮组
├── MainLayout                      // 主布局容器
│   ├── MainArea                    // 主内容区域
│   │   ├── WorkflowArea            // 工作流展示区域
│   │   │   └── WorkflowTimeline    // 工作流时间线
│   │   │       ├── UserNode        // 用户消息节点
│   │   │       ├── ThinkingNode    // 思考过程节点
│   │   │       ├── ToolNode        // 工具调用节点
│   │   │       ├── ResultNode      // 执行结果节点
│   │   │       ├── ReplyNode       // 回复内容节点
│   │   │       └── ConfirmNode     // 确认请求节点
│   │   └── InputArea               // 输入区域
│   │       ├── AttachmentButton    // 附件按钮
│   │       ├── InputField          // 输入框
│   │       ├── TemplateButton      // 模板按钮
│   │       └── SendButton          // 发送按钮
│   └── Sidebar                     // 侧边栏
│       ├── FileTreeSection         // 文件树区域
│       │   ├── SearchBox           // 搜索框
│       │   └── FileTree            // 文件树
│       ├── AgentInfoSection        // Agent 信息区域
│       │   ├── ModelInfo           // 模型信息
│       │   ├── AuthorName          // 作者名
│       │   └── ConfirmLevel        // 确认级别
│       ├── TodoSection             // 任务列表区域
│       └── TokenSection            // Token 统计区域
├── PreviewOverlay                  // 预览覆盖层
│   └── PreviewPanel                // 预览面板
│       ├── MarkdownPreview         // Markdown 预览
│       └── DiffView                // 差异对比视图
├── SettingsOverlay                 // 设置覆盖层
│   └── SettingsDialog              // 设置对话框
│       ├── LLMConfig              // LLM 配置
│       ├── WorkspaceManager        // 工作区管理
│       ├── SkillManager            // 技能管理
│       ├── TemplateManager         // 模板管理
│       └── GeneralSettings         // 通用设置
└── HistoryPanel                    // 历史会话面板
```

---

## 2. 组件详细设计

### 2.1 App

根组件，负责全局 Provider 注入和顶层布局编排。

```typescript
interface AppProps {
  initialWorkspaceId?: string;
}
```

**职责：**
- 初始化所有 Zustand Store
- 提供全局 Theme Provider
- 监听 IPC 事件（Electron 主进程通信）
- 渲染顶层布局：TopBar + MainLayout + 各 Overlay

---

### 2.2 TopBar

顶部导航栏，包含工作区切换、状态展示和全局操作入口。

```typescript
interface TopBarProps {
  className?: string;
}

interface WorkspaceSelectorProps {
  workspaces: Workspace[];
  currentWorkspaceId: string;
  onWorkspaceChange: (workspaceId: string) => void;
}

interface StatusIndicatorProps {
  status: AgentStatus;
  message?: string;
}

interface ActionButtonsProps {
  onOpenSettings: () => void;
  onOpenHistory: () => void;
  onToggleSidebar: () => void;
  isSidebarVisible: boolean;
}
```

**状态来源：**
- `useWorkspaceStore` → 当前工作区、工作区列表
- `useWorkflowStore` → Agent 执行状态

---

### 2.3 MainLayout

主布局容器，采用 Flex 布局管理 MainArea 和 Sidebar 的空间分配。

```typescript
interface MainLayoutProps {
  children: React.ReactNode;
  sidebarVisible: boolean;
  sidebarWidth: number;
  onSidebarResize: (width: number) => void;
}
```

**特性：**
- 支持拖拽调整 Sidebar 宽度
- Sidebar 折叠/展开动画
- 响应式布局：窄屏自动折叠 Sidebar

---

### 2.4 MainArea

主内容区域，纵向排列 WorkflowArea 和 InputArea。

```typescript
interface MainAreaProps {
  className?: string;
}
```

---

### 2.5 WorkflowArea

工作流展示区域，包含可滚动的时间线视图。

```typescript
interface WorkflowAreaProps {
  className?: string;
}
```

**特性：**
- 自动滚动到最新节点
- 支持手动滚动锁定（用户向上浏览时暂停自动滚动）
- 虚拟滚动优化长列表性能

---

### 2.6 WorkflowTimeline

工作流时间线，渲染工作流节点列表。

```typescript
interface WorkflowTimelineProps {
  nodes: WorkflowNode[];
  onNodeAction: (nodeId: string, action: NodeAction) => void;
  onConfirmNode: (nodeId: string, confirmed: boolean) => void;
  onRetryNode: (nodeId: string) => void;
}

type NodeAction =
  | { type: 'copy' }
  | { type: 'expand' }
  | { type: 'collapse' }
  | { type: 'retry' }
  | { type: 'confirm'; value: boolean };
```

**渲染策略：**
- 根据 `node.type` 动态渲染对应的 Node 组件
- 使用 `React.memo` 优化单个节点渲染
- 节点间通过连接线可视化执行流程

---

### 2.7 WorkflowNode（多态节点）

工作流节点的多态组件，根据 `node.type` 渲染不同形态。

```typescript
// 基础节点类型
interface BaseNodeProps {
  nodeId: string;
  timestamp: number;
  status: NodeStatus;
}

type NodeStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

// 用户消息节点
interface UserNodeProps extends BaseNodeProps {
  type: 'user';
  content: string;
  attachments?: Attachment[];
}

// 思考过程节点
interface ThinkingNodeProps extends BaseNodeProps {
  type: 'thinking';
  content: string;
  duration?: number;
  isExpanded: boolean;
  onToggleExpand: () => void;
}

// 工具调用节点
interface ToolNodeProps extends BaseNodeProps {
  type: 'tool';
  toolName: string;
  toolIcon?: string;
  input: Record<string, unknown>;
  output?: Record<string, unknown>;
  isExpanded: boolean;
  onToggleExpand: () => void;
}

// 执行结果节点
interface ResultNodeProps extends BaseNodeProps {
  type: 'result';
  content: string;
  filePaths?: string[];
  diffStats?: DiffStats;
  onPreview: (filePath: string) => void;
}

interface DiffStats {
  additions: number;
  deletions: number;
  files: number;
}

// 回复内容节点
interface ReplyNodeProps extends BaseNodeProps {
  type: 'reply';
  content: string;
  markdown?: boolean;
  onCopy: () => void;
}

// 确认请求节点
interface ConfirmNodeProps extends BaseNodeProps {
  type: 'confirm';
  title: string;
  description: string;
  confirmLabel: string;
  cancelLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
}

// 联合类型
type WorkflowNodeProps =
  | UserNodeProps
  | ThinkingNodeProps
  | ToolNodeProps
  | ResultNodeProps
  | ReplyNodeProps
  | ConfirmNodeProps;
```

**多态渲染逻辑：**

```typescript
function WorkflowNodeRenderer(props: WorkflowNodeProps) {
  switch (props.type) {
    case 'user':     return <UserNode {...props} />;
    case 'thinking': return <ThinkingNode {...props} />;
    case 'tool':     return <ToolNode {...props} />;
    case 'result':   return <ResultNode {...props} />;
    case 'reply':    return <ReplyNode {...props} />;
    case 'confirm':  return <ConfirmNode {...props} />;
  }
}
```

---

### 2.8 InputArea

输入区域，包含附件上传、文本输入、模板选择和发送功能。

```typescript
interface InputAreaProps {
  onSend: (message: SendMessagePayload) => void;
  onAttach: (files: File[]) => void;
  onSelectTemplate: (templateId: string) => void;
  isDisabled: boolean;
  placeholder?: string;
}

interface SendMessagePayload {
  content: string;
  attachments?: Attachment[];
  templateId?: string;
}

interface Attachment {
  id: string;
  name: string;
  path: string;
  size: number;
  mimeType: string;
}

interface AttachmentButtonProps {
  onAttach: (files: File[]) => void;
  disabled?: boolean;
}

interface InputFieldProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  placeholder?: string;
  disabled?: boolean;
  attachments?: Attachment[];
  onRemoveAttachment: (attachmentId: string) => void;
}

interface TemplateButtonProps {
  templates: PromptTemplate[];
  onSelectTemplate: (templateId: string) => void;
  disabled?: boolean;
}

interface SendButtonProps {
  onClick: () => void;
  disabled?: boolean;
  isLoading?: boolean;
}
```

**特性：**
- 支持 `Ctrl+Enter` 快捷键发送
- 输入框自适应高度（最大 200px 后出现滚动条）
- 附件拖拽上传
- Token 实时计数显示

---

### 2.9 Sidebar

侧边栏容器，纵向堆叠各功能面板，支持折叠/展开各 Section。

```typescript
interface SidebarProps {
  className?: string;
  visible: boolean;
  width: number;
}
```

---

### 2.10 FileTreeSection

文件树区域，包含搜索和文件浏览功能。

```typescript
interface FileTreeSectionProps {
  className?: string;
}

interface SearchBoxProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  resultCount?: number;
}

interface FileTreeProps {
  nodes: FileTreeNode[];
  expandedKeys: string[];
  selectedKeys: string[];
  onExpand: (key: string) => void;
  onCollapse: (key: string) => void;
  onSelect: (key: string) => void;
  onContextMenu: (key: string, event: React.MouseEvent) => void;
  searchKeyword?: string;
}

interface FileTreeNode {
  key: string;
  name: string;
  type: 'file' | 'directory';
  children?: FileTreeNode[];
  path: string;
  extension?: string;
  isModified?: boolean;
  isIgnored?: boolean;
}
```

---

### 2.11 AgentInfoSection

Agent 信息展示区域。

```typescript
interface AgentInfoSectionProps {
  className?: string;
}

interface ModelInfoProps {
  provider: LLMProvider;
  modelName: string;
  modelVersion?: string;
  contextWindow: number;
}

type LLMProvider = 'openai' | 'anthropic' | 'google' | 'local' | 'custom';

interface AuthorNameProps {
  name: string;
  avatar?: string;
}

interface ConfirmLevelProps {
  level: ConfirmLevel;
  onLevelChange: (level: ConfirmLevel) => void;
}

type ConfirmLevel = 'auto' | 'low' | 'medium' | 'high';
// auto: 自动执行  low: 仅危险操作确认  medium: 文件修改确认  high: 所有操作确认
```

---

### 2.12 TodoSection

任务列表区域，展示当前会话的待办事项。

```typescript
interface TodoSectionProps {
  className?: string;
}

interface TodoItemProps {
  id: string;
  content: string;
  status: TodoStatus;
  onToggle: (id: string) => void;
}

type TodoStatus = 'pending' | 'in_progress' | 'completed' | 'failed';
```

---

### 2.13 TokenSection

Token 统计区域。

```typescript
interface TokenSectionProps {
  className?: string;
}

interface TokenStatsProps {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  budget: number;
  usagePercent: number;
}
```

---

### 2.14 PreviewOverlay

预览覆盖层，模态展示文件预览内容。

```typescript
interface PreviewOverlayProps {
  visible: boolean;
  onClose: () => void;
  filePath: string;
  fileType: PreviewFileType;
}

type PreviewFileType = 'markdown' | 'diff' | 'image' | 'pdf' | 'text';
```

---

### 2.15 PreviewPanel

预览面板，根据文件类型切换渲染模式。

```typescript
interface PreviewPanelProps {
  filePath: string;
  fileType: PreviewFileType;
  content: string;
  diffData?: DiffData;
}

interface MarkdownPreviewProps {
  content: string;
  onLinkClick?: (href: string) => void;
}

interface DiffViewProps {
  oldContent: string;
  newContent: string;
  filePath: string;
  language?: string;
}

interface DiffData {
  oldContent: string;
  newContent: string;
  hunks: DiffHunk[];
}

interface DiffHunk {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  content: string;
}
```

---

### 2.16 SettingsOverlay

设置覆盖层。

```typescript
interface SettingsOverlayProps {
  visible: boolean;
  onClose: () => void;
}
```

---

### 2.17 SettingsDialog

设置对话框，包含多个设置标签页。

```typescript
interface SettingsDialogProps {
  activeTab: SettingsTab;
  onTabChange: (tab: SettingsTab) => void;
}

type SettingsTab = 'llm' | 'workspace' | 'skill' | 'template' | 'general';

// LLM 配置
interface LLMConfigProps {
  providers: LLMProviderConfig[];
  activeProviderId: string;
  onAddProvider: (config: LLMProviderConfig) => void;
  onUpdateProvider: (id: string, config: Partial<LLMProviderConfig>) => void;
  onRemoveProvider: (id: string) => void;
  onTestConnection: (id: string) => Promise<boolean>;
}

interface LLMProviderConfig {
  id: string;
  provider: LLMProvider;
  name: string;
  apiKey: string;
  baseUrl?: string;
  modelName: string;
  maxTokens: number;
  temperature: number;
  topP: number;
  enabled: boolean;
}

// 工作区管理
interface WorkspaceManagerProps {
  workspaces: Workspace[];
  onAddWorkspace: (workspace: Omit<Workspace, 'id'>) => void;
  onRemoveWorkspace: (id: string) => void;
  onUpdateWorkspace: (id: string, updates: Partial<Workspace>) => void;
}

interface Workspace {
  id: string;
  name: string;
  path: string;
  description?: string;
  createdAt: number;
  lastOpenedAt: number;
  isDefault: boolean;
}

// 技能管理
interface SkillManagerProps {
  skills: Skill[];
  onToggleSkill: (id: string) => void;
  onConfigureSkill: (id: string, config: Record<string, unknown>) => void;
}

interface Skill {
  id: string;
  name: string;
  description: string;
  icon?: string;
  enabled: boolean;
  config: Record<string, unknown>;
}

// 模板管理
interface TemplateManagerProps {
  templates: PromptTemplate[];
  onAddTemplate: (template: Omit<PromptTemplate, 'id'>) => void;
  onUpdateTemplate: (id: string, updates: Partial<PromptTemplate>) => void;
  onRemoveTemplate: (id: string) => void;
}

interface PromptTemplate {
  id: string;
  name: string;
  description: string;
  content: string;
  category: string;
  variables?: TemplateVariable[];
  createdAt: number;
  updatedAt: number;
}

interface TemplateVariable {
  name: string;
  type: 'string' | 'number' | 'boolean' | 'select';
  label: string;
  defaultValue?: unknown;
  options?: string[];
}

// 通用设置
interface GeneralSettingsProps {
  settings: AppSettings;
  onUpdateSettings: (updates: Partial<AppSettings>) => void;
}

interface AppSettings {
  theme: 'light' | 'dark' | 'system';
  language: 'zh-CN' | 'en-US';
  fontSize: number;
  autoScroll: boolean;
  showTimestamps: boolean;
  confirmLevel: ConfirmLevel;
  tokenBudget: number;
  enableSounds: boolean;
  minimizeToTray: boolean;
  autoUpdate: boolean;
}
```

---

### 2.18 HistoryPanel

历史会话面板。

```typescript
interface HistoryPanelProps {
  visible: boolean;
  onClose: () => void;
  sessions: SessionSummary[];
  onLoadSession: (sessionId: string) => void;
  onDeleteSession: (sessionId: string) => void;
  onSearchSessions: (keyword: string) => void;
}

interface SessionSummary {
  id: string;
  title: string;
  workspaceId: string;
  workspaceName: string;
  createdAt: number;
  updatedAt: number;
  messageCount: number;
  preview: string;
  tags?: string[];
}
```

---

## 3. Zustand Store 定义

### 3.1 useWorkflowStore

管理工作流节点的状态和操作。

```typescript
interface WorkflowState {
  // 状态
  nodes: WorkflowNodeData[];
  currentNodeId: string | null;
  executionStatus: ExecutionStatus;
  error: string | null;
  autoScroll: boolean;

  // 操作
  addNode: (node: Omit<WorkflowNodeData, 'id'>) => string;
  updateNode: (id: string, updates: Partial<WorkflowNodeData>) => void;
  removeNode: (id: string) => void;
  clearNodes: () => void;
  setExecutionStatus: (status: ExecutionStatus) => void;
  setError: (error: string | null) => void;
  setAutoScroll: (autoScroll: boolean) => void;
  confirmNode: (id: string, confirmed: boolean) => void;
  retryNode: (id: string) => void;
}

type ExecutionStatus = 'idle' | 'running' | 'paused' | 'completed' | 'failed' | 'cancelled';

interface WorkflowNodeData {
  id: string;
  type: WorkflowNodeType;
  status: NodeStatus;
  timestamp: number;
  parentId: string | null;
  data: NodeDataMap[WorkflowNodeType];
  isExpanded: boolean;
  error?: string;
}

type WorkflowNodeType = 'user' | 'thinking' | 'tool' | 'result' | 'reply' | 'confirm';

// 各类型节点的数据映射
interface NodeDataMap {
  user: {
    content: string;
    attachments: Attachment[];
  };
  thinking: {
    content: string;
    duration: number;
  };
  tool: {
    toolName: string;
    toolIcon: string;
    input: Record<string, unknown>;
    output: Record<string, unknown>;
  };
  result: {
    content: string;
    filePaths: string[];
    diffStats: DiffStats | null;
  };
  reply: {
    content: string;
    markdown: boolean;
  };
  confirm: {
    title: string;
    description: string;
    confirmLabel: string;
    cancelLabel: string;
    confirmed: boolean | null;
  };
}
```

**使用示例：**

```typescript
const { nodes, executionStatus, addNode, updateNode } = useWorkflowStore();

// 添加用户消息节点
const nodeId = addNode({
  type: 'user',
  status: 'completed',
  timestamp: Date.now(),
  parentId: null,
  isExpanded: false,
  data: {
    content: '请帮我翻译这个文档',
    attachments: [],
  },
});

// 更新节点状态
updateNode(nodeId, { status: 'completed' });
```

---

### 3.2 useSessionStore

管理会话的创建、切换和历史记录。

```typescript
interface SessionState {
  // 状态
  currentSessionId: string | null;
  sessions: Session[];
  isLoading: boolean;
  error: string | null;

  // 操作
  createSession: (workspaceId: string, title?: string) => Promise<string>;
  switchSession: (sessionId: string) => Promise<void>;
  deleteSession: (sessionId: string) => Promise<void>;
  updateSessionTitle: (sessionId: string, title: string) => void;
  loadSessions: () => Promise<void>;
  getCurrentSession: () => Session | undefined;
}

interface Session {
  id: string;
  title: string;
  workspaceId: string;
  createdAt: number;
  updatedAt: number;
  messageCount: number;
  workflowNodeIds: string[];
  metadata: SessionMetadata;
}

interface SessionMetadata {
  totalTokens: number;
  totalCost: number;
  tags: string[];
  isPinned: boolean;
}
```

---

### 3.3 useWorkspaceStore

管理工作区的状态和文件数据。

```typescript
interface WorkspaceState {
  // 状态
  currentWorkspaceId: string | null;
  workspaces: Workspace[];
  isLoading: boolean;
  error: string | null;

  // 操作
  addWorkspace: (workspace: Omit<Workspace, 'id' | 'createdAt' | 'lastOpenedAt'>) => Promise<string>;
  removeWorkspace: (id: string) => Promise<void>;
  updateWorkspace: (id: string, updates: Partial<Workspace>) => Promise<void>;
  switchWorkspace: (id: string) => Promise<void>;
  loadWorkspaces: () => Promise<void>;
  getCurrentWorkspace: () => Workspace | undefined;
}
```

---

### 3.4 useSettingsStore

管理应用设置和配置。

```typescript
interface SettingsState {
  // 状态
  settings: AppSettings;
  llmProviders: LLMProviderConfig[];
  activeProviderId: string;
  skills: Skill[];
  templates: PromptTemplate[];
  isSettingsOpen: boolean;
  activeSettingsTab: SettingsTab;

  // 应用设置操作
  updateSettings: (updates: Partial<AppSettings>) => void;
  resetSettings: () => void;

  // LLM 配置操作
  addLLMProvider: (config: Omit<LLMProviderConfig, 'id'>) => string;
  updateLLMProvider: (id: string, updates: Partial<LLMProviderConfig>) => void;
  removeLLMProvider: (id: string) => void;
  setActiveProvider: (id: string) => void;
  testLLMConnection: (id: string) => Promise<boolean>;

  // 技能操作
  toggleSkill: (id: string) => void;
  configureSkill: (id: string, config: Record<string, unknown>) => void;

  // 模板操作
  addTemplate: (template: Omit<PromptTemplate, 'id' | 'createdAt' | 'updatedAt'>) => string;
  updateTemplate: (id: string, updates: Partial<PromptTemplate>) => void;
  removeTemplate: (id: string) => void;

  // UI 状态操作
  openSettings: (tab?: SettingsTab) => void;
  closeSettings: () => void;
}
```

---

### 3.5 useFileTreeStore

管理文件树的状态。

```typescript
interface FileTreeState {
  // 状态
  treeData: FileTreeNode[];
  expandedKeys: Set<string>;
  selectedKeys: Set<string>;
  searchKeyword: string;
  filteredNodes: FileTreeNode[];
  isLoading: boolean;
  error: string | null;

  // 操作
  loadTree: (workspacePath: string) => Promise<void>;
  expandNode: (key: string) => void;
  collapseNode: (key: string) => void;
  toggleNode: (key: string) => void;
  expandAll: () => void;
  collapseAll: () => void;
  selectNode: (key: string) => void;
  deselectNode: (key: string) => void;
  setSearchKeyword: (keyword: string) => void;
  refreshTree: () => Promise<void>;
  getNodeByPath: (path: string) => FileTreeNode | undefined;
}
```

---

### 3.6 useTokenStore

管理 Token 统计和预算。

```typescript
interface TokenState {
  // 状态
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  budget: number;
  sessionTokens: SessionTokenBreakdown[];
  isOverBudget: boolean;

  // 计算属性
  usagePercent: number;
  remainingTokens: number;

  // 操作
  addTokenUsage: (usage: TokenUsage) => void;
  resetSessionTokens: () => void;
  setBudget: (budget: number) => void;
  checkBudget: () => boolean;
  getTokenStats: () => TokenStats;
}

interface TokenUsage {
  promptTokens: number;
  completionTokens: number;
  model: string;
  nodeId: string;
  timestamp: number;
}

interface SessionTokenBreakdown {
  model: string;
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  requestCount: number;
}

interface TokenStats {
  totalPromptTokens: number;
  totalCompletionTokens: number;
  totalTokens: number;
  estimatedCost: number;
  breakdown: SessionTokenBreakdown[];
}
```

---

## 4. 自定义 Hooks

### 4.1 useAgent

Agent 交互的核心 Hook，负责消息发送、执行控制和事件监听。

```typescript
interface UseAgentOptions {
  sessionId: string | null;
  workspaceId: string | null;
  onNodeAdded?: (node: WorkflowNodeData) => void;
  onNodeUpdated?: (node: WorkflowNodeData) => void;
  onExecutionComplete?: () => void;
  onExecutionFailed?: (error: string) => void;
  onConfirmRequired?: (nodeId: string, data: NodeDataMap['confirm']) => void;
}

interface UseAgentReturn {
  // 状态
  isExecuting: boolean;
  currentStatus: ExecutionStatus;
  error: string | null;

  // 操作
  sendMessage: (payload: SendMessagePayload) => Promise<void>;
  interruptExecution: () => Promise<void>;
  confirmAction: (nodeId: string, confirmed: boolean) => Promise<void>;
  retryExecution: (nodeId: string) => Promise<void>;

  // 事件监听
  onNodeEvent: (callback: (event: AgentNodeEvent) => void) => () => void;
}

interface AgentNodeEvent {
  type: 'node_added' | 'node_updated' | 'node_completed' | 'node_failed';
  nodeId: string;
  nodeType: WorkflowNodeType;
  data: Partial<WorkflowNodeData>;
  timestamp: number;
}

function useAgent(options: UseAgentOptions): UseAgentReturn;
```

**实现要点：**
- 通过 IPC 与 Electron 主进程通信
- 使用 SSE（Server-Sent Events）或 WebSocket 接收 Agent 流式输出
- 自动将 Agent 事件同步到 `useWorkflowStore`
- 支持中断正在执行的任务
- 确认请求节点触发 `onConfirmRequired` 回调

**使用示例：**

```typescript
function WorkflowArea() {
  const { nodes } = useWorkflowStore();
  const { sendMessage, isExecuting, interruptExecution, confirmAction } = useAgent({
    sessionId: currentSessionId,
    workspaceId: currentWorkspaceId,
    onConfirmRequired: (nodeId, data) => {
      // 显示确认对话框或高亮确认节点
    },
  });

  return (
    <div>
      <WorkflowTimeline
        nodes={nodes}
        onConfirmNode={confirmAction}
      />
      <InputArea onSend={sendMessage} isDisabled={isExecuting} />
    </div>
  );
}
```

---

### 4.2 useFileTree

文件树操作 Hook，封装文件树的状态管理和搜索逻辑。

```typescript
interface UseFileTreeOptions {
  workspacePath: string | null;
  autoLoad?: boolean;
  debounceMs?: number;
}

interface UseFileTreeReturn {
  // 状态
  treeData: FileTreeNode[];
  expandedKeys: Set<string>;
  selectedKeys: Set<string>;
  searchKeyword: string;
  filteredNodes: FileTreeNode[];
  isLoading: boolean;

  // 操作
  loadTree: () => Promise<void>;
  expandNode: (key: string) => void;
  collapseNode: (key: string) => void;
  toggleNode: (key: string) => void;
  expandAll: () => void;
  collapseAll: () => void;
  selectNode: (key: string) => void;
  setSearchKeyword: (keyword: string) => void;
  refreshTree: () => Promise<void>;
  getNodeByPath: (path: string) => FileTreeNode | undefined;

  // 便捷方法
  expandToNode: (path: string) => void;
  getSelectedFilePath: () => string | undefined;
}

function useFileTree(options: UseFileTreeOptions): UseFileTreeReturn;
```

**实现要点：**
- 搜索使用防抖（默认 300ms）
- 搜索结果高亮匹配文本
- `expandToNode` 自动展开到指定路径的所有父级
- 文件变更通过 `chokidar`（Electron 主进程）监听并同步

---

### 4.3 useTokenCounter

实时 Token 计数 Hook。

```typescript
interface UseTokenCounterOptions {
  model: string;
  debounceMs?: number;
}

interface UseTokenCounterReturn {
  // 状态
  tokenCount: number;
  isCounting: boolean;

  // 操作
  countTokens: (text: string) => void;
  resetCount: () => void;

  // 便捷方法
  estimateTokens: (text: string) => number;
  getRemainingBudget: () => number;
  isOverBudget: () => boolean;
}

function useTokenCounter(options: UseTokenCounterOptions): UseTokenCounterReturn;
```

**实现要点：**
- 使用 `tiktoken` 或类似库进行精确 Token 计数
- 计数过程在 Web Worker 中执行，避免阻塞 UI
- 输入变化时防抖计数（默认 500ms）
- `estimateTokens` 提供快速但不精确的估算（字符数 / 4）

---

## 5. 组件间通信方式

### 5.1 通信架构总览

```
┌─────────────────────────────────────────────────────────────┐
│                        App (Root)                            │
│                                                              │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐               │
│  │ Zustand  │    │ Zustand  │    │ Zustand  │               │
│  │ Workflow │    │ Session  │    │ Workspace│               │
│  │  Store   │    │  Store   │    │  Store   │               │
│  └────┬─────┘    └────┬─────┘    └────┬─────┘               │
│       │               │               │                      │
│  ┌────┴───────────────┴───────────────┴─────┐               │
│  │           组件订阅 (useXxxStore)          │               │
│  └────┬───────────────┬───────────────┬─────┘               │
│       │               │               │                      │
│  ┌────▼────┐    ┌─────▼─────┐   ┌─────▼─────┐              │
│  │TopBar   │    │MainArea   │   │ Sidebar   │              │
│  └─────────┘    └───────────┘   └───────────┘              │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              自定义 Hooks (useAgent 等)                │   │
│  │     连接 Store 与 IPC，封装业务逻辑                     │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              IPC Bridge (Electron)                    │   │
│  │     渲染进程 ←→ 主进程 双向通信                         │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 5.2 通信方式分类

| 通信方式 | 适用场景 | 示例 |
|---------|---------|------|
| Zustand Store | 跨组件全局状态共享 | WorkflowTimeline 读取 useWorkflowStore 的 nodes |
| Props 传递 | 父子组件直接通信 | InputArea 向 SendButton 传递 disabled 状态 |
| 回调函数 | 子组件向父组件通信 | ConfirmNode 的 onConfirm 回调 |
| 自定义 Hooks | 封装业务逻辑和 Store 交互 | useAgent 连接 IPC 和 WorkflowStore |
| Event Bus | 松耦合的跨模块通信 | Agent 执行完成通知 HistoryPanel 刷新 |
| IPC Bridge | 渲染进程与主进程通信 | 文件操作、LLM API 调用 |

### 5.3 典型通信流程

#### 流程一：用户发送消息

```
InputArea.onSend(payload)
  → useAgent.sendMessage(payload)
    → IPC: renderer → main (发送消息请求)
    → main: 调用 LLM API
    → IPC: main → renderer (流式返回节点数据)
      → useWorkflowStore.addNode(node)
        → WorkflowTimeline 重新渲染
      → useTokenStore.addTokenUsage(usage)
        → TokenSection 更新统计
```

#### 流程二：确认操作

```
ConfirmNode.onConfirm()
  → useAgent.confirmAction(nodeId, confirmed)
    → useWorkflowStore.updateNode(nodeId, { data: { confirmed: true } })
    → IPC: renderer → main (确认结果)
      → main: 继续执行 Agent 工作流
```

#### 流程三：切换工作区

```
WorkspaceSelector.onWorkspaceChange(workspaceId)
  → useWorkspaceStore.switchWorkspace(workspaceId)
    → useFileTreeStore.loadTree(workspacePath)
      → FileTreeSection 重新渲染
    → useSessionStore.loadSessions()
      → HistoryPanel 更新会话列表
    → useWorkflowStore.clearNodes()
      → WorkflowTimeline 清空
```

#### 流程四：文件预览

```
ResultNode.onPreview(filePath)
  → useWorkflowStore 设置预览状态
  → PreviewOverlay 变为可见
    → IPC: 读取文件内容
    → PreviewPanel 根据 fileType 渲染
      → MarkdownPreview 或 DiffView
```

### 5.4 Store 间依赖关系

```
useWorkspaceStore ──────→ useFileTreeStore
       │                       │
       │                       ↓
       │               useSessionStore
       │                       │
       ↓                       ↓
useSettingsStore ──────→ useWorkflowStore
       │                       │
       ↓                       ↓
useTokenStore ←────────────────┘
```

**依赖说明：**
- 切换工作区时需要重新加载文件树和会话列表
- 工作流执行消耗 Token，需同步更新 Token 统计
- LLM 配置变更影响工作流执行和 Token 计数
- 会话切换时需要清空并重新加载工作流节点

### 5.5 性能优化策略

1. **Store 切片订阅**：组件仅订阅所需的 Store 切片，避免不必要的重渲染
   ```typescript
   // 仅订阅 nodes，不订阅 executionStatus
   const nodes = useWorkflowStore((state) => state.nodes);
   ```

2. **React.memo**：WorkflowNode 等高频渲染组件使用 `React.memo` 包裹

3. **虚拟滚动**：WorkflowTimeline 使用虚拟滚动，仅渲染可视区域内的节点

4. **防抖搜索**：FileTreeSection 和 HistoryPanel 的搜索输入使用防抖

5. **Web Worker**：Token 计数在 Web Worker 中执行

6. **懒加载**：PreviewOverlay 和 SettingsOverlay 使用 `React.lazy` 按需加载

---

## 6. 类型定义汇总

### 枚举与常量

```typescript
// Agent 执行状态
type ExecutionStatus = 'idle' | 'running' | 'paused' | 'completed' | 'failed' | 'cancelled';

// 节点状态
type NodeStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

// 工作流节点类型
type WorkflowNodeType = 'user' | 'thinking' | 'tool' | 'result' | 'reply' | 'confirm';

// 确认级别
type ConfirmLevel = 'auto' | 'low' | 'medium' | 'high';

// LLM 提供商
type LLMProvider = 'openai' | 'anthropic' | 'google' | 'local' | 'custom';

// 预览文件类型
type PreviewFileType = 'markdown' | 'diff' | 'image' | 'pdf' | 'text';

// 设置标签页
type SettingsTab = 'llm' | 'workspace' | 'skill' | 'template' | 'general';

// 任务状态
type TodoStatus = 'pending' | 'in_progress' | 'completed' | 'failed';
```

### 核心数据模型

```typescript
// 工作区
interface Workspace {
  id: string;
  name: string;
  path: string;
  description?: string;
  createdAt: number;
  lastOpenedAt: number;
  isDefault: boolean;
}

// 会话
interface Session {
  id: string;
  title: string;
  workspaceId: string;
  createdAt: number;
  updatedAt: number;
  messageCount: number;
  workflowNodeIds: string[];
  metadata: SessionMetadata;
}

// 工作流节点
interface WorkflowNodeData {
  id: string;
  type: WorkflowNodeType;
  status: NodeStatus;
  timestamp: number;
  parentId: string | null;
  data: NodeDataMap[WorkflowNodeType];
  isExpanded: boolean;
  error?: string;
}

// 文件树节点
interface FileTreeNode {
  key: string;
  name: string;
  type: 'file' | 'directory';
  children?: FileTreeNode[];
  path: string;
  extension?: string;
  isModified?: boolean;
  isIgnored?: boolean;
}

// 附件
interface Attachment {
  id: string;
  name: string;
  path: string;
  size: number;
  mimeType: string;
}

// LLM 配置
interface LLMProviderConfig {
  id: string;
  provider: LLMProvider;
  name: string;
  apiKey: string;
  baseUrl?: string;
  modelName: string;
  maxTokens: number;
  temperature: number;
  topP: number;
  enabled: boolean;
}

// 提示词模板
interface PromptTemplate {
  id: string;
  name: string;
  description: string;
  content: string;
  category: string;
  variables?: TemplateVariable[];
  createdAt: number;
  updatedAt: number;
}

// 技能
interface Skill {
  id: string;
  name: string;
  description: string;
  icon?: string;
  enabled: boolean;
  config: Record<string, unknown>;
}

// 应用设置
interface AppSettings {
  theme: 'light' | 'dark' | 'system';
  language: 'zh-CN' | 'en-US';
  fontSize: number;
  autoScroll: boolean;
  showTimestamps: boolean;
  confirmLevel: ConfirmLevel;
  tokenBudget: number;
  enableSounds: boolean;
  minimizeToTray: boolean;
  autoUpdate: boolean;
}
```

---

> 本文档描述了 DocAgent AI 文档处理桌面应用的完整前端组件架构设计。所有组件均基于 React 函数式组件 + TypeScript 强类型 + Zustand 状态管理 + Tailwind CSS / Shadcn/ui 样式方案实现。实际开发中可根据需求变化迭代更新本文档。
