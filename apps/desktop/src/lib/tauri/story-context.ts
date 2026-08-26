import { invoke } from "@tauri-apps/api/core";
import { normalizeAppError, type AppError } from "./health";
import { STORYTELLING_WORKFLOW_ID } from "./workflow";

export const STORY_CONTEXT_CATALOG_COMMAND = "story_context_catalog";
export const STORY_CONTEXT_BUILD_COMMAND = "story_context_build";
export const STORY_CONTEXT_SCHEMA_VERSION = 2;

export type StoryAction = "create" | "revise";
export type StoryContextSourceKind =
  | "builtinProtocol"
  | "builtinSchema"
  | "builtinTemplate"
  | "projectRules"
  | "plot"
  | "outline"
  | "style"
  | "ledger"
  | "chapterDraft"
  | "character"
  | "world"
  | "material";
export type StoryContextSelectionReason =
  | "required"
  | "explicit"
  | "defaultPlot"
  | "defaultOutline"
  | "defaultStyle"
  | "defaultLedger"
  | "recentChapter"
  | "available";

export type StoryContextCatalogEntry = {
  sourceId: string;
  kind: StoryContextSourceKind;
  relativePath: string | null;
  builtinResourceId: string | null;
  displayName: string;
  selectionReason: StoryContextSelectionReason;
  required: boolean;
  userDeselectable: boolean;
  defaultSelected: boolean;
  priority: number;
  byteLength: number;
  contentHash: string;
  modifiedTimeMs: number;
};

export type StoryContextCatalogSnapshot = {
  schemaVersion: typeof STORY_CONTEXT_SCHEMA_VERSION;
  workflowId: typeof STORYTELLING_WORKFLOW_ID;
  suggestedChapterNumber: number;
  sources: StoryContextCatalogEntry[];
};

export type StoryContextTruncationReason =
  "singleSourceLimit" | "protocolBudget" | "contextBudget" | "notSelected";

export type StoryContextManifestSource = Omit<
  StoryContextCatalogEntry,
  "byteLength" | "defaultSelected"
> & {
  sourceByteCount: number;
  originalByteLength: number;
  includedByteLength: number;
  includedContentHash: string;
  truncation: {
    reason: StoryContextTruncationReason;
    originalByteLength: number;
    retainedByteLength: number;
  } | null;
  included: boolean;
};

export type StoryContextManifest = {
  manifestVersion: typeof STORY_CONTEXT_SCHEMA_VERSION;
  workflowId: typeof STORYTELLING_WORKFLOW_ID;
  workflowVersion: 1;
  action: StoryAction;
  projectRootIdentity: string;
  targetChapter: {
    chapterNumber: number;
    chapterTitle: string;
    relativePath: string;
  };
  selectedSources: StoryContextManifestSource[];
  excludedSources: StoryContextManifestSource[];
  sourceOrder: string[];
  budget: {
    requestLimitBytes: number;
    protocolBudgetBytes: number;
    projectContextBudgetBytes: number;
    safetyReserveBytes: number;
    singleSourceLimitBytes: number;
    protocolBytesUsed: number;
    projectContextBytesUsed: number;
    totalIncludedBytes: number;
  };
  aggregateHash: string;
  createdAt: number;
};

export type StoryContextBuildInput = {
  action: StoryAction;
  chapterNumber: number;
  chapterTitle: string;
  selectedSourcePaths: string[];
};

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const SHA256 = /^[0-9a-f]{64}$/;
const SOURCE_KINDS = new Set<StoryContextSourceKind>([
  "builtinProtocol",
  "builtinSchema",
  "builtinTemplate",
  "projectRules",
  "plot",
  "outline",
  "style",
  "ledger",
  "chapterDraft",
  "character",
  "world",
  "material",
]);
const SELECTION_REASONS = new Set<StoryContextSelectionReason>([
  "required",
  "explicit",
  "defaultPlot",
  "defaultOutline",
  "defaultStyle",
  "defaultLedger",
  "recentChapter",
  "available",
]);
const TRUNCATION_REASONS = new Set<StoryContextTruncationReason>([
  "singleSourceLimit",
  "protocolBudget",
  "contextBudget",
  "notSelected",
]);
const CATALOG_KEYS = ["schemaVersion", "sources", "suggestedChapterNumber", "workflowId"];
const CATALOG_SOURCE_KEYS = [
  "builtinResourceId",
  "byteLength",
  "contentHash",
  "defaultSelected",
  "displayName",
  "kind",
  "modifiedTimeMs",
  "priority",
  "relativePath",
  "required",
  "selectionReason",
  "sourceId",
  "userDeselectable",
];
const MANIFEST_KEYS = [
  "action",
  "aggregateHash",
  "budget",
  "createdAt",
  "excludedSources",
  "manifestVersion",
  "projectRootIdentity",
  "selectedSources",
  "sourceOrder",
  "targetChapter",
  "workflowId",
  "workflowVersion",
];
const MANIFEST_SOURCE_KEYS = [
  "builtinResourceId",
  "contentHash",
  "displayName",
  "included",
  "includedByteLength",
  "includedContentHash",
  "kind",
  "modifiedTimeMs",
  "originalByteLength",
  "priority",
  "relativePath",
  "required",
  "selectionReason",
  "sourceByteCount",
  "sourceId",
  "truncation",
  "userDeselectable",
];
const BUDGET_KEYS = [
  "projectContextBudgetBytes",
  "projectContextBytesUsed",
  "protocolBudgetBytes",
  "protocolBytesUsed",
  "requestLimitBytes",
  "safetyReserveBytes",
  "singleSourceLimitBytes",
  "totalIncludedBytes",
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

function isSafeInteger(value: unknown, minimum = 0, maximum = Number.MAX_SAFE_INTEGER): boolean {
  return Number.isSafeInteger(value) && Number(value) >= minimum && Number(value) <= maximum;
}

function isSafeText(value: unknown, maximumCharacters: number): value is string {
  return (
    typeof value === "string" &&
    value === value.trim() &&
    value.length > 0 &&
    [...value].length <= maximumCharacters &&
    ![...value].some((character) => {
      const code = character.codePointAt(0) ?? 0;
      return code <= 0x1f || code === 0x7f;
    })
  );
}

function isSafeRelativeMarkdownPath(value: unknown): value is string {
  if (typeof value !== "string" || value.length === 0 || value.length > 512) {
    return false;
  }
  if (value.includes("\\") || value.includes(":") || !value.endsWith(".md")) {
    return false;
  }
  return value
    .split("/")
    .every(
      (part) =>
        part.length > 0 &&
        part !== "." &&
        part !== ".." &&
        !part.startsWith(".") &&
        ![...part].some((character) => (character.codePointAt(0) ?? 0) <= 0x1f),
    );
}

function parseCatalogSource(value: unknown): StoryContextCatalogEntry {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, CATALOG_SOURCE_KEYS) ||
    !isSafeText(value.sourceId, 600) ||
    !SOURCE_KINDS.has(value.kind as StoryContextSourceKind) ||
    !isSafeText(value.displayName, 160) ||
    !SELECTION_REASONS.has(value.selectionReason as StoryContextSelectionReason) ||
    typeof value.required !== "boolean" ||
    typeof value.userDeselectable !== "boolean" ||
    typeof value.defaultSelected !== "boolean" ||
    !isSafeInteger(value.priority, 0, 1_000) ||
    !isSafeInteger(value.byteLength) ||
    typeof value.contentHash !== "string" ||
    !SHA256.test(value.contentHash) ||
    !isSafeInteger(value.modifiedTimeMs)
  ) {
    throw new Error("Invalid Story Context source");
  }
  const projectSource = value.relativePath !== null && value.builtinResourceId === null;
  const builtinSource = value.relativePath === null && isSafeText(value.builtinResourceId, 200);
  if (
    (!projectSource && !builtinSource) ||
    (projectSource && !isSafeRelativeMarkdownPath(value.relativePath)) ||
    (value.required && value.userDeselectable) ||
    (value.required && !value.defaultSelected)
  ) {
    throw new Error("Invalid Story Context source location");
  }
  return value as StoryContextCatalogEntry;
}

export function parseStoryContextCatalog(value: unknown): StoryContextCatalogSnapshot {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, CATALOG_KEYS) ||
    value.schemaVersion !== STORY_CONTEXT_SCHEMA_VERSION ||
    value.workflowId !== STORYTELLING_WORKFLOW_ID ||
    !isSafeInteger(value.suggestedChapterNumber, 1, 999_999) ||
    !Array.isArray(value.sources) ||
    value.sources.length < 6 ||
    value.sources.length > 5_000
  ) {
    throw new Error("Invalid Story Context Catalog");
  }
  const sources = value.sources.map(parseCatalogSource);
  const ids = new Set(sources.map((source) => source.sourceId));
  const paths = sources
    .map((source) => source.relativePath)
    .filter((path): path is string => path !== null);
  if (ids.size !== sources.length || new Set(paths).size !== paths.length) {
    throw new Error("Duplicate Story Context source");
  }
  return { ...value, sources } as StoryContextCatalogSnapshot;
}

function parseManifestSource(
  value: unknown,
  expectedIncluded: boolean,
): StoryContextManifestSource {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, MANIFEST_SOURCE_KEYS) ||
    !isSafeText(value.sourceId, 600) ||
    !SOURCE_KINDS.has(value.kind as StoryContextSourceKind) ||
    !isSafeText(value.displayName, 160) ||
    !SELECTION_REASONS.has(value.selectionReason as StoryContextSelectionReason) ||
    typeof value.required !== "boolean" ||
    typeof value.userDeselectable !== "boolean" ||
    !isSafeInteger(value.priority, 0, 1_000) ||
    !isSafeInteger(value.sourceByteCount) ||
    !isSafeInteger(value.originalByteLength) ||
    !isSafeInteger(value.includedByteLength) ||
    Number(value.includedByteLength) > Number(value.originalByteLength) ||
    typeof value.contentHash !== "string" ||
    !SHA256.test(value.contentHash) ||
    typeof value.includedContentHash !== "string" ||
    !SHA256.test(value.includedContentHash) ||
    value.included !== expectedIncluded ||
    !isSafeInteger(value.modifiedTimeMs)
  ) {
    throw new Error("Invalid Story Context Manifest source");
  }
  const projectSource = value.relativePath !== null && value.builtinResourceId === null;
  const builtinSource = value.relativePath === null && isSafeText(value.builtinResourceId, 200);
  if (
    (!projectSource && !builtinSource) ||
    (projectSource && !isSafeRelativeMarkdownPath(value.relativePath)) ||
    (value.required && value.userDeselectable)
  ) {
    throw new Error("Invalid Story Context Manifest source location");
  }
  if (value.truncation !== null) {
    if (
      !isRecord(value.truncation) ||
      !hasExactKeys(value.truncation, ["originalByteLength", "reason", "retainedByteLength"]) ||
      !TRUNCATION_REASONS.has(value.truncation.reason as StoryContextTruncationReason) ||
      !isSafeInteger(value.truncation.originalByteLength) ||
      !isSafeInteger(value.truncation.retainedByteLength) ||
      value.truncation.originalByteLength !== value.originalByteLength ||
      value.truncation.retainedByteLength !== value.includedByteLength
    ) {
      throw new Error("Invalid Story Context truncation");
    }
  }
  return value as StoryContextManifestSource;
}

export function parseStoryContextManifest(value: unknown): StoryContextManifest {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, MANIFEST_KEYS) ||
    value.manifestVersion !== STORY_CONTEXT_SCHEMA_VERSION ||
    value.workflowId !== STORYTELLING_WORKFLOW_ID ||
    value.workflowVersion !== 1 ||
    (value.action !== "create" && value.action !== "revise") ||
    !isSafeText(value.projectRootIdentity, 128) ||
    !Array.isArray(value.selectedSources) ||
    !Array.isArray(value.excludedSources) ||
    !Array.isArray(value.sourceOrder) ||
    typeof value.aggregateHash !== "string" ||
    !SHA256.test(value.aggregateHash) ||
    !isSafeInteger(value.createdAt)
  ) {
    throw new Error("Invalid Story Context Manifest");
  }
  if (
    !isRecord(value.targetChapter) ||
    !hasExactKeys(value.targetChapter, ["chapterNumber", "chapterTitle", "relativePath"]) ||
    !isSafeInteger(value.targetChapter.chapterNumber, 1, 999_999) ||
    !isSafeText(value.targetChapter.chapterTitle, 120) ||
    !isSafeRelativeMarkdownPath(value.targetChapter.relativePath)
  ) {
    throw new Error("Invalid Story Context target");
  }
  if (!isRecord(value.budget) || !hasExactKeys(value.budget, BUDGET_KEYS)) {
    throw new Error("Invalid Story Context budget");
  }
  for (const key of BUDGET_KEYS) {
    if (!isSafeInteger(value.budget[key])) {
      throw new Error("Invalid Story Context budget value");
    }
  }
  if (
    value.budget.requestLimitBytes !== 96 * 1024 ||
    value.budget.protocolBudgetBytes !== 24 * 1024 ||
    value.budget.projectContextBudgetBytes !== 48 * 1024 ||
    value.budget.safetyReserveBytes !== 24 * 1024 ||
    value.budget.singleSourceLimitBytes !== 8 * 1024 ||
    value.budget.totalIncludedBytes !==
      Number(value.budget.protocolBytesUsed) + Number(value.budget.projectContextBytesUsed)
  ) {
    throw new Error("Invalid Story Context fixed budget");
  }
  const selectedSources = value.selectedSources.map((source) => parseManifestSource(source, true));
  const excludedSources = value.excludedSources.map((source) => parseManifestSource(source, false));
  const all = [...selectedSources, ...excludedSources];
  if (new Set(all.map((source) => source.sourceId)).size !== all.length) {
    throw new Error("Duplicate Story Context Manifest source");
  }
  if (
    value.sourceOrder.length !== selectedSources.length ||
    value.sourceOrder.some((sourceId, index) => sourceId !== selectedSources[index]?.sourceId)
  ) {
    throw new Error("Invalid Story Context source order");
  }
  return { ...value, selectedSources, excludedSources } as StoryContextManifest;
}

function validateBuildInput(input: StoryContextBuildInput): void {
  const title = input.chapterTitle.trim();
  if (
    (input.action !== "create" && input.action !== "revise") ||
    !isSafeInteger(input.chapterNumber, 1, 999_999) ||
    !isSafeText(title, 120) ||
    input.selectedSourcePaths.length > 256 ||
    new Set(input.selectedSourcePaths).size !== input.selectedSourcePaths.length ||
    input.selectedSourcePaths.some((path) => !isSafeRelativeMarkdownPath(path))
  ) {
    throw {
      code: "workflow.input_invalid",
      messageKey: "errors.workflowInputInvalid",
    } satisfies AppError;
  }
}

const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);

export async function requestStoryContextCatalog(
  sessionId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<StoryContextCatalogSnapshot> {
  try {
    return parseStoryContextCatalog(
      await invokeCommand(STORY_CONTEXT_CATALOG_COMMAND, { sessionId }),
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

export async function requestStoryContextBuild(
  sessionId: string,
  input: StoryContextBuildInput,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<StoryContextManifest> {
  validateBuildInput(input);
  try {
    return parseStoryContextManifest(
      await invokeCommand(STORY_CONTEXT_BUILD_COMMAND, {
        sessionId,
        input: {
          schemaVersion: STORY_CONTEXT_SCHEMA_VERSION,
          action: input.action,
          chapterNumber: input.chapterNumber,
          chapterTitle: input.chapterTitle.trim(),
          selectedSourcePaths: [...input.selectedSourcePaths].sort(),
        },
      }),
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
