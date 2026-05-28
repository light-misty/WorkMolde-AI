import { useState, useCallback, useEffect, useRef } from "react";

import * as tauriCmd from "../services/tauri";
import {
  onAgentThinking,
  onAgentDeepThinking,
  onAgentContent,
  onAgentToolCall,
  onAgentToolResult,
  onAgentConfirm,
  onAgentTodoUpdate,
  onAgentDone,
  onAgentError,
  onAgentStopped,
  type ThinkingPayload,
  type DeepThinkingPayload,
  type ToolCallPayload,
  type ToolResultPayload,
  type ConfirmPayload,
  type TodoUpdatePayload,
  type DonePayload,
} from "../services/event";

export interface UseAgentReturn {
  isLoading: boolean;
  error: string | null;
  sessionId: string | null;
  lastThinking: ThinkingPayload | null;
  deepThinking: DeepThinkingPayload | null;
  content: string;
  currentToolCall: ToolCallPayload | null;
  lastToolResult: ToolResultPayload | null;
  pendingConfirmation: ConfirmPayload | null;
  todos: TodoUpdatePayload | null;
  doneResult: DonePayload | null;
  isStopped: boolean;
  sendMessage: (prompt: string, options?: Record<string, unknown>) => Promise<void>;
  stopAgent: () => Promise<void>;
  confirmOperation: (operationId: string, approved: boolean, feedback?: string) => Promise<void>;
  reset: () => void;
  setSessionId: (id: string) => void;
}

const initialState = {
  isLoading: false,
  error: null as string | null,
  sessionId: null as string | null,
  lastThinking: null as ThinkingPayload | null,
  deepThinking: null as DeepThinkingPayload | null,
  content: "",
  currentToolCall: null as ToolCallPayload | null,
  lastToolResult: null as ToolResultPayload | null,
  pendingConfirmation: null as ConfirmPayload | null,
  todos: null as TodoUpdatePayload | null,
  doneResult: null as DonePayload | null,
  isStopped: false,
};

export function useAgent(): UseAgentReturn {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [lastThinking, setLastThinking] = useState<ThinkingPayload | null>(null);
  const [deepThinking, setDeepThinking] = useState<DeepThinkingPayload | null>(null);
  const [content, setContent] = useState("");
  const [currentToolCall, setCurrentToolCall] = useState<ToolCallPayload | null>(null);
  const [lastToolResult, setLastToolResult] = useState<ToolResultPayload | null>(null);
  const [pendingConfirmation, setPendingConfirmation] = useState<ConfirmPayload | null>(null);
  const [todos, setTodos] = useState<TodoUpdatePayload | null>(null);
  const [doneResult, setDoneResult] = useState<DonePayload | null>(null);
  const [isStopped, setIsStopped] = useState(false);

  const unlistenRefs = useRef<(() => void)[]>([]);
  const sessionIdRef = useRef<string | null>(null);
  const contentEpochRef = useRef(0);
  const lastContentEpochRef = useRef(0);
  // 深度思考链内容累积：后端每次发送增量 delta，在 ref 中同步累积避免 React 批量渲染丢失
  const deepThinkingContentRef = useRef("");
  // 追踪上一次深度思考的 step，用于检测新一轮思考开始
  const lastDeepThinkingStepRef = useRef(0);

  useEffect(() => {
    sessionIdRef.current = sessionId;
  }, [sessionId]);

  useEffect(() => {
    let cancelled = false;

    const registerListeners = async () => {
      const unlisteners = await Promise.all([
        onAgentThinking((payload) => {
          if (payload.sessionId !== sessionIdRef.current) return;
          setLastThinking(payload);
        }),
        onAgentDeepThinking((payload) => {
          if (payload.sessionId !== sessionIdRef.current) return;
          if (payload.isStreaming) {
            // step 变化表示新一轮思考开始，重置累积内容并递增 epoch
            if (payload.step !== lastDeepThinkingStepRef.current) {
              lastDeepThinkingStepRef.current = payload.step;
              deepThinkingContentRef.current = "";
              contentEpochRef.current += 1;
            }
            // 同步累积增量 delta，避免 React 批量渲染导致中间 chunk 丢失
            deepThinkingContentRef.current += payload.thought;
          }
          // 将累积的完整内容传递给状态，而非仅传递当前 delta
          setDeepThinking({
            ...payload,
            thought: deepThinkingContentRef.current,
          });
        }),
        onAgentContent((payload) => {
          if (payload.sessionId !== sessionIdRef.current) return;
          if (contentEpochRef.current !== lastContentEpochRef.current) {
            lastContentEpochRef.current = contentEpochRef.current;
            setContent(payload.content);
          } else {
            setContent((prev) => prev + payload.content);
          }
        }),
        onAgentToolCall((payload) => {
          if (payload.sessionId !== sessionIdRef.current) return;
          contentEpochRef.current += 1;
          setCurrentToolCall(payload);
        }),
        onAgentToolResult((payload) => {
          if (payload.sessionId !== sessionIdRef.current) return;
          setLastToolResult(payload);
          setCurrentToolCall(null);
        }),
        onAgentConfirm((payload) => {
          if (payload.sessionId !== sessionIdRef.current) return;
          setPendingConfirmation(payload);
        }),
        onAgentTodoUpdate((payload) => {
          if (payload.sessionId !== sessionIdRef.current) return;
          setTodos(payload);
        }),
        onAgentDone((payload) => {
          if (payload.sessionId !== sessionIdRef.current) return;
          setIsLoading(false);
          setDoneResult(payload);
        }),
        onAgentError((payload) => {
          if (payload.sessionId !== sessionIdRef.current) return;
          setIsLoading(false);
          setError(payload.message);
        }),
        onAgentStopped((payload) => {
          if (payload.sessionId !== sessionIdRef.current) return;
          setIsLoading(false);
          setIsStopped(true);
        }),
      ]);

      if (cancelled) {
        unlisteners.forEach((fn) => fn());
        return;
      }

      unlistenRefs.current = unlisteners;
    };

    registerListeners();

    return () => {
      cancelled = true;
      unlistenRefs.current.forEach((unlisten) => unlisten());
      unlistenRefs.current = [];
    };
  }, []);

  const sendMessage = useCallback(
    async (prompt: string, options?: Record<string, unknown>) => {
      setError(null);
      setContent("");
      setLastThinking(null);
      setDeepThinking(null);
      setCurrentToolCall(null);
      setLastToolResult(null);
      setPendingConfirmation(null);
      setTodos(null);
      setDoneResult(null);
      setIsStopped(false);
      setIsLoading(true);
      contentEpochRef.current += 1;
      lastContentEpochRef.current = contentEpochRef.current;
      deepThinkingContentRef.current = "";
      lastDeepThinkingStepRef.current = 0;

      try {
        let sid = sessionId;
        if (!sid) {
          const session = await tauriCmd.createSession({});
          sid = session.id;
          setSessionId(sid);
          sessionIdRef.current = sid;
        }

        await tauriCmd.startAgent(sid, prompt, options);
      } catch (err) {
        setIsLoading(false);
        // 从 Tauri invoke 错误对象中提取 message，避免 `[object Object]`
        let errorMessage: string;
        if (err instanceof Error) {
          errorMessage = err.message;
        } else if (err && typeof err === "object") {
          errorMessage = (err as Record<string, unknown>)?.message as string ?? JSON.stringify(err);
        } else {
          errorMessage = String(err);
        }
        setError(errorMessage);
      }
    },
    [sessionId],
  );

  const stopAgent = useCallback(async () => {
    if (!sessionId) return;

    try {
      await tauriCmd.stopAgent(sessionId);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [sessionId]);

  const confirmOperation = useCallback(
    async (operationId: string, approved: boolean, feedback?: string) => {
      if (!sessionId) return;

      try {
        await tauriCmd.confirmOperation(sessionId, operationId, approved, feedback);
        setPendingConfirmation(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [sessionId],
  );

  const reset = useCallback(() => {
    setIsLoading(initialState.isLoading);
    setError(initialState.error);
    setSessionId(initialState.sessionId);
    setLastThinking(initialState.lastThinking);
    setDeepThinking(initialState.deepThinking);
    setContent(initialState.content);
    setCurrentToolCall(initialState.currentToolCall);
    setLastToolResult(initialState.lastToolResult);
    setPendingConfirmation(initialState.pendingConfirmation);
    setTodos(initialState.todos);
    setDoneResult(initialState.doneResult);
    setIsStopped(initialState.isStopped);
    deepThinkingContentRef.current = "";
    lastDeepThinkingStepRef.current = 0;
  }, []);

  const setSessionIdExternal = useCallback((id: string) => {
    setSessionId(id);
    sessionIdRef.current = id;
  }, []);

  return {
    isLoading,
    error,
    sessionId,
    lastThinking,
    deepThinking,
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
    reset,
    setSessionId: setSessionIdExternal,
  };
}
