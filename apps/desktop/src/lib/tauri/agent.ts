import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { normalizeAppError, parseAppError, type AppError } from "./health";
import type { ProviderId } from "./provider";

export const AGENT_SKILL_REGISTRY_LOAD_COMMAND = "agent_skill_registry_load";
export const AGENT_SESSION_LOAD_COMMAND = "agent_session_load";
export const AGENT_TURN_START_COMMAND = "agent_turn_start";
export const AGENT_TURN_CANCEL_COMMAND = "agent_turn_cancel";
export const AGENT_SESSION_CLEAR_COMMAND = "agent_session_clear";
export const AGENT_POLICY_SET_COMMAND = "agent_policy_set";
export const AGENT_WRITE_CONFIRM_COMMAND = "agent_write_confirm";
export const AGENT_WRITE_REJECT_COMMAND = "agent_write_reject";
export const AGENT_UNDO_APPLY_COMMAND = "agent_undo_apply";
export const AGENT_TURN_EVENT_CHANNEL = "agent-turn-event";

export type AgentSkill = {
  id: string;
  displayName: string;
  description: string;
  readScopes: string[];
  writeScopes: string[];
};

export type AgentSkillRegistry = {
  schemaVersion: 1;
  packId: string;
  packVersion: string;
  packHash: string;
  runtimeEnabled: true;
  skills: AgentSkill[];
  readToolIds: string[];
  writeToolIds: string[];
};

export type AgentAuthorizationPolicy = "plan_only" | "review_every_write" | "auto_apply_safe";

export type AgentDiffLine = {
  kind: "context" | "add" | "remove";
  oldLine: number | null;
  newLine: number | null;
  content: string;
};

export type AgentWriteFilePreview = {
  callId: string;
  toolId: "project.create_markdown" | "project.patch_markdown";
  relativePath: string;
  operation: "create" | "replace";
  mutationShape: string;
  action: string;
  risk: "write_safe" | "always_confirm";
  expectedSourceHash: string | null;
  resultHash: string;
  resultBytes: number;
  fullContent: string;
  lineDiff: AgentDiffLine[];
};

export type AgentWritePreview = {
  schemaVersion: 1;
  pendingId: string;
  policy: AgentAuthorizationPolicy;
  authorizationEffect: "execute" | "await_confirmation" | "block";
  requiresConfirmation: boolean;
  canConfirm: boolean;
  transactionHash: string;
  actionPlanHash: string;
  sourceSetHash: string;
  policySnapshotHash: string;
  highestRisk: "write_safe" | "always_confirm";
  files: AgentWriteFilePreview[];
  projectWriteCount: 0;
};

export type AgentUndoSnapshot = {
  transactionHash: string;
  turnId: string;
  status: "ready" | "consumed" | "expired" | "hash_conflict" | "decrypt_failed" | "cleared";
  targetPaths: string[];
  createdAtMs: number;
  expiresAtMs: number;
};

export type AgentRecentTurn = {
  turnId: string;
  terminalState: "completed" | "cancelled" | "failed";
  skillIds: string[];
  toolIds: string[];
  actionCount: number;
  toolCallCount: number;
  updatedAtMs: number;
};

export type AgentSessionSnapshot = {
  schemaVersion: 1;
  projectSessionId: string;
  projectId: string;
  agentSessionId: string;
  providerId: ProviderId;
  model: string;
  packVersion: string;
  packHash: string;
  summaryText: string | null;
  summaryVersion: number;
  policy: AgentAuthorizationPolicy;
  recentTurns: AgentRecentTurn[];
  undoRecords: AgentUndoSnapshot[];
  pendingWrites: AgentWritePreview[];
  fullChatPersisted: false;
  projectWriteCount: 0;
};

export type AgentCommitResult = {
  receipt: {
    transactionHash: string;
    committedResults: Array<{ path: string; resultHash: string }>;
  };
  undoRecord: { transactionHash: string; status: "ready" };
  projectWriteCount: number;
};

export type AgentUndoApplyResult = {
  transactionHash: string;
  restoredFileCount: number;
  projectWriteCount: number;
};

export type AgentTurnStarted = {
  runId: string;
  turnId: string;
  agentSessionId: string;
  providerId: ProviderId;
  model: string;
  selectedSkillIds: string[];
};

type AgentTurnEventBase = AgentTurnStarted & { sequence: number };

export type AgentTurnEvent =
  | (AgentTurnEventBase & { kind: "turnState"; state: string })
  | (AgentTurnEventBase & {
      kind: "assistantMessage";
      text: string;
      finalSummary: boolean;
    })
  | (AgentTurnEventBase & {
      kind: "toolStarted";
      callId: string;
      toolId: string;
      purpose: string;
      path: string;
    })
  | (AgentTurnEventBase & {
      kind: "toolCompleted";
      callId: string;
      toolId: string;
      status: "succeeded" | "failed" | "cancelled";
      sourcePaths: string[];
      truncated: boolean;
      projectWriteCount: 0;
    })
  | (AgentTurnEventBase & { kind: "writePreview"; preview: AgentWritePreview })
  | (AgentTurnEventBase & { kind: "writeCommitted"; result: AgentCommitResult })
  | (AgentTurnEventBase & {
      kind: "completed";
      summary: string;
      usage: AgentUsage | null;
      projectWriteCount: number;
    })
  | (AgentTurnEventBase & { kind: "failed"; error: AppError })
  | (AgentTurnEventBase & { kind: "cancelled"; projectWriteCount: 0 });

export type AgentUsage = {
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
};

export type AgentSessionClearResult = {
  counts: {
    sessionsRemoved: number;
    turnsRemoved: number;
    messageIndexesRemoved: number;
    undoRecordsRemoved: number;
    canonWriteCount: 0;
  };
  session: AgentSessionSnapshot;
};

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
type ListenEvent = (
  channel: string,
  handler: (event: { payload: unknown }) => void,
) => Promise<UnlistenFn>;

const PROVIDER_MODELS: Record<ProviderId, string> = {
  deepseek: "deepseek-v4-pro",
  openai: "gpt-5.6-terra",
};
const SKILL_IDS = [
  "create-novel-project",
  "create-setting",
  "create-character",
  "create-material",
  "create-plot",
  "novel-story-telling",
  "analytical-review",
  "create-visual-asset",
  "publish-novel",
] as const;
const READ_TOOL_IDS = [
  "project.list_markdown",
  "project.read_markdown",
  "project.search_markdown",
  "project.validate_markdown",
] as const;
const WRITE_TOOL_IDS = ["project.create_markdown", "project.patch_markdown"] as const;
const TOOL_IDS = [...READ_TOOL_IDS, ...WRITE_TOOL_IDS] as const;
const RUN_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;
const OPAQUE_ID = /^(?:ses|trn|call|pnd|ubl)_[A-Za-z0-9_-]{8,91}$/u;
const HASH = /^[0-9a-f]{64}$/u;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function exact(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function safeCount(value: unknown, maximum = Number.MAX_SAFE_INTEGER): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0 && Number(value) <= maximum;
}

function provider(value: unknown): value is ProviderId {
  return value === "deepseek" || value === "openai";
}

function stringArray(value: unknown, allowed?: readonly string[]): value is string[] {
  return (
    Array.isArray(value) &&
    value.every((item) => typeof item === "string" && (!allowed || allowed.includes(item)))
  );
}

function frozenIdentity(value: Record<string, unknown>): boolean {
  return (
    typeof value.runId === "string" &&
    RUN_ID.test(value.runId) &&
    typeof value.turnId === "string" &&
    OPAQUE_ID.test(value.turnId) &&
    typeof value.agentSessionId === "string" &&
    OPAQUE_ID.test(value.agentSessionId) &&
    provider(value.providerId) &&
    value.model === PROVIDER_MODELS[value.providerId]
  );
}

export function parseAgentSkillRegistry(value: unknown): AgentSkillRegistry {
  if (
    !isRecord(value) ||
    !exact(value, [
      "packHash",
      "packId",
      "packVersion",
      "pendingWrites",
      "policy",
      "readToolIds",
      "runtimeEnabled",
      "schemaVersion",
      "skills",
      "writeToolIds",
    ]) ||
    value.schemaVersion !== 1 ||
    value.runtimeEnabled !== true ||
    typeof value.packId !== "string" ||
    typeof value.packVersion !== "string" ||
    typeof value.packHash !== "string" ||
    !HASH.test(value.packHash) ||
    !stringArray(value.readToolIds, READ_TOOL_IDS) ||
    value.readToolIds.length !== READ_TOOL_IDS.length ||
    !stringArray(value.writeToolIds, WRITE_TOOL_IDS) ||
    value.writeToolIds.length !== WRITE_TOOL_IDS.length ||
    !Array.isArray(value.skills) ||
    value.skills.length !== SKILL_IDS.length
  ) {
    throw new Error("Invalid Agent registry contract");
  }
  const skills = value.skills.map((item) => {
    if (
      !isRecord(item) ||
      !exact(item, ["description", "displayName", "id", "readScopes", "writeScopes"]) ||
      typeof item.id !== "string" ||
      !SKILL_IDS.includes(item.id as (typeof SKILL_IDS)[number]) ||
      typeof item.displayName !== "string" ||
      typeof item.description !== "string" ||
      !stringArray(item.readScopes) ||
      !stringArray(item.writeScopes)
    ) {
      throw new Error("Invalid Agent skill contract");
    }
    return item as AgentSkill;
  });
  if (new Set(skills.map((skill) => skill.id)).size !== SKILL_IDS.length) {
    throw new Error("Invalid Agent skill identities");
  }
  return { ...(value as Omit<AgentSkillRegistry, "skills">), skills };
}

function parseRecentTurn(value: unknown): AgentRecentTurn {
  if (
    !isRecord(value) ||
    !exact(value, [
      "actionCount",
      "skillIds",
      "terminalState",
      "toolCallCount",
      "toolIds",
      "turnId",
      "updatedAtMs",
    ]) ||
    typeof value.turnId !== "string" ||
    !OPAQUE_ID.test(value.turnId) ||
    !["completed", "cancelled", "failed"].includes(String(value.terminalState)) ||
    !stringArray(value.skillIds, SKILL_IDS) ||
    value.skillIds.length < 1 ||
    !stringArray(value.toolIds, TOOL_IDS) ||
    !safeCount(value.actionCount, 8) ||
    !safeCount(value.toolCallCount, 16) ||
    value.toolCallCount !== value.toolIds.length ||
    !safeCount(value.updatedAtMs)
  ) {
    throw new Error("Invalid Agent recent turn contract");
  }
  return value as AgentRecentTurn;
}

function policy(value: unknown): value is AgentAuthorizationPolicy {
  return ["plan_only", "review_every_write", "auto_apply_safe"].includes(String(value));
}

function parseDiffLine(value: unknown): AgentDiffLine {
  if (
    !isRecord(value) ||
    !exact(value, ["content", "kind", "newLine", "oldLine"]) ||
    !["context", "add", "remove"].includes(String(value.kind)) ||
    !(value.oldLine === null || (safeCount(value.oldLine) && value.oldLine > 0)) ||
    !(value.newLine === null || (safeCount(value.newLine) && value.newLine > 0)) ||
    typeof value.content !== "string"
  ) {
    throw new Error("Invalid Agent diff line");
  }
  return value as AgentDiffLine;
}

function parseWriteFilePreview(value: unknown): AgentWriteFilePreview {
  if (
    !isRecord(value) ||
    !exact(value, [
      "action",
      "callId",
      "expectedSourceHash",
      "fullContent",
      "lineDiff",
      "mutationShape",
      "operation",
      "relativePath",
      "resultBytes",
      "resultHash",
      "risk",
      "toolId",
    ]) ||
    typeof value.callId !== "string" ||
    !OPAQUE_ID.test(value.callId) ||
    typeof value.toolId !== "string" ||
    !WRITE_TOOL_IDS.includes(value.toolId as (typeof WRITE_TOOL_IDS)[number]) ||
    typeof value.relativePath !== "string" ||
    !["create", "replace"].includes(String(value.operation)) ||
    typeof value.mutationShape !== "string" ||
    typeof value.action !== "string" ||
    !["write_safe", "always_confirm"].includes(String(value.risk)) ||
    !(
      value.expectedSourceHash === null ||
      (typeof value.expectedSourceHash === "string" && HASH.test(value.expectedSourceHash))
    ) ||
    typeof value.resultHash !== "string" ||
    !HASH.test(value.resultHash) ||
    !safeCount(value.resultBytes, 2 * 1024 * 1024) ||
    typeof value.fullContent !== "string" ||
    new TextEncoder().encode(value.fullContent).byteLength !== value.resultBytes ||
    !Array.isArray(value.lineDiff)
  ) {
    throw new Error("Invalid Agent write file preview");
  }
  return {
    ...(value as Omit<AgentWriteFilePreview, "lineDiff">),
    lineDiff: value.lineDiff.map(parseDiffLine),
  };
}

export function parseAgentWritePreview(value: unknown): AgentWritePreview {
  if (
    !isRecord(value) ||
    !exact(value, [
      "actionPlanHash",
      "authorizationEffect",
      "canConfirm",
      "files",
      "highestRisk",
      "pendingId",
      "policy",
      "policySnapshotHash",
      "projectWriteCount",
      "requiresConfirmation",
      "schemaVersion",
      "sourceSetHash",
      "transactionHash",
    ]) ||
    value.schemaVersion !== 1 ||
    typeof value.pendingId !== "string" ||
    !OPAQUE_ID.test(value.pendingId) ||
    !policy(value.policy) ||
    !["execute", "await_confirmation", "block"].includes(String(value.authorizationEffect)) ||
    typeof value.requiresConfirmation !== "boolean" ||
    typeof value.canConfirm !== "boolean" ||
    typeof value.transactionHash !== "string" ||
    !HASH.test(value.transactionHash) ||
    typeof value.actionPlanHash !== "string" ||
    !HASH.test(value.actionPlanHash) ||
    typeof value.sourceSetHash !== "string" ||
    !HASH.test(value.sourceSetHash) ||
    typeof value.policySnapshotHash !== "string" ||
    !HASH.test(value.policySnapshotHash) ||
    !["write_safe", "always_confirm"].includes(String(value.highestRisk)) ||
    !Array.isArray(value.files) ||
    value.files.length < 1 ||
    value.files.length > 8 ||
    value.projectWriteCount !== 0 ||
    (value.authorizationEffect === "block" && value.canConfirm) ||
    (value.authorizationEffect === "await_confirmation" && !value.requiresConfirmation)
  ) {
    throw new Error("Invalid Agent write preview");
  }
  return {
    ...(value as Omit<AgentWritePreview, "files">),
    files: value.files.map(parseWriteFilePreview),
  };
}

function parseUndoSnapshot(value: unknown): AgentUndoSnapshot {
  if (
    !isRecord(value) ||
    !exact(value, [
      "createdAtMs",
      "expiresAtMs",
      "status",
      "targetPaths",
      "transactionHash",
      "turnId",
    ]) ||
    typeof value.transactionHash !== "string" ||
    !HASH.test(value.transactionHash) ||
    typeof value.turnId !== "string" ||
    !OPAQUE_ID.test(value.turnId) ||
    !["ready", "consumed", "expired", "hash_conflict", "decrypt_failed", "cleared"].includes(
      String(value.status),
    ) ||
    !stringArray(value.targetPaths) ||
    value.targetPaths.length < 1 ||
    value.targetPaths.length > 8 ||
    !safeCount(value.createdAtMs) ||
    !safeCount(value.expiresAtMs) ||
    value.expiresAtMs <= value.createdAtMs
  ) {
    throw new Error("Invalid Agent undo snapshot");
  }
  return value as AgentUndoSnapshot;
}

export function parseAgentSession(value: unknown): AgentSessionSnapshot {
  if (
    !isRecord(value) ||
    !exact(value, [
      "agentSessionId",
      "fullChatPersisted",
      "model",
      "packHash",
      "packVersion",
      "pendingWrites",
      "policy",
      "projectId",
      "projectSessionId",
      "projectWriteCount",
      "providerId",
      "recentTurns",
      "schemaVersion",
      "summaryText",
      "summaryVersion",
      "undoRecords",
    ]) ||
    value.schemaVersion !== 1 ||
    typeof value.projectSessionId !== "string" ||
    typeof value.projectId !== "string" ||
    typeof value.agentSessionId !== "string" ||
    !OPAQUE_ID.test(value.agentSessionId) ||
    !provider(value.providerId) ||
    value.model !== PROVIDER_MODELS[value.providerId] ||
    typeof value.packVersion !== "string" ||
    typeof value.packHash !== "string" ||
    !HASH.test(value.packHash) ||
    !(value.summaryText === null || typeof value.summaryText === "string") ||
    !safeCount(value.summaryVersion) ||
    !policy(value.policy) ||
    !Array.isArray(value.recentTurns) ||
    value.recentTurns.length > 100 ||
    !Array.isArray(value.undoRecords) ||
    value.undoRecords.length > 20 ||
    !Array.isArray(value.pendingWrites) ||
    value.pendingWrites.length > 1 ||
    value.fullChatPersisted !== false ||
    value.projectWriteCount !== 0
  ) {
    throw new Error("Invalid Agent session contract");
  }
  const recentTurns = value.recentTurns.map(parseRecentTurn);
  const undoRecords = value.undoRecords.map(parseUndoSnapshot);
  const pendingWrites = value.pendingWrites.map(parseAgentWritePreview);
  return {
    ...(value as Omit<AgentSessionSnapshot, "recentTurns" | "undoRecords" | "pendingWrites">),
    recentTurns,
    undoRecords,
    pendingWrites,
  };
}

export function parseAgentTurnStarted(value: unknown): AgentTurnStarted {
  if (
    !isRecord(value) ||
    !exact(value, [
      "agentSessionId",
      "model",
      "providerId",
      "runId",
      "selectedSkillIds",
      "turnId",
    ]) ||
    !frozenIdentity(value) ||
    !stringArray(value.selectedSkillIds, SKILL_IDS) ||
    value.selectedSkillIds.length < 1 ||
    value.selectedSkillIds.length > 4
  ) {
    throw new Error("Invalid Agent turn start contract");
  }
  return value as AgentTurnStarted;
}

function parseUsage(value: unknown): AgentUsage | null {
  if (value === null) return null;
  if (
    !isRecord(value) ||
    !exact(value, ["inputTokens", "outputTokens", "totalTokens"]) ||
    !safeCount(value.inputTokens) ||
    !safeCount(value.outputTokens) ||
    !safeCount(value.totalTokens) ||
    value.totalTokens < value.inputTokens ||
    value.totalTokens < value.outputTokens
  ) {
    throw new Error("Invalid Agent usage contract");
  }
  return value as AgentUsage;
}

function parseCommitResult(value: unknown): AgentCommitResult {
  if (
    !isRecord(value) ||
    !exact(value, ["projectWriteCount", "receipt", "undoRecord"]) ||
    !safeCount(value.projectWriteCount, 8) ||
    value.projectWriteCount < 1 ||
    !isRecord(value.receipt) ||
    !exact(value.receipt, [
      "committedAtMs",
      "committedResults",
      "projectId",
      "recovery",
      "resultSetHash",
      "schemaVersion",
      "sessionId",
      "transactionHash",
      "turnId",
    ]) ||
    value.receipt.schemaVersion !== 1 ||
    typeof value.receipt.transactionHash !== "string" ||
    !HASH.test(value.receipt.transactionHash) ||
    typeof value.receipt.resultSetHash !== "string" ||
    !HASH.test(value.receipt.resultSetHash) ||
    value.receipt.recovery !== "not_required" ||
    !safeCount(value.receipt.committedAtMs) ||
    !Array.isArray(value.receipt.committedResults) ||
    value.receipt.committedResults.length !== value.projectWriteCount ||
    !value.receipt.committedResults.every(
      (item) =>
        isRecord(item) &&
        exact(item, ["path", "resultHash"]) &&
        typeof item.path === "string" &&
        typeof item.resultHash === "string" &&
        HASH.test(item.resultHash),
    ) ||
    !isRecord(value.undoRecord) ||
    !exact(value.undoRecord, [
      "blobHash",
      "blobId",
      "clearGeneration",
      "createdAtMs",
      "encryptedBytes",
      "expiresAtMs",
      "projectId",
      "protection",
      "schemaVersion",
      "sessionId",
      "status",
      "targetSetHash",
      "targets",
      "transactionHash",
      "turnId",
    ]) ||
    value.undoRecord.schemaVersion !== 1 ||
    value.undoRecord.status !== "ready" ||
    value.undoRecord.protection !== "windows_user_context" ||
    typeof value.undoRecord.transactionHash !== "string" ||
    value.undoRecord.transactionHash !== value.receipt.transactionHash ||
    !Array.isArray(value.undoRecord.targets) ||
    value.undoRecord.targets.length !== value.projectWriteCount
  ) {
    throw new Error("Invalid Agent commit result");
  }
  return {
    receipt: {
      transactionHash: value.receipt.transactionHash,
      committedResults: value.receipt.committedResults as Array<{
        path: string;
        resultHash: string;
      }>,
    },
    undoRecord: {
      transactionHash: value.undoRecord.transactionHash,
      status: "ready",
    },
    projectWriteCount: value.projectWriteCount,
  };
}

export function parseAgentTurnEvent(value: unknown): AgentTurnEvent {
  if (
    !isRecord(value) ||
    !frozenIdentity(value) ||
    !safeCount(value.sequence) ||
    value.sequence === 0 ||
    typeof value.kind !== "string"
  ) {
    throw new Error("Invalid Agent event contract");
  }
  const base = ["agentSessionId", "kind", "model", "providerId", "runId", "sequence", "turnId"];
  if (
    value.kind === "turnState" &&
    exact(value, [...base, "state"]) &&
    typeof value.state === "string"
  ) {
    return value as AgentTurnEvent;
  }
  if (
    value.kind === "assistantMessage" &&
    exact(value, [...base, "finalSummary", "text"]) &&
    typeof value.text === "string" &&
    value.text.length > 0 &&
    typeof value.finalSummary === "boolean"
  ) {
    return value as AgentTurnEvent;
  }
  if (
    value.kind === "toolStarted" &&
    exact(value, [...base, "callId", "path", "purpose", "toolId"]) &&
    typeof value.callId === "string" &&
    OPAQUE_ID.test(value.callId) &&
    typeof value.toolId === "string" &&
    TOOL_IDS.includes(value.toolId as (typeof TOOL_IDS)[number]) &&
    typeof value.purpose === "string" &&
    typeof value.path === "string"
  ) {
    return value as AgentTurnEvent;
  }
  if (
    value.kind === "toolCompleted" &&
    exact(value, [
      ...base,
      "callId",
      "projectWriteCount",
      "sourcePaths",
      "status",
      "toolId",
      "truncated",
    ]) &&
    typeof value.callId === "string" &&
    OPAQUE_ID.test(value.callId) &&
    typeof value.toolId === "string" &&
    READ_TOOL_IDS.includes(value.toolId as (typeof READ_TOOL_IDS)[number]) &&
    ["succeeded", "failed", "cancelled"].includes(String(value.status)) &&
    stringArray(value.sourcePaths) &&
    typeof value.truncated === "boolean" &&
    value.projectWriteCount === 0
  ) {
    return value as AgentTurnEvent;
  }
  if (value.kind === "writePreview" && exact(value, [...base, "preview"])) {
    return {
      ...(value as AgentTurnEventBase),
      kind: "writePreview",
      preview: parseAgentWritePreview(value.preview),
    };
  }
  if (value.kind === "writeCommitted" && exact(value, [...base, "result"])) {
    return {
      ...(value as AgentTurnEventBase),
      kind: "writeCommitted",
      result: parseCommitResult(value.result),
    };
  }
  if (
    value.kind === "completed" &&
    exact(value, [...base, "projectWriteCount", "summary", "usage"]) &&
    typeof value.summary === "string" &&
    value.summary.length > 0 &&
    safeCount(value.projectWriteCount, 8)
  ) {
    return {
      ...(value as AgentTurnEventBase),
      kind: "completed",
      summary: value.summary,
      usage: parseUsage(value.usage),
      projectWriteCount: value.projectWriteCount,
    };
  }
  if (value.kind === "failed" && exact(value, [...base, "error"])) {
    const error = parseAppError(value.error);
    if (error) return { ...(value as AgentTurnEventBase), kind: "failed", error };
  }
  if (
    value.kind === "cancelled" &&
    exact(value, [...base, "projectWriteCount"]) &&
    value.projectWriteCount === 0
  ) {
    return value as AgentTurnEvent;
  }
  throw new Error("Invalid Agent event contract");
}

function parseCancel(value: unknown): { cancelRequested: true } {
  if (!isRecord(value) || !exact(value, ["cancelRequested"]) || value.cancelRequested !== true) {
    throw new Error("Invalid Agent cancel contract");
  }
  return { cancelRequested: true };
}

function parseClear(value: unknown): AgentSessionClearResult {
  if (!isRecord(value) || !exact(value, ["counts", "session"]) || !isRecord(value.counts)) {
    throw new Error("Invalid Agent clear contract");
  }
  const counts = value.counts;
  if (
    !exact(counts, [
      "canonWriteCount",
      "messageIndexesRemoved",
      "sessionsRemoved",
      "turnsRemoved",
      "undoRecordsRemoved",
    ]) ||
    !safeCount(counts.sessionsRemoved) ||
    !safeCount(counts.turnsRemoved) ||
    !safeCount(counts.messageIndexesRemoved) ||
    !safeCount(counts.undoRecordsRemoved) ||
    counts.canonWriteCount !== 0
  ) {
    throw new Error("Invalid Agent clear counts");
  }
  return {
    counts: counts as AgentSessionClearResult["counts"],
    session: parseAgentSession(value.session),
  };
}

function parsePolicySet(value: unknown): {
  policy: AgentAuthorizationPolicy;
  projectWriteCount: 0;
} {
  if (
    !isRecord(value) ||
    !exact(value, ["policy", "projectWriteCount"]) ||
    !policy(value.policy) ||
    value.projectWriteCount !== 0
  ) {
    throw new Error("Invalid Agent policy result");
  }
  return value as { policy: AgentAuthorizationPolicy; projectWriteCount: 0 };
}

function parseReject(value: unknown): { pendingId: string; rejected: true; projectWriteCount: 0 } {
  if (
    !isRecord(value) ||
    !exact(value, ["pendingId", "projectWriteCount", "rejected"]) ||
    typeof value.pendingId !== "string" ||
    !OPAQUE_ID.test(value.pendingId) ||
    value.rejected !== true ||
    value.projectWriteCount !== 0
  ) {
    throw new Error("Invalid Agent reject result");
  }
  return value as { pendingId: string; rejected: true; projectWriteCount: 0 };
}

function parseUndoApply(value: unknown): AgentUndoApplyResult {
  if (
    !isRecord(value) ||
    !exact(value, ["projectWriteCount", "restoredFileCount", "transactionHash"]) ||
    typeof value.transactionHash !== "string" ||
    !HASH.test(value.transactionHash) ||
    !safeCount(value.restoredFileCount, 8) ||
    value.restoredFileCount < 1 ||
    value.projectWriteCount !== value.restoredFileCount
  ) {
    throw new Error("Invalid Agent undo result");
  }
  return value as AgentUndoApplyResult;
}

const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);
const listenTauri: ListenEvent = (channel, handler) => listen<unknown>(channel, handler);

async function invokeAgent<T>(
  command: string,
  args: Record<string, unknown>,
  parse: (value: unknown) => T,
  invokeCommand: InvokeCommand,
): Promise<T> {
  let response: unknown;
  try {
    response = await invokeCommand(command, args);
  } catch (error: unknown) {
    throw normalizeAppError(error);
  }
  try {
    return parse(response);
  } catch {
    throw {
      code: "contract.invalid_response",
      messageKey: "errors.invalidRuntimeResponse",
    } satisfies AppError;
  }
}

export function requestAgentSkillRegistry(invokeCommand: InvokeCommand = invokeTauri) {
  return invokeAgent(AGENT_SKILL_REGISTRY_LOAD_COMMAND, {}, parseAgentSkillRegistry, invokeCommand);
}

export function requestAgentSession(
  projectSessionId: string,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_SESSION_LOAD_COMMAND,
    { projectSessionId },
    parseAgentSession,
    invokeCommand,
  );
}

export function requestAgentTurnStart(
  projectSessionId: string,
  expectedProviderId: ProviderId,
  text: string,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_TURN_START_COMMAND,
    { projectSessionId, expectedProviderId, text },
    parseAgentTurnStarted,
    invokeCommand,
  );
}

export function requestAgentTurnCancel(runId: string, invokeCommand: InvokeCommand = invokeTauri) {
  return invokeAgent(AGENT_TURN_CANCEL_COMMAND, { runId }, parseCancel, invokeCommand);
}

export function requestAgentSessionClear(
  projectSessionId: string,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(AGENT_SESSION_CLEAR_COMMAND, { projectSessionId }, parseClear, invokeCommand);
}

export function requestAgentPolicySet(
  projectSessionId: string,
  policyValue: AgentAuthorizationPolicy,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_POLICY_SET_COMMAND,
    { projectSessionId, policy: policyValue },
    parsePolicySet,
    invokeCommand,
  );
}

export function requestAgentWriteConfirm(
  projectSessionId: string,
  preview: AgentWritePreview,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_WRITE_CONFIRM_COMMAND,
    {
      projectSessionId,
      pendingId: preview.pendingId,
      transactionHash: preview.transactionHash,
    },
    parseCommitResult,
    invokeCommand,
  );
}

export function requestAgentWriteReject(
  projectSessionId: string,
  pendingId: string,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_WRITE_REJECT_COMMAND,
    { projectSessionId, pendingId },
    parseReject,
    invokeCommand,
  );
}

export function requestAgentUndoApply(
  projectSessionId: string,
  transactionHash: string,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_UNDO_APPLY_COMMAND,
    { projectSessionId, transactionHash },
    parseUndoApply,
    invokeCommand,
  );
}

export async function listenAgentTurnEvents(
  onEvent: (event: AgentTurnEvent) => void,
  listenEvent: ListenEvent = listenTauri,
): Promise<UnlistenFn> {
  return listenEvent(AGENT_TURN_EVENT_CHANNEL, ({ payload }) => {
    try {
      onEvent(parseAgentTurnEvent(payload));
    } catch {
      // Strict boundary: malformed or forward-incompatible events never enter UI state.
    }
  });
}
