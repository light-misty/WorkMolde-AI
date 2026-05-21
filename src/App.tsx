import { useState, useCallback, useEffect, useRef } from "react";
import { TopBar } from "./components/layout/TopBar";
import { MainLayout } from "./components/layout/MainLayout";
import { MainArea } from "./components/layout/MainArea";
import { InputArea } from "./components/layout/InputArea";
import { WorkflowTimeline } from "./components/workflow/WorkflowTimeline";
import { FileTreeSection } from "./components/sidebar/FileTreeSection";
import { AgentInfoSection } from "./components/sidebar/AgentInfoSection";
import { TodoSection } from "./components/sidebar/TodoSection";
import { TokenSection } from "./components/sidebar/TokenSection";
import { PreviewOverlay } from "./components/preview/PreviewOverlay";
import { SettingsDialog } from "./components/settings/SettingsDialog";
import { HistoryPanel } from "./components/session/HistoryPanel";
import { useWorkflowStore } from "./stores/useWorkflowStore";
import { useSessionStore } from "./stores/useSessionStore";
import { useSettingsStore } from "./stores/useSettingsStore";
import { useWorkspaceStore } from "./stores/useWorkspaceStore";
import { useFileTreeStore } from "./stores/useFileTreeStore";
import { useTokenStore } from "./stores/useTokenStore";
import { useAgent } from "./hooks/useAgent";
import * as tauriCmd from "./services/tauri";

export default function App() {
  const [historyOpen, setHistoryOpen] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [templateLabel, setTemplateLabel] = useState<string | undefined>(undefined);

  const { addNode, updateNode, setExecutionStatus, clearNodes, setConfirmHandler, loadFromMessages, executionStatus } = useWorkflowStore();
  const { switchSession, loadSessions, clearCurrentSession } = useSessionStore();
  const { loadSettings } = useSettingsStore();
  const { loadWorkspaces, currentWorkspaceId, workspaces } = useWorkspaceStore();
  const { loadTree, initFileChangeListener, destroyFileChangeListener } = useFileTreeStore();
  const { initTokenListener, destroyTokenListener } = useTokenStore();

  const {
    error: agentError,
    lastThinking,
    content,
    currentToolCall,
    lastToolResult,
    pendingConfirmation,
    todos,
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
  const confirmNodeIdRef = useRef<string | null>(null);
  // 追踪 Agent 上一次的 sessionId，用于检测新会话创建
  const prevAgentSessionIdRef = useRef<string | null>(null);

  useEffect(() => {
    loadSettings();
    loadWorkspaces();
    loadSessions();
  }, []);

  // 当 Agent 创建新会话时，同步刷新 session store 并选中新会话
  useEffect(() => {
    if (agentSessionId && !prevAgentSessionIdRef.current) {
      loadSessions();
      switchSession(agentSessionId);
    }
    prevAgentSessionIdRef.current = agentSessionId;
  }, [agentSessionId, loadSessions, switchSession]);

  // 工作区切换时自动加载文件树
  useEffect(() => {
    if (currentWorkspaceId) {
      loadTree(currentWorkspaceId);
    }
  }, [currentWorkspaceId, loadTree]);

  // 初始化文件变更事件监听（由 store 统一管理）
  useEffect(() => {
    initFileChangeListener();
    return () => {
      destroyFileChangeListener();
    };
  }, [initFileChangeListener, destroyFileChangeListener]);

  // 初始化 Token 用量更新事件监听（由 store 统一管理）
  useEffect(() => {
    initTokenListener();
    return () => {
      destroyTokenListener();
    };
  }, [initTokenListener, destroyTokenListener]);

  // Agent 事件 -> WorkflowStore 节点映射：思考过程
  useEffect(() => {
    if (lastThinking) {
      addNode("thinking", {
        content: lastThinking.thought,
        duration: 0,
      }, "running");
    }
  }, [lastThinking, addNode]);

  // Agent 事件 -> WorkflowStore 节点映射：Tool 调用开始
  useEffect(() => {
    if (currentToolCall) {
      addNode("tool", {
        toolName: currentToolCall.toolName,
        input: currentToolCall.arguments,
      }, "running");
    }
  }, [currentToolCall, addNode]);

  // Agent 事件 -> WorkflowStore 节点映射：Tool 执行结果
  useEffect(() => {
    if (lastToolResult) {
      addNode("result", {
        content: lastToolResult.success
          ? JSON.stringify(lastToolResult.result)
          : lastToolResult.error || "执行失败",
        success: lastToolResult.success,
        filePaths: [],
      });
    }
  }, [lastToolResult, addNode]);

  useEffect(() => {
    if (content) {
      if (!streamingNodeIdRef.current) {
        const nodeId = addNode("reply", {
          content,
        }, "running");
        streamingNodeIdRef.current = nodeId;
      } else {
        updateNode(streamingNodeIdRef.current, {
          data: { content },
        });
      }
    }
  }, [content, addNode, updateNode]);

  useEffect(() => {
    if (doneResult) {
      if (streamingNodeIdRef.current) {
        updateNode(streamingNodeIdRef.current, {
          data: { content: doneResult.summary || content },
          status: "completed",
        });
        streamingNodeIdRef.current = null;
      } else {
        addNode("reply", {
          content: doneResult.summary || content,
        });
      }
      setExecutionStatus("completed");
    }
  }, [doneResult, content, addNode, updateNode, setExecutionStatus]);

  useEffect(() => {
    if (agentError) {
      if (streamingNodeIdRef.current) {
        updateNode(streamingNodeIdRef.current, {
          status: "failed",
        });
        streamingNodeIdRef.current = null;
      }
      setExecutionStatus("failed");
    }
  }, [agentError, updateNode, setExecutionStatus]);

  // 处理 Agent 被用户停止的情况
  useEffect(() => {
    if (isStopped) {
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
    confirmNodeIdRef.current = null;

    addNode("user", { content: text, attachments: [] });
    setExecutionStatus("running");

    // 获取当前工作区路径，传递给 Agent 以正确解析文件路径
    const currentWorkspace = workspaces.find((w) => w.id === currentWorkspaceId);
    const workingDirectory = currentWorkspace?.path;

    try {
      await sendMessage(text, workingDirectory ? { workingDirectory } : undefined);
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

  // 新建会话
  const handleNewSession = useCallback(() => {
    clearNodes();
    resetAgent();
    clearCurrentSession();
    streamingNodeIdRef.current = null;
    confirmNodeIdRef.current = null;
  }, [clearNodes, resetAgent, clearCurrentSession]);

  // 切换到历史会话：清空当前节点，从后端加载消息并转换为工作流节点
  const handleSwitchSession = useCallback(async (sessionId: string) => {
    // 清空当前工作流和 Agent 状态
    clearNodes();
    resetAgent();
    streamingNodeIdRef.current = null;
    confirmNodeIdRef.current = null;

    // 更新 session store 中的当前会话 ID
    switchSession(sessionId);
    // 同步 Agent hook 的 sessionId，使后续消息发送到正确的会话
    setAgentSessionId(sessionId);

    // 从后端加载会话详情（包含消息列表）
    try {
      const detail = await tauriCmd.getSession(sessionId);
      // 将消息转换为工作流节点并填充到 store
      loadFromMessages(detail.messages);
    } catch (err) {
      console.error("[App] 加载历史会话失败:", err);
    }
  }, [clearNodes, resetAgent, switchSession, setAgentSessionId, loadFromMessages]);

  // 删除当前会话后的处理：清空工作流或切换到其他会话
  const handleDeleteCurrentSession = useCallback(async (nextSessionId: string | null) => {
    // 清空当前工作流和 Agent 状态
    clearNodes();
    resetAgent();
    streamingNodeIdRef.current = null;
    confirmNodeIdRef.current = null;

    if (nextSessionId) {
      // 切换到下一个可用会话
      switchSession(nextSessionId);
      setAgentSessionId(nextSessionId);
      
      // 加载新会话的内容
      try {
        const detail = await tauriCmd.getSession(nextSessionId);
        loadFromMessages(detail.messages);
      } catch (err) {
        console.error("[App] 加载切换后的会话失败:", err);
      }
    }
    // 如果 nextSessionId 为 null，表示没有其他会话，工作流保持清空状态
  }, [clearNodes, resetAgent, switchSession, setAgentSessionId, loadFromMessages]);

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
            workflow={<WorkflowTimeline />}
            inputArea={
              <InputArea
                onSend={handleSend}
                templateLabel={templateLabel}
                onToggleTemplate={() => setTemplateLabel(templateLabel ? undefined : "生成周报")}
                executionStatus={executionStatus}
                onStop={handleStop}
              />
            }
          />
        }
        sidebar={
          <>
            <FileTreeSection />
            <AgentInfoSection />
            <TodoSection
              items={todos?.todos.map((t) => ({
                id: t.id,
                text: t.content,
                done: t.status === "completed",
                active: t.status === "in_progress",
              }))}
            />
            <TokenSection />
          </>
        }
      />

      {/* 浮层面板 */}
      <PreviewOverlay open={previewOpen} onClose={() => setPreviewOpen(false)} />
      <SettingsDialog />
      <HistoryPanel open={historyOpen} onClose={() => setHistoryOpen(false)} onSwitchSession={handleSwitchSession} onDeleteCurrentSession={handleDeleteCurrentSession} />

      <style>{`
        .app { display: flex; flex-direction: column; height: 100vh; }
        .topbar-btn {
          width: 34px; height: 34px; border-radius: var(--radius-sm);
          display: flex; align-items: center; justify-content: center;
          transition: background 0.15s; color: var(--color-text-secondary);
        }
        .topbar-btn:hover { background: var(--color-bg-sub); color: var(--color-text-primary); }
        .input-btn {
          width: 32px; height: 32px; border-radius: var(--radius-sm);
          display: flex; align-items: center; justify-content: center;
          transition: background 0.15s; color: var(--color-text-tertiary);
        }
        .input-btn:hover { background: var(--color-bg-sub); color: var(--color-text-secondary); }
      `}</style>
    </div>
  );
}
