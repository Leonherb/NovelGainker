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
  mergeTaskActivity,
  requestConversationSession,
  requestConversationTurnCancel,
  requestConversationTurnStart,
  requestConversationTurnRetry,
  type ConversationTurn,
  type ConversationSession,
  type ConversationRuntimeView,
  type ConversationTurnEvent,
  type ConversationTurnStatus,
  type TaskActivity,
} from "../lib/tauri/conversation";
import { normalizeAppError, type AppError } from "../lib/tauri/health";
import { PROVIDER_PRESENTATION } from "./provider-settings-model";
import { CanonChangeView } from "./CanonChangeView";
import {
  requestCanonChangesPage,
  type ChangeActivity,
  type ChapterCandidateActivity,
} from "../lib/tauri/canon-change";
import { ChapterCandidateView } from "./ChapterCandidateView";
import { PublicationView } from "./PublicationView";
import { usePublicationRecords } from "./use-publication-history";
import { ConversationEventCursor } from "../lib/conversation-event-cursor";

export type ConversationViewProps = {
  projectSessionId: string | null;
  conversationSessionId?: string | null;
  otherTurnActive?: boolean;
  readOnly?: boolean;
  onBranch?: (turnId: string) => void;
  initialDraft?: string;
  onDraftChange?: (text: string) => void;
  providerState: ProviderStateSnapshot;
  onRunActiveChange?: (active: boolean) => void;
  onCanonChanged?: () => void;
  changeRefreshToken?: number;
};

type ActiveTurn = {
  attemptId: string | null;
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

const TASK_STATUS_LABELS: Record<TaskActivity["status"], string> = {
  queued: "正在处理……",
  running: "正在处理……",
  succeeded: "已完成相关操作。",
  failed: "相关操作未完成。",
  cancelled: "相关操作未完成。",
};

export function ConversationView({
  projectSessionId,
  conversationSessionId = null,
  otherTurnActive = false,
  readOnly = false,
  onBranch,
  initialDraft = "",
  onDraftChange,
  providerState,
  onRunActiveChange = () => undefined,
  onCanonChanged,
  changeRefreshToken = 0,
}: ConversationViewProps) {
  const [session, setSession] = useState<ConversationSession | null>(null);
  const [taskActivities, setTaskActivities] = useState<TaskActivity[]>([]);
  const publicationTasks = taskActivities
    .filter(
      (task) =>
        task.displayLabel === "本地 EPUB 预检 / 构建" &&
        !["queued", "running"].includes(task.status),
    )
    .map((task) => `${task.taskId}:${task.taskRevision}`)
    .join("|");
  const lastStoredTurn = session?.turns[session.turns.length - 1];
  const publications = usePublicationRecords(
    projectSessionId,
    `${changeRefreshToken}:${lastStoredTurn?.turnId}:${lastStoredTurn?.status}:${publicationTasks}`,
  );
  const [changes, setChanges] = useState<ChangeActivity[]>([]);
  const [chapterCandidates, setChapterCandidates] = useState<ChapterCandidateActivity[]>([]);
  const [changeError, setChangeError] = useState(false);
  const [moreChanges, setMoreChanges] = useState(false);
  const changePagesRef = useRef(1);
  const changesBusyRef = useRef(false);
  const changesAgainRef = useRef(false);
  const savedSignatureRef = useRef("");
  const onCanonChangedRef = useRef(onCanonChanged);
  useEffect(() => {
    onCanonChangedRef.current = onCanonChanged;
  }, [onCanonChanged]);
  const loadChanges = useCallback(async () => {
    if (!projectSessionId) return;
    if (changesBusyRef.current) {
      changesAgainRef.current = true;
      return;
    }
    changesBusyRef.current = true;
    do {
      changesAgainRef.current = false;
      try {
        let page = await requestCanonChangesPage(
          projectSessionId,
          null,
          null,
          undefined,
          conversationSessionId,
        );
        const incoming = [...page.items];
        const candidates = page.candidates ?? [];
        const cursors = new Set<string>();
        for (
          let index = 1;
          index < changePagesRef.current && page.nextCursor !== null;
          index += 1
        ) {
          if (cursors.has(page.nextCursor)) throw new Error("Repeated change cursor");
          cursors.add(page.nextCursor);
          page = await requestCanonChangesPage(
            projectSessionId,
            page.nextCursor,
            null,
            undefined,
            conversationSessionId,
          );
          incoming.push(...page.items);
        }
        if (
          incoming.length > 2048 ||
          new Set(incoming.map((entry) => entry.receipt.changeId)).size !== incoming.length
        )
          throw new Error("Invalid change pagination");
        setChanges(incoming);
        setChapterCandidates(candidates);
        setMoreChanges(page.nextCursor !== null);
        setChangeError(false);
        const signature = incoming
          .filter((entry) => ["committed", "undone"].includes(entry.receipt.status))
          .map((entry) => `${entry.receipt.changeId}:${entry.receipt.revision}`)
          .sort()
          .join("|");
        if (signature !== savedSignatureRef.current) {
          savedSignatureRef.current = signature;
          onCanonChangedRef.current?.();
        }
      } catch {
        setChangeError(true);
        setChanges((current) =>
          current.map((entry) =>
            entry.chapterSourceState ? { ...entry, chapterSourceState: "unavailable" } : entry,
          ),
        );
      }
    } while (changesAgainRef.current);
    changesBusyRef.current = false;
  }, [projectSessionId, conversationSessionId]);
  useEffect(() => {
    let disposed = false;
    if (changeRefreshToken > 0)
      queueMicrotask(() => {
        if (!disposed) void loadChanges();
      });
    return () => {
      disposed = true;
    };
  }, [changeRefreshToken, loadChanges]);
  const [draft, setDraft] = useState(initialDraft);
  useEffect(() => {
    onDraftChange?.(draft);
  }, [draft, onDraftChange]);
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
  const eventCursorRef = useRef(new ConversationEventCursor());
  const retryBusyRef = useRef(false);

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
      setTaskActivities([]);
      return null;
    }
    const loaded = await requestConversationSession(projectSessionId, conversationSessionId);
    if (conversationSessionId !== null && loaded.conversation.sessionId !== conversationSessionId) {
      throw { code: "contract.invalid_response", messageKey: "errors.invalidRuntimeResponse" };
    }
    if (eventCursorRef.current.isOlder(loaded.conversation)) return null;
    void loadChanges();
    eventCursorRef.current.observe(loaded.conversation);
    setSession(loaded.conversation);
    setTaskActivities((current) => loaded.taskActivities.reduce(mergeTaskActivity, current));
    return loaded;
  }, [projectSessionId, conversationSessionId, loadChanges]);

  const applyLoadedSession = useCallback(
    (loaded: ConversationRuntimeView, expectedTurnId: string | null) => {
      const conversation = loaded.conversation;
      const expectedTurn = expectedTurnId
        ? conversation.turns.find((turn) => turn.turnId === expectedTurnId)
        : undefined;
      const runningTurn =
        expectedTurn?.status === "running"
          ? expectedTurn
          : expectedTurnId
            ? undefined
            : [...conversation.turns].reverse().find((turn) => turn.status === "running");

      setSynchronizationFailure(null);
      setStarting(false);

      if (runningTurn) {
        trackTurn(runningTurn.turnId);
        setActiveTurn((current) =>
          current?.turnId === runningTurn.turnId &&
          current.attemptId === (runningTurn.retryState?.attemptId ?? current.attemptId)
            ? current
            : {
                turnId: runningTurn.turnId,
                attemptId: runningTurn.retryState?.attemptId ?? null,
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
      if (
        !active ||
        event.sessionId !== projectSessionId ||
        (conversationSessionId !== null && event.conversationSessionId !== conversationSessionId) ||
        !eventCursorRef.current.accept(event)
      ) {
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
                attemptId: event.attemptId,
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

      if (event.kind === "taskActivity") {
        trackTurn(event.turnId);
        setTaskActivities((current) => mergeTaskActivity(current, event.taskActivity));
        void loadChanges();
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
    loadChanges,
    loadSession,
    projectSessionId,
    conversationSessionId,
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
      otherTurnActive ||
      readOnly ||
      draft.trim().length === 0
    ) {
      return;
    }

    const message = draft;
    setStarting(true);
    setError(null);
    try {
      const started = await requestConversationTurnStart(
        projectSessionId,
        message,
        conversationSessionId,
      );
      eventCursorRef.current.expect(started);
      setDraft("");
      trackTurn(started.turnId);
      setStarting(false);
      setActiveTurn({
        turnId: started.turnId,
        attemptId: started.attemptId,
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

  const retryTurn = async (turn: ConversationTurn) => {
    if (
      !projectSessionId ||
      !session ||
      !turn.retryState ||
      turn.retryState.outcome !== "eligible" ||
      turn.turnId !== lastStoredTurn?.turnId ||
      readOnly ||
      runActive ||
      otherTurnActive ||
      synchronizationBlocked ||
      retryBusyRef.current
    )
      return;
    retryBusyRef.current = true;
    setStarting(true);
    setError(null);
    trackTurn(turn.turnId);
    try {
      const started = await requestConversationTurnRetry(
        projectSessionId,
        session.sessionId,
        turn.turnId,
        turn.retryState.attemptId,
      );
      if (
        started.turnId !== turn.turnId ||
        started.conversationSessionId !== session.sessionId ||
        started.attemptNumber !== 2
      )
        throw new Error("Retry identity mismatch");
      eventCursorRef.current.expect(started);
      setActiveTurn({
        turnId: started.turnId,
        attemptId: started.attemptId,
        providerId: started.providerId,
        model: started.model,
        text: "",
        phase: "connecting",
      });
      await reconcileTurn(turn.turnId);
    } catch (failure: unknown) {
      setError(normalizeAppError(failure));
      // Lost acknowledgement reconciles Store only, never resubmits the request.
      await reconcileTurn(turn.turnId).catch(() => undefined);
    } finally {
      setStarting(false);
      retryBusyRef.current = false;
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
      const result = await requestConversationTurnCancel(
        projectSessionId,
        turnId,
        undefined,
        activeTurn.attemptId,
        session?.sessionId ?? conversationSessionId,
      );
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

  const composerHint = readOnly
    ? "当前会话只读；可以保留输入草稿，但不能发送。"
    : !projectSessionId
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

  const taskForTurn = (turnId: string) =>
    taskActivities
      .filter((activity) => activity.turnId === turnId)
      .sort((left, right) => right.taskRevision - left.taskRevision)[0];

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
                {onBranch && turn.status !== "running" && index === turn.messages.length - 1 ? (
                  <button
                    type="button"
                    disabled={runActive || otherTurnActive}
                    onClick={() => onBranch(turn.turnId)}
                  >
                    从此轮分支
                  </button>
                ) : null}
                {message.role === "assistant" && index === turn.messages.length - 1 ? (
                  <>
                    {projectSessionId
                      ? chapterCandidates
                          .filter((candidate) => candidate.turnId === turn.turnId)
                          .map((candidate) => (
                            <ChapterCandidateView
                              key={candidate.candidateId}
                              sessionId={projectSessionId}
                              candidate={candidate}
                              conversationSessionId={session.sessionId}
                              onChanged={() => void loadChanges()}
                            />
                          ))
                      : null}
                    {projectSessionId
                      ? changes
                          .filter(
                            (entry) =>
                              entry.turnId === turn.turnId &&
                              entry.conversationSessionId === session.sessionId,
                          )
                          .map((activity) => (
                            <CanonChangeView
                              key={activity.receipt.changeId}
                              sessionId={projectSessionId}
                              activity={activity}
                              confirmationDisabled={readOnly}
                              onChanged={() => void loadChanges()}
                            />
                          ))
                      : null}
                    {taskForTurn(turn.turnId) ? (
                      <div className="conversation-task-activity">
                        {TASK_STATUS_LABELS[taskForTurn(turn.turnId).status]}
                      </div>
                    ) : null}
                    {projectSessionId
                      ? publications.items
                          .filter(
                            (item) =>
                              item.sessionId === session.sessionId && item.turnId === turn.turnId,
                          )
                          .map((receipt) => (
                            <PublicationView
                              key={`${receipt.artifactId}:${receipt.status}`}
                              projectSessionId={projectSessionId}
                              receipt={receipt}
                            />
                          ))
                      : null}
                    <footer>{STATUS_LABELS[turn.status]}</footer>
                    {turn.turnId === lastStoredTurn?.turnId &&
                    ["failed", "not_generated", "cancelled"].includes(turn.status) ? (
                      <div className="conversation-task-activity">
                        {turn.retryState?.outcome === "eligible" ? (
                          <>
                            <p>
                              本轮没有生成正文或执行工具。可用原消息重试一次，会重新请求模型并计入原轮次预算；不是断点续传。
                            </p>
                            <button
                              type="button"
                              disabled={
                                readOnly ||
                                runActive ||
                                otherTurnActive ||
                                synchronizationBlocked ||
                                !providerState.providers.some(
                                  (p) => p.providerId === turn.providerId && p.configured,
                                )
                              }
                              onClick={() => void retryTurn(turn)}
                            >
                              用原消息重试一次
                            </button>
                          </>
                        ) : (
                          <p>
                            本轮不能整轮重放。原内容及已保存结果会保留；如需继续，请确认后发送新的一轮。
                          </p>
                        )}
                        <button
                          type="button"
                          disabled={readOnly || runActive || otherTurnActive || draft.length > 0}
                          onClick={() => setDraft(turn.messages[0].content)}
                        >
                          准备新一轮（发送前可修改）
                        </button>
                      </div>
                    ) : null}
                  </>
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
              {projectSessionId
                ? changes
                    .filter((entry) => entry.turnId === activeTurn.turnId)
                    .map((activity) => (
                      <CanonChangeView
                        key={activity.receipt.changeId}
                        sessionId={projectSessionId}
                        activity={activity}
                        confirmationDisabled={readOnly}
                        onChanged={() => void loadChanges()}
                      />
                    ))
                : null}
              {taskForTurn(activeTurn.turnId) ? (
                <div
                  className="conversation-task-activity"
                  aria-atomic="true"
                  aria-live="polite"
                  role="status"
                >
                  {TASK_STATUS_LABELS[taskForTurn(activeTurn.turnId).status]}
                </div>
              ) : null}
            </article>
          ) : null}
          {projectSessionId && session
            ? changes
                .filter(
                  (entry) =>
                    entry.conversationSessionId === session.sessionId &&
                    entry.turnId !== activeTurn?.turnId &&
                    !session.turns.some((turn) => turn.turnId === entry.turnId),
                )
                .map((activity) => (
                  <div key={activity.receipt.changeId} className="conversation-local-change">
                    <p>本地文件保存记录（不是聊天消息）</p>
                    <CanonChangeView
                      sessionId={projectSessionId}
                      activity={activity}
                      confirmationDisabled={readOnly}
                      onChanged={() => void loadChanges()}
                    />
                  </div>
                ))
            : null}
          {moreChanges ? (
            <button
              type="button"
              onClick={() => {
                changePagesRef.current = Math.min(changePagesRef.current + 1, 21);
                void loadChanges();
              }}
            >
              加载更早的保存记录（每页最多 100 条）
            </button>
          ) : null}
          {publications.more ? (
            <button type="button" disabled={publications.busy} onClick={publications.loadMore}>
              加载更早的出版记录（每页最多 20 条）
            </button>
          ) : null}
        </div>
      </div>

      <form className="composer-shell" onSubmit={submit}>
        {publications.failed ? (
          <div className="canon-change-load-error" role="status">
            出版结果暂时无法核对，聊天仍可继续；先前回执不代表当前产物仍有效。
            <button type="button" disabled={publications.busy} onClick={publications.refresh}>
              重新核对出版结果
            </button>
          </div>
        ) : null}
        {changeError ? (
          <div className="canon-change-load-error" role="status">
            保存结果暂时无法核对，聊天仍可继续。
            <button type="button" onClick={() => void loadChanges()}>
              重新核对保存结果
            </button>
          </div>
        ) : null}
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
                otherTurnActive ||
                readOnly ||
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
