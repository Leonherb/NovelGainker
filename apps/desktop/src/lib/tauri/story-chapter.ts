import { invoke } from "@tauri-apps/api/core";
import { normalizeAppError, parseAppError, type AppError } from "./health";
import type { ProviderId } from "./provider";
import type {
  StoryAction,
  StoryContextRequestPreviewInput,
  StoryReaderMentalModel,
} from "./story-context";

export const STORY_CHAPTER_GENERATE_START_COMMAND = "story_chapter_generate_start";
export const STORY_CHAPTER_STATUS_LOAD_COMMAND = "story_chapter_status_load";
export const STORY_CHAPTER_GENERATE_CANCEL_COMMAND = "story_chapter_generate_cancel";
export const STORY_CHAPTER_RUN_SCHEMA_VERSION = 1;

export type StoryChapterRunState =
  "generating" | "repairing" | "completed" | "failed" | "cancelled";

export type ChapterContinuityClaim = {
  claim: string;
  sourceIds: string[];
};

export type ChapterLedgerCandidate = {
  summary: string;
  sourceIds: string[];
};

export type ChapterProposal = {
  proposalVersion: 1;
  proposalHash: string;
  action: StoryAction;
  targetChapter: {
    chapterNumber: number;
    chapterTitle: string;
    relativePath: string;
  };
  title: string;
  draftMarkdown: string;
  intentSummary: string;
  readerModelDelta: StoryReaderMentalModel;
  continuityClaims: ChapterContinuityClaim[];
  ledgerCandidates: ChapterLedgerCandidate[];
  sourceManifestHash: string;
  currentManifestHash: string;
  requestHash: string;
  providerId: ProviderId;
  modelId: string;
  startedAt: number;
  generatedAt: number;
  finishReason: "stop";
  responseComplete: true;
  sourceStatus: "current" | "stale";
  continuityCheckVersion: null;
  writeEnabled: false;
};

export type StoryChapterRun = {
  schemaVersion: typeof STORY_CHAPTER_RUN_SCHEMA_VERSION;
  runId: string;
  state: StoryChapterRunState;
  providerId: ProviderId;
  modelId: string;
  sourceManifestHash: string;
  requestHash: string;
  startedAt: number;
  completedAt: number | null;
  repairAttempted: boolean;
  canCancel: boolean;
  canRetry: boolean;
  proposal: ChapterProposal | null;
  invalidResponsePreview: string | null;
  error: AppError | null;
};

export type StoryChapterStatusSnapshot = {
  schemaVersion: typeof STORY_CHAPTER_RUN_SCHEMA_VERSION;
  run: StoryChapterRun | null;
};

export type StoryChapterGenerateInput = StoryContextRequestPreviewInput & {
  expectedRequestHash: string;
  expectedProviderId?: ProviderId;
};

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const SHA256 = /^[0-9a-f]{64}$/u;
const CONTRACT_TOKEN = /^[a-zA-Z0-9._-]{1,160}$/u;
const RUN_STATES = new Set<StoryChapterRunState>([
  "generating",
  "repairing",
  "completed",
  "failed",
  "cancelled",
]);
const RUN_KEYS = [
  "canCancel",
  "canRetry",
  "completedAt",
  "error",
  "invalidResponsePreview",
  "modelId",
  "proposal",
  "providerId",
  "repairAttempted",
  "requestHash",
  "runId",
  "schemaVersion",
  "sourceManifestHash",
  "startedAt",
  "state",
];
const PROPOSAL_KEYS = [
  "action",
  "continuityCheckVersion",
  "continuityClaims",
  "currentManifestHash",
  "draftMarkdown",
  "finishReason",
  "generatedAt",
  "intentSummary",
  "ledgerCandidates",
  "modelId",
  "proposalHash",
  "proposalVersion",
  "providerId",
  "readerModelDelta",
  "requestHash",
  "responseComplete",
  "sourceManifestHash",
  "sourceStatus",
  "startedAt",
  "targetChapter",
  "title",
  "writeEnabled",
];
const READER_MODEL_KEYS = [
  "chapterPromises",
  "forbiddenReveals",
  "readerInfers",
  "readerKnows",
  "readerMisreads",
  "readerQuestions",
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, expected: string[]): boolean {
  const actual = Object.keys(value).sort();
  const sortedExpected = [...expected].sort();
  return (
    actual.length === sortedExpected.length &&
    actual.every((key, index) => key === sortedExpected[index])
  );
}

function isSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0;
}

function isProviderId(value: unknown): value is ProviderId {
  return value === "deepseek" || value === "openai";
}

function providerModelMatches(providerId: ProviderId, modelId: unknown): modelId is string {
  return (
    (providerId === "deepseek" && modelId === "deepseek-v4-pro") ||
    (providerId === "openai" && modelId === "gpt-5.6-terra")
  );
}

function isSafeMultilineText(
  value: unknown,
  maximumCharacters: number,
  allowEmpty: boolean,
): value is string {
  if (
    typeof value !== "string" ||
    (!allowEmpty && value.length === 0) ||
    (value.length > 0 && value !== value.trim()) ||
    [...value].length > maximumCharacters
  ) {
    return false;
  }
  return ![...value].some((character) => {
    const code = character.codePointAt(0) ?? 0;
    return (
      ((code <= 0x1f || code === 0x7f) &&
        character !== "\n" &&
        character !== "\r" &&
        character !== "\t") ||
      code === 0x061c ||
      (code >= 0x200b && code <= 0x200f) ||
      (code >= 0x202a && code <= 0x202e) ||
      (code >= 0x2060 && code <= 0x2069) ||
      code === 0xfeff ||
      code === 0xfffd
    );
  });
}

function parseReaderModel(value: unknown): StoryReaderMentalModel {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, READER_MODEL_KEYS) ||
    !isSafeMultilineText(value.readerKnows, 4_096, true) ||
    !isSafeMultilineText(value.readerInfers, 4_096, true) ||
    !isSafeMultilineText(value.readerMisreads, 4_096, true) ||
    !isSafeMultilineText(value.readerQuestions, 4_096, true) ||
    !isSafeMultilineText(value.chapterPromises, 4_096, true) ||
    !isSafeMultilineText(value.forbiddenReveals, 4_096, true)
  ) {
    throw new Error("Invalid Chapter Proposal Reader Model delta");
  }
  return value as StoryReaderMentalModel;
}

function parseSourceIds(value: unknown): string[] {
  if (
    !Array.isArray(value) ||
    value.length > 16 ||
    value.some(
      (sourceId) =>
        typeof sourceId !== "string" ||
        sourceId.length === 0 ||
        sourceId.length > 512 ||
        sourceId !== sourceId.trim(),
    ) ||
    new Set(value).size !== value.length
  ) {
    throw new Error("Invalid Chapter Proposal source IDs");
  }
  return value as string[];
}

function parseContinuityClaims(value: unknown): ChapterContinuityClaim[] {
  if (!Array.isArray(value) || value.length > 64) {
    throw new Error("Invalid Chapter Proposal continuity claims");
  }
  return value.map((claim) => {
    if (
      !isRecord(claim) ||
      !hasExactKeys(claim, ["claim", "sourceIds"]) ||
      !isSafeMultilineText(claim.claim, 1_000, false)
    ) {
      throw new Error("Invalid Chapter Proposal continuity claim");
    }
    return { claim: claim.claim, sourceIds: parseSourceIds(claim.sourceIds) };
  });
}

function parseLedgerCandidates(value: unknown): ChapterLedgerCandidate[] {
  if (!Array.isArray(value) || value.length > 64) {
    throw new Error("Invalid Chapter Proposal ledger candidates");
  }
  return value.map((candidate) => {
    if (
      !isRecord(candidate) ||
      !hasExactKeys(candidate, ["sourceIds", "summary"]) ||
      !isSafeMultilineText(candidate.summary, 1_000, false)
    ) {
      throw new Error("Invalid Chapter Proposal ledger candidate");
    }
    return { summary: candidate.summary, sourceIds: parseSourceIds(candidate.sourceIds) };
  });
}

export function parseChapterProposal(value: unknown): ChapterProposal {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, PROPOSAL_KEYS) ||
    value.proposalVersion !== 1 ||
    typeof value.proposalHash !== "string" ||
    !SHA256.test(value.proposalHash) ||
    (value.action !== "create" && value.action !== "revise") ||
    !isSafeMultilineText(value.title, 120, false) ||
    !isSafeMultilineText(value.draftMarkdown, 65_536, false) ||
    new TextEncoder().encode(value.draftMarkdown).length > 64 * 1024 ||
    value.draftMarkdown.startsWith("---") ||
    value.draftMarkdown.includes("```") ||
    !isSafeMultilineText(value.intentSummary, 4_096, false) ||
    typeof value.sourceManifestHash !== "string" ||
    !SHA256.test(value.sourceManifestHash) ||
    typeof value.currentManifestHash !== "string" ||
    !SHA256.test(value.currentManifestHash) ||
    typeof value.requestHash !== "string" ||
    !SHA256.test(value.requestHash) ||
    !isProviderId(value.providerId) ||
    !providerModelMatches(value.providerId, value.modelId) ||
    !isSafeInteger(value.startedAt) ||
    !isSafeInteger(value.generatedAt) ||
    value.generatedAt < value.startedAt ||
    value.finishReason !== "stop" ||
    value.responseComplete !== true ||
    (value.sourceStatus !== "current" && value.sourceStatus !== "stale") ||
    (value.sourceStatus === "current" && value.currentManifestHash !== value.sourceManifestHash) ||
    (value.sourceStatus === "stale" && value.currentManifestHash === value.sourceManifestHash) ||
    value.continuityCheckVersion !== null ||
    value.writeEnabled !== false
  ) {
    throw new Error("Invalid Chapter Proposal");
  }
  if (
    !isRecord(value.targetChapter) ||
    !hasExactKeys(value.targetChapter, ["chapterNumber", "chapterTitle", "relativePath"]) ||
    !Number.isSafeInteger(value.targetChapter.chapterNumber) ||
    Number(value.targetChapter.chapterNumber) < 1 ||
    Number(value.targetChapter.chapterNumber) > 999_999 ||
    !isSafeMultilineText(value.targetChapter.chapterTitle, 120, false) ||
    value.targetChapter.relativePath !==
      `chapters/${String(value.targetChapter.chapterNumber).padStart(3, "0")}.md`
  ) {
    throw new Error("Invalid Chapter Proposal target");
  }
  return {
    ...value,
    targetChapter: value.targetChapter,
    readerModelDelta: parseReaderModel(value.readerModelDelta),
    continuityClaims: parseContinuityClaims(value.continuityClaims),
    ledgerCandidates: parseLedgerCandidates(value.ledgerCandidates),
  } as ChapterProposal;
}

export function parseStoryChapterRun(value: unknown): StoryChapterRun {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, RUN_KEYS) ||
    value.schemaVersion !== STORY_CHAPTER_RUN_SCHEMA_VERSION ||
    typeof value.runId !== "string" ||
    !CONTRACT_TOKEN.test(value.runId) ||
    !RUN_STATES.has(value.state as StoryChapterRunState) ||
    !isProviderId(value.providerId) ||
    !providerModelMatches(value.providerId, value.modelId) ||
    typeof value.sourceManifestHash !== "string" ||
    !SHA256.test(value.sourceManifestHash) ||
    typeof value.requestHash !== "string" ||
    !SHA256.test(value.requestHash) ||
    !isSafeInteger(value.startedAt) ||
    (value.completedAt !== null && !isSafeInteger(value.completedAt)) ||
    typeof value.repairAttempted !== "boolean" ||
    typeof value.canCancel !== "boolean" ||
    typeof value.canRetry !== "boolean"
  ) {
    throw new Error("Invalid Story Chapter run");
  }
  const proposal = value.proposal === null ? null : parseChapterProposal(value.proposal);
  const error = value.error === null ? null : parseAppError(value.error);
  if (value.error !== null && error === null) {
    throw new Error("Invalid Story Chapter error");
  }
  if (
    (value.invalidResponsePreview !== null &&
      (typeof value.invalidResponsePreview !== "string" ||
        value.invalidResponsePreview.length === 0 ||
        new TextEncoder().encode(value.invalidResponsePreview).length > 4 * 1024)) ||
    ((value.state === "generating" || value.state === "repairing") &&
      (!value.canCancel || value.canRetry || value.completedAt !== null || proposal !== null)) ||
    (value.state === "completed" &&
      (value.canCancel ||
        value.canRetry ||
        value.completedAt === null ||
        proposal === null ||
        error !== null)) ||
    (value.state === "failed" &&
      (value.canCancel ||
        !value.canRetry ||
        value.completedAt === null ||
        proposal !== null ||
        error === null)) ||
    (value.state === "cancelled" &&
      (value.canCancel ||
        !value.canRetry ||
        value.completedAt === null ||
        proposal !== null ||
        error !== null)) ||
    (proposal !== null &&
      (proposal.providerId !== value.providerId ||
        proposal.modelId !== value.modelId ||
        proposal.sourceManifestHash !== value.sourceManifestHash ||
        proposal.requestHash !== value.requestHash ||
        proposal.startedAt !== value.startedAt))
  ) {
    throw new Error("Invalid Story Chapter run state");
  }
  return { ...value, proposal, error } as StoryChapterRun;
}

export function parseStoryChapterStatus(value: unknown): StoryChapterStatusSnapshot {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["run", "schemaVersion"]) ||
    value.schemaVersion !== STORY_CHAPTER_RUN_SCHEMA_VERSION
  ) {
    throw new Error("Invalid Story Chapter status");
  }
  return {
    schemaVersion: STORY_CHAPTER_RUN_SCHEMA_VERSION,
    run: value.run === null ? null : parseStoryChapterRun(value.run),
  };
}

function validateGenerateInput(input: StoryChapterGenerateInput): void {
  if (!SHA256.test(input.reviewedManifestHash) || !SHA256.test(input.expectedRequestHash)) {
    throw {
      code: "workflow.input_invalid",
      messageKey: "errors.workflowInputInvalid",
    } satisfies AppError;
  }
}

function requestPayload(input: StoryChapterGenerateInput): Record<string, unknown> {
  return {
    schemaVersion: STORY_CHAPTER_RUN_SCHEMA_VERSION,
    request: {
      schemaVersion: 1,
      context: {
        schemaVersion: 2,
        action: input.action,
        chapterNumber: input.chapterNumber,
        chapterTitle: input.chapterTitle.trim(),
        selectedSourcePaths: [...input.selectedSourcePaths].sort(),
      },
      reviewedManifestHash: input.reviewedManifestHash,
      readerMentalModel: { ...input.readerMentalModel },
    },
    expectedRequestHash: input.expectedRequestHash,
    expectedProviderId: input.expectedProviderId ?? null,
    confirmed: true,
  };
}

const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);

export async function requestStoryChapterGenerateStart(
  sessionId: string,
  input: StoryChapterGenerateInput,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<StoryChapterRun> {
  validateGenerateInput(input);
  try {
    const run = parseStoryChapterRun(
      await invokeCommand(STORY_CHAPTER_GENERATE_START_COMMAND, {
        sessionId,
        input: requestPayload(input),
      }),
    );
    if (
      run.requestHash !== input.expectedRequestHash ||
      run.sourceManifestHash !== input.reviewedManifestHash ||
      (input.expectedProviderId !== undefined && run.providerId !== input.expectedProviderId)
    ) {
      throw new Error("Story Chapter run binding mismatch");
    }
    return run;
  } catch (error: unknown) {
    if (error instanceof Error) {
      throw {
        code: "contract.invalid_response",
        messageKey: "errors.invalidRuntimeResponse",
      } satisfies AppError;
    }
    throw normalizeAppError(error);
  }
}

export async function requestStoryChapterStatus(
  sessionId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<StoryChapterStatusSnapshot> {
  try {
    return parseStoryChapterStatus(
      await invokeCommand(STORY_CHAPTER_STATUS_LOAD_COMMAND, { sessionId }),
    );
  } catch (error: unknown) {
    if (error instanceof Error) {
      throw {
        code: "contract.invalid_response",
        messageKey: "errors.invalidRuntimeResponse",
      } satisfies AppError;
    }
    throw normalizeAppError(error);
  }
}

export async function requestStoryChapterCancel(
  sessionId: string,
  runId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<StoryChapterRun> {
  try {
    return parseStoryChapterRun(
      await invokeCommand(STORY_CHAPTER_GENERATE_CANCEL_COMMAND, { sessionId, runId }),
    );
  } catch (error: unknown) {
    if (error instanceof Error) {
      throw {
        code: "contract.invalid_response",
        messageKey: "errors.invalidRuntimeResponse",
      } satisfies AppError;
    }
    throw normalizeAppError(error);
  }
}
