import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { presentAppError } from "../errors/present-app-error";
import { ZH_CN } from "../i18n/zh-CN";
import {
  listenAgentTurnEvents,
  requestAgentSession,
  requestAgentSessionClear,
  requestAgentPolicySet,
  requestAgentSkillRegistry,
  requestAgentTurnCancel,
  requestAgentTurnStart,
  requestAgentUndoApply,
  requestAgentWriteConfirm,
  requestAgentWriteReject,
  type AgentSessionSnapshot,
  type AgentSkillRegistry,
  type AgentTurnEvent,
  type AgentTurnStarted,
  type AgentWritePreview,
} from "../lib/tauri/agent";
import type { AppError } from "../lib/tauri/health";
import type { ProviderStateSnapshot } from "../lib/tauri/provider";
import { PROVIDER_PRESENTATION } from "./provider-settings-model";

const MAX_INPUT_BYTES = 16 * 1024;

type AgentTimelinePanelProps = {
  desktopRuntime: boolean;
  providerState: ProviderStateSnapshot;
  projectSessionId: string | null;
  onRunActiveChange?: (active: boolean) => void;
  onProjectCreateRequested?: () => void;
};

type TimelineMessage = {
  id: string;
  role: "user" | "assistant";
  text: string;
  status: "sent" | "completed" | "cancelled" | "failed";
};

type TimelineTool = {
  callId: string;
  toolId: string;
  purpose: string;
  path: string;
  status: "running" | "succeeded" | "failed" | "cancelled";
  sourcePaths: string[];
  truncated: boolean;
};

function byteLength(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function asAppError(error: unknown): AppError {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string" &&
    "messageKey" in error &&
    typeof error.messageKey === "string"
  ) {
    return error as AppError;
  }
  return { code: "runtime.unknown", messageKey: "errors.unknown" };
}

export function AgentTimelinePanel({
  desktopRuntime,
  providerState,
  projectSessionId,
  onRunActiveChange,
  onProjectCreateRequested,
}: AgentTimelinePanelProps) {
  const [registry, setRegistry] = useState<AgentSkillRegistry | null>(null);
  const [session, setSession] = useState<AgentSessionSnapshot | null>(null);
  const [messages, setMessages] = useState<TimelineMessage[]>([]);
  const [tools, setTools] = useState<TimelineTool[]>([]);
  const [input, setInput] = useState("");
  const [activeRun, setActiveRun] = useState<AgentTurnStarted | null>(null);
  const [starting, setStarting] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [loadingSession, setLoadingSession] = useState(false);
  const [turnState, setTurnState] = useState<string | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [clearConfirm, setClearConfirm] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [writePreviews, setWritePreviews] = useState<AgentWritePreview[]>([]);
  const [writeBusyId, setWriteBusyId] = useState<string | null>(null);
  const [undoBusyHash, setUndoBusyHash] = useState<string | null>(null);
  const [policyBusy, setPolicyBusy] = useState(false);
  const activeRunRef = useRef<AgentTurnStarted | null>(null);
  const lastSequenceRef = useRef(0);
  const earlyEventsRef = useRef<AgentTurnEvent[]>([]);
  const submittedInputRef = useRef<string | null>(null);
  const selectedProvider =
    providerState.providers.find((provider) => provider.selected) ?? providerState.providers[0];
  const presentation = PROVIDER_PRESENTATION[selectedProvider.providerId];
  const providerReady =
    selectedProvider.configured && selectedProvider.connectionStatus === "connected";
  const trimmedInput = input.trim();
  const inputBytes = useMemo(() => byteLength(trimmedInput), [trimmedInput]);
  const active = activeRun !== null;

  useEffect(() => {
    activeRunRef.current = activeRun;
    onRunActiveChange?.(active);
  }, [active, activeRun, onRunActiveChange]);

  useEffect(() => {
    if (!desktopRuntime || !projectSessionId) return undefined;
    let disposed = false;
    void requestAgentSkillRegistry()
      .then((value) => {
        if (!disposed) setRegistry(value);
      })
      .catch((loadError: unknown) => {
        if (!disposed) setError(asAppError(loadError));
      });
    return () => {
      disposed = true;
    };
  }, [desktopRuntime, projectSessionId]);

  const loadSession = useCallback(
    async (preserveError = false) => {
      if (!desktopRuntime || !projectSessionId || activeRunRef.current) {
        if (!projectSessionId) {
          setSession(null);
          setError(null);
        }
        return;
      }
      setLoadingSession(true);
      try {
        const value = await requestAgentSession(projectSessionId);
        setSession(value);
        setWritePreviews((current) =>
          value.pendingWrites.length
            ? value.pendingWrites
            : current.filter((preview) => !preview.canConfirm),
        );
        if (!preserveError) setError(null);
      } catch (loadError: unknown) {
        setSession(null);
        setError(asAppError(loadError));
      } finally {
        setLoadingSession(false);
      }
    },
    [desktopRuntime, projectSessionId],
  );

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void loadSession();
    }, 0);
    return () => window.clearTimeout(timer);
  }, [loadSession, providerState.selectedProviderId]);

  const applyEvent = useCallback(
    (event: AgentTurnEvent) => {
      const current = activeRunRef.current;
      if (!current || current.runId !== event.runId) {
        earlyEventsRef.current.push(event);
        return;
      }
      if (event.sequence <= lastSequenceRef.current) return;
      lastSequenceRef.current = event.sequence;
      if (event.kind === "turnState") {
        setTurnState(event.state);
      } else if (event.kind === "assistantMessage") {
        setMessages((items) => [
          ...items,
          {
            id: `${event.turnId}-${event.sequence}`,
            role: "assistant",
            text: event.text,
            status: "completed",
          },
        ]);
      } else if (event.kind === "toolStarted") {
        setTools((items) => [
          ...items.filter((item) => item.callId !== event.callId),
          {
            callId: event.callId,
            toolId: event.toolId,
            purpose: event.purpose,
            path: event.path,
            status: "running",
            sourcePaths: [],
            truncated: false,
          },
        ]);
      } else if (event.kind === "toolCompleted") {
        setTools((items) =>
          items.map((item) =>
            item.callId === event.callId
              ? {
                  ...item,
                  status: event.status,
                  sourcePaths: event.sourcePaths,
                  truncated: event.truncated,
                }
              : item,
          ),
        );
      } else if (event.kind === "writePreview") {
        setWritePreviews((items) => [
          ...items.filter((item) => item.pendingId !== event.preview.pendingId),
          event.preview,
        ]);
        setTools((items) =>
          items.map((item) =>
            event.preview.files.some((file) => file.callId === item.callId)
              ? { ...item, status: "succeeded" }
              : item,
          ),
        );
      } else if (event.kind === "writeCommitted") {
        setWritePreviews((items) =>
          items.filter((item) => item.transactionHash !== event.result.receipt.transactionHash),
        );
      } else if (event.kind === "completed") {
        submittedInputRef.current = null;
        setTurnState("completed");
        setActiveRun(null);
        activeRunRef.current = null;
        setCancelling(false);
        void loadSession();
      } else if (event.kind === "cancelled") {
        submittedInputRef.current = null;
        setTurnState("cancelled");
        setMessages((items) =>
          items.map((item) =>
            item.role === "user" && item.status === "sent"
              ? { ...item, status: "cancelled" }
              : item,
          ),
        );
        setActiveRun(null);
        activeRunRef.current = null;
        setCancelling(false);
        void loadSession();
      } else if (event.kind === "failed") {
        const submitted = submittedInputRef.current;
        submittedInputRef.current = null;
        if (submitted) {
          setInput((current) => current || submitted);
          setMessages((items) =>
            items.map((item) =>
              item.role === "user" && item.status === "sent" ? { ...item, status: "failed" } : item,
            ),
          );
        }
        setTurnState("failed");
        setError(event.error);
        setActiveRun(null);
        activeRunRef.current = null;
        setCancelling(false);
        void loadSession(true);
      }
    },
    [loadSession],
  );

  useEffect(() => {
    if (!desktopRuntime || !projectSessionId) return undefined;
    let unlisten: (() => void) | undefined;
    void listenAgentTurnEvents(applyEvent).then((dispose) => {
      unlisten = dispose;
    });
    return () => unlisten?.();
  }, [applyEvent, desktopRuntime, projectSessionId]);

  const send = async () => {
    if (
      !desktopRuntime ||
      !projectSessionId ||
      !session ||
      !providerReady ||
      !trimmedInput ||
      inputBytes > MAX_INPUT_BYTES ||
      active ||
      starting
    ) {
      return;
    }
    const submitted = trimmedInput;
    const messageId = `user-${Date.now()}`;
    submittedInputRef.current = submitted;
    setStarting(true);
    setError(null);
    setTurnState("starting");
    setMessages((items) => [
      ...items,
      { id: messageId, role: "user", text: submitted, status: "sent" },
    ]);
    setInput("");
    try {
      const started = await requestAgentTurnStart(
        projectSessionId,
        selectedProvider.providerId,
        submitted,
      );
      lastSequenceRef.current = 0;
      activeRunRef.current = started;
      setActiveRun(started);
      const early = earlyEventsRef.current.filter((event) => event.runId === started.runId);
      earlyEventsRef.current = earlyEventsRef.current.filter(
        (event) => event.runId !== started.runId,
      );
      early.sort((left, right) => left.sequence - right.sequence).forEach(applyEvent);
    } catch (startError: unknown) {
      submittedInputRef.current = null;
      setMessages((items) => items.filter((item) => item.id !== messageId));
      setInput(submitted);
      setError(asAppError(startError));
      setTurnState("failed");
    } finally {
      setStarting(false);
    }
  };

  const cancel = async () => {
    if (!activeRun || cancelling) return;
    setCancelling(true);
    try {
      await requestAgentTurnCancel(activeRun.runId);
    } catch (cancelError: unknown) {
      setError(asAppError(cancelError));
      setCancelling(false);
    }
  };

  const clearSession = async () => {
    if (!projectSessionId || active || clearing) return;
    setClearing(true);
    try {
      const result = await requestAgentSessionClear(projectSessionId);
      setSession(result.session);
      setMessages([]);
      setTools([]);
      setWritePreviews([]);
      setTurnState(null);
      setError(null);
      submittedInputRef.current = null;
      setClearConfirm(false);
    } catch (clearError: unknown) {
      setError(asAppError(clearError));
    } finally {
      setClearing(false);
    }
  };

  const changePolicy = async (nextPolicy: AgentSessionSnapshot["policy"]) => {
    if (!projectSessionId || !session || policyBusy || active || writePreviews.length) return;
    setPolicyBusy(true);
    try {
      const result = await requestAgentPolicySet(projectSessionId, nextPolicy);
      setSession((current) => (current ? { ...current, policy: result.policy } : current));
      setError(null);
    } catch (policyError: unknown) {
      setError(asAppError(policyError));
    } finally {
      setPolicyBusy(false);
    }
  };

  const confirmWrite = async (preview: AgentWritePreview) => {
    if (!projectSessionId || writeBusyId || active) return;
    setWriteBusyId(preview.pendingId);
    try {
      await requestAgentWriteConfirm(projectSessionId, preview);
      setWritePreviews((items) => items.filter((item) => item.pendingId !== preview.pendingId));
      setError(null);
      await loadSession();
    } catch (writeError: unknown) {
      setError(asAppError(writeError));
    } finally {
      setWriteBusyId(null);
    }
  };

  const rejectWrite = async (preview: AgentWritePreview) => {
    if (!projectSessionId || writeBusyId || active) return;
    setWriteBusyId(preview.pendingId);
    try {
      await requestAgentWriteReject(projectSessionId, preview.pendingId);
      setWritePreviews((items) => items.filter((item) => item.pendingId !== preview.pendingId));
      setError(null);
    } catch (writeError: unknown) {
      setError(asAppError(writeError));
    } finally {
      setWriteBusyId(null);
    }
  };

  const undoWrite = async (transactionHash: string) => {
    if (!projectSessionId || undoBusyHash || active) return;
    setUndoBusyHash(transactionHash);
    try {
      await requestAgentUndoApply(projectSessionId, transactionHash);
      setError(null);
      await loadSession();
    } catch (undoError: unknown) {
      setError(asAppError(undoError));
    } finally {
      setUndoBusyHash(null);
    }
  };

  const canSend =
    desktopRuntime &&
    Boolean(projectSessionId && session) &&
    providerReady &&
    trimmedInput.length > 0 &&
    inputBytes <= MAX_INPUT_BYTES &&
    !active &&
    !starting;
  const footer = !desktopRuntime
    ? ZH_CN.agent.desktopOnly
    : !projectSessionId
      ? ZH_CN.agent.openProject
      : !providerReady
        ? ZH_CN.agent.providerRequired
        : inputBytes > MAX_INPUT_BYTES
          ? ZH_CN.agent.inputTooLong
          : active || starting
            ? ZH_CN.agent.running(turnState)
            : ZH_CN.agent.ready;
  const presentedError = error ? presentAppError(error) : null;

  return (
    <>
      <header className="conversation-heading">
        <div>
          <p className="section-kicker">{ZH_CN.agent.kicker}</p>
          <h1>{ZH_CN.agent.heading}</h1>
        </div>
        <span className="story-badge">
          {ZH_CN.conversation.currentProvider} · {presentation.name} · {presentation.model}
        </span>
      </header>

      <section aria-label={ZH_CN.agent.timelineAriaLabel} className="message-canvas agent-canvas">
        <div className="agent-timeline-content">
          <section className="agent-session-summary">
            <header>
              <div>
                <p className="section-kicker">{ZH_CN.agent.sessionKicker}</p>
                <strong>{session ? ZH_CN.agent.sessionReady : ZH_CN.agent.sessionWaiting}</strong>
              </div>
              {session && !active ? (
                <button
                  className="agent-clear-button"
                  onClick={() => setClearConfirm(true)}
                  type="button"
                >
                  {ZH_CN.agent.clear}
                </button>
              ) : null}
            </header>
            <p>{session?.summaryText ?? ZH_CN.agent.noSummary}</p>
            {session ? (
              <label className="agent-policy-control">
                <span>{ZH_CN.agent.policyLabel}</span>
                <select
                  aria-label={ZH_CN.agent.policyLabel}
                  disabled={active || policyBusy || writePreviews.length > 0}
                  onChange={(event) =>
                    void changePolicy(event.currentTarget.value as AgentSessionSnapshot["policy"])
                  }
                  value={session.policy}
                >
                  <option value="plan_only">{ZH_CN.agent.policies.planOnly}</option>
                  <option value="review_every_write">
                    {ZH_CN.agent.policies.reviewEveryWrite}
                  </option>
                  <option value="auto_apply_safe">{ZH_CN.agent.policies.autoApplySafe}</option>
                </select>
              </label>
            ) : null}
            <small>
              {registry
                ? ZH_CN.agent.pack(
                    registry.packVersion,
                    registry.skills.length,
                    registry.readToolIds.length,
                    registry.writeToolIds.length,
                  )
                : ZH_CN.agent.loadingPack}
            </small>
          </section>

          {clearConfirm ? (
            <section
              className="agent-clear-confirm"
              role="alertdialog"
              aria-label={ZH_CN.agent.clearTitle}
            >
              <strong>{ZH_CN.agent.clearTitle}</strong>
              <p>{ZH_CN.agent.clearBody}</p>
              <div>
                <button disabled={clearing} onClick={() => setClearConfirm(false)} type="button">
                  {ZH_CN.agent.keep}
                </button>
                <button disabled={clearing} onClick={() => void clearSession()} type="button">
                  {clearing ? ZH_CN.agent.clearing : ZH_CN.agent.confirmClear}
                </button>
              </div>
            </section>
          ) : null}

          {!projectSessionId ? (
            <section className="empty-state agent-empty-state">
              <div className="empty-icon" aria-hidden="true">
                文
              </div>
              <h2>{ZH_CN.agent.openProjectTitle}</h2>
              <p>{ZH_CN.agent.openProjectBody}</p>
              <button onClick={onProjectCreateRequested} type="button">
                {ZH_CN.agent.createProject}
              </button>
            </section>
          ) : null}

          {session?.recentTurns.length ? (
            <details className="agent-history">
              <summary>{ZH_CN.agent.recentTurns(session.recentTurns.length)}</summary>
              <ul>
                {session.recentTurns.map((turn) => (
                  <li key={turn.turnId}>
                    <span>{ZH_CN.agent.turnStates[turn.terminalState]}</span>
                    <strong>{turn.skillIds.join(" · ")}</strong>
                    <small>{ZH_CN.agent.traceCounts(turn.actionCount, turn.toolCallCount)}</small>
                  </li>
                ))}
              </ul>
            </details>
          ) : null}

          {writePreviews.map((preview) => (
            <section
              className="agent-write-preview"
              data-risk={preview.highestRisk}
              key={preview.pendingId}
            >
              <header>
                <div>
                  <p className="section-kicker">{ZH_CN.agent.writePreviewKicker}</p>
                  <strong>{ZH_CN.agent.writePreviewTitle(preview.files.length)}</strong>
                </div>
                <span>{ZH_CN.agent.riskLabels[preview.highestRisk]}</span>
              </header>
              <p>{ZH_CN.agent.writePreviewBoundary(preview.policy)}</p>
              <dl className="agent-write-hashes">
                <div>
                  <dt>{ZH_CN.agent.transactionHashLabel}</dt>
                  <dd>
                    <code>{preview.transactionHash}</code>
                  </dd>
                </div>
                <div>
                  <dt>{ZH_CN.agent.sourceSetHashLabel}</dt>
                  <dd>
                    <code>{preview.sourceSetHash}</code>
                  </dd>
                </div>
              </dl>
              {preview.files.map((file) => (
                <details className="agent-write-file" key={file.callId} open>
                  <summary>
                    <code>{file.relativePath}</code>
                    <span>{ZH_CN.agent.operationLabels[file.operation]}</span>
                  </summary>
                  <dl className="agent-write-file-hashes">
                    <div>
                      <dt>{ZH_CN.agent.sourceHashLabel}</dt>
                      <dd>
                        <code>{file.expectedSourceHash ?? ZH_CN.agent.newFileSourceHash}</code>
                      </dd>
                    </div>
                    <div>
                      <dt>{ZH_CN.agent.resultHashLabel}</dt>
                      <dd>
                        <code>{file.resultHash}</code>
                      </dd>
                    </div>
                  </dl>
                  <div className="agent-line-diff" aria-label={ZH_CN.agent.diffAriaLabel}>
                    {file.lineDiff.map((line, index) => (
                      <div data-kind={line.kind} key={`${file.callId}-${index}`}>
                        <span>{line.oldLine ?? ""}</span>
                        <span>{line.newLine ?? ""}</span>
                        <code>{line.content || " "}</code>
                      </div>
                    ))}
                  </div>
                </details>
              ))}
              <footer>
                {preview.canConfirm ? (
                  <button
                    disabled={writeBusyId !== null || active}
                    onClick={() => void confirmWrite(preview)}
                    type="button"
                  >
                    {writeBusyId === preview.pendingId
                      ? ZH_CN.agent.writeApplying
                      : ZH_CN.agent.confirmWrite}
                  </button>
                ) : null}
                {preview.canConfirm ? (
                  <button
                    disabled={writeBusyId !== null || active}
                    onClick={() => void rejectWrite(preview)}
                    type="button"
                  >
                    {ZH_CN.agent.rejectWrite}
                  </button>
                ) : (
                  <span>{ZH_CN.agent.planOnlyBlocked}</span>
                )}
              </footer>
            </section>
          ))}

          {session?.undoRecords.some((record) => record.status === "ready") ? (
            <details className="agent-undo-list">
              <summary>{ZH_CN.agent.undoTitle}</summary>
              {session.undoRecords
                .filter((record) => record.status === "ready")
                .map((record) => (
                  <div key={record.transactionHash}>
                    <span>{record.targetPaths.join(" · ")}</span>
                    <button
                      disabled={active || undoBusyHash !== null}
                      onClick={() => void undoWrite(record.transactionHash)}
                      type="button"
                    >
                      {undoBusyHash === record.transactionHash
                        ? ZH_CN.agent.undoing
                        : ZH_CN.agent.undo}
                    </button>
                  </div>
                ))}
            </details>
          ) : null}

          {messages.length || tools.length ? (
            <section
              aria-label={ZH_CN.agent.messagesAriaLabel}
              className="conversation-message-list"
            >
              {messages.map((message) => (
                <article className="conversation-message" data-role={message.role} key={message.id}>
                  <header>
                    <strong>
                      {message.role === "user" ? ZH_CN.conversation.you : presentation.name}
                    </strong>
                    <span>
                      {message.status === "cancelled"
                        ? ZH_CN.agent.cancelled
                        : message.status === "failed"
                          ? ZH_CN.agent.failedInputRestored
                          : ZH_CN.agent.messageDone}
                    </span>
                  </header>
                  <p>{message.text}</p>
                </article>
              ))}
              {tools.map((tool) => (
                <article className="agent-tool-card" data-status={tool.status} key={tool.callId}>
                  <header>
                    <strong>{tool.toolId}</strong>
                    <span>{ZH_CN.agent.toolStatus[tool.status]}</span>
                  </header>
                  <p>{tool.purpose}</p>
                  <code>{tool.path}</code>
                  {tool.sourcePaths.length ? <small>{tool.sourcePaths.join(" · ")}</small> : null}
                  {tool.truncated ? <em>{ZH_CN.agent.truncated}</em> : null}
                </article>
              ))}
            </section>
          ) : null}

          {loadingSession ? <p className="status-message">{ZH_CN.agent.loadingSession}</p> : null}
          <aside className="capability-note agent-boundary">
            <strong>{ZH_CN.agent.boundaryTitle}</strong>
            <span>{ZH_CN.agent.boundaryBody}</span>
          </aside>
        </div>
      </section>

      <section aria-label={ZH_CN.agent.composerAriaLabel} className="composer-shell">
        {presentedError ? (
          <div className="composer-error" id="agent-composer-error" role="alert">
            <strong>{presentedError.title}</strong>
            <span>{presentedError.message}</span>
            <small>{presentedError.recovery}</small>
            <code>
              {ZH_CN.agent.errorCodeLabel}：{presentedError.code}
            </code>
          </div>
        ) : null}
        <textarea
          aria-describedby={presentedError ? "agent-composer-error" : undefined}
          aria-label={ZH_CN.agent.inputAriaLabel}
          disabled={!desktopRuntime || !projectSessionId || active || starting}
          onChange={(event) => setInput(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              void send();
            }
          }}
          placeholder={ZH_CN.agent.placeholder}
          rows={3}
          value={input}
        />
        <div className="composer-footer">
          <span>{footer}</span>
          {active ? (
            <button
              className="send-button stop-button"
              disabled={cancelling}
              onClick={() => void cancel()}
              type="button"
            >
              {cancelling ? ZH_CN.agent.cancelling : ZH_CN.agent.cancel}
            </button>
          ) : (
            <button
              className="send-button"
              disabled={!canSend}
              onClick={() => void send()}
              type="button"
            >
              {ZH_CN.conversation.send}
              <span aria-hidden="true">↑</span>
            </button>
          )}
        </div>
      </section>
    </>
  );
}
