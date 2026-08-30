import { createPortal } from "react-dom";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { presentAppError } from "../errors/present-app-error";
import {
  listenAgentTurnEvents,
  requestAgentConflictResolution,
  requestAgentContextMarkRemove,
  requestAgentContextMarkUpsert,
  requestAgentHistoryBranchResolve,
  requestAgentMemoryCompact,
  requestAgentSession,
  requestAgentSessionClear,
  requestAgentSkillRegistry,
  requestAgentTurnCancel,
  requestAgentTurnStart,
  requestAgentWriteConfirmation,
  requestAgentWritePlanReject,
  requestAgentWritePolicySet,
  requestAgentWriteUndo,
  type AgentCommitReceiptV2,
  type AgentWritePlanV2,
  type AgentSessionSnapshotV2,
  type AnchorReferenceV1,
  type AnchorResolutionV1,
  type CanonCandidateV1,
  type ConflictResolutionChoiceV1,
  type ConflictSetV1,
  type ContextMarkStoreV1,
  type AgentSkillRegistryV2,
  type AgentTurnEventV2,
  type AgentTurnStartedV2,
  type SourceRefV2,
  type TurnBudgetUsageV2,
} from "../lib/tauri/agent";
import type { AppError } from "../lib/tauri/health";
import type { ProviderStateSnapshot } from "../lib/tauri/provider";
import { PROVIDER_PRESENTATION } from "./provider-settings-model";

const MAX_INPUT_BYTES = 16 * 1024;

type Props = {
  activityHost: HTMLElement | null;
  desktopRuntime: boolean;
  projectSessionId: string | null;
  providerState: ProviderStateSnapshot;
  onRunActiveChange?: (active: boolean) => void;
};

type PendingUserMessage = { id: string; body: string; status: "sending" | "failed" };

function appError(error: unknown): AppError {
  if (typeof error === "object" && error !== null && "code" in error && "messageKey" in error) {
    return error as AppError;
  }
  return { code: "runtime.unknown", messageKey: "errors.unknown" };
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

async function sha256Hex(value: string): Promise<string> {
  const digest = await globalThis.crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function stateLabel(state: string | null): string {
  switch (state) {
    case "starting":
      return "正在启动 AI";
    case "selecting_skills":
      return "正在选择写作能力";
    case "planning_context":
      return "正在规划上下文";
    case "gathering_context":
      return "正在读取工程设定";
    case "responding":
      return "正在回复";
    case "cancelling":
      return "正在停止";
    case "failed":
      return "本轮失败";
    case "cancelled":
      return "本轮已取消";
    default:
      return "就绪";
  }
}

function mergeSources(previous: SourceRefV2[], incoming: SourceRefV2[]): SourceRefV2[] {
  const byId = new Map(previous.map((source) => [source.sourceId, source]));
  incoming.forEach((source) => byId.set(source.sourceId, source));
  return [...byId.values()];
}

export function AgentConversation({
  activityHost,
  desktopRuntime,
  projectSessionId,
  providerState,
  onRunActiveChange,
}: Props) {
  const [registry, setRegistry] = useState<AgentSkillRegistryV2 | null>(null);
  const [session, setSession] = useState<AgentSessionSnapshotV2 | null>(null);
  const [input, setInput] = useState("");
  const [pendingUser, setPendingUser] = useState<PendingUserMessage | null>(null);
  const [streamingText, setStreamingText] = useState("");
  const [activeRun, setActiveRun] = useState<AgentTurnStartedV2 | null>(null);
  const [turnState, setTurnState] = useState<string | null>(null);
  const [visibleReason, setVisibleReason] = useState<string | null>(null);
  const [selectedSkills, setSelectedSkills] = useState<string[]>([]);
  const [sources, setSources] = useState<SourceRefV2[]>([]);
  const [usage, setUsage] = useState<TurnBudgetUsageV2 | null>(null);
  const [anchorReference, setAnchorReference] = useState<AnchorReferenceV1 | null>(null);
  const [anchorResolution, setAnchorResolution] = useState<AnchorResolutionV1 | null>(null);
  const [canonCandidate, setCanonCandidate] = useState<CanonCandidateV1 | null>(null);
  const [markStore, setMarkStore] = useState<ContextMarkStoreV1 | null>(null);
  const [conflictSet, setConflictSet] = useState<ConflictSetV1 | null>(null);
  const [writePlan, setWritePlan] = useState<AgentWritePlanV2 | null>(null);
  const [commitReceipt, setCommitReceipt] = useState<AgentCommitReceiptV2 | null>(null);
  const [commitNotice, setCommitNotice] = useState<string | null>(null);
  const [canonWriteCount, setCanonWriteCount] = useState(0);
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);
  const [actionBusy, setActionBusy] = useState(false);
  const [clearBusy, setClearBusy] = useState(false);
  const [eventChannelReady, setEventChannelReady] = useState(false);
  const activeRef = useRef<AgentTurnStartedV2 | null>(null);
  const lastSequence = useRef(0);
  const earlyEvents = useRef<AgentTurnEventV2[]>([]);
  const selectedProvider =
    providerState.providers.find((item) => item.selected) ?? providerState.providers[0];
  const presentation = PROVIDER_PRESENTATION[selectedProvider.providerId];
  const providerReady =
    selectedProvider.configured && selectedProvider.connectionStatus === "connected";
  const active = activeRun !== null;
  const trimmed = input.trim();
  const inputBytes = useMemo(() => bytes(trimmed), [trimmed]);

  useEffect(() => {
    activeRef.current = activeRun;
    onRunActiveChange?.(active);
  }, [active, activeRun, onRunActiveChange]);

  const loadSession = useCallback(
    async (historyConsent = false) => {
      if (!desktopRuntime || !projectSessionId || activeRef.current) return;
      setBusy(true);
      try {
        const loaded = await requestAgentSession(projectSessionId, historyConsent);
        setSession(loaded);
        setMarkStore(loaded.contextMarks);
        setError(null);
      } catch (loadError: unknown) {
        setSession(null);
        setError(appError(loadError));
      } finally {
        setBusy(false);
      }
    },
    [desktopRuntime, projectSessionId],
  );

  useEffect(() => {
    if (!desktopRuntime || !projectSessionId) return undefined;
    let disposed = false;
    void Promise.all([requestAgentSession(projectSessionId, false), requestAgentSkillRegistry()])
      .then(([loadedSession, loadedRegistry]) => {
        if (disposed) return;
        setSession(loadedSession);
        setMarkStore(loadedSession.contextMarks);
        setRegistry(loadedRegistry);
        setError(null);
      })
      .catch((loadError: unknown) => {
        if (!disposed) setError(appError(loadError));
      });
    return () => {
      disposed = true;
    };
  }, [desktopRuntime, projectSessionId]);

  const finishRun = useCallback(
    async (event: Extract<AgentTurnEventV2, { kind: "completed" | "failed" | "cancelled" }>) => {
      setUsage(event.usage);
      setCanonWriteCount(event.agentCanonWriteCount);
      setActiveRun(null);
      activeRef.current = null;
      setPendingUser(null);
      setStreamingText("");
      setTurnState(event.kind);
      if (event.kind === "failed") setError(event.error);
      await loadSession();
    },
    [loadSession],
  );

  const applyEvent = useCallback(
    (event: AgentTurnEventV2) => {
      const current = activeRef.current;
      if (!current || current.runId !== event.runId) {
        earlyEvents.current.push(event);
        return;
      }
      if (event.sequence <= lastSequence.current) return;
      lastSequence.current = event.sequence;
      if (event.kind === "turnState") {
        setTurnState(event.state);
        setVisibleReason(event.visibleReason);
      } else if (event.kind === "skillSelection") {
        setSelectedSkills(event.selectedSkillIds);
        setVisibleReason(event.visibleReason);
      } else if (event.kind === "contextSources") {
        setSources((currentSources) => mergeSources(currentSources, event.sources));
      } else if (event.kind === "anchorCandidate") {
        setAnchorResolution(event.resolution);
      } else if (event.kind === "canonCandidate") {
        setCanonCandidate({ artifact: event.artifact, proposal: event.proposal });
      } else if (event.kind === "contextMarks") {
        setMarkStore(event.store);
      } else if (event.kind === "conflict") {
        setConflictSet(event.conflictSet);
        setWritePlan(null);
      } else if (event.kind === "writePlan") {
        setWritePlan(event.plan);
      } else if (event.kind === "commitReceipt") {
        setCommitReceipt(event.receipt);
        setWritePlan(null);
        setCommitNotice(
          `已将 ${event.receipt.relativePaths.join("、")} 写入工程，可在右栏查看并撤销。`,
        );
      } else if (event.kind === "conversationDelta") {
        setStreamingText((currentText) => currentText + event.delta);
      } else if (
        event.kind === "completed" ||
        event.kind === "failed" ||
        event.kind === "cancelled"
      ) {
        void finishRun(event);
      }
    },
    [finishRun],
  );

  useEffect(() => {
    if (!desktopRuntime || !projectSessionId) return undefined;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listenAgentTurnEvents(applyEvent)
      .then((dispose) => {
        if (disposed) {
          dispose();
          return;
        }
        unlisten = dispose;
        setEventChannelReady(true);
      })
      .catch((listenError: unknown) => {
        if (!disposed) {
          setEventChannelReady(false);
          setError(appError(listenError));
        }
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [applyEvent, desktopRuntime, projectSessionId]);

  async function send() {
    if (
      !projectSessionId ||
      !session ||
      session.historyConsentRequired ||
      session.branchSelectionRequired ||
      !providerReady ||
      !eventChannelReady ||
      !trimmed ||
      inputBytes > MAX_INPUT_BYTES ||
      active ||
      conflictSet?.requiresResolution ||
      writePlan ||
      busy
    )
      return;
    const submitted = trimmed;
    const optimistic = { id: `pending-${Date.now()}`, body: submitted, status: "sending" as const };
    setPendingUser(optimistic);
    setInput("");
    setStreamingText("");
    setSources([]);
    setSelectedSkills([]);
    setUsage(null);
    setCanonWriteCount(0);
    setAnchorResolution(null);
    setCanonCandidate(null);
    setConflictSet(null);
    setWritePlan(null);
    setCommitNotice(null);
    setTurnState("starting");
    setError(null);
    setBusy(true);
    try {
      const started = await requestAgentTurnStart(
        projectSessionId,
        selectedProvider.providerId,
        submitted,
        anchorReference ?? undefined,
      );
      setAnchorReference(null);
      lastSequence.current = 0;
      activeRef.current = started;
      setActiveRun(started);
      earlyEvents.current
        .filter((event) => event.runId === started.runId)
        .sort((left, right) => left.sequence - right.sequence)
        .forEach(applyEvent);
      earlyEvents.current = earlyEvents.current.filter((event) => event.runId !== started.runId);
    } catch (startError: unknown) {
      setInput(submitted);
      setPendingUser({ ...optimistic, status: "failed" });
      setTurnState("failed");
      setError(appError(startError));
    } finally {
      setBusy(false);
    }
  }

  async function cancel() {
    if (!activeRun || busy) return;
    setBusy(true);
    setTurnState("cancelling");
    try {
      await requestAgentTurnCancel(activeRun.runId);
    } catch (cancelError: unknown) {
      setError(appError(cancelError));
      setBusy(false);
    }
  }

  async function resolveBranch(branchId: string) {
    if (!projectSessionId || busy) return;
    setBusy(true);
    try {
      const loaded = await requestAgentHistoryBranchResolve(projectSessionId, branchId);
      setSession(loaded);
      setMarkStore(loaded.contextMarks);
      setError(null);
    } catch (branchError: unknown) {
      setError(appError(branchError));
    } finally {
      setBusy(false);
    }
  }

  async function clear(scope: "session" | "all_history") {
    if (!projectSessionId || active || clearBusy) return;
    setClearBusy(true);
    try {
      const result = await requestAgentSessionClear(projectSessionId, scope);
      setSession(result.snapshot);
      setMarkStore(result.snapshot.contextMarks);
      setSources([]);
      setSelectedSkills([]);
      setUsage(null);
      setError(null);
    } catch (clearError: unknown) {
      setError(appError(clearError));
    } finally {
      setClearBusy(false);
    }
  }

  async function compact() {
    if (!projectSessionId || !session?.sessionId || !session.memory || active || busy) return;
    setBusy(true);
    try {
      const memory = await requestAgentMemoryCompact(
        projectSessionId,
        session.sessionId,
        session.memory.memoryHash,
        selectedProvider.providerId,
      );
      setSession((current) => (current ? { ...current, memory } : current));
      setError(null);
    } catch (compactError: unknown) {
      setError(appError(compactError));
    } finally {
      setBusy(false);
    }
  }

  async function setPolicy(policy: AgentSessionSnapshotV2["writePolicy"]) {
    if (!projectSessionId || !session?.writePolicyHash || active || actionBusy) return;
    setActionBusy(true);
    try {
      const snapshot = await requestAgentWritePolicySet(
        projectSessionId,
        policy,
        session.writePolicyHash,
      );
      setSession((current) =>
        current
          ? { ...current, writePolicy: snapshot.policy, writePolicyHash: snapshot.policyHash }
          : current,
      );
      setError(null);
    } catch (policyError: unknown) {
      setError(appError(policyError));
    } finally {
      setActionBusy(false);
    }
  }

  async function markSource(
    source: SourceRefV2,
    scope: "session" | "project",
    priority: "medium" | "high",
  ) {
    if (!projectSessionId || active || actionBusy) return;
    setActionBusy(true);
    try {
      setMarkStore(
        await requestAgentContextMarkUpsert(
          projectSessionId,
          source.sourceId,
          source.contentHash,
          scope,
          priority,
        ),
      );
      setError(null);
    } catch (markError: unknown) {
      setError(appError(markError));
    } finally {
      setActionBusy(false);
    }
  }

  async function removeMark(markId: string, bindingHash: string) {
    if (!projectSessionId || active || actionBusy) return;
    setActionBusy(true);
    try {
      setMarkStore(await requestAgentContextMarkRemove(projectSessionId, markId, bindingHash));
      setError(null);
    } catch (markError: unknown) {
      setError(appError(markError));
    } finally {
      setActionBusy(false);
    }
  }

  async function resolveConflict(resolution: ConflictResolutionChoiceV1) {
    if (!projectSessionId || !conflictSet || active || actionBusy) return;
    setActionBusy(true);
    try {
      const result = await requestAgentConflictResolution(
        projectSessionId,
        conflictSet.conflictSetId,
        conflictSet.conflictSetHash,
        resolution,
      );
      setConflictSet(null);
      setWritePlan(result.writePlan);
      if (!result.writePlan) {
        setCanonCandidate(null);
        setVisibleReason(
          resolution === "merge_manual" || resolution === "save_as_alternative"
            ? "请在中栏继续说明合并结果或替代实体；本次没有写入。"
            : "本次候选已处理，Canon 未发生写入。",
        );
      }
      setError(null);
    } catch (conflictError: unknown) {
      setError(appError(conflictError));
    } finally {
      setActionBusy(false);
    }
  }

  async function confirmWrite() {
    if (!projectSessionId || !writePlan || !writePlan.preview.canConfirm || actionBusy) return;
    setActionBusy(true);
    try {
      const receipt = await requestAgentWriteConfirmation(projectSessionId, writePlan);
      setCommitReceipt(receipt);
      setWritePlan(null);
      setCanonWriteCount((count) => count + receipt.projectWriteCount);
      setCommitNotice(`已将 ${receipt.relativePaths.join("、")} 写入工程，可在右栏撤销。`);
      setError(null);
    } catch (writeError: unknown) {
      setError(appError(writeError));
    } finally {
      setActionBusy(false);
    }
  }

  async function rejectWrite() {
    if (!projectSessionId || !writePlan || actionBusy) return;
    setActionBusy(true);
    try {
      await requestAgentWritePlanReject(projectSessionId, writePlan.pendingId, writePlan.planHash);
      setWritePlan(null);
      setCanonCandidate(null);
      setVisibleReason("写入预览已拒绝，Canon 未发生写入。");
      setError(null);
    } catch (writeError: unknown) {
      setError(appError(writeError));
    } finally {
      setActionBusy(false);
    }
  }

  async function undoWrite() {
    if (!projectSessionId || !commitReceipt || active || actionBusy) return;
    setActionBusy(true);
    try {
      const result = await requestAgentWriteUndo(
        projectSessionId,
        commitReceipt.receiptId,
        commitReceipt.resultHash,
      );
      setCommitReceipt(null);
      setCommitNotice(`已撤销 ${result.restoredFileCount} 个文件的上一次 Agent 写入。`);
      setCanonWriteCount((count) => count + result.projectWriteCount);
      setError(null);
    } catch (undoError: unknown) {
      setError(appError(undoError));
    } finally {
      setActionBusy(false);
    }
  }

  async function anchorVisibleSelection(message: AgentSessionSnapshotV2["messages"][number]) {
    const selection = globalThis.getSelection?.();
    if (!selection || selection.rangeCount !== 1 || selection.isCollapsed) {
      setVisibleReason("请先在这一条可见消息中选择要锚定的正文。 ");
      return;
    }
    const range = selection.getRangeAt(0);
    const container = document.querySelector(`[data-message-id="${message.messageId}"] p`);
    if (!container || !container.contains(range.commonAncestorContainer)) {
      setVisibleReason("选区必须完整位于同一条可见消息中。 ");
      return;
    }
    const prefix = range.cloneRange();
    prefix.selectNodeContents(container);
    prefix.setEnd(range.startContainer, range.startOffset);
    const selected = range.toString();
    const startUtf8Byte = bytes(prefix.toString());
    const endUtf8Byte = startUtf8Byte + bytes(selected);
    setAnchorReference({
      sourceMessageId: message.messageId,
      messageBodyHash: message.bodyHash,
      startUtf8Byte,
      endUtf8Byte,
      selectionHash: await sha256Hex(selected),
      candidateId: null,
    });
    selection.removeAllRanges();
    setVisibleReason("已绑定可见选区；请说明写入人物、设定、素材、剧情或章节。 ");
  }

  const presentedError = error ? presentAppError(error) : null;
  const messages = session?.messages ?? [];
  const pendingAuthorDecision = Boolean(conflictSet?.requiresResolution || writePlan);
  const canSend =
    desktopRuntime &&
    Boolean(session) &&
    providerReady &&
    eventChannelReady &&
    trimmed.length > 0 &&
    inputBytes <= MAX_INPUT_BYTES &&
    !active &&
    !busy &&
    !pendingAuthorDecision &&
    !session?.historyConsentRequired &&
    !session?.branchSelectionRequired;

  const activity = (
    <section aria-label="Agent 活动" className="agent-v2-activity">
      <header>
        <div>
          <p className="section-kicker">AGENT ACTIVITY</p>
          <strong>{stateLabel(turnState)}</strong>
        </div>
        <span className="agent-v2-readonly">AM-R2 · Canon</span>
      </header>
      {visibleReason ? <p className="agent-v2-reason">{visibleReason}</p> : null}
      {anchorResolution?.status === "ready" ? (
        <div className="agent-v2-activity-block">
          <strong>Anchor Candidate</strong>
          <p>
            {anchorResolution.anchor.targetKind} · {anchorResolution.anchor.intentKind}
          </p>
          <small>已由 Host 校验来源与 Hash；尚未写入 Canon。</small>
        </div>
      ) : null}
      {anchorResolution?.status === "clarification_required" ? (
        <div className="agent-v2-activity-block">
          <strong>需要澄清</strong>
          <p>{anchorResolution.visibleReason}</p>
        </div>
      ) : null}
      {session?.writePolicyHash ? (
        <div className="agent-v2-activity-block">
          <strong>写入策略</strong>
          <p>
            {session.writePolicy === "auto_apply_safe"
              ? "安全变更自动提交"
              : session.writePolicy === "review_every_write"
                ? "每次写入都确认"
                : "只生成预览"}
          </p>
          <div className="agent-v2-inline-actions">
            <button
              disabled={active || actionBusy || pendingAuthorDecision}
              onClick={() => void setPolicy("auto_apply_safe")}
              type="button"
            >
              自动提交安全变更
            </button>
            <button
              disabled={active || actionBusy || pendingAuthorDecision}
              onClick={() => void setPolicy("review_every_write")}
              type="button"
            >
              每次确认
            </button>
            <button
              disabled={active || actionBusy || pendingAuthorDecision}
              onClick={() => void setPolicy("plan_only")}
              type="button"
            >
              仅预览
            </button>
          </div>
        </div>
      ) : null}
      {canonCandidate ? (
        <div className="agent-v2-activity-block">
          <strong>Canon Candidate</strong>
          <p>
            {canonCandidate.proposal.proposal.targetKind} ·{" "}
            {canonCandidate.proposal.proposal.displayName}
          </p>
          <details>
            <summary>已显示 Draft Artifact</summary>
            <p>{canonCandidate.artifact.content}</p>
          </details>
          <ul>
            {canonCandidate.proposal.proposal.facts.map((fact, index) => (
              <li key={`${fact.field}-${index}`}>
                <strong>{fact.field}</strong>：{fact.value}
              </li>
            ))}
          </ul>
          <small>候选只引用可见消息/Artifact；最终路径与风险由 Host 决定。</small>
        </div>
      ) : null}
      <div className="agent-v2-activity-block">
        <strong>本轮 Skill</strong>
        <p>{selectedSkills.length ? selectedSkills.join(" · ") : "等待本轮自动选择"}</p>
        <small>
          {registry
            ? `${registry.packVersion} · ${registry.skills.length} 项完整 bundled Skill`
            : "正在校验 Skill Pack"}
        </small>
      </div>
      <div className="agent-v2-activity-block">
        <strong>实际来源</strong>
        {sources.length ? (
          sources.map((source) => (
            <details key={source.sourceId}>
              <summary>{source.relativePath}</summary>
              <p>{source.excerpt}</p>
              <code>{source.sourceId}</code>
              <div className="agent-v2-inline-actions">
                <button
                  disabled={active || actionBusy}
                  onClick={() => void markSource(source, "session", "medium")}
                  type="button"
                >
                  本会话关注
                </button>
                <button
                  disabled={active || actionBusy}
                  onClick={() => void markSource(source, "project", "high")}
                  type="button"
                >
                  工程固定
                </button>
              </div>
            </details>
          ))
        ) : (
          <p>本轮尚未读取工程来源。</p>
        )}
      </div>
      {markStore?.marks.length ? (
        <div className="agent-v2-activity-block">
          <strong>Context Mark</strong>
          {markStore.marks.map((mark) => (
            <div className="agent-v2-mark-row" key={mark.markId}>
              <p>
                {mark.relativePath} · {mark.scope} · {mark.priority} · {mark.status}
              </p>
              {mark.creator === "user" && mark.status !== "archived" ? (
                <button
                  disabled={active || actionBusy}
                  onClick={() => void removeMark(mark.markId, mark.bindingHash)}
                  type="button"
                >
                  移除
                </button>
              ) : null}
            </div>
          ))}
        </div>
      ) : null}
      {conflictSet ? (
        <div className="agent-v2-activity-block agent-v2-decision-block">
          <strong>{conflictSet.requiresResolution ? "Canon 冲突" : "需要澄清"}</strong>
          {conflictSet.conflicts.map((conflict) => (
            <div key={conflict.conflictId}>
              <p>{conflict.visibleExplanation}</p>
              <small>{conflict.relativePath}</small>
              <p>原值：{conflict.existingValue}</p>
              <p>候选：{conflict.candidateValue}</p>
            </div>
          ))}
          {conflictSet.requiresResolution ? (
            <div className="agent-v2-inline-actions">
              <button
                disabled={actionBusy}
                onClick={() => void resolveConflict("keep_existing")}
                type="button"
              >
                保留原设定
              </button>
              <button
                disabled={actionBusy}
                onClick={() => void resolveConflict("adopt_candidate")}
                type="button"
              >
                采用候选
              </button>
              <button
                disabled={actionBusy}
                onClick={() => void resolveConflict("merge_manual")}
                type="button"
              >
                手动合并
              </button>
              <button
                disabled={actionBusy}
                onClick={() => void resolveConflict("save_as_alternative")}
                type="button"
              >
                保存为替代项
              </button>
              <button
                disabled={actionBusy}
                onClick={() => void resolveConflict("cancel_change")}
                type="button"
              >
                取消变更
              </button>
            </div>
          ) : null}
        </div>
      ) : null}
      {writePlan ? (
        <div className="agent-v2-activity-block agent-v2-decision-block">
          <strong>Write Preview</strong>
          <p>
            {writePlan.preview.highestRisk} · {writePlan.policy} ·{" "}
            {writePlan.preview.authorizationEffect}
          </p>
          {writePlan.preview.files.map((file) => (
            <details key={file.callId} open>
              <summary>{file.relativePath}</summary>
              <small>
                {file.mutationShape} · {file.risk} · result {file.resultHash.slice(0, 12)}
              </small>
              <pre className="agent-v2-diff">
                {file.lineDiff
                  .map(
                    (line) =>
                      `${line.kind === "add" ? "+" : line.kind === "remove" ? "-" : " "}${line.content}`,
                  )
                  .join("\n")}
              </pre>
            </details>
          ))}
          <div className="agent-v2-inline-actions">
            {writePlan.preview.canConfirm ? (
              <button disabled={actionBusy} onClick={() => void confirmWrite()} type="button">
                确认写入
              </button>
            ) : null}
            <button disabled={actionBusy} onClick={() => void rejectWrite()} type="button">
              拒绝
            </button>
          </div>
        </div>
      ) : null}
      {commitReceipt ? (
        <div className="agent-v2-activity-block">
          <strong>Commit Receipt</strong>
          <p>{commitReceipt.relativePaths.join(" · ")}</p>
          <small>
            transaction {commitReceipt.transactionHash.slice(0, 12)} · result{" "}
            {commitReceipt.resultHash.slice(0, 12)}
          </small>
          <button disabled={active || actionBusy} onClick={() => void undoWrite()} type="button">
            撤销本次写入
          </button>
        </div>
      ) : null}
      {session?.memory ? (
        <div className="agent-v2-activity-block">
          <strong>会话记忆</strong>
          <p>{session.memory.summary || "由 Host 增量维护，尚未形成压缩摘要。"}</p>
          <small>
            {session.memory.completedTurns} 轮 · {session.memory.visibleMessageCount} 条可见消息
          </small>
          {session.memory.needsCompaction ? (
            <button disabled={!providerReady || busy} onClick={() => void compact()} type="button">
              压缩会话记忆
            </button>
          ) : null}
        </div>
      ) : null}
      {usage ? (
        <small className="agent-v2-usage">
          累计调用 {usage.providerCalls} · 输入 {usage.totalProviderInputTokens} tokens · 输出{" "}
          {usage.totalProviderOutputTokens} tokens · Canon 写入 {canonWriteCount}
        </small>
      ) : null}
      {session && !session.historyConsentRequired ? (
        <div className="agent-v2-clear-actions">
          <button
            disabled={active || clearBusy || pendingAuthorDecision}
            onClick={() => void clear("session")}
            type="button"
          >
            清空当前会话
          </button>
          <button
            disabled={active || clearBusy || pendingAuthorDecision}
            onClick={() => void clear("all_history")}
            type="button"
          >
            清空全部历史
          </button>
        </div>
      ) : null}
    </section>
  );

  return (
    <>
      <header className="conversation-heading">
        <div>
          <p className="section-kicker">CONVERSATION</p>
          <h1>小说创作对话</h1>
        </div>
        <span className="story-badge">
          当前 Provider · {presentation.name} · {presentation.model}
        </span>
      </header>
      <section aria-label="小说创作对话消息" className="message-canvas agent-v2-conversation">
        <div className="conversation-message-list">
          {messages.length === 0 && !pendingUser ? (
            <section className="empty-state">
              <div aria-hidden="true" className="empty-icon">
                文
              </div>
              <h2>从一句话开始</h2>
              <p>可以聊角色、世界、剧情，也可以直接开始写。空工程不要求先填写设定。</p>
            </section>
          ) : null}
          {messages.map((message) => (
            <article
              className="conversation-message"
              data-message-id={message.messageId}
              data-role={message.role}
              key={message.messageId}
            >
              <header>
                <strong>{message.role === "user" ? "你" : presentation.name}</strong>
                <span>{message.status === "complete" ? "已保存到工程历史" : message.status}</span>
              </header>
              <p>{message.body}</p>
              <footer className="conversation-message-actions">
                <button
                  disabled={active || pendingAuthorDecision}
                  onClick={() =>
                    setAnchorReference({
                      sourceMessageId: message.messageId,
                      messageBodyHash: message.bodyHash,
                      startUtf8Byte: null,
                      endUtf8Byte: null,
                      selectionHash: null,
                      candidateId: null,
                    })
                  }
                  type="button"
                >
                  锚定整条
                </button>
                <button
                  disabled={active || pendingAuthorDecision}
                  onClick={() => void anchorVisibleSelection(message)}
                  type="button"
                >
                  锚定所选文字
                </button>
              </footer>
            </article>
          ))}
          {pendingUser ? (
            <article className="conversation-message" data-role="user">
              <header>
                <strong>你</strong>
                <span>{pendingUser.status === "failed" ? "发送失败，内容已恢复" : "处理中"}</span>
              </header>
              <p>{pendingUser.body}</p>
            </article>
          ) : null}
          {streamingText ? (
            <article className="conversation-message" data-role="assistant">
              <header>
                <strong>{presentation.name}</strong>
                <span>正在回复</span>
              </header>
              <p>{streamingText}</p>
            </article>
          ) : null}
          {active && !streamingText ? (
            <article aria-live="polite" className="conversation-message" data-role="assistant">
              <header>
                <strong>{presentation.name}</strong>
                <span>{stateLabel(turnState)}</span>
              </header>
              <p>{visibleReason ?? "正在处理你的请求，请稍候……"}</p>
            </article>
          ) : null}
          {commitNotice ? (
            <article className="conversation-message" data-role="assistant" role="status">
              <header>
                <strong>工程</strong>
                <span>Host 回执</span>
              </header>
              <p>{commitNotice}</p>
            </article>
          ) : null}
        </div>
      </section>
      <section aria-label="小说创作输入" className="composer-shell">
        {presentedError ? (
          <div className="composer-error" role="alert">
            <strong>{presentedError.title}</strong>
            <span>{presentedError.message}</span>
            <small>{presentedError.recovery}</small>
            <code>{presentedError.code}</code>
          </div>
        ) : null}
        {anchorReference ? (
          <div className="agent-anchor-reference" role="status">
            <span>已绑定可见消息；请在消息中说明写入人物、设定、素材、剧情或章节。</span>
            <button onClick={() => setAnchorReference(null)} type="button">
              取消绑定
            </button>
          </div>
        ) : null}
        <textarea
          aria-label="输入给 AI 的消息"
          disabled={!desktopRuntime || !projectSessionId || active || pendingAuthorDecision}
          onChange={(event) => setInput(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              void send();
            }
          }}
          placeholder="和 AI 聊角色、世界、剧情，或直接开始写……"
          rows={3}
          value={input}
        />
        <div className="composer-footer">
          <span aria-live="polite">
            {!providerReady
              ? "请先连接左栏中的 Provider"
              : pendingAuthorDecision
                ? "请先在右栏处理冲突或写入预览"
                : !eventChannelReady
                  ? "正在连接本地事件通道"
                  : inputBytes > MAX_INPUT_BYTES
                    ? "输入超过 16 KiB"
                    : stateLabel(turnState)}
          </span>
          {active ? (
            <button
              className="send-button stop-button"
              disabled={busy}
              onClick={() => void cancel()}
              type="button"
            >
              停止
            </button>
          ) : (
            <button
              className="send-button"
              disabled={!canSend}
              onClick={() => void send()}
              type="button"
            >
              发送 <span aria-hidden="true">↑</span>
            </button>
          )}
        </div>
      </section>
      {activityHost ? createPortal(activity, activityHost) : null}
      {session?.historyConsentRequired ? (
        <div className="agent-v2-modal-backdrop">
          <section
            aria-labelledby="agent-privacy-title"
            aria-modal="true"
            className="agent-v2-modal"
            role="dialog"
          >
            <p className="section-kicker">工程聊天历史</p>
            <h2 id="agent-privacy-title">是否在工程内保存完整可见对话？</h2>
            <p>
              聊天会保存在该工程的 <code>.novelgainker/agent/</code> 中，可能随 Git
              或同步工具复制，也可能包含私人创作内容。它不会进入小说 Canon、普通搜索或出版输入。
            </p>
            <div>
              <button disabled={busy} onClick={() => void loadSession(true)} type="button">
                继续并保存
              </button>
            </div>
          </section>
        </div>
      ) : null}
      {session?.branchSelectionRequired ? (
        <div className="agent-v2-modal-backdrop">
          <section
            aria-labelledby="agent-branch-title"
            aria-modal="true"
            className="agent-v2-modal"
            role="dialog"
          >
            <p className="section-kicker">检测到同步分支</p>
            <h2 id="agent-branch-title">选择要继续的聊天历史</h2>
            <p>系统不会自动合并、覆盖或删除任何分支。损坏分支不可选择。</p>
            <div className="agent-v2-branch-list">
              {session.branches.map((branch) => (
                <button
                  disabled={busy || branch.hashStatus === "invalid"}
                  key={branch.branchId}
                  onClick={() => void resolveBranch(branch.branchId)}
                  type="button"
                >
                  <strong>{branch.branchId}</strong>
                  <span>
                    {branch.messageCount} 条消息 · {branch.hashStatus}
                  </span>
                </button>
              ))}
            </div>
          </section>
        </div>
      ) : null}
    </>
  );
}
