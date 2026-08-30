import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { normalizeAppError, parseAppError, type AppError } from "./health";
import type { ProviderId } from "./provider";

export const AGENT_SKILL_REGISTRY_LOAD_COMMAND = "agent_skill_registry_load";
export const AGENT_SESSION_LOAD_COMMAND = "agent_session_load";
export const AGENT_TURN_START_COMMAND = "agent_turn_start";
export const AGENT_TURN_CANCEL_COMMAND = "agent_turn_cancel";
export const AGENT_SESSION_CLEAR_COMMAND = "agent_session_clear";
export const AGENT_MEMORY_COMPACT_COMMAND = "agent_memory_compact";
export const AGENT_HISTORY_BRANCH_RESOLVE_COMMAND = "agent_history_branch_resolve";
export const AGENT_WRITE_POLICY_SET_COMMAND = "agent_write_policy_set";
export const AGENT_CONTEXT_MARK_UPSERT_COMMAND = "agent_context_mark_upsert";
export const AGENT_CONTEXT_MARK_REMOVE_COMMAND = "agent_context_mark_remove";
export const AGENT_CONFLICT_RESOLUTION_SUBMIT_COMMAND = "agent_conflict_resolution_submit";
export const AGENT_WRITE_CONFIRMATION_SUBMIT_COMMAND = "agent_write_confirmation_submit";
export const AGENT_WRITE_PLAN_REJECT_COMMAND = "agent_write_plan_reject";
export const AGENT_WRITE_UNDO_APPLY_COMMAND = "agent_write_undo_apply";
export const AGENT_TURN_EVENT_CHANNEL = "agent-turn-v2-event";

export const AGENT_V2_COMMANDS = [
  AGENT_SKILL_REGISTRY_LOAD_COMMAND,
  AGENT_SESSION_LOAD_COMMAND,
  AGENT_TURN_START_COMMAND,
  AGENT_TURN_CANCEL_COMMAND,
  AGENT_SESSION_CLEAR_COMMAND,
  AGENT_MEMORY_COMPACT_COMMAND,
  AGENT_HISTORY_BRANCH_RESOLVE_COMMAND,
  AGENT_WRITE_POLICY_SET_COMMAND,
  AGENT_CONTEXT_MARK_UPSERT_COMMAND,
  AGENT_CONTEXT_MARK_REMOVE_COMMAND,
  AGENT_CONFLICT_RESOLUTION_SUBMIT_COMMAND,
  AGENT_WRITE_CONFIRMATION_SUBMIT_COMMAND,
  AGENT_WRITE_PLAN_REJECT_COMMAND,
  AGENT_WRITE_UNDO_APPLY_COMMAND,
] as const;

export type AgentSkillResourceV2 = {
  path: string;
  kind: "instruction" | "template" | "reference";
  sha256: string;
};

export type AgentSkillV2 = {
  id: string;
  displayName: string;
  description: string;
  triggerSummary: string;
  manifestSha256: string;
  resources: AgentSkillResourceV2[];
};

export type AgentSkillRegistryV2 = {
  schemaVersion: 2;
  packId: string;
  packVersion: string;
  packHash: string;
  sourceCommit: string;
  runtimeSource: "bundled_release_only";
  runtimeEnabled: true;
  eligibleSkillIds: string[];
  skills: AgentSkillV2[];
};

export type ConversationMessageV2 = {
  schemaVersion: 2;
  messageId: string;
  responseId: string | null;
  turnId: string;
  role: "user" | "assistant";
  body: string;
  bodyHash: string;
  sourceIds: string[];
  status: "complete" | "cancelled" | "failed";
  createdAtMs: number;
};

export type AnchorReferenceV1 = {
  sourceMessageId: string;
  messageBodyHash: string;
  startUtf8Byte: number | null;
  endUtf8Byte: number | null;
  selectionHash: string | null;
  candidateId: string | null;
};

export type AnchorIntentV1 = {
  schemaVersion: 1;
  anchorId: string;
  projectId: string;
  sessionId: string;
  branchId: string;
  turnId: string;
  intentKind:
    | "decision_declaration"
    | "write_directive"
    | "candidate_acceptance"
    | "text_selection"
    | "agent_suggestion";
  targetKind: "character" | "setting" | "material" | "plot" | "chapter";
  displayName: string | null;
  sourceBindings: Array<{
    sourceMessageId: string;
    messageBodyHash: string;
    startUtf8Byte: number | null;
    endUtf8Byte: number | null;
    selectionHash: string | null;
  }>;
  candidateId: string | null;
  confirmationProvenance:
    | "explicit_author_message"
    | "unique_candidate_short_confirmation"
    | "right_panel_confirmation"
    | "ai_suggestion_only";
  state: "suggested" | "confirmed" | "materialized" | "committed" | "discarded" | "stale";
  createdAtMs: number;
  anchorHash: string;
};

export type AnchorResolutionV1 =
  | { status: "no_anchor" }
  | { status: "clarification_required"; visibleReason: string }
  | { status: "ready"; anchor: AnchorIntentV1 };

export type ContextMarkV1 = {
  schemaVersion: 1;
  markId: string;
  projectId: string;
  sessionId: string;
  branchId: string;
  scope: "session" | "project";
  priority: "medium" | "high";
  creator: "user" | "agent";
  sourceId: string;
  relativePath: string;
  fieldPath: string | null;
  headingPath: string[];
  anchorHash: string;
  documentHash: string;
  status: "active" | "re_read" | "rebound" | "stale" | "archived";
  createdAtMs: number;
  updatedAtMs: number;
  bindingHash: string;
};

export type ContextMarkStoreV1 = {
  schemaVersion: 1;
  projectId: string;
  marks: ContextMarkV1[];
  storeHash: string;
};

export type DraftArtifactV1 = {
  schemaVersion: 1;
  artifactId: string;
  projectId: string;
  sessionId: string;
  branchId: string;
  turnId: string;
  anchorId: string;
  targetKind: "character" | "setting" | "material" | "plot" | "chapter";
  sourceBindings: AnchorIntentV1["sourceBindings"];
  content: string;
  contentHash: string;
  state: "complete" | "discarded" | "stale";
  validated: boolean;
  createdAtMs: number;
  artifactHash: string;
};

export type CanonCandidateV1 = {
  artifact: DraftArtifactV1;
  proposal: {
    schemaVersion: 1;
    projectId: string;
    sessionId: string;
    branchId: string;
    turnId: string;
    anchorId: string;
    anchorHash: string;
    artifactId: string;
    artifactHash: string;
    proposal: {
      schemaVersion: 1;
      requestId: string;
      proposalId: string;
      targetKind: "character" | "setting" | "material" | "plot" | "chapter";
      displayName: string;
      operation: "create" | "extend" | "append_chapter";
      facts: Array<{
        field: string;
        value: string;
        sources: Array<{
          kind: "visible_message" | "draft_artifact" | "canon_source";
          sourceId: string;
          sourceHash: string;
        }>;
      }>;
    };
    proposalHash: string;
    createdAtMs: number;
  };
};

export type ConflictResolutionChoiceV1 =
  "keep_existing" | "adopt_candidate" | "merge_manual" | "save_as_alternative" | "cancel_change";

export type ConflictSetV1 = {
  schemaVersion: 1;
  conflictSetId: string;
  projectId: string;
  sessionId: string;
  branchId: string;
  turnId: string;
  proposalId: string;
  artifactId: string;
  proposalHash: string;
  conflictSetHash: string;
  requiresResolution: boolean;
  createdAtMs: number;
  conflicts: Array<{
    conflictId: string;
    kind:
      | "identity"
      | "field_value"
      | "relationship"
      | "timeline"
      | "world_rule"
      | "plot_thread"
      | "chapter_state"
      | "source_hash"
      | "semantic";
    confidence: "confirmed_conflict" | "clarification_required";
    proposition: string;
    existingValue: string;
    candidateValue: string;
    existingSourceId: string;
    existingSourceHash: string;
    candidateSourceIds: string[];
    relativePath: string;
    headingPath: string[];
    visibleExplanation: string;
  }>;
};

export type AgentWritePlanV2 = {
  schemaVersion: 2;
  pendingId: string;
  projectId: string;
  sessionId: string;
  branchId: string;
  turnId: string;
  proposalId: string;
  artifactId: string;
  proposalHash: string;
  planHash: string;
  sourceBindingHash: string;
  conflictSetHash: string | null;
  policy: "plan_only" | "review_every_write" | "auto_apply_safe";
  policyHash: string;
  preview: {
    schemaVersion: 1;
    pendingId: string;
    policy: "plan_only" | "review_every_write" | "auto_apply_safe";
    requiresConfirmation: boolean;
    canConfirm: boolean;
    authorizationEffect: "execute" | "await_confirmation" | "block";
    transactionHash: string;
    actionPlanHash: string;
    sourceSetHash: string;
    policySnapshotHash: string;
    highestRisk: "read-safe" | "write-safe" | "always-confirm" | "forbidden-v1";
    projectWriteCount: number;
    files: Array<{
      callId: string;
      toolId: "project.create_markdown" | "project.patch_markdown";
      relativePath: string;
      operation: "create" | "replace";
      mutationShape: string;
      action: string;
      risk: "read-safe" | "write-safe" | "always-confirm" | "forbidden-v1";
      expectedSourceHash: string | null;
      fullContent: string;
      resultHash: string;
      resultBytes: number;
      lineDiff: Array<{
        kind: "context" | "add" | "remove";
        oldLine: number | null;
        newLine: number | null;
        content: string;
      }>;
    }>;
  };
};

export type AgentWritePolicySnapshotV2 = {
  policy: "plan_only" | "review_every_write" | "auto_apply_safe";
  policyHash: string;
};

export type AgentConflictResolutionResponseV2 = {
  resolution: {
    conflictSetId: string;
    conflictSetHash: string;
    choice: ConflictResolutionChoiceV1;
    outcome:
      | "candidate_discarded"
      | "candidate_requires_confirmation"
      | "manual_conversation_required"
      | "alternative_requires_renormalization"
      | "change_cancelled";
    requiresWriteConfirmation: boolean;
    consumed: true;
  };
  writePlan: AgentWritePlanV2 | null;
};

export type AgentUndoApplyResultV1 = {
  transactionHash: string;
  restoredFileCount: number;
  projectWriteCount: number;
};

export type AgentCommitReceiptV2 = {
  schemaVersion: 2;
  receiptId: string;
  transactionHash: string;
  resultHash: string;
  relativePaths: string[];
  committedAtMs: number;
  undoExpiresAtMs: number;
  projectWriteCount: number;
};

export type SourceRefV2 = {
  sourceId: string;
  relativePath: string;
  entity: string | null;
  fieldPath: string | null;
  headingPath: string[];
  contentHash: string;
  excerptHash: string;
  excerpt: string;
  truncated: boolean;
  untrusted: true;
};

export type AgentHistoryBranchV2 = {
  branchId: string;
  sessionId: string;
  lastTurnAtMs: number | null;
  messageCount: number;
  hashStatus: "valid" | "empty" | "invalid";
};

export type SessionMemoryV2 = {
  schemaVersion: 2;
  projectId: string;
  sessionId: string;
  branchId: string;
  completedTurns: number;
  visibleMessageCount: number;
  visibleCharacterCount: number;
  summary: string;
  compactedThroughTurnId: string | null;
  lastTurnId: string | null;
  lastRecordHash: string | null;
  needsCompaction: boolean;
  memoryHash: string;
};

export type AgentSessionSnapshotV2 = {
  schemaVersion: 2;
  projectId: string;
  historyConsentRequired: boolean;
  branchSelectionRequired: boolean;
  activeBranchId: string | null;
  sessionId: string | null;
  branches: AgentHistoryBranchV2[];
  messages: ConversationMessageV2[];
  memory: SessionMemoryV2 | null;
  draftQuota: 0;
  writePolicy: "plan_only" | "review_every_write" | "auto_apply_safe";
  writePolicyHash: string | null;
  contextMarks: ContextMarkStoreV1;
};

export type TurnBudgetUsageV2 = {
  providerCalls: number;
  totalProviderInputBytes: number;
  totalProviderOutputBytes: number;
  accumulatedContextResultBytes: number;
  structuredRepairCalls: number;
  totalProviderInputTokens: number;
  totalProviderOutputTokens: number;
};

export type AgentTurnStartedV2 = {
  schemaVersion: 2;
  runId: string;
  turnId: string;
  providerId: ProviderId;
  model: string;
};

type AgentTurnEventBaseV2 = AgentTurnStartedV2 & {
  projectSessionId: string;
  sequence: number;
};

export type AgentTurnEventV2 =
  | (AgentTurnEventBaseV2 & {
      kind: "turnState";
      state: string;
      visibleReason: string | null;
    })
  | (AgentTurnEventBaseV2 & {
      kind: "skillSelection";
      selectedSkillIds: string[];
      intentClass: string;
      visibleReason: string;
    })
  | (AgentTurnEventBaseV2 & {
      kind: "contextSources";
      sources: SourceRefV2[];
      indexHash: string;
    })
  | (AgentTurnEventBaseV2 & {
      kind: "anchorCandidate";
      resolution: AnchorResolutionV1;
    })
  | (AgentTurnEventBaseV2 & { kind: "canonCandidate" } & CanonCandidateV1)
  | (AgentTurnEventBaseV2 & { kind: "contextMarks"; store: ContextMarkStoreV1 })
  | (AgentTurnEventBaseV2 & { kind: "conflict"; conflictSet: ConflictSetV1 })
  | (AgentTurnEventBaseV2 & { kind: "writePlan"; plan: AgentWritePlanV2 })
  | (AgentTurnEventBaseV2 & { kind: "commitReceipt"; receipt: AgentCommitReceiptV2 })
  | (AgentTurnEventBaseV2 & { kind: "conversationDelta"; delta: string })
  | (AgentTurnEventBaseV2 & {
      kind: "completed";
      message: ConversationMessageV2;
      sources: SourceRefV2[];
      usage: TurnBudgetUsageV2;
      agentCanonWriteCount: number;
    })
  | (AgentTurnEventBaseV2 & {
      kind: "failed";
      message: ConversationMessageV2 | null;
      error: AppError;
      usage: TurnBudgetUsageV2;
      agentCanonWriteCount: number;
    })
  | (AgentTurnEventBaseV2 & {
      kind: "cancelled";
      message: ConversationMessageV2 | null;
      usage: TurnBudgetUsageV2;
      agentCanonWriteCount: number;
    });

export type AgentHistoryClearResultV2 = {
  scope: "session" | "all_history";
  clearedBranchCount: number;
  clearedMessageCount: number;
  snapshot: AgentSessionSnapshotV2;
};

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
type ListenEvent = (
  channel: string,
  handler: (event: { payload: unknown }) => void,
) => Promise<UnlistenFn>;

const HASH = /^[a-f0-9]{64}$/u;
const ID =
  /^(?:anc|art|brn|cand|conf|cset|mark|msg|pnd|prop|rcpt|req|rsp|run|ses|src|trn)_[A-Za-z0-9_-]{8,80}$/u;
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

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function exact(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function count(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0;
}

function strings(value: unknown, maximum = 48): value is string[] {
  return (
    Array.isArray(value) &&
    value.length <= maximum &&
    value.every((item) => typeof item === "string")
  );
}

function provider(value: unknown): value is ProviderId {
  return value === "deepseek" || value === "openai";
}

function writePolicy(
  value: unknown,
): value is "plan_only" | "review_every_write" | "auto_apply_safe" {
  return ["plan_only", "review_every_write", "auto_apply_safe"].includes(String(value));
}

function nullableId(value: unknown): value is string | null {
  return value === null || (typeof value === "string" && ID.test(value));
}

function parseSource(value: unknown): SourceRefV2 {
  if (
    !record(value) ||
    !exact(value, [
      "sourceId",
      "relativePath",
      "entity",
      "fieldPath",
      "headingPath",
      "contentHash",
      "excerptHash",
      "excerpt",
      "truncated",
      "untrusted",
    ]) ||
    typeof value.sourceId !== "string" ||
    !ID.test(value.sourceId) ||
    typeof value.relativePath !== "string" ||
    value.relativePath.length === 0 ||
    value.relativePath.startsWith(".novelgainker/") ||
    value.relativePath.includes("..") ||
    !(value.entity === null || typeof value.entity === "string") ||
    !(value.fieldPath === null || typeof value.fieldPath === "string") ||
    !strings(value.headingPath, 16) ||
    typeof value.contentHash !== "string" ||
    !HASH.test(value.contentHash) ||
    typeof value.excerptHash !== "string" ||
    !HASH.test(value.excerptHash) ||
    typeof value.excerpt !== "string" ||
    value.excerpt.length > 65_536 ||
    typeof value.truncated !== "boolean" ||
    value.untrusted !== true
  )
    throw new Error("Invalid SourceRefV2");
  return value as SourceRefV2;
}

function parseMessage(value: unknown): ConversationMessageV2 {
  if (
    !record(value) ||
    !exact(value, [
      "schemaVersion",
      "messageId",
      "responseId",
      "turnId",
      "role",
      "body",
      "bodyHash",
      "sourceIds",
      "status",
      "createdAtMs",
    ]) ||
    value.schemaVersion !== 2 ||
    typeof value.messageId !== "string" ||
    !ID.test(value.messageId) ||
    !nullableId(value.responseId) ||
    typeof value.turnId !== "string" ||
    !ID.test(value.turnId) ||
    (value.role !== "user" && value.role !== "assistant") ||
    typeof value.body !== "string" ||
    value.body.length === 0 ||
    value.body.length > 131_072 ||
    typeof value.bodyHash !== "string" ||
    !HASH.test(value.bodyHash) ||
    !strings(value.sourceIds) ||
    !value.sourceIds.every((item) => ID.test(item)) ||
    !["complete", "cancelled", "failed"].includes(String(value.status)) ||
    !count(value.createdAtMs)
  )
    throw new Error("Invalid ConversationMessageV2");
  return value as ConversationMessageV2;
}

function parseAnchorResolution(value: unknown): AnchorResolutionV1 {
  if (!record(value) || typeof value.status !== "string") {
    throw new Error("Invalid AnchorResolutionV1");
  }
  if (value.status === "no_anchor" && exact(value, ["status"])) return value as AnchorResolutionV1;
  if (
    value.status === "clarification_required" &&
    exact(value, ["status", "visibleReason"]) &&
    typeof value.visibleReason === "string" &&
    value.visibleReason.length > 0
  )
    return value as AnchorResolutionV1;
  if (value.status !== "ready" || !exact(value, ["status", "anchor"]) || !record(value.anchor))
    throw new Error("Invalid AnchorResolutionV1");
  const anchor = value.anchor;
  if (
    !exact(anchor, [
      "schemaVersion",
      "anchorId",
      "projectId",
      "sessionId",
      "branchId",
      "turnId",
      "intentKind",
      "targetKind",
      "displayName",
      "sourceBindings",
      "candidateId",
      "confirmationProvenance",
      "state",
      "createdAtMs",
      "anchorHash",
    ]) ||
    anchor.schemaVersion !== 1 ||
    typeof anchor.anchorId !== "string" ||
    !ID.test(anchor.anchorId) ||
    typeof anchor.projectId !== "string" ||
    typeof anchor.sessionId !== "string" ||
    !ID.test(anchor.sessionId) ||
    typeof anchor.branchId !== "string" ||
    !ID.test(anchor.branchId) ||
    typeof anchor.turnId !== "string" ||
    !ID.test(anchor.turnId) ||
    ![
      "decision_declaration",
      "write_directive",
      "candidate_acceptance",
      "text_selection",
      "agent_suggestion",
    ].includes(String(anchor.intentKind)) ||
    !["character", "setting", "material", "plot", "chapter"].includes(String(anchor.targetKind)) ||
    !(anchor.displayName === null || typeof anchor.displayName === "string") ||
    !Array.isArray(anchor.sourceBindings) ||
    anchor.sourceBindings.length === 0 ||
    anchor.sourceBindings.length > 8 ||
    !(
      anchor.candidateId === null ||
      (typeof anchor.candidateId === "string" && ID.test(anchor.candidateId))
    ) ||
    ![
      "explicit_author_message",
      "unique_candidate_short_confirmation",
      "right_panel_confirmation",
      "ai_suggestion_only",
    ].includes(String(anchor.confirmationProvenance)) ||
    !["suggested", "confirmed", "materialized", "committed", "discarded", "stale"].includes(
      String(anchor.state),
    ) ||
    !count(anchor.createdAtMs) ||
    typeof anchor.anchorHash !== "string" ||
    !HASH.test(anchor.anchorHash)
  )
    throw new Error("Invalid AnchorIntentV1");
  anchor.sourceBindings.forEach((binding) => {
    if (
      !record(binding) ||
      !exact(binding, [
        "sourceMessageId",
        "messageBodyHash",
        "startUtf8Byte",
        "endUtf8Byte",
        "selectionHash",
      ]) ||
      typeof binding.sourceMessageId !== "string" ||
      !ID.test(binding.sourceMessageId) ||
      typeof binding.messageBodyHash !== "string" ||
      !HASH.test(binding.messageBodyHash) ||
      !(binding.startUtf8Byte === null || count(binding.startUtf8Byte)) ||
      !(binding.endUtf8Byte === null || count(binding.endUtf8Byte)) ||
      !(
        binding.selectionHash === null ||
        (typeof binding.selectionHash === "string" && HASH.test(binding.selectionHash))
      )
    )
      throw new Error("Invalid AnchorSourceBindingV1");
  });
  return { status: "ready", anchor: anchor as AnchorIntentV1 };
}

function parseCanonCandidate(artifactValue: unknown, proposalValue: unknown): CanonCandidateV1 {
  if (
    !record(artifactValue) ||
    !exact(artifactValue, [
      "schemaVersion",
      "artifactId",
      "projectId",
      "sessionId",
      "branchId",
      "turnId",
      "anchorId",
      "targetKind",
      "sourceBindings",
      "content",
      "contentHash",
      "state",
      "validated",
      "createdAtMs",
      "artifactHash",
    ]) ||
    artifactValue.schemaVersion !== 1 ||
    typeof artifactValue.artifactId !== "string" ||
    !ID.test(artifactValue.artifactId) ||
    typeof artifactValue.projectId !== "string" ||
    typeof artifactValue.sessionId !== "string" ||
    !ID.test(artifactValue.sessionId) ||
    typeof artifactValue.branchId !== "string" ||
    !ID.test(artifactValue.branchId) ||
    typeof artifactValue.turnId !== "string" ||
    !ID.test(artifactValue.turnId) ||
    typeof artifactValue.anchorId !== "string" ||
    !ID.test(artifactValue.anchorId) ||
    !["character", "setting", "material", "plot", "chapter"].includes(
      String(artifactValue.targetKind),
    ) ||
    !Array.isArray(artifactValue.sourceBindings) ||
    artifactValue.sourceBindings.length === 0 ||
    artifactValue.sourceBindings.length > 8 ||
    typeof artifactValue.content !== "string" ||
    typeof artifactValue.contentHash !== "string" ||
    !HASH.test(artifactValue.contentHash) ||
    !["complete", "discarded", "stale"].includes(String(artifactValue.state)) ||
    typeof artifactValue.validated !== "boolean" ||
    !count(artifactValue.createdAtMs) ||
    typeof artifactValue.artifactHash !== "string" ||
    !HASH.test(artifactValue.artifactHash)
  )
    throw new Error("Invalid DraftArtifactV1");
  for (const binding of artifactValue.sourceBindings) {
    if (
      !record(binding) ||
      !exact(binding, [
        "sourceMessageId",
        "messageBodyHash",
        "startUtf8Byte",
        "endUtf8Byte",
        "selectionHash",
      ]) ||
      typeof binding.sourceMessageId !== "string" ||
      !ID.test(binding.sourceMessageId) ||
      typeof binding.messageBodyHash !== "string" ||
      !HASH.test(binding.messageBodyHash) ||
      !(binding.startUtf8Byte === null || count(binding.startUtf8Byte)) ||
      !(binding.endUtf8Byte === null || count(binding.endUtf8Byte)) ||
      !(
        binding.selectionHash === null ||
        (typeof binding.selectionHash === "string" && HASH.test(binding.selectionHash))
      )
    )
      throw new Error("Invalid AnchorSourceBindingV1");
  }
  if (
    !record(proposalValue) ||
    !exact(proposalValue, [
      "schemaVersion",
      "projectId",
      "sessionId",
      "branchId",
      "turnId",
      "anchorId",
      "anchorHash",
      "artifactId",
      "artifactHash",
      "proposal",
      "proposalHash",
      "createdAtMs",
    ]) ||
    proposalValue.schemaVersion !== 1 ||
    proposalValue.projectId !== artifactValue.projectId ||
    proposalValue.sessionId !== artifactValue.sessionId ||
    proposalValue.branchId !== artifactValue.branchId ||
    proposalValue.turnId !== artifactValue.turnId ||
    proposalValue.anchorId !== artifactValue.anchorId ||
    typeof proposalValue.anchorHash !== "string" ||
    !HASH.test(proposalValue.anchorHash) ||
    proposalValue.artifactId !== artifactValue.artifactId ||
    proposalValue.artifactHash !== artifactValue.artifactHash ||
    typeof proposalValue.proposalHash !== "string" ||
    !HASH.test(proposalValue.proposalHash) ||
    !count(proposalValue.createdAtMs) ||
    !record(proposalValue.proposal)
  )
    throw new Error("Invalid PersistedCanonProposalV1");
  const proposal = proposalValue.proposal;
  if (
    !exact(proposal, [
      "schemaVersion",
      "requestId",
      "proposalId",
      "artifactId",
      "targetKind",
      "displayName",
      "operation",
      "facts",
    ]) ||
    proposal.schemaVersion !== 1 ||
    typeof proposal.requestId !== "string" ||
    !ID.test(proposal.requestId) ||
    typeof proposal.proposalId !== "string" ||
    !ID.test(proposal.proposalId) ||
    proposal.targetKind !== artifactValue.targetKind ||
    typeof proposal.displayName !== "string" ||
    !["create", "extend", "append_chapter"].includes(String(proposal.operation)) ||
    !Array.isArray(proposal.facts) ||
    proposal.facts.length === 0 ||
    proposal.facts.length > 32
  )
    throw new Error("Invalid CanonProposalV1");
  for (const fact of proposal.facts) {
    if (
      !record(fact) ||
      !exact(fact, ["field", "value", "sources"]) ||
      typeof fact.field !== "string" ||
      typeof fact.value !== "string" ||
      !Array.isArray(fact.sources) ||
      fact.sources.length === 0 ||
      fact.sources.length > 8
    )
      throw new Error("Invalid CanonFactV1");
    for (const source of fact.sources) {
      if (
        !record(source) ||
        !exact(source, ["kind", "sourceId", "sourceHash"]) ||
        !["visible_message", "draft_artifact", "canon_source"].includes(String(source.kind)) ||
        typeof source.sourceId !== "string" ||
        !ID.test(source.sourceId) ||
        typeof source.sourceHash !== "string" ||
        !HASH.test(source.sourceHash)
      )
        throw new Error("Invalid CanonFactSourceV1");
    }
  }
  return {
    artifact: artifactValue as DraftArtifactV1,
    proposal: proposalValue as CanonCandidateV1["proposal"],
  };
}

function parseMarkStore(value: unknown): ContextMarkStoreV1 {
  if (
    !record(value) ||
    !exact(value, ["schemaVersion", "projectId", "marks", "storeHash"]) ||
    value.schemaVersion !== 1 ||
    typeof value.projectId !== "string" ||
    !Array.isArray(value.marks) ||
    value.marks.length > 64 ||
    typeof value.storeHash !== "string" ||
    !HASH.test(value.storeHash)
  )
    throw new Error("Invalid ContextMarkStoreV1");
  const marks = value.marks.map((mark) => {
    if (
      !record(mark) ||
      !exact(mark, [
        "schemaVersion",
        "markId",
        "projectId",
        "sessionId",
        "branchId",
        "scope",
        "priority",
        "creator",
        "sourceId",
        "relativePath",
        "fieldPath",
        "headingPath",
        "anchorHash",
        "documentHash",
        "status",
        "createdAtMs",
        "updatedAtMs",
        "bindingHash",
      ]) ||
      mark.schemaVersion !== 1 ||
      typeof mark.markId !== "string" ||
      !ID.test(mark.markId) ||
      typeof mark.projectId !== "string" ||
      typeof mark.sessionId !== "string" ||
      !ID.test(mark.sessionId) ||
      typeof mark.branchId !== "string" ||
      !ID.test(mark.branchId) ||
      !["session", "project"].includes(String(mark.scope)) ||
      !["medium", "high"].includes(String(mark.priority)) ||
      !["user", "agent"].includes(String(mark.creator)) ||
      typeof mark.sourceId !== "string" ||
      !ID.test(mark.sourceId) ||
      typeof mark.relativePath !== "string" ||
      mark.relativePath.startsWith(".novelgainker/") ||
      !(mark.fieldPath === null || typeof mark.fieldPath === "string") ||
      !strings(mark.headingPath, 16) ||
      typeof mark.anchorHash !== "string" ||
      !HASH.test(mark.anchorHash) ||
      typeof mark.documentHash !== "string" ||
      !HASH.test(mark.documentHash) ||
      !["active", "re_read", "rebound", "stale", "archived"].includes(String(mark.status)) ||
      !count(mark.createdAtMs) ||
      !count(mark.updatedAtMs) ||
      typeof mark.bindingHash !== "string" ||
      !HASH.test(mark.bindingHash)
    )
      throw new Error("Invalid ContextMarkV1");
    return mark as ContextMarkV1;
  });
  return { ...(value as ContextMarkStoreV1), marks };
}

function parseConflictSet(value: unknown): ConflictSetV1 {
  if (
    !record(value) ||
    !exact(value, [
      "schemaVersion",
      "conflictSetId",
      "projectId",
      "sessionId",
      "branchId",
      "turnId",
      "proposalId",
      "artifactId",
      "proposalHash",
      "conflicts",
      "requiresResolution",
      "createdAtMs",
      "conflictSetHash",
    ]) ||
    value.schemaVersion !== 1 ||
    typeof value.conflictSetId !== "string" ||
    !ID.test(value.conflictSetId) ||
    typeof value.conflictSetHash !== "string" ||
    !HASH.test(value.conflictSetHash) ||
    typeof value.projectId !== "string" ||
    typeof value.sessionId !== "string" ||
    !ID.test(value.sessionId) ||
    typeof value.branchId !== "string" ||
    !ID.test(value.branchId) ||
    typeof value.turnId !== "string" ||
    !ID.test(value.turnId) ||
    typeof value.proposalId !== "string" ||
    !ID.test(value.proposalId) ||
    typeof value.artifactId !== "string" ||
    !ID.test(value.artifactId) ||
    typeof value.artifactId !== "string" ||
    !ID.test(value.artifactId) ||
    typeof value.proposalHash !== "string" ||
    !HASH.test(value.proposalHash) ||
    typeof value.requiresResolution !== "boolean" ||
    !count(value.createdAtMs) ||
    !Array.isArray(value.conflicts) ||
    value.conflicts.length === 0 ||
    value.conflicts.length > 32
  )
    throw new Error("Invalid ConflictSetV1");
  const conflicts = value.conflicts.map((conflict) => {
    if (
      !record(conflict) ||
      !exact(conflict, [
        "conflictId",
        "kind",
        "confidence",
        "proposition",
        "existingValue",
        "candidateValue",
        "existingSourceId",
        "existingSourceHash",
        "candidateSourceIds",
        "relativePath",
        "headingPath",
        "visibleExplanation",
      ]) ||
      typeof conflict.conflictId !== "string" ||
      !ID.test(conflict.conflictId) ||
      ![
        "identity",
        "field_value",
        "relationship",
        "timeline",
        "world_rule",
        "plot_thread",
        "chapter_state",
        "source_hash",
        "semantic",
      ].includes(String(conflict.kind)) ||
      !["confirmed_conflict", "clarification_required"].includes(String(conflict.confidence)) ||
      typeof conflict.proposition !== "string" ||
      typeof conflict.existingValue !== "string" ||
      typeof conflict.candidateValue !== "string" ||
      typeof conflict.existingSourceId !== "string" ||
      !ID.test(conflict.existingSourceId) ||
      typeof conflict.existingSourceHash !== "string" ||
      !HASH.test(conflict.existingSourceHash) ||
      !strings(conflict.candidateSourceIds, 8) ||
      !conflict.candidateSourceIds.every((sourceId) => ID.test(sourceId)) ||
      typeof conflict.relativePath !== "string" ||
      conflict.relativePath.startsWith(".novelgainker/") ||
      !strings(conflict.headingPath, 16) ||
      typeof conflict.visibleExplanation !== "string"
    )
      throw new Error("Invalid ConflictItemV1");
    return conflict as ConflictSetV1["conflicts"][number];
  });
  return { ...(value as ConflictSetV1), conflicts };
}

function parseWritePlan(value: unknown): AgentWritePlanV2 {
  if (
    !record(value) ||
    !exact(value, [
      "schemaVersion",
      "pendingId",
      "projectId",
      "sessionId",
      "branchId",
      "turnId",
      "proposalId",
      "proposalHash",
      "sourceBindingHash",
      "conflictSetHash",
      "policy",
      "policyHash",
      "preview",
      "planHash",
    ]) ||
    value.schemaVersion !== 2 ||
    typeof value.pendingId !== "string" ||
    !ID.test(value.pendingId) ||
    typeof value.planHash !== "string" ||
    !HASH.test(value.planHash) ||
    typeof value.projectId !== "string" ||
    typeof value.sessionId !== "string" ||
    !ID.test(value.sessionId) ||
    typeof value.branchId !== "string" ||
    !ID.test(value.branchId) ||
    typeof value.turnId !== "string" ||
    !ID.test(value.turnId) ||
    typeof value.proposalId !== "string" ||
    !ID.test(value.proposalId) ||
    typeof value.proposalHash !== "string" ||
    !HASH.test(value.proposalHash) ||
    typeof value.sourceBindingHash !== "string" ||
    !HASH.test(value.sourceBindingHash) ||
    !(
      value.conflictSetHash === null ||
      (typeof value.conflictSetHash === "string" && HASH.test(value.conflictSetHash))
    ) ||
    !writePolicy(value.policy) ||
    typeof value.policyHash !== "string" ||
    !HASH.test(value.policyHash) ||
    !record(value.preview) ||
    !exact(value.preview, [
      "schemaVersion",
      "pendingId",
      "policy",
      "authorizationEffect",
      "requiresConfirmation",
      "canConfirm",
      "transactionHash",
      "actionPlanHash",
      "sourceSetHash",
      "policySnapshotHash",
      "highestRisk",
      "files",
      "projectWriteCount",
    ]) ||
    value.preview.schemaVersion !== 1 ||
    value.preview.pendingId !== value.pendingId ||
    value.preview.policy !== value.policy ||
    typeof value.preview.requiresConfirmation !== "boolean" ||
    typeof value.preview.canConfirm !== "boolean" ||
    !["execute", "await_confirmation", "block"].includes(
      String(value.preview.authorizationEffect),
    ) ||
    typeof value.preview.transactionHash !== "string" ||
    !HASH.test(value.preview.transactionHash) ||
    typeof value.preview.actionPlanHash !== "string" ||
    !HASH.test(value.preview.actionPlanHash) ||
    typeof value.preview.sourceSetHash !== "string" ||
    !HASH.test(value.preview.sourceSetHash) ||
    value.preview.policySnapshotHash !== value.policyHash ||
    !["read-safe", "write-safe", "always-confirm", "forbidden-v1"].includes(
      String(value.preview.highestRisk),
    ) ||
    !count(value.preview.projectWriteCount) ||
    !Array.isArray(value.preview.files) ||
    value.preview.files.length === 0 ||
    value.preview.files.length > 8
  )
    throw new Error("Invalid WritePlanV2");
  const files = value.preview.files.map((file) => {
    if (
      !record(file) ||
      !exact(file, [
        "callId",
        "toolId",
        "relativePath",
        "operation",
        "mutationShape",
        "action",
        "risk",
        "expectedSourceHash",
        "resultHash",
        "resultBytes",
        "fullContent",
        "lineDiff",
      ]) ||
      typeof file.callId !== "string" ||
      !file.callId.startsWith("call_") ||
      !["project.create_markdown", "project.patch_markdown"].includes(String(file.toolId)) ||
      typeof file.relativePath !== "string" ||
      file.relativePath.startsWith(".novelgainker/") ||
      (file.operation !== "create" && file.operation !== "replace") ||
      typeof file.mutationShape !== "string" ||
      typeof file.action !== "string" ||
      !["read-safe", "write-safe", "always-confirm", "forbidden-v1"].includes(String(file.risk)) ||
      !(
        file.expectedSourceHash === null ||
        (typeof file.expectedSourceHash === "string" && HASH.test(file.expectedSourceHash))
      ) ||
      typeof file.fullContent !== "string" ||
      typeof file.resultHash !== "string" ||
      !HASH.test(file.resultHash) ||
      !count(file.resultBytes) ||
      !Array.isArray(file.lineDiff)
    )
      throw new Error("Invalid AgentWriteFilePreviewV1");
    const lineDiff = file.lineDiff.map((line) => {
      if (
        !record(line) ||
        !exact(line, ["kind", "oldLine", "newLine", "content"]) ||
        !["context", "add", "remove"].includes(String(line.kind)) ||
        !(line.oldLine === null || count(line.oldLine)) ||
        !(line.newLine === null || count(line.newLine)) ||
        typeof line.content !== "string"
      )
        throw new Error("Invalid AgentDiffLineV1");
      return line as AgentWritePlanV2["preview"]["files"][number]["lineDiff"][number];
    });
    return { ...file, lineDiff } as AgentWritePlanV2["preview"]["files"][number];
  });
  return {
    ...(value as AgentWritePlanV2),
    preview: { ...(value.preview as AgentWritePlanV2["preview"]), files },
  };
}

function parseCommitReceipt(value: unknown): AgentCommitReceiptV2 {
  if (
    !record(value) ||
    !exact(value, [
      "schemaVersion",
      "receiptId",
      "transactionHash",
      "resultHash",
      "relativePaths",
      "committedAtMs",
      "undoExpiresAtMs",
      "projectWriteCount",
    ]) ||
    value.schemaVersion !== 2 ||
    typeof value.receiptId !== "string" ||
    !ID.test(value.receiptId) ||
    typeof value.transactionHash !== "string" ||
    !HASH.test(value.transactionHash) ||
    typeof value.resultHash !== "string" ||
    !HASH.test(value.resultHash) ||
    !strings(value.relativePaths, 8) ||
    !count(value.committedAtMs) ||
    !count(value.undoExpiresAtMs) ||
    !count(value.projectWriteCount)
  )
    throw new Error("Invalid AgentCommitReceiptV2");
  return value as AgentCommitReceiptV2;
}

function parseMemory(value: unknown): SessionMemoryV2 {
  if (
    !record(value) ||
    !exact(value, [
      "schemaVersion",
      "projectId",
      "sessionId",
      "branchId",
      "completedTurns",
      "visibleMessageCount",
      "visibleCharacterCount",
      "summary",
      "compactedThroughTurnId",
      "lastTurnId",
      "lastRecordHash",
      "needsCompaction",
      "memoryHash",
    ]) ||
    value.schemaVersion !== 2 ||
    typeof value.projectId !== "string" ||
    typeof value.sessionId !== "string" ||
    !ID.test(value.sessionId) ||
    typeof value.branchId !== "string" ||
    !ID.test(value.branchId) ||
    !count(value.completedTurns) ||
    !count(value.visibleMessageCount) ||
    !count(value.visibleCharacterCount) ||
    typeof value.summary !== "string" ||
    !nullableId(value.compactedThroughTurnId) ||
    !nullableId(value.lastTurnId) ||
    !(
      value.lastRecordHash === null ||
      (typeof value.lastRecordHash === "string" && HASH.test(value.lastRecordHash))
    ) ||
    typeof value.needsCompaction !== "boolean" ||
    typeof value.memoryHash !== "string" ||
    !HASH.test(value.memoryHash)
  )
    throw new Error("Invalid SessionMemoryV2");
  return value as SessionMemoryV2;
}

export function parseAgentSession(value: unknown): AgentSessionSnapshotV2 {
  if (
    !record(value) ||
    !exact(value, [
      "schemaVersion",
      "projectId",
      "historyConsentRequired",
      "branchSelectionRequired",
      "activeBranchId",
      "sessionId",
      "branches",
      "messages",
      "memory",
      "draftQuota",
      "writePolicy",
      "writePolicyHash",
      "contextMarks",
    ]) ||
    value.schemaVersion !== 2 ||
    typeof value.projectId !== "string" ||
    typeof value.historyConsentRequired !== "boolean" ||
    typeof value.branchSelectionRequired !== "boolean" ||
    !nullableId(value.activeBranchId) ||
    !nullableId(value.sessionId) ||
    value.draftQuota !== 0 ||
    !writePolicy(value.writePolicy) ||
    !(
      value.writePolicyHash === null ||
      (typeof value.writePolicyHash === "string" && HASH.test(value.writePolicyHash))
    ) ||
    !Array.isArray(value.branches) ||
    !Array.isArray(value.messages)
  )
    throw new Error("Invalid AgentSessionSnapshotV2");
  const branches = value.branches.map((item) => {
    if (
      !record(item) ||
      !exact(item, ["branchId", "sessionId", "lastTurnAtMs", "messageCount", "hashStatus"]) ||
      typeof item.branchId !== "string" ||
      !ID.test(item.branchId) ||
      typeof item.sessionId !== "string" ||
      !ID.test(item.sessionId) ||
      !(item.lastTurnAtMs === null || count(item.lastTurnAtMs)) ||
      !count(item.messageCount) ||
      !["valid", "empty", "invalid"].includes(String(item.hashStatus))
    )
      throw new Error("Invalid AgentHistoryBranchV2");
    return item as AgentHistoryBranchV2;
  });
  return {
    ...(value as Omit<AgentSessionSnapshotV2, "branches" | "messages" | "memory">),
    branches,
    messages: value.messages.map(parseMessage),
    memory: value.memory === null ? null : parseMemory(value.memory),
    contextMarks: parseMarkStore(value.contextMarks),
  };
}

export function parseAgentSkillRegistry(value: unknown): AgentSkillRegistryV2 {
  if (
    !record(value) ||
    !exact(value, [
      "schemaVersion",
      "packId",
      "packVersion",
      "packHash",
      "sourceCommit",
      "runtimeSource",
      "runtimeEnabled",
      "eligibleSkillIds",
      "skills",
    ]) ||
    value.schemaVersion !== 2 ||
    typeof value.packId !== "string" ||
    typeof value.packVersion !== "string" ||
    typeof value.packHash !== "string" ||
    !HASH.test(value.packHash) ||
    typeof value.sourceCommit !== "string" ||
    value.runtimeSource !== "bundled_release_only" ||
    value.runtimeEnabled !== true ||
    !strings(value.eligibleSkillIds, 9) ||
    value.eligibleSkillIds.length !== 9 ||
    !value.eligibleSkillIds.every((id) => SKILL_IDS.includes(id as (typeof SKILL_IDS)[number])) ||
    !Array.isArray(value.skills)
  )
    throw new Error("Invalid AgentSkillRegistryV2");
  const skills = value.skills.map((item) => {
    if (
      !record(item) ||
      !exact(item, [
        "id",
        "displayName",
        "description",
        "triggerSummary",
        "manifestSha256",
        "resources",
      ]) ||
      typeof item.id !== "string" ||
      !SKILL_IDS.includes(item.id as (typeof SKILL_IDS)[number]) ||
      typeof item.displayName !== "string" ||
      typeof item.description !== "string" ||
      typeof item.triggerSummary !== "string" ||
      typeof item.manifestSha256 !== "string" ||
      !HASH.test(item.manifestSha256) ||
      !Array.isArray(item.resources)
    )
      throw new Error("Invalid AgentSkillV2");
    const resources = item.resources.map((resource) => {
      if (
        !record(resource) ||
        !exact(resource, ["path", "kind", "sha256"]) ||
        typeof resource.path !== "string" ||
        !["instruction", "template", "reference"].includes(String(resource.kind)) ||
        typeof resource.sha256 !== "string" ||
        !HASH.test(resource.sha256)
      )
        throw new Error("Invalid AgentSkillResourceV2");
      return resource as AgentSkillResourceV2;
    });
    return { ...(item as Omit<AgentSkillV2, "resources">), resources };
  });
  if (skills.length !== 9) throw new Error("Invalid AgentSkillRegistryV2");
  return { ...(value as Omit<AgentSkillRegistryV2, "skills">), skills };
}

function parseUsage(value: unknown): TurnBudgetUsageV2 {
  const keys = [
    "providerCalls",
    "totalProviderInputBytes",
    "totalProviderOutputBytes",
    "accumulatedContextResultBytes",
    "structuredRepairCalls",
    "totalProviderInputTokens",
    "totalProviderOutputTokens",
  ];
  if (!record(value) || !exact(value, keys) || keys.some((key) => !count(value[key])))
    throw new Error("Invalid TurnBudgetUsageV2");
  return value as TurnBudgetUsageV2;
}

export function parseAgentTurnEvent(value: unknown): AgentTurnEventV2 {
  const base = [
    "schemaVersion",
    "runId",
    "turnId",
    "projectSessionId",
    "providerId",
    "model",
    "sequence",
    "kind",
  ];
  if (
    !record(value) ||
    value.schemaVersion !== 2 ||
    typeof value.runId !== "string" ||
    !ID.test(value.runId) ||
    typeof value.turnId !== "string" ||
    !ID.test(value.turnId) ||
    typeof value.projectSessionId !== "string" ||
    !provider(value.providerId) ||
    typeof value.model !== "string" ||
    !count(value.sequence) ||
    value.sequence === 0
  )
    throw new Error("Invalid AgentTurnEventV2");
  if (
    value.kind === "turnState" &&
    exact(value, [...base, "state", "visibleReason"]) &&
    typeof value.state === "string" &&
    (value.visibleReason === null || typeof value.visibleReason === "string")
  )
    return value as AgentTurnEventV2;
  if (
    value.kind === "skillSelection" &&
    exact(value, [...base, "selectedSkillIds", "intentClass", "visibleReason"]) &&
    strings(value.selectedSkillIds, 4) &&
    typeof value.intentClass === "string" &&
    typeof value.visibleReason === "string"
  )
    return value as AgentTurnEventV2;
  if (
    value.kind === "contextSources" &&
    exact(value, [...base, "sources", "indexHash"]) &&
    Array.isArray(value.sources) &&
    typeof value.indexHash === "string" &&
    HASH.test(value.indexHash)
  )
    return {
      ...(value as AgentTurnEventBaseV2 & { kind: "contextSources"; indexHash: string }),
      sources: value.sources.map(parseSource),
    };
  if (value.kind === "anchorCandidate" && exact(value, [...base, "resolution"]))
    return {
      ...(value as AgentTurnEventBaseV2 & { kind: "anchorCandidate" }),
      resolution: parseAnchorResolution(value.resolution),
    };
  if (value.kind === "canonCandidate" && exact(value, [...base, "artifact", "proposal"]))
    return {
      ...(value as AgentTurnEventBaseV2 & { kind: "canonCandidate" }),
      ...parseCanonCandidate(value.artifact, value.proposal),
    };
  if (value.kind === "contextMarks" && exact(value, [...base, "store"]))
    return {
      ...(value as AgentTurnEventBaseV2 & { kind: "contextMarks" }),
      store: parseMarkStore(value.store),
    };
  if (value.kind === "conflict" && exact(value, [...base, "conflictSet"]))
    return {
      ...(value as AgentTurnEventBaseV2 & { kind: "conflict" }),
      conflictSet: parseConflictSet(value.conflictSet),
    };
  if (value.kind === "writePlan" && exact(value, [...base, "plan"]))
    return {
      ...(value as AgentTurnEventBaseV2 & { kind: "writePlan" }),
      plan: parseWritePlan(value.plan),
    };
  if (value.kind === "commitReceipt" && exact(value, [...base, "receipt"]))
    return {
      ...(value as AgentTurnEventBaseV2 & { kind: "commitReceipt" }),
      receipt: parseCommitReceipt(value.receipt),
    };
  if (
    value.kind === "conversationDelta" &&
    exact(value, [...base, "delta"]) &&
    typeof value.delta === "string" &&
    value.delta.length > 0
  )
    return value as AgentTurnEventV2;
  if (
    (value.kind === "completed" || value.kind === "failed" || value.kind === "cancelled") &&
    count(value.agentCanonWriteCount)
  ) {
    const usage = parseUsage(value.usage);
    const message = value.message === null ? null : parseMessage(value.message);
    if (
      value.kind === "completed" &&
      exact(value, [...base, "message", "sources", "usage", "agentCanonWriteCount"]) &&
      message &&
      Array.isArray(value.sources)
    )
      return {
        ...(value as AgentTurnEventBaseV2 & { kind: "completed"; agentCanonWriteCount: number }),
        message,
        sources: value.sources.map(parseSource),
        usage,
      };
    if (
      value.kind === "failed" &&
      exact(value, [...base, "message", "error", "usage", "agentCanonWriteCount"])
    ) {
      const error = parseAppError(value.error);
      if (error)
        return {
          ...(value as AgentTurnEventBaseV2 & { kind: "failed"; agentCanonWriteCount: number }),
          message,
          error,
          usage,
        };
    }
    if (
      value.kind === "cancelled" &&
      exact(value, [...base, "message", "usage", "agentCanonWriteCount"])
    )
      return {
        ...(value as AgentTurnEventBaseV2 & { kind: "cancelled"; agentCanonWriteCount: number }),
        message,
        usage,
      };
  }
  throw new Error("Invalid AgentTurnEventV2");
}

function parseStarted(value: unknown): AgentTurnStartedV2 {
  if (
    !record(value) ||
    !exact(value, ["schemaVersion", "runId", "turnId", "providerId", "model"]) ||
    value.schemaVersion !== 2 ||
    typeof value.runId !== "string" ||
    !ID.test(value.runId) ||
    typeof value.turnId !== "string" ||
    !ID.test(value.turnId) ||
    !provider(value.providerId) ||
    typeof value.model !== "string"
  )
    throw new Error("Invalid AgentTurnStartedV2");
  return value as AgentTurnStartedV2;
}

const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);
const listenTauri: ListenEvent = (channel, handler) => listen<unknown>(channel, handler);

async function invokeAgent<T>(
  command: string,
  args: Record<string, unknown>,
  parser: (value: unknown) => T,
  invokeCommand: InvokeCommand,
): Promise<T> {
  try {
    return parser(await invokeCommand(command, args));
  } catch (error: unknown) {
    if (parseAppError(error)) throw normalizeAppError(error);
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
  historyConsent = false,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_SESSION_LOAD_COMMAND,
    { projectSessionId, historyConsent },
    parseAgentSession,
    invokeCommand,
  );
}

export function requestAgentHistoryBranchResolve(
  projectSessionId: string,
  branchId: string,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_HISTORY_BRANCH_RESOLVE_COMMAND,
    { projectSessionId, branchId },
    parseAgentSession,
    invokeCommand,
  );
}

export function requestAgentTurnStart(
  projectSessionId: string,
  expectedProviderId: ProviderId,
  userText: string,
  anchorReference?: AnchorReferenceV1,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_TURN_START_COMMAND,
    {
      input: {
        schemaVersion: 2,
        projectSessionId,
        expectedProviderId,
        userText,
        ...(anchorReference ? { anchorReference } : {}),
      },
    },
    parseStarted,
    invokeCommand,
  );
}

export function requestAgentTurnCancel(runId: string, invokeCommand: InvokeCommand = invokeTauri) {
  return invokeAgent(
    AGENT_TURN_CANCEL_COMMAND,
    { runId },
    (value) => {
      if (!record(value) || !exact(value, ["cancelRequested"]) || value.cancelRequested !== true)
        throw new Error("Invalid cancel result");
      return { cancelRequested: true as const };
    },
    invokeCommand,
  );
}

export function requestAgentSessionClear(
  projectSessionId: string,
  scope: "session" | "all_history",
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_SESSION_CLEAR_COMMAND,
    { projectSessionId, scope },
    (value) => {
      if (
        !record(value) ||
        !exact(value, ["scope", "clearedBranchCount", "clearedMessageCount", "snapshot"]) ||
        (value.scope !== "session" && value.scope !== "all_history") ||
        !count(value.clearedBranchCount) ||
        !count(value.clearedMessageCount)
      )
        throw new Error("Invalid clear result");
      return { ...value, snapshot: parseAgentSession(value.snapshot) } as AgentHistoryClearResultV2;
    },
    invokeCommand,
  );
}

export function requestAgentMemoryCompact(
  projectSessionId: string,
  sessionId: string,
  memoryHash: string,
  expectedProviderId: ProviderId,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_MEMORY_COMPACT_COMMAND,
    { input: { schemaVersion: 2, projectSessionId, sessionId, memoryHash, expectedProviderId } },
    parseMemory,
    invokeCommand,
  );
}

export function requestAgentWritePolicySet(
  projectSessionId: string,
  policy: AgentWritePolicySnapshotV2["policy"],
  expectedPolicyHash: string,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_WRITE_POLICY_SET_COMMAND,
    {
      input: {
        schemaVersion: 2,
        projectSessionId,
        policy,
        expectedPolicyHash,
      },
    },
    (value) => {
      if (
        !record(value) ||
        !exact(value, ["policy", "policyHash"]) ||
        !writePolicy(value.policy) ||
        typeof value.policyHash !== "string" ||
        !HASH.test(value.policyHash)
      )
        throw new Error("Invalid AgentWritePolicySnapshotV2");
      return value as AgentWritePolicySnapshotV2;
    },
    invokeCommand,
  );
}

export function requestAgentContextMarkUpsert(
  projectSessionId: string,
  sourceId: string,
  sourceBindingHash: string,
  scope: "session" | "project",
  priority: "medium" | "high",
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_CONTEXT_MARK_UPSERT_COMMAND,
    {
      input: {
        schemaVersion: 2,
        projectSessionId,
        sourceId,
        sourceBindingHash,
        scope,
        priority,
      },
    },
    parseMarkStore,
    invokeCommand,
  );
}

export function requestAgentContextMarkRemove(
  projectSessionId: string,
  markId: string,
  bindingHash: string,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_CONTEXT_MARK_REMOVE_COMMAND,
    { input: { schemaVersion: 2, projectSessionId, markId, bindingHash } },
    parseMarkStore,
    invokeCommand,
  );
}

export function requestAgentConflictResolution(
  projectSessionId: string,
  conflictSetId: string,
  conflictSetHash: string,
  resolution: ConflictResolutionChoiceV1,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_CONFLICT_RESOLUTION_SUBMIT_COMMAND,
    {
      input: {
        schemaVersion: 2,
        projectSessionId,
        conflictSetId,
        conflictSetHash,
        resolution,
        confirmed: true,
      },
    },
    (value) => {
      if (
        !record(value) ||
        !exact(value, ["resolution", "writePlan"]) ||
        !record(value.resolution) ||
        !exact(value.resolution, [
          "conflictSetId",
          "conflictSetHash",
          "choice",
          "outcome",
          "requiresWriteConfirmation",
          "consumed",
        ]) ||
        typeof value.resolution.conflictSetId !== "string" ||
        !ID.test(value.resolution.conflictSetId) ||
        typeof value.resolution.conflictSetHash !== "string" ||
        !HASH.test(value.resolution.conflictSetHash) ||
        ![
          "keep_existing",
          "adopt_candidate",
          "merge_manual",
          "save_as_alternative",
          "cancel_change",
        ].includes(String(value.resolution.choice)) ||
        ![
          "candidate_discarded",
          "candidate_requires_confirmation",
          "manual_conversation_required",
          "alternative_requires_renormalization",
          "change_cancelled",
        ].includes(String(value.resolution.outcome)) ||
        typeof value.resolution.requiresWriteConfirmation !== "boolean" ||
        value.resolution.consumed !== true
      )
        throw new Error("Invalid AgentConflictResolutionResponseV2");
      return {
        resolution: value.resolution,
        writePlan: value.writePlan === null ? null : parseWritePlan(value.writePlan),
      } as AgentConflictResolutionResponseV2;
    },
    invokeCommand,
  );
}

export function requestAgentWriteConfirmation(
  projectSessionId: string,
  plan: AgentWritePlanV2,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_WRITE_CONFIRMATION_SUBMIT_COMMAND,
    {
      input: {
        schemaVersion: 2,
        projectSessionId,
        pendingId: plan.pendingId,
        planHash: plan.planHash,
        sourceBindingHash: plan.sourceBindingHash,
        policyHash: plan.policyHash,
        confirmed: true,
      },
    },
    parseCommitReceipt,
    invokeCommand,
  );
}

export function requestAgentWritePlanReject(
  projectSessionId: string,
  pendingId: string,
  planHash: string,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_WRITE_PLAN_REJECT_COMMAND,
    { input: { schemaVersion: 2, projectSessionId, pendingId, planHash } },
    (value) => {
      if (
        !record(value) ||
        !exact(value, ["rejected", "projectWriteCount"]) ||
        value.rejected !== true ||
        value.projectWriteCount !== 0
      )
        throw new Error("Invalid AgentWritePlanRejectResultV2");
      return { rejected: true as const, projectWriteCount: 0 as const };
    },
    invokeCommand,
  );
}

export function requestAgentWriteUndo(
  projectSessionId: string,
  receiptId: string,
  resultHash: string,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  return invokeAgent(
    AGENT_WRITE_UNDO_APPLY_COMMAND,
    {
      input: {
        schemaVersion: 2,
        projectSessionId,
        receiptId,
        resultHash,
        confirmed: true,
      },
    },
    (value) => {
      if (
        !record(value) ||
        !exact(value, ["transactionHash", "restoredFileCount", "projectWriteCount"]) ||
        typeof value.transactionHash !== "string" ||
        !HASH.test(value.transactionHash) ||
        !count(value.restoredFileCount) ||
        !count(value.projectWriteCount)
      )
        throw new Error("Invalid AgentUndoApplyResultV1");
      return value as AgentUndoApplyResultV1;
    },
    invokeCommand,
  );
}

export async function listenAgentTurnEvents(
  onEvent: (event: AgentTurnEventV2) => void,
  listenEvent: ListenEvent = listenTauri,
): Promise<UnlistenFn> {
  return listenEvent(AGENT_TURN_EVENT_CHANNEL, ({ payload }) => {
    try {
      onEvent(parseAgentTurnEvent(payload));
    } catch {
      /* malformed events are rejected */
    }
  });
}
