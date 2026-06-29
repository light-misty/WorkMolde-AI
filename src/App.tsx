import { useState, useCallback, useEffect, useRef, lazy, Suspense } from "react";
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from "@tauri-apps/api/window";
import { TopBar } from "./components/layout/TopBar";
import { MainLayout } from "./components/layout/MainLayout";
import { MainArea } from "./components/layout/MainArea";
import { InputArea } from "./components/layout/InputArea";
import { WorkflowTimeline } from "./components/workflow/WorkflowTimeline";
import { RightSidebar } from "./components/layout/RightSidebar";

import { ToastContainer } from "./components/common/Toast";
import { NetworkStatusBanner } from "./components/layout/NetworkStatusBanner";
import { useWorkflowStore } from "./stores/useWorkflowStore";
import { useAttachmentStore } from "./stores/useAttachmentStore";
import { useSessionStore } from "./stores/useSessionStore";
import { useSettingsStore } from "./stores/useSettingsStore";
import { useWorkspaceStore } from "./stores/useWorkspaceStore";
import { useFileTreeStore } from "./stores/useFileTreeStore";
import { useUpdateStore } from "./stores/useUpdateStore";
import { useToastStore } from "./stores/useToastStore";
import { useAgent } from "./hooks/useAgent";
import { parseError } from "./services/errorHandler";
import { generateToolBrief } from "./utils/format";
import type { NodeStatus, ToolNodeData } from "./types";
import { onSessionUpdated, onWorkspaceDirectoryDeleted } from "./services/event";
import * as tauriCmd from "./services/tauri";

// 懒加载浮层组件：这些组件体积较大且仅在用户打开时才需要，延迟加载可减少首屏 bundle 体积
const PreviewOverlay = lazy(() =>
  import("./components/preview/PreviewOverlay").then((m) => ({ default: m.PreviewOverlay }))
);
const SettingsDialog = lazy(() =>
  import("./components/settings/SettingsDialog").then((m) => ({ default: m.SettingsDialog }))
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
  const { t } = useTranslation();
  const [previewOpen, setPreviewOpen] = useState(false);
  const [updateNotificationOpen, setUpdateNotificationOpen] = useState(false);
  const [sidebarVisible, setSidebarVisible] = useState(true);

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

  const { addNode, updateNode, setExecutionStatus, clearNodes, setConfirmHandler, loadFromMessages, executionStatus, initContextUsageListener, loadContextUsage, clearContextUsage, saveSessionToCache, restoreSessionFromCache, clearSessionCache, getCachedStreamingRefs } = useWorkflowStore();
  const { switchSession, loadSessions, clearCurrentSession, currentSessionId, createSession, sessions } = useSessionStore();
  const updateSessionTitleLocal = useSessionStore((s) => s.updateSessionTitleLocal);
  const { loadSettings, initThemeListener } = useSettingsStore();
  const settings = useSettingsStore((s) => s.settings);
  const { loadWorkspaces, switchWorkspace, currentWorkspaceId, workspaces, handleWorkspaceDirectoryDeleted } = useWorkspaceStore();
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
    networkRetry,
    codeStreaming,
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
  // 暂存 code_streaming 数据：当 ToolNode 尚未创建时缓存增量，等创建后一次性应用
  const pendingCodeStreamingRef = useRef<Map<string, { code: string; isStreaming: boolean }>>(new Map());
  // 追踪当前迭代轮次，用于将 iteration 传递给 content/tool 节点
  const currentIterationRef = useRef<number | undefined>(undefined);
  // 追踪最后一次 tool_call 的迭代轮次，用于过滤残余 content 事件
  // 当 tool_call 已关闭 streaming 节点后，同一迭代的残余 content 不应创建新节点
  const lastToolCallIterationRef = useRef<number | null>(null);
  // 记录被 tool_call 关闭的 streaming content 节点 ID
  // 用于在收到 is_streaming=false 的最终内容事件时，更新已有节点（修复内容截断）
  const lastClosedStreamingNodeIdRef = useRef<string | null>(null);
  // 追踪 Agent 上一次的 sessionId，用于检测新会话创建
  const prevAgentSessionIdRef = useRef<string | null>(null);
  // 保存最后一次发送的文本，用于错误重试
  const lastSentTextRef = useRef<string | null>(null);
  // 保存最后一次发送的选项，用于错误重试
  const lastSentOptionsRef = useRef<Record<string, unknown> | undefined>(undefined);

  // 关闭 thinking 节点并设置状态
  function closeThinkingNode(status: NodeStatus) {
    if (thinkingNodeIdRef.current) {
      const node = useWorkflowStore.getState().nodes.find((n) => n.id === thinkingNodeIdRef.current);
      updateNode(thinkingNodeIdRef.current, {
        status,
        data: { content: (node?.data as { content: string })?.content ?? "", duration: 0, isStreaming: false },
      });
      thinkingNodeIdRef.current = null;
    }
  }

  // 关闭 streaming 节点并设置状态，支持可选回退内容（用于 doneResult）
  function closeStreamingNode(status: NodeStatus, fallbackContent?: string) {
    if (streamingNodeIdRef.current) {
      const node = useWorkflowStore.getState().nodes.find((n) => n.id === streamingNodeIdRef.current);
      updateNode(streamingNodeIdRef.current, {
        status,
        data: { content: (node?.data as { content: string })?.content ?? fallbackContent ?? "", isStreaming: false },
      });
      streamingNodeIdRef.current = null;
    }
  }

  // 重置所有工作流引用
  function resetRefs() {
    streamingNodeIdRef.current = null;
    thinkingNodeIdRef.current = null;
    confirmNodeIdRef.current = null;
    currentIterationRef.current = undefined;
    lastToolCallIterationRef.current = null;
    lastClosedStreamingNodeIdRef.current = null;
  }

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

    // 监听窗口关闭事件：如果有待安装的更新，在关闭前安装（不自动重启）
    let closeUnlisten: (() => void) | null = null;
    getCurrentWindow().onCloseRequested(async (event) => {
      const { pendingUpdatePath, clearPendingUpdatePath } = useUpdateStore.getState();
      if (pendingUpdatePath) {
        // 阻止默认关闭行为，先安装更新
        event.preventDefault();
        try {
          // 安装更新但不自动重启（restart=false，NSIS 不传 /R 参数）
          // install_downloaded_update 内部会调用 ShellExecuteW 启动安装器 + std::process::exit(0)
          await tauriCmd.installDownloadedUpdate(pendingUpdatePath, false);
          // 如果执行到这里说明安装启动失败，清除状态并关闭窗口
          clearPendingUpdatePath();
          await getCurrentWindow().destroy();
        } catch (err) {
          console.error("[App] 关闭时安装更新失败:", err);
          // 安装失败也允许关闭，避免用户无法退出应用
          clearPendingUpdatePath();
          await getCurrentWindow().destroy();
        }
      }
    }).then((unlisten) => {
      closeUnlisten = unlisten;
    });

    return () => {
      cleanup();
      if (contextUsageCleanup) contextUsageCleanup();
      if (closeUnlisten) closeUnlisten();
    };
  }, []);

  // 数据修复：将 workspaceId 为空的旧会话归入第一个工作区
  // 避免多工作区场景下这些会话在分组列表中消失
  useEffect(() => {
    if (workspaces.length === 0 || sessions.length === 0) return;
    const wsId = workspaces[0].id;
    const needFix = sessions.filter((s) => !s.workspaceId);
    if (needFix.length === 0) return;
    // 后台异步修复，不阻塞 UI
    (async () => {
      for (const s of needFix) {
        try {
          await tauriCmd.updateSessionWorkspace(s.id, wsId);
        } catch (err) {
          console.warn("[App] 修复会话 workspace_id 失败:", s.id, err);
        }
      }
      // 修复后重新加载会话列表，使本地状态与数据库一致
      loadSessions();
    })();
  }, [workspaces, sessions, loadSessions]);

  // 检测当前会话是否失效（例如删除工作区时连同该工作区下的会话一起被删除）
  // 失效时清空工作流和 Agent 状态，避免 UI 残留已删除会话的内容
  useEffect(() => {
    if (!currentSessionId) return;
    // sessions 为空时不触发（避免初始化阶段误清空）
    if (sessions.length === 0) return;
    const stillExists = sessions.some((s) => s.id === currentSessionId);
    if (stillExists) return;

    // 当前会话已不在列表中，说明已被外部删除（如工作区删除）
    console.warn("[App] 当前会话已失效，清空状态:", currentSessionId);
    clearNodes();
    resetAgent();
    clearContextUsage();
    clearCurrentSession();
    resetRefs();
  }, [currentSessionId, sessions, clearNodes, resetAgent, clearContextUsage, clearCurrentSession]);

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

  // 监听工作区目录被外部删除事件：自动移除工作区、切换到其他工作区、Toast 通知
  useEffect(() => {
    let unlistenFn: (() => void) | null = null;

    onWorkspaceDirectoryDeleted(async (payload) => {
      console.warn("[App] 收到工作区目录删除事件:", payload);
      // 调用 store 方法处理：移除工作区、切换活动工作区、清理后端配置
      await handleWorkspaceDirectoryDeleted(payload.workspaceId);
      // 显示 Toast 通知用户
      useToastStore.getState().addToast("warning", `工作区 "${payload.workspaceName}" 的目录已被删除，已自动移除`);
    }).then((unlisten) => {
      unlistenFn = unlisten;
    });

    return () => {
      if (unlistenFn) unlistenFn();
    };
  }, [handleWorkspaceDirectoryDeleted]);

  // 应用启动后自动检查更新（延迟5秒，避免启动时阻塞；开发环境下跳过自动检查）
  useEffect(() => {
    const timer = setTimeout(async () => {
      if (!settings.update.autoCheck) return;
      // 开发环境下跳过自动检查更新
      if (import.meta.env.DEV) return;
      try {
        const result = await tauriCmd.checkUpdate();
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
      closeStreamingNode("completed");
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
      // 记录最后一次 tool_call 的迭代轮次
      if (currentToolCall.iteration !== undefined) {
        lastToolCallIterationRef.current = currentToolCall.iteration;
      }
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
        closeThinkingNode("completed");
        if (streamingNodeIdRef.current) {
          lastClosedStreamingNodeIdRef.current = streamingNodeIdRef.current;
          closeStreamingNode("completed");
        }
        const toolIteration = currentToolCall.iteration ?? currentIterationRef.current;
        // 检查是否有暂存的 code_streaming 数据，在创建节点时一并注入
        const pendingStreaming = currentToolCall.callId
          ? pendingCodeStreamingRef.current.get(currentToolCall.callId)
          : undefined;
        addNode("tool", {
          toolName: currentToolCall.toolName,
          input: currentToolCall.arguments,
          briefDescription: generateToolBrief(currentToolCall.toolName, currentToolCall.arguments),
          callId: currentToolCall.callId,
          streamingCode: pendingStreaming?.code,
          isCodeStreaming: pendingStreaming?.isStreaming,
        }, "running", toolIteration);
        // 清除暂存
        if (currentToolCall.callId && pendingStreaming) {
          pendingCodeStreamingRef.current.delete(currentToolCall.callId);
        }
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
        const existingData = targetNode.data as ToolNodeData;
        updateNode(targetNode.id, {
          status: lastToolResult.success ? "completed" : "failed",
          data: {
            ...existingData,
            success: lastToolResult.success,
            error: lastToolResult.success ? undefined : (lastToolResult.error || t('toolNode.executionFailed')),
            // 工具执行完毕，代码流式输出也必然结束
            isCodeStreaming: false,
          },
        });
      }
    }
  }, [lastToolResult, updateNode]);

  // 代码流式事件：更新对应 ToolNode 的流式代码内容
  // 后端每次发射完整的反转义代码（非增量），前端直接替换
  useEffect(() => {
    if (codeStreaming) {
      // 通过 callId 匹配已有的 ToolNode
      const toolNode = codeStreaming.callId
        ? useWorkflowStore.getState().nodes.find(
            (n) => n.type === "tool" && (n.data as ToolNodeData).callId === codeStreaming.callId
          )
        : undefined;

      if (toolNode) {
        const existingData = toolNode.data as ToolNodeData;
        // is_final 事件仅更新流式状态，不替换代码内容（codeDelta 为空）
        const isFinal = codeStreaming.isFinal;
        updateNode(toolNode.id, {
          data: {
            ...existingData,
            streamingCode: isFinal ? existingData.streamingCode : codeStreaming.codeDelta,
            isCodeStreaming: !isFinal,
          },
        });
      } else if (codeStreaming.callId) {
        // ToolNode 尚未创建，暂存最新数据（直接覆盖，非追加）
        // is_final 时不暂存空代码
        if (!codeStreaming.isFinal) {
          pendingCodeStreamingRef.current.set(codeStreaming.callId, {
            code: codeStreaming.codeDelta,
            isStreaming: true,
          });
        }
      }
    }
  }, [codeStreaming, updateNode]);

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
        // 防御性检查：当 streaming 节点已被 tool_call 关闭时，
        // 如果当前迭代的 content 属于已有 tool_call 的同一迭代，不创建新节点
        // 这防止了 tool_call 之后的残余 content 创建重复节点
        if (content
          && !(lastToolCallIterationRef.current !== null
            && currentIterationRef.current !== undefined
            && currentIterationRef.current <= lastToolCallIterationRef.current)) {
          const nodeId = addNode("content", {
            content,
            isStreaming: true,
          }, "running", currentIterationRef.current);
          streamingNodeIdRef.current = nodeId;
        } else if (lastClosedStreamingNodeIdRef.current && content) {
          // 后端在流式结束后发射 is_streaming=false 的完整内容事件，
          // 用于更新被 tool_call 关闭的 content 节点（修复 LLM 在 tool_use 块后
          // 继续输出文本内容导致的截断问题）
          updateNode(lastClosedStreamingNodeIdRef.current, {
            data: { content, isStreaming: false },
          });
          lastClosedStreamingNodeIdRef.current = null;
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
      closeThinkingNode("completed");
      if (streamingNodeIdRef.current) {
        closeStreamingNode("completed", doneResult.summary);
      } else if (doneResult.summary) {
        addNode("content", {
          content: doneResult.summary,
          isStreaming: false,
        });
      }
      setExecutionStatus("completed");
      lastToolCallIterationRef.current = null;
      lastClosedStreamingNodeIdRef.current = null;
    }
  }, [doneResult, addNode, updateNode, setExecutionStatus]);

  // 网络重试状态：在工作流中显示"正在重连"提示
  useEffect(() => {
    if (!networkRetry) return;
    // 将当前 thinking 节点更新为重连状态，或创建新的 thinking 节点
    const retryMessage = `${networkRetry.reason}${t('network.retryAttempt', { attempt: networkRetry.attempt, maxAttempts: networkRetry.maxAttempts })}`;
    if (thinkingNodeIdRef.current) {
      updateNode(thinkingNodeIdRef.current, {
        status: "running",
        data: { content: retryMessage, duration: 0, isStreaming: true },
      });
    } else {
      const nodeId = addNode("thinking", {
        content: retryMessage,
        duration: 0,
        isStreaming: true,
      }, "running");
      thinkingNodeIdRef.current = nodeId;
    }
  }, [networkRetry, addNode, updateNode]);

  useEffect(() => {
    if (agentError) {
      closeThinkingNode("failed");
      closeStreamingNode("failed");
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
      closeThinkingNode("cancelled");
      closeStreamingNode("cancelled");
      setExecutionStatus("cancelled");
    }
  }, [isStopped, updateNode, setExecutionStatus]);

  useEffect(() => {
    if (pendingConfirmation) {
      // 防御性检查：code_interpreter_handler 不应再触发确认流程
      // 如果仍然收到，直接自动确认（兼容性兜底）
      if (pendingConfirmation.operationType === "code_interpreter_handler") {
        console.warn("[Confirm] code_interpreter_handler 不应触发确认流程，已自动批准（兼容性兜底）");
        confirmOperation(pendingConfirmation.operationId, true);
        return;
      }

      // 从 details 中提取代码（仅 code_interpreter_handler 操作时存在）
      const details = pendingConfirmation.details as Record<string, unknown> | undefined;
      const code = details?.code as string | undefined;
      // 当有代码预览时，description 中已包含代码摘要，需要分离出纯描述
      // Rust 端格式: "执行代码: {desc}\n{code_preview}"
      let displayDescription = pendingConfirmation.description;
      if (code) {
        const newlineIdx = displayDescription.indexOf('\n');
        if (newlineIdx !== -1) {
          // 只保留第一行（描述部分），代码部分由代码预览区域展示
          displayDescription = displayDescription.substring(0, newlineIdx);
        }
      }
      const confirmData = {
        title: pendingConfirmation.operationType,
        description: displayDescription,
        confirmLabel: t('confirmNode.confirmExecute'),
        cancelLabel: t('confirmNode.cancelOperation'),
        confirmed: null,
        ...(code ? { code } : {}),
      };
      const nodeId = addNode("confirm", confirmData, "running");
      confirmNodeIdRef.current = nodeId;

      setConfirmHandler(async (approved: boolean, feedback?: string) => {
        if (confirmNodeIdRef.current) {
          updateNode(confirmNodeIdRef.current, {
            data: { ...confirmData, confirmed: approved, feedback },
            status: approved ? "completed" : "cancelled",
          });
          confirmNodeIdRef.current = null;
        }
        await confirmOperation(pendingConfirmation.operationId, approved, feedback);
        setConfirmHandler(null);
      });
    }
  }, [pendingConfirmation, addNode, updateNode, confirmOperation, setConfirmHandler]);

  // 发送用户消息
  const handleSend = useCallback(async (text: string) => {
    if (!text.trim()) return;

    // 检查是否正在等待用户确认，防止 UI 状态不一致
    if (confirmNodeIdRef.current !== null) {
      console.warn("[App] 正在等待操作确认，忽略新消息");
      return;
    }

    resetRefs();
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
    resetRefs();
  }, [clearNodes, resetAgent, clearCurrentSession, clearContextUsage, saveSessionToCache, currentSessionId]);

  // 切换到历史会话：先保存当前会话状态到缓存，再从缓存或后端恢复目标会话
  const handleSwitchSession = useCallback(async (sessionId: string, workspaceId?: string) => {
    // 仅当 workspaceId 是真实工作区且与当前不同时才切换
    if (workspaceId && workspaceId !== currentWorkspaceId && workspaces.some((w) => w.id === workspaceId)) {
      await switchWorkspace(workspaceId);
    }

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
    clearContextUsage();
    resetRefs();

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
    let hasMessages = false;
    try {
      const detail = await tauriCmd.getSession(sessionId);
      hasMessages = detail.messages.length > 0;
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

    // 仅当会话有消息时才加载上下文窗口使用信息
    // 新会话无消息，后端会回退计算系统提示词 token 数导致显示残留数据，应跳过
    if (hasMessages) {
      loadContextUsage(sessionId);
    }
  }, [clearNodes, resetAgent, clearContextUsage, switchSession, setAgentSessionId, loadFromMessages, loadContextUsage, saveSessionToCache, restoreSessionFromCache, getCachedStreamingRefs, setExecutionStatus, currentSessionId, switchWorkspace, currentWorkspaceId, workspaces]);

  // 为指定工作区新建会话并切换过去
  const handleCreateSessionForWorkspace = useCallback(async (workspaceId: string) => {
    const newSessionId = await createSession(undefined, workspaceId);
    if (workspaceId !== currentWorkspaceId) {
      await switchWorkspace(workspaceId);
    }
    await handleSwitchSession(newSessionId, workspaceId);
  }, [createSession, switchWorkspace, currentWorkspaceId, handleSwitchSession]);

  // 查看指定工作区的文件树
  const handleShowFilesForWorkspace = useCallback(async (workspaceId: string) => {
    if (workspaceId !== currentWorkspaceId) {
      await switchWorkspace(workspaceId);
    }
  }, [switchWorkspace, currentWorkspaceId]);

  // 删除当前会话后的处理：清空缓存，切换到其他会话或清空工作流
  const handleDeleteCurrentSession = useCallback(async (nextSessionId: string | null) => {
    // 清除被删除会话的缓存
    if (currentSessionId) {
      clearSessionCache(currentSessionId);
    }

    clearNodes();
    resetAgent();
    resetRefs();

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
    }
  }, [clearNodes, resetAgent, switchSession, setAgentSessionId, loadFromMessages, loadContextUsage, clearContextUsage, clearSessionCache, restoreSessionFromCache, getCachedStreamingRefs, setExecutionStatus, currentSessionId]);

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
      setPreviewContent(`[${t('preview.previewFailed')}] ${err instanceof Error ? err.message : String(err)}`);
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
    setPreviewTitle(t('preview.versionDiff'));
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

    resetRefs();
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
      // 忽略在输入框/文本域中的快捷键（除了特定的全局快捷键）
      const target = e.target as HTMLElement;
      const isInputFocused = target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;

      if (e.key === "Escape") {
        setPreviewOpen(false);
      }
      // Ctrl+N: 新建会话
      if (e.ctrlKey && e.key === "n") {
        e.preventDefault();
        handleNewSession();
      }
      // Ctrl+W: 关闭当前会话
      if (e.ctrlKey && e.key === "w") {
        e.preventDefault();
        if (currentSessionId) {
          handleNewSession();
        }
      }
      // Ctrl+B: 切换侧边栏
      if (e.ctrlKey && e.key === "b") {
        e.preventDefault();
        setSidebarVisible((prev) => !prev);
      }
      // Ctrl+,: 打开设置（仅在非输入框聚焦时生效）
      if (e.ctrlKey && e.key === "," && !isInputFocused) {
        e.preventDefault();
        useSettingsStore.getState().openSettings();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleNewSession, currentSessionId]);

  return (
    <div className="app flex flex-col h-screen">
      <NetworkStatusBanner />
      <TopBar onNewSession={handleNewSession} />

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
        sidebarVisible={sidebarVisible}
        sidebar={
          <RightSidebar
            onOpenPreview={handleOpenPreview}
            onOpenVersionHistory={handleOpenVersionHistory}
            onSwitchSession={handleSwitchSession}
            onCreateSession={handleCreateSessionForWorkspace}
            onShowFiles={handleShowFilesForWorkspace}
            onDeleteCurrentSession={handleDeleteCurrentSession}
          />
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
            {t('preview.loading')}
          </div>
        </div>
      )}
      <Suspense fallback={<LazyFallback />}>
        <SettingsDialog />
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
