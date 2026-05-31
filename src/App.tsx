import { useState, useCallback, useEffect, useRef, lazy, Suspense } from "react";
import { TopBar } from "./components/layout/TopBar";
import { MainLayout } from "./components/layout/MainLayout";
import { MainArea } from "./components/layout/MainArea";
import { InputArea } from "./components/layout/InputArea";
import { WorkflowTimeline } from "./components/workflow/WorkflowTimeline";
import { FileTreeSection } from "./components/sidebar/FileTreeSection";
import { AgentInfoSection } from "./components/sidebar/AgentInfoSection";
import { ContextWindowSection } from "./components/sidebar/ContextWindowSection";
import { TodoSection } from "./components/sidebar/TodoSection";
import { ToastContainer } from "./components/common/Toast";
import { useWorkflowStore } from "./stores/useWorkflowStore";
import { useAttachmentStore } from "./stores/useAttachmentStore";
import { useSessionStore } from "./stores/useSessionStore";
import { useSettingsStore } from "./stores/useSettingsStore";
import { useWorkspaceStore } from "./stores/useWorkspaceStore";
import { useFileTreeStore } from "./stores/useFileTreeStore";
import { useAgent } from "./hooks/useAgent";
import { parseError } from "./services/errorHandler";
import { generateToolBrief } from "./utils/format";
import type { ToolNodeData } from "./types";
import { onSessionUpdated } from "./services/event";
import * as tauriCmd from "./services/tauri";

// 懒加载浮层组件：这些组件体积较大且仅在用户打开时才需要，延迟加载可减少首屏 bundle 体积
const PreviewOverlay = lazy(() =>
  import("./components/preview/PreviewOverlay").then((m) => ({ default: m.PreviewOverlay }))
);
const SettingsDialog = lazy(() =>
  import("./components/settings/SettingsDialog").then((m) => ({ default: m.SettingsDialog }))
);
const HistoryPanel = lazy(() =>
  import("./components/session/HistoryPanel").then((m) => ({ default: m.HistoryPanel }))
);
const VersionHistoryPanel = lazy(() =>
  import("./components/preview/VersionHistoryPanel").then((m) => ({ default: m.VersionHistoryPanel }))
);
const UpdateNotification = lazy(() =>
  import("./components/common/UpdateNotification").then((m) => ({ default: m.UpdateNotification }))
);

/** 懒加载组件的通用加载占位符 */
function LazyFallback() {
  return null;
}

export default function App() {
  const [historyOpen, setHistoryOpen] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [updateNotificationOpen, setUpdateNotificationOpen] = useState(false);

  // 文档预览状态
  const [previewTitle, setPreviewTitle] = useState("");
  const [previewContent, setPreviewContent] = useState("");
  const [previewFileType, setPreviewFileType] = useState<string | undefined>(undefined);
  const [previewLoading, setPreviewLoading] = useState(false);
  // PDF 文件的 base64 编码数据，用于 pdfjs-dist 渲染
  const [previewPdfBase64, setPreviewPdfBase64] = useState<string | null>(null);
  // 差异对比数据
  const [previewDiffData, setPreviewDiffData] = useState<{ oldContent: string; newContent: string } | null>(null);

  // 版本历史面板状态
  const [versionHistoryOpen, setVersionHistoryOpen] = useState(false);
  const [versionHistoryFilePath, setVersionHistoryFilePath] = useState("");
  const [versionHistoryFileName, setVersionHistoryFileName] = useState("");

  const { addNode, updateNode, setExecutionStatus, clearNodes, setConfirmHandler, loadFromMessages, executionStatus, initContextUsageListener, loadContextUsage, clearContextUsage, saveSessionToCache, restoreSessionFromCache, clearSessionCache, getCachedStreamingRefs, todos: workflowTodos, setTodos } = useWorkflowStore();
  const { switchSession, loadSessions, clearCurrentSession, currentSessionId } = useSessionStore();
  const updateSessionTitleLocal = useSessionStore((s) => s.updateSessionTitleLocal);
  const { loadSettings, initThemeListener } = useSettingsStore();
  const settings = useSettingsStore((s) => s.settings);
  const { loadWorkspaces, currentWorkspaceId, workspaces } = useWorkspaceStore();
  const { loadTree, clearTree, initFileChangeListener, destroyFileChangeListener } = useFileTreeStore();

  const {
    error: agentError,
    deepThinking,
    content,
    currentToolCall,
    lastToolResult,
    pendingConfirmation,
    doneResult,
    isStopped,
    sendMessage,
    stopAgent,
    confirmOperation,
    reset: resetAgent,
    setSessionId: setAgentSessionId,
    sessionId: agentSessionId,
  } = useAgent();

  const streamingNodeIdRef = useRef<string | null>(null);
  const thinkingNodeIdRef = useRef<string | null>(null);
  const confirmNodeIdRef = useRef<string | null>(null);
  // 追踪当前迭代轮次，用于将 iteration 传递给 content/tool 节点
  const currentIterationRef = useRef<number | undefined>(undefined);
  // 追踪 Agent 上一次的 sessionId，用于检测新会话创建
  const prevAgentSessionIdRef = useRef<string | null>(null);
  // 保存最后一次发送的文本，用于错误重试
  const lastSentTextRef = useRef<string | null>(null);
  // 保存最后一次发送的选项，用于错误重试
  const lastSentOptionsRef = useRef<Record<string, unknown> | undefined>(undefined);

  useEffect(() => {
    loadSettings();
    loadWorkspaces();
    loadSessions();
    // 初始化系统主题偏好监听
    const cleanup = initThemeListener();
    // 初始化上下文窗口使用情况事件监听
    let contextUsageCleanup: (() => void) | null = null;
    initContextUsageListener().then((unlisten) => {
      contextUsageCleanup = unlisten;
    });
    return () => {
      cleanup();
      if (contextUsageCleanup) contextUsageCleanup();
    };
  }, []);

  // 监听会话标题自动更新事件（后端生成标题后通知前端）
  useEffect(() => {
    let unlistenFn: (() => void) | null = null;

    onSessionUpdated((payload) => {
      if (payload.changeType === "title_updated" && payload.data) {
        const data = payload.data as { title?: string };
        if (data.title) {
          updateSessionTitleLocal(payload.sessionId, data.title);
        }
      }
    }).then((unlisten) => {
      unlistenFn = unlisten;
    });

    return () => {
      if (unlistenFn) unlistenFn();
    };
  }, [updateSessionTitleLocal]);

  // 应用启动后自动检查更新（延迟5秒，避免启动时阻塞）
  useEffect(() => {
    const timer = setTimeout(async () => {
      if (!settings.update.autoCheck) return;
      try {
        const { check } = await import("@tauri-apps/plugin-updater");
        const result = await check();
        if (result) {
          setUpdateNotificationOpen(true);
        }
      } catch {
        // 静默处理检查失败
      }
    }, 5000);
    return () => clearTimeout(timer);
  }, [settings.update.autoCheck]);

  // 当 Agent 创建新会话时，同步刷新 session store 并选中新会话
  useEffect(() => {
    if (agentSessionId && !prevAgentSessionIdRef.current) {
      loadSessions();
      switchSession(agentSessionId);
    }
    prevAgentSessionIdRef.current = agentSessionId;
  }, [agentSessionId, loadSessions, switchSession]);

  // 工作区切换时自动加载文件树，工作区被清空时清空文件树
  useEffect(() => {
    if (currentWorkspaceId) {
      loadTree(currentWorkspaceId);
    } else {
      // 当前没有工作区时，清空文件树显示
      clearTree();
    }
  }, [currentWorkspaceId, loadTree, clearTree]);

  // 初始化文件变更事件监听（由 store 统一管理）
  useEffect(() => {
    initFileChangeListener();
    return () => {
      destroyFileChangeListener();
    };
  }, [initFileChangeListener, destroyFileChangeListener]);

  useEffect(() => {
    if (deepThinking) {
      // 更新当前迭代轮次追踪
      if (deepThinking.iteration !== undefined) {
        currentIterationRef.current = deepThinking.iteration;
      }
      if (!deepThinking.isStreaming && !thinkingNodeIdRef.current) {
        return;
      }
      if (streamingNodeIdRef.current) {
        const node = useWorkflowStore.getState().nodes.find((n) => n.id === streamingNodeIdRef.current);
        updateNode(streamingNodeIdRef.current, {
          status: "completed",
          data: { content: (node?.data as { content: string })?.content ?? "", isStreaming: false },
        });
        streamingNodeIdRef.current = null;
      }
      if (!thinkingNodeIdRef.current) {
        const nodeId = addNode("thinking", {
          content: deepThinking.thought,
          duration: 0,
          isStreaming: deepThinking.isStreaming,
        }, "running", deepThinking.iteration);
        thinkingNodeIdRef.current = nodeId;
      } else {
        // 使用 useAgent 中累积的完整内容替换节点内容，而非追加 delta
        updateNode(thinkingNodeIdRef.current, {
          data: {
            content: deepThinking.thought,
            duration: 0,
            isStreaming: deepThinking.isStreaming,
          },
          status: deepThinking.isStreaming ? "running" : "completed",
          iteration: deepThinking.iteration,
        });
        if (!deepThinking.isStreaming) {
          thinkingNodeIdRef.current = null;
        }
      }
    }
  }, [deepThinking, addNode, updateNode]);

  useEffect(() => {
    if (currentToolCall) {
      // 通过 callId 去重：如果已存在相同 callId 的工具节点，仅更新参数和简要描述
      // 这处理了流式阶段提前发射（参数不完整）后，流式结束重新发射（参数完整）的场景
      // 重新发射时不应关闭 thinking/streaming 节点，因为它们可能属于下一迭代
      const existingToolNode = currentToolCall.callId
        ? useWorkflowStore.getState().nodes.find(
            (n) => n.type === "tool" && (n.data as ToolNodeData).callId === currentToolCall.callId
          )
        : undefined;

      if (existingToolNode) {
        updateNode(existingToolNode.id, {
          data: {
            ...existingToolNode.data,
            toolName: currentToolCall.toolName,
            input: currentToolCall.arguments,
            briefDescription: generateToolBrief(currentToolCall.toolName, currentToolCall.arguments),
          } as ToolNodeData,
        });
      } else {
        // 首次收到 tool_call：关闭当前 thinking/streaming 节点，创建工具节点
        if (thinkingNodeIdRef.current) {
          const node = useWorkflowStore.getState().nodes.find((n) => n.id === thinkingNodeIdRef.current);
          updateNode(thinkingNodeIdRef.current, {
            status: "completed",
            data: { content: (node?.data as { content: string })?.content ?? "", duration: 0, isStreaming: false },
          });
          thinkingNodeIdRef.current = null;
        }
        if (streamingNodeIdRef.current) {
          const node = useWorkflowStore.getState().nodes.find((n) => n.id === streamingNodeIdRef.current);
          updateNode(streamingNodeIdRef.current, {
            status: "completed",
            data: { content: (node?.data as { content: string })?.content ?? "", isStreaming: false },
          });
          streamingNodeIdRef.current = null;
        }
        const toolIteration = currentToolCall.iteration ?? currentIterationRef.current;
        addNode("tool", {
          toolName: currentToolCall.toolName,
          input: currentToolCall.arguments,
          briefDescription: generateToolBrief(currentToolCall.toolName, currentToolCall.arguments),
          callId: currentToolCall.callId,
        }, "running", toolIteration);
      }
    }
  }, [currentToolCall, addNode, updateNode]);

  useEffect(() => {
    if (lastToolResult) {
      // 优先通过 callId 精确匹配工具节点，回退到最后一个 running 的工具节点
      const toolNode = lastToolResult.callId
        ? useWorkflowStore.getState().nodes.find(
            (n) => n.type === "tool" && n.status === "running" && (n.data as ToolNodeData).callId === lastToolResult.callId
          )
        : undefined;
      const targetNode = toolNode ?? (() => {
        const runningTools = useWorkflowStore.getState().nodes.filter((n) => n.type === "tool" && n.status === "running");
        return runningTools.length > 0 ? runningTools[runningTools.length - 1] : undefined;
      })();
      if (targetNode) {
        updateNode(targetNode.id, {
          status: lastToolResult.success ? "completed" : "failed",
          data: {
            ...targetNode.data,
            success: lastToolResult.success,
            error: lastToolResult.success ? undefined : (lastToolResult.error || "执行失败"),
          },
        });
      }
    }
  }, [lastToolResult, updateNode]);

  useEffect(() => {
    if (content !== undefined && content !== null) {
      if (thinkingNodeIdRef.current) {
        const node = useWorkflowStore.getState().nodes.find((n) => n.id === thinkingNodeIdRef.current);
        updateNode(thinkingNodeIdRef.current, {
          status: "completed",
          data: { content: (node?.data as { content: string })?.content ?? "", duration: 0, isStreaming: false },
        });
        thinkingNodeIdRef.current = null;
      }
      if (!streamingNodeIdRef.current) {
        if (content) {
          const nodeId = addNode("content", {
            content,
            isStreaming: true,
          }, "running", currentIterationRef.current);
          streamingNodeIdRef.current = nodeId;
        }
      } else {
        updateNode(streamingNodeIdRef.current, {
          data: { content, isStreaming: true },
        });
      }
    }
  }, [content, addNode, updateNode]);

  useEffect(() => {
    if (doneResult) {
      if (thinkingNodeIdRef.current) {
        const node = useWorkflowStore.getState().nodes.find((n) => n.id === thinkingNodeIdRef.current);
        updateNode(thinkingNodeIdRef.current, {
          status: "completed",
          data: { content: (node?.data as { content: string })?.content ?? "", duration: 0, isStreaming: false },
        });
        thinkingNodeIdRef.current = null;
      }
      if (streamingNodeIdRef.current) {
        const node = useWorkflowStore.getState().nodes.find((n) => n.id === streamingNodeIdRef.current);
        updateNode(streamingNodeIdRef.current, {
          status: "completed",
          data: { content: (node?.data as { content: string })?.content ?? doneResult.summary ?? "", isStreaming: false },
        });
        streamingNodeIdRef.current = null;
      } else if (doneResult.summary) {
        addNode("content", {
          content: doneResult.summary,
          isStreaming: false,
        });
      }
      setExecutionStatus("completed");
    }
  }, [doneResult, addNode, updateNode, setExecutionStatus]);

  useEffect(() => {
    if (agentError) {
      if (thinkingNodeIdRef.current) {
        const node = useWorkflowStore.getState().nodes.find((n) => n.id === thinkingNodeIdRef.current);
        updateNode(thinkingNodeIdRef.current, {
          status: "failed",
          data: { content: (node?.data as { content: string })?.content ?? "", duration: 0, isStreaming: false },
        });
        thinkingNodeIdRef.current = null;
      }
      if (streamingNodeIdRef.current) {
        updateNode(streamingNodeIdRef.current, {
          status: "failed",
        });
        streamingNodeIdRef.current = null;
      }
      const parsed = parseError(agentError);
      addNode("error", {
        code: parsed.code,
        message: parsed.userMessage,
        recoverable: parsed.recoverable,
        module: parsed.module,
      });
      setExecutionStatus("failed");
    }
  }, [agentError, updateNode, setExecutionStatus, addNode]);

  useEffect(() => {
    if (isStopped) {
      if (thinkingNodeIdRef.current) {
        const node = useWorkflowStore.getState().nodes.find((n) => n.id === thinkingNodeIdRef.current);
        updateNode(thinkingNodeIdRef.current, {
          status: "cancelled",
          data: { content: (node?.data as { content: string })?.content ?? "", duration: 0, isStreaming: false },
        });
        thinkingNodeIdRef.current = null;
      }
      if (streamingNodeIdRef.current) {
        updateNode(streamingNodeIdRef.current, {
          status: "cancelled",
        });
        streamingNodeIdRef.current = null;
      }
      setExecutionStatus("cancelled");
    }
  }, [isStopped, updateNode, setExecutionStatus]);

  useEffect(() => {
    if (pendingConfirmation) {
      const nodeId = addNode("confirm", {
        title: pendingConfirmation.operationType,
        description: pendingConfirmation.description,
        confirmLabel: "确认执行",
        cancelLabel: "取消操作",
        confirmed: null,
      }, "running");
      confirmNodeIdRef.current = nodeId;

      setConfirmHandler(async (approved: boolean) => {
        if (confirmNodeIdRef.current) {
          updateNode(confirmNodeIdRef.current, {
            data: {
              title: pendingConfirmation.operationType,
              description: pendingConfirmation.description,
              confirmLabel: "确认执行",
              cancelLabel: "取消操作",
              confirmed: approved,
            },
            status: approved ? "completed" : "cancelled",
          });
          confirmNodeIdRef.current = null;
        }
        await confirmOperation(pendingConfirmation.operationId, approved);
        setConfirmHandler(null);
      });
    }
  }, [pendingConfirmation, addNode, updateNode, confirmOperation, setConfirmHandler]);

  // 发送用户消息
  const handleSend = useCallback(async (text: string) => {
    if (!text.trim()) return;

    streamingNodeIdRef.current = null;
    thinkingNodeIdRef.current = null;
    confirmNodeIdRef.current = null;

    lastSentTextRef.current = text;

    // 从附件 store 获取当前附件，映射为工作流节点附件格式
    const currentAttachments = useAttachmentStore.getState().attachments;
    const nodeAttachments = currentAttachments.map((att, idx) => ({
      id: `att_${idx}`,
      name: att.name,
      path: att.path || att.absolutePath || "",
      size: att.size,
      mimeType: att.mimeType,
    }));
    addNode("user", { content: text, attachments: nodeAttachments });
    setExecutionStatus("running");

    // 获取当前工作区路径，传递给 Agent 以正确解析文件路径
    const currentWorkspace = workspaces.find((w) => w.id === currentWorkspaceId);
    const workingDirectory = currentWorkspace?.path;
    const workspaceId = currentWorkspaceId;
    const options = workingDirectory ? { workingDirectory, workspaceId } : undefined;

    // 保存发送选项，用于错误重试
    lastSentOptionsRef.current = options;

    try {
      await sendMessage(text, options);
    } catch (err) {
      console.error("[App] 发送消息失败:", err);
      setExecutionStatus("failed");
    }
  }, [addNode, setExecutionStatus, sendMessage, workspaces, currentWorkspaceId]);

  // 停止 Agent 执行，先显示加载状态，等待后端确认停止
  const handleStop = useCallback(async () => {
    // 设置为 stopping 状态，显示加载中
    setExecutionStatus("stopping");

    try {
      await stopAgent();
      // 停止成功后，状态会由 isStopped 的 useEffect 更新为 cancelled
    } catch (err) {
      console.error("[App] 停止 Agent 失败:", err);
      // 停止失败，恢复为 running 状态
      setExecutionStatus("running");
    }
  }, [setExecutionStatus, stopAgent]);

  // 新建会话：先保存当前会话状态到缓存，再清空 UI
  const handleNewSession = useCallback(() => {
    // 如果当前有会话，保存其状态到缓存
    if (currentSessionId) {
      saveSessionToCache(currentSessionId, {
        streamingNodeId: streamingNodeIdRef.current,
        thinkingNodeId: thinkingNodeIdRef.current,
        confirmNodeId: confirmNodeIdRef.current,
        currentIteration: currentIterationRef.current,
      });
    }
    clearNodes();
    resetAgent();
    clearCurrentSession();
    clearContextUsage();
    setTodos(null);
    streamingNodeIdRef.current = null;
    thinkingNodeIdRef.current = null;
    confirmNodeIdRef.current = null;
    currentIterationRef.current = undefined;
  }, [clearNodes, resetAgent, clearCurrentSession, clearContextUsage, saveSessionToCache, currentSessionId, setTodos]);

  // 切换到历史会话：先保存当前会话状态到缓存，再从缓存或后端恢复目标会话
  const handleSwitchSession = useCallback(async (sessionId: string) => {
    // 如果当前有会话，保存其状态到缓存
    if (currentSessionId) {
      saveSessionToCache(currentSessionId, {
        streamingNodeId: streamingNodeIdRef.current,
        thinkingNodeId: thinkingNodeIdRef.current,
        confirmNodeId: confirmNodeIdRef.current,
        currentIteration: currentIterationRef.current,
      });
    }

    clearNodes();
    resetAgent();
    streamingNodeIdRef.current = null;
    thinkingNodeIdRef.current = null;
    confirmNodeIdRef.current = null;
    currentIterationRef.current = undefined;

    // 更新 session store 中的当前会话 ID
    switchSession(sessionId);
    // 同步 Agent hook 的 sessionId，使后续消息发送到正确的会话
    setAgentSessionId(sessionId);

    // 尝试从缓存恢复（即时显示）
    const cacheHit = restoreSessionFromCache(sessionId);

    // 从缓存恢复流式状态引用
    const cachedRefs = getCachedStreamingRefs(sessionId);
    if (cachedRefs) {
      streamingNodeIdRef.current = cachedRefs.streamingNodeId;
      thinkingNodeIdRef.current = cachedRefs.thinkingNodeId;
      confirmNodeIdRef.current = cachedRefs.confirmNodeId;
      currentIterationRef.current = cachedRefs.currentIteration;
    }

    // 无论是否缓存命中，都从后端加载最新消息以确保数据一致性
    try {
      const detail = await tauriCmd.getSession(sessionId);
      // 仅在缓存未命中时使用后端数据覆盖（缓存命中时后端数据作为补充验证）
      if (!cacheHit) {
        loadFromMessages(detail.messages);
      }
    } catch (err) {
      console.error("[App] 加载历史会话失败:", err);
    }

    // 检查该会话的 Agent 是否仍在运行，恢复正确的执行状态
    try {
      const running = await tauriCmd.isAgentRunning(sessionId);
      if (running) {
        setExecutionStatus("running");
      }
    } catch {
      // 查询失败时不影响主流程
    }

    // 加载该会话的上下文窗口使用信息
    loadContextUsage(sessionId);
  }, [clearNodes, resetAgent, switchSession, setAgentSessionId, loadFromMessages, loadContextUsage, saveSessionToCache, restoreSessionFromCache, getCachedStreamingRefs, setExecutionStatus, currentSessionId]);

  // 删除当前会话后的处理：清空缓存，切换到其他会话或清空工作流
  const handleDeleteCurrentSession = useCallback(async (nextSessionId: string | null) => {
    // 清除被删除会话的缓存
    if (currentSessionId) {
      clearSessionCache(currentSessionId);
    }

    clearNodes();
    resetAgent();
    streamingNodeIdRef.current = null;
    thinkingNodeIdRef.current = null;
    confirmNodeIdRef.current = null;
    currentIterationRef.current = undefined;

    if (nextSessionId) {
      // 切换到下一个可用会话
      switchSession(nextSessionId);
      setAgentSessionId(nextSessionId);

      // 尝试从缓存恢复
      const cacheHit = restoreSessionFromCache(nextSessionId);

      // 从缓存恢复流式状态引用
      const cachedRefs = getCachedStreamingRefs(nextSessionId);
      if (cachedRefs) {
        streamingNodeIdRef.current = cachedRefs.streamingNodeId;
        thinkingNodeIdRef.current = cachedRefs.thinkingNodeId;
        confirmNodeIdRef.current = cachedRefs.confirmNodeId;
        currentIterationRef.current = cachedRefs.currentIteration;
      }

      // 缓存未命中时从后端加载
      if (!cacheHit) {
        try {
          const detail = await tauriCmd.getSession(nextSessionId);
          loadFromMessages(detail.messages);
        } catch (err) {
          console.error("[App] 加载切换后的会话失败:", err);
        }
      }

      // 检查该会话的 Agent 是否仍在运行
      try {
        const running = await tauriCmd.isAgentRunning(nextSessionId);
        if (running) {
          setExecutionStatus("running");
        }
      } catch {
        // 查询失败时不影响主流程
      }

      // 加载该会话的上下文窗口使用信息
      loadContextUsage(nextSessionId);
    } else {
      // 没有其他会话，清除上下文窗口使用信息
      clearContextUsage();
      setTodos(null);
    }
  }, [clearNodes, resetAgent, switchSession, setAgentSessionId, loadFromMessages, loadContextUsage, clearContextUsage, clearSessionCache, restoreSessionFromCache, getCachedStreamingRefs, setExecutionStatus, currentSessionId, setTodos]);

  // 打开文档预览：从后端获取文档内容并显示预览浮层
  const handleOpenPreview = useCallback(async (filePath: string, fileName: string) => {
    if (!currentWorkspaceId) return;

    setPreviewLoading(true);
    setPreviewOpen(true);
    setPreviewTitle(fileName);
    setPreviewContent("");
    setPreviewFileType(undefined);
    setPreviewPdfBase64(null);

    try {
      const result = await tauriCmd.previewDocument(currentWorkspaceId, filePath);
      setPreviewContent(result.content);
      setPreviewFileType(result.fileType);

      // 对 PDF 文件额外获取 base64 数据用于 pdfjs-dist 渲染
      const ext = fileName.split(".").pop()?.toLowerCase();
      if (ext === "pdf") {
        try {
          const base64Data = await tauriCmd.getPdfData(currentWorkspaceId, filePath);
          setPreviewPdfBase64(base64Data);
        } catch (pdfErr) {
          console.error("[App] 获取PDF数据失败，降级为文本预览:", pdfErr);
          // 降级：不设置 pdfBase64，PreviewOverlay 将使用文本模式
        }
      }
    } catch (err) {
      console.error("[App] 预览文档失败:", err);
      setPreviewContent(`[预览失败] ${err instanceof Error ? err.message : String(err)}`);
      setPreviewFileType(undefined);
    } finally {
      setPreviewLoading(false);
    }
  }, [currentWorkspaceId]);

  // 关闭文档预览
  const handleClosePreview = useCallback(() => {
    setPreviewOpen(false);
    setPreviewContent("");
    setPreviewTitle("");
    setPreviewFileType(undefined);
    setPreviewPdfBase64(null);
    setPreviewDiffData(null);
  }, []);

  // 打开版本历史面板
  const handleOpenVersionHistory = useCallback((filePath: string, fileName: string) => {
    setVersionHistoryFilePath(filePath);
    setVersionHistoryFileName(fileName);
    setVersionHistoryOpen(true);
  }, []);

  // 版本对比回调：将两个版本的内容传入 PreviewOverlay 的 DiffView
  const handleCompareVersions = useCallback((oldContent: string, newContent: string, fileType: string) => {
    setPreviewTitle("版本差异对比");
    setPreviewContent(newContent);
    setPreviewFileType(fileType);
    setPreviewDiffData({ oldContent, newContent });
    setPreviewPdfBase64(null);
    setPreviewOpen(true);
    setVersionHistoryOpen(false);
  }, []);

  // 版本回滚完成回调
  const handleRollbackComplete = useCallback(() => {
    if (currentWorkspaceId) {
      loadTree(currentWorkspaceId);
    }
  }, [currentWorkspaceId, loadTree]);

  // 错误重试回调：使用最后一次发送的文本重新发送消息
  const handleRetryError = useCallback(async () => {
    const text = lastSentTextRef.current;
    if (!text) return;

    streamingNodeIdRef.current = null;
    thinkingNodeIdRef.current = null;
    confirmNodeIdRef.current = null;

    addNode("user", { content: text, attachments: [] }); // 重试时不带附件
    setExecutionStatus("running");

    try {
      await sendMessage(text, lastSentOptionsRef.current);
    } catch (err) {
      console.error("[App] 重试发送消息失败:", err);
      setExecutionStatus("failed");
    }
  }, [addNode, setExecutionStatus, sendMessage]);

  // 监听键盘快捷键
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setPreviewOpen(false);
      }
      if (e.ctrlKey && e.key === "n") {
        e.preventDefault();
        handleNewSession();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleNewSession]);

  return (
    <div className="app flex flex-col h-screen">
      <TopBar
        onToggleHistory={() => setHistoryOpen(!historyOpen)}
        onNewSession={handleNewSession}
      />

      <MainLayout
        mainArea={
          <MainArea
            workflow={<WorkflowTimeline onRetryError={handleRetryError} />}
            inputArea={
              <InputArea
                onSend={handleSend}
                executionStatus={executionStatus}
                onStop={handleStop}
              />
            }
          />
        }
        sidebar={
          <>
            <FileTreeSection onOpenPreview={handleOpenPreview} onOpenVersionHistory={handleOpenVersionHistory} />
            <AgentInfoSection />
            <ContextWindowSection />
            <TodoSection
              items={workflowTodos?.map((t) => ({
                id: t.id,
                text: t.content,
                done: t.status === "completed",
                active: t.status === "in_progress",
              }))}
            />
          </>
        }
      />

      {/* 浮层面板（懒加载） */}
      <Suspense fallback={<LazyFallback />}>
        <PreviewOverlay
          open={previewOpen}
          onClose={handleClosePreview}
          title={previewTitle}
          content={previewContent}
          fileType={previewFileType}
          pdfBase64Data={previewPdfBase64}
          diffData={previewDiffData}
        />
      </Suspense>
      {previewLoading && (
        <div className="fixed inset-0 bg-black/10 z-[199] flex items-center justify-center pointer-events-none">
          <div className="bg-bg-elevated px-5 py-3 rounded-[var(--radius-md)] shadow-md text-[13px] text-text-secondary flex items-center gap-2">
            <svg className="animate-spin w-4 h-4" viewBox="0 0 24 24" fill="none">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" />
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
            加载预览中...
          </div>
        </div>
      )}
      <Suspense fallback={<LazyFallback />}>
        <SettingsDialog />
      </Suspense>
      <Suspense fallback={<LazyFallback />}>
        <HistoryPanel open={historyOpen} onClose={() => setHistoryOpen(false)} onSwitchSession={handleSwitchSession} onDeleteCurrentSession={handleDeleteCurrentSession} />
      </Suspense>
      <Suspense fallback={<LazyFallback />}>
        {versionHistoryOpen && currentWorkspaceId && (
          <VersionHistoryPanel
            open={versionHistoryOpen}
            onClose={() => setVersionHistoryOpen(false)}
            workspaceId={currentWorkspaceId}
            filePath={versionHistoryFilePath}
            fileName={versionHistoryFileName}
            onCompareVersions={handleCompareVersions}
            onRollbackComplete={handleRollbackComplete}
          />
        )}
      </Suspense>

      {/* 全局 Toast 提示容器 */}
      <ToastContainer />

      {/* 更新通知组件（懒加载） */}
      <Suspense fallback={<LazyFallback />}>
        <UpdateNotification
          open={updateNotificationOpen}
          onClose={() => setUpdateNotificationOpen(false)}
        />
      </Suspense>

      <style>{`
        .app { display: flex; flex-direction: column; height: 100vh; }
        .topbar-btn {
          width: 34px; height: 34px; border-radius: var(--radius-sm);
          display: flex; align-items: center; justify-content: center;
          transition: all 0.15s ease; color: var(--color-text-secondary);
        }
        .topbar-btn:hover { background: var(--color-bg-sub); color: var(--color-text-primary); }
        .topbar-btn:active:not(:disabled) { transform: scale(0.92); }
        .input-btn {
          width: 32px; height: 32px; border-radius: var(--radius-sm);
          display: flex; align-items: center; justify-content: center;
          transition: all 0.15s ease; color: var(--color-text-tertiary);
        }
        .input-btn:hover { background: var(--color-bg-sub); color: var(--color-text-secondary); }
        .input-btn:active:not(:disabled) { transform: scale(0.92); }
      `}</style>
    </div>
  );
}
