import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
} from "react";
import type { ProviderStateSnapshot } from "../lib/tauri/provider";
import {
  listenConversationTurnEvents,
  requestConversationSession,
  requestConversationTurnCancel,
  requestConversationTurnStart,
  type ConversationSession,
  type ConversationTurnEvent,
  type ConversationTurnStatus,
} from "../lib/tauri/conversation";
import { normalizeAppError, type AppError } from "../lib/tauri/health";
import { PROVIDER_PRESENTATION } from "./provider-settings-model";

type ConversationViewProps = {
  projectSessionId: string | null;
  providerState: ProviderStateSnapshot;
  onRunActiveChange?: (active: boolean) => void;
};

type ActiveTurn = {
  turnId: string;
  providerId: "deepseek" | "openai";
  model: string;
  text: string;
  phase: "connecting" | "replying" | "stopping";
};

type ListenerState = "idle" | "pending" | "ready" | "failed";

const SESSION_RECONCILIATION_INTERVAL_MS = 1_000;

const STATUS_LABELS: Record<ConversationTurnStatus, string> = {
  running: "正在回复",
  completed: "已完成",
  cancelled: "已停止 · 未完成",
  failed: "未完成",
  not_generated: "未生成",
};

export function ConversationView({
  projectSessionId,
  providerState,
  onRunActiveChange = () => undefined,
}: ConversationViewProps) {
  const [session, setSession] = useState<ConversationSession | null>(null);
  const [draft, setDraft] = useState("");
  const [activeTurn, setActiveTurn] = useState<ActiveTurn | null>(null);
  const [trackedTurnId, setTrackedTurnId] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const [listenerState, setListenerState] = useState<ListenerState>(
    projectSessionId ? "pending" : "idle",
  );
  const [listenerAttempt, setListenerAttempt] = useState(0);
  const [synchronizationFailure, setSynchronizationFailure] = useState<AppError | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const messageCanvasRef = useRef<HTMLDivElement>(null);
  const trackedTurnIdRef = useRef<string | null>(null);

  const selectedProvider = providerState.providers.find(
    (provider) => provider.providerId === providerState.selectedProviderId,
  );
  const providerName = PROVIDER_PRESENTATION[providerState.selectedProviderId].name;
  const runActive = starting || trackedTurnId !== null || activeTurn !== null;
  const synchronizationBlocked = listenerState !== "ready" || synchronizationFailure !== null;

  const trackTurn = useCallback((turnId: string | null) => {
    trackedTurnIdRef.current = turnId;
    setTrackedTurnId(turnId);
  }, []);

  useEffect(() => {
    onRunActiveChange(runActive);
  }, [onRunActiveChange, runActive]);

  const loadSession = useCallback(async () => {
    if (!projectSessionId) {
      setSession(null);
      return null;
    }
    const loaded = await requestConversationSession(projectSessionId);
    setSession(loaded);
    return loaded;
  }, [projectSessionId]);

  const applyLoadedSession = useCallback(
    (loaded: ConversationSession, expectedTurnId: string | null) => {
      const expectedTurn = expectedTurnId
        ? loaded.turns.find((turn) => turn.turnId === expectedTurnId)
        : undefined;
      const runningTurn =
        expectedTurn?.status === "running"
          ? expectedTurn
          : expectedTurnId
            ? undefined
            : [...loaded.turns].reverse().find((turn) => turn.status === "running");

      setSynchronizationFailure(null);
      setStarting(false);

      if (runningTurn) {
        trackTurn(runningTurn.turnId);
        setActiveTurn((current) =>
          current?.turnId === runningTurn.turnId
            ? current
            : {
                turnId: runningTurn.turnId,
                providerId: runningTurn.providerId,
                model: runningTurn.model,
                text: "",
                phase: "replying",
              },
        );
        return;
      }

      if (!expectedTurnId || trackedTurnIdRef.current === expectedTurnId) {
        trackTurn(null);
        setActiveTurn(null);
      }
    },
    [trackTurn],
  );

  const recordSynchronizationFailure = useCallback((failure: unknown) => {
    setSynchronizationFailure(normalizeAppError(failure));
    setStarting(false);
    setActiveTurn(null);
  }, []);

  const reconcileTurn = useCallback(
    async (turnId: string) => {
      try {
        const loaded = await loadSession();
        if (loaded) applyLoadedSession(loaded, turnId);
      } catch (failure: unknown) {
        if (trackedTurnIdRef.current === turnId) {
          recordSynchronizationFailure(failure);
        }
        throw failure;
      }
    },
    [applyLoadedSession, loadSession, recordSynchronizationFailure],
  );

  useEffect(() => {
    if (!projectSessionId) {
      return undefined;
    }

    let active = true;
    let unlisten: (() => void) | undefined;

    const handleEvent = (event: ConversationTurnEvent) => {
      if (!active || event.sessionId !== projectSessionId) {
        return;
      }

      if (event.kind === "textDelta") {
        trackTurn(event.turnId);
        setActiveTurn((current) =>
          current?.turnId === event.turnId
            ? {
                ...current,
                text: current.text + event.delta,
                phase: "replying",
              }
            : {
                turnId: event.turnId,
                providerId: event.providerId,
                model: event.model,
                text: event.delta,
                phase: "replying",
              },
        );
        return;
      }

      if (event.kind === "usage") {
        return;
      }

      trackTurn(event.turnId);
      setActiveTurn(null);
      setStarting(false);
      void reconcileTurn(event.turnId).catch(() => undefined);
    };

    void listenConversationTurnEvents(handleEvent)
      .then(async (dispose) => {
        if (!active) {
          dispose();
          return;
        }
        unlisten = dispose;
        setListenerState("ready");
        try {
          const loaded = await loadSession();
          if (active && loaded) {
            applyLoadedSession(loaded, trackedTurnIdRef.current);
          }
        } catch (failure: unknown) {
          if (active) recordSynchronizationFailure(failure);
        }
      })
      .catch((failure: unknown) => {
        if (!active) return;
        setListenerState("failed");
        recordSynchronizationFailure(failure);
      });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [
    applyLoadedSession,
    listenerAttempt,
    loadSession,
    projectSessionId,
    reconcileTurn,
    recordSynchronizationFailure,
    trackTurn,
  ]);

  useEffect(() => {
    if (
      !projectSessionId ||
      !trackedTurnId ||
      listenerState !== "ready" ||
      synchronizationFailure
    ) {
      return undefined;
    }

    let active = true;
    let timeoutId: ReturnType<typeof setTimeout> | undefined;

    const schedule = () => {
      timeoutId = setTimeout(() => {
        void reconcileTurn(trackedTurnId)
          .then(() => {
            if (active && trackedTurnIdRef.current === trackedTurnId) {
              schedule();
            }
          })
          .catch(() => undefined);
      }, SESSION_RECONCILIATION_INTERVAL_MS);
    };

    schedule();
    return () => {
      active = false;
      if (timeoutId) clearTimeout(timeoutId);
    };
  }, [listenerState, projectSessionId, reconcileTurn, synchronizationFailure, trackedTurnId]);

  useEffect(() => {
    const canvas = messageCanvasRef.current;
    if (canvas) canvas.scrollTop = canvas.scrollHeight;
  }, [activeTurn?.text, session]);

  const send = async () => {
    if (
      !projectSessionId ||
      !selectedProvider?.configured ||
      synchronizationBlocked ||
      runActive ||
      draft.trim().length === 0
    ) {
      return;
    }

    const message = draft;
    setStarting(true);
    setError(null);
    try {
      const started = await requestConversationTurnStart(projectSessionId, message);
      setDraft("");
      trackTurn(started.turnId);
      setStarting(false);
      setActiveTurn({
        turnId: started.turnId,
        providerId: started.providerId,
        model: started.model,
        text: "",
        phase: "connecting",
      });
      await reconcileTurn(started.turnId);
    } catch (failure: unknown) {
      setError(normalizeAppError(failure));
      setStarting(false);
      setActiveTurn(null);
      if (!trackedTurnIdRef.current) {
        void loadSession()
          .then((loaded) => {
            if (loaded) applyLoadedSession(loaded, null);
          })
          .catch(() => undefined);
      }
    }
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    void send();
  };

  const handleComposerKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void send();
    }
  };

  const stop = async () => {
    if (!projectSessionId || !activeTurn || activeTurn.phase === "stopping") {
      return;
    }
    const turnId = activeTurn.turnId;
    setActiveTurn((current) =>
      current?.turnId === turnId ? { ...current, phase: "stopping" } : current,
    );
    try {
      const result = await requestConversationTurnCancel(projectSessionId, turnId);
      if (!result.cancelRequested) {
        await reconcileTurn(turnId);
      }
    } catch (failure: unknown) {
      setError(normalizeAppError(failure));
      setActiveTurn((current) =>
        current?.turnId === turnId ? { ...current, phase: "replying" } : current,
      );
    }
  };

  const retrySynchronization = () => {
    setError(null);
    setSynchronizationFailure(null);
    if (listenerState === "failed") {
      setListenerState("pending");
      setListenerAttempt((attempt) => attempt + 1);
      return;
    }

    const turnId = trackedTurnIdRef.current;
    if (turnId) {
      void reconcileTurn(turnId).catch(() => undefined);
      return;
    }

    void loadSession()
      .then((loaded) => {
        if (loaded) applyLoadedSession(loaded, null);
      })
      .catch(recordSynchronizationFailure);
  };

  const composerHint = !projectSessionId
    ? "请先新建或打开一个小说工程。"
    : listenerState === "pending"
      ? "正在准备对话连接。"
      : listenerState === "failed" || synchronizationFailure
        ? "状态同步失败，请重试。"
        : !selectedProvider?.configured
          ? "请先在左侧配置 " + providerName + "。"
          : activeTurn?.phase === "stopping"
            ? "正在停止，本次已生成的内容会保留。"
            : trackedTurnId && !activeTurn
              ? "正在同步本次回复状态。"
              : runActive
                ? "AI 正在回复。"
                : "Enter 发送，Shift + Enter 换行";

  return (
    <section className="conversation-runtime" aria-label="小说创作对话消息">
      <div className="message-canvas" ref={messageCanvasRef}>
        <div className="conversation-message-list" aria-live="polite">
          {!projectSessionId ? (
            <div className="conversation-empty-state">
              <p>请先新建或打开一个小说工程。</p>
            </div>
          ) : session?.turns.length === 0 && !activeTurn ? (
            <div className="conversation-empty-state">
              <strong>从任何话题开始</strong>
              <p>可以闲聊、构思、讨论人物，也可以直接开始写正文。</p>
            </div>
          ) : null}

          {session?.turns.flatMap((turn) =>
            turn.messages.map((message, index) => (
              <article
                className="conversation-message"
                data-role={message.role}
                key={message.messageId}
              >
                <header>
                  <strong>
                    {message.role === "user" ? "你" : PROVIDER_PRESENTATION[turn.providerId].name}
                  </strong>
                  <span>{message.role === "user" ? "已保存" : turn.model}</span>
                </header>
                <p>{message.content}</p>
                {message.role === "assistant" && index === turn.messages.length - 1 ? (
                  <footer>{STATUS_LABELS[turn.status]}</footer>
                ) : null}
              </article>
            )),
          )}

          {activeTurn && !synchronizationFailure ? (
            <article className="conversation-message" data-role="assistant" data-streaming="true">
              <header>
                <strong>{PROVIDER_PRESENTATION[activeTurn.providerId].name}</strong>
                <span>
                  {activeTurn.phase === "connecting"
                    ? "正在连接"
                    : activeTurn.phase === "stopping"
                      ? "正在停止"
                      : "正在回复"}
                </span>
              </header>
              <p>{activeTurn.text || "…"}</p>
            </article>
          ) : null}
        </div>
      </div>

      <form className="composer-shell" onSubmit={submit}>
        {synchronizationFailure ? (
          <div className="composer-error" role="alert">
            <strong>状态同步失败</strong>
            <span>无法确认本次回复状态，请重试状态同步。</span>
            <code>{synchronizationFailure.code}</code>
            <button onClick={retrySynchronization} type="button">
              重试状态同步
            </button>
          </div>
        ) : error ? (
          <div className="composer-error" role="alert">
            <strong>本次操作未完成</strong>
            <span>请检查 Provider 配置或稍后重试。</span>
            <code>{error.code}</code>
          </div>
        ) : null}
        <textarea
          aria-label="对话输入"
          disabled={!projectSessionId || synchronizationBlocked || runActive}
          maxLength={32 * 1024}
          onChange={(event) => setDraft(event.currentTarget.value)}
          onKeyDown={handleComposerKeyDown}
          placeholder="和 AI 自然聊聊，或直接交代你想写的内容……"
          rows={3}
          value={draft}
        />
        <div className="composer-footer">
          <span>{composerHint}</span>
          {activeTurn && !synchronizationFailure ? (
            <button
              className="conversation-stop-button"
              disabled={activeTurn.phase === "stopping"}
              onClick={() => void stop()}
              type="button"
            >
              {activeTurn.phase === "stopping" ? "正在停止" : "停止"}
            </button>
          ) : (
            <button
              className="send-button"
              disabled={
                !projectSessionId ||
                !selectedProvider?.configured ||
                synchronizationBlocked ||
                runActive ||
                draft.trim().length === 0
              }
              type="submit"
            >
              发送
            </button>
          )}
        </div>
      </form>
    </section>
  );
}
