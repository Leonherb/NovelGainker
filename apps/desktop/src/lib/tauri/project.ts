import { invoke } from "@tauri-apps/api/core";
import { normalizeAppError, type AppError } from "./health";

export const PROJECT_SCAN_COMMAND = "project_scan";
export const PROJECT_CREATE_COMMAND = "project_create";
export const PROJECT_CREATE_PREVIEW_COMMAND = "project_create_preview";
export const PROJECT_OPEN_COMMAND = "project_open";
export const PROJECT_OPEN_RECENT_COMMAND = "project_open_recent";
export const PROJECT_CLOSE_COMMAND = "project_close";

export type ProjectScanMode = "create" | "open";
export type PlannedEntryKind = "file" | "directory";
export type ProjectConflictKind =
  "existingTarget" | "wrongType" | "symbolicLink" | "notMarkdownProject";

export type PlannedEntry = {
  relativePath: string;
  kind: PlannedEntryKind;
};

export type ProjectConflict = {
  relativePath: string;
  kind: ProjectConflictKind;
};

export type ProjectCandidate = {
  candidateId: string;
  mode: ProjectScanMode;
  displayName: string;
  existingEntryCount: number;
  markdownFileCount: number;
  hasProjectMetadata: boolean;
  requiresMetadataConfirmation: boolean;
  canConfirm: boolean;
  plannedEntries: PlannedEntry[];
  conflicts: ProjectConflict[];
};

export type ProjectCreateInputV1 = {
  schemaVersion: 1;
  title: string;
  authorDisplayName: string | null;
  genre: string | null;
  premise: string | null;
};

export type ProjectCreateFilePreview = {
  relativePath: string;
  byteLength: number;
  contentSha256: string;
  content: string;
};

export type ProjectCreatePreview = {
  schemaVersion: 1;
  previewHash: string;
  projectId: string;
  writingLanguage: "zh-CN";
  workflowPackVersion: "1.0.0";
  defaultProviderId: "openai";
  existingUnrelatedEntryCount: number;
  directories: string[];
  files: ProjectCreateFilePreview[];
};

export type ProjectSession = {
  sessionId: string;
  projectId: string;
  displayName: string;
  schemaVersion: 1;
  writingLanguage: "zh-CN";
  markdownFileCount: number;
  restoredCurrentFile: string | null;
  accessMode: "writer" | "readOnly";
  recoveryStatus: "ready" | "recovered" | "required" | "notChecked";
  recoveredTransactionCount: number;
  requiredTransactionCount: number;
  capabilities: {
    canRead: true;
    canWrite: boolean;
    canBrowseFiles: true;
    canSaveFiles: boolean;
  };
};

export type ProjectCloseResult = {
  closed: boolean;
};

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu;
const SAFE_RELATIVE_PATH = /^[\p{L}\p{N} ._/-]{1,180}$/u;
const CANDIDATE_KEYS = [
  "canConfirm",
  "candidateId",
  "conflicts",
  "displayName",
  "existingEntryCount",
  "hasProjectMetadata",
  "markdownFileCount",
  "mode",
  "plannedEntries",
  "requiresMetadataConfirmation",
] as const;
const PLANNED_ENTRY_KEYS = ["kind", "relativePath"] as const;
const CONFLICT_KEYS = ["kind", "relativePath"] as const;
const SESSION_KEYS = [
  "accessMode",
  "capabilities",
  "displayName",
  "markdownFileCount",
  "projectId",
  "recoveredTransactionCount",
  "recoveryStatus",
  "requiredTransactionCount",
  "restoredCurrentFile",
  "schemaVersion",
  "sessionId",
  "writingLanguage",
] as const;
const CAPABILITY_KEYS = ["canBrowseFiles", "canRead", "canSaveFiles", "canWrite"] as const;
const SHA256 = /^[0-9a-f]{64}$/u;
const MAX_PREVIEW_FILE_BYTES = 64 * 1024;
const CREATE_PREVIEW_KEYS = [
  "defaultProviderId",
  "directories",
  "existingUnrelatedEntryCount",
  "files",
  "previewHash",
  "projectId",
  "schemaVersion",
  "workflowPackVersion",
  "writingLanguage",
] as const;
const CREATE_FILE_PREVIEW_KEYS = [
  "byteLength",
  "content",
  "contentSha256",
  "relativePath",
] as const;
const CREATE_DIRECTORY_PATHS = [
  "characters",
  "materials",
  "macguffins",
  "plot",
  "outlines",
  "world",
  "style",
  "assets",
  "chapters",
  "published",
  ".novelgainker",
] as const;
const CREATE_FILE_PATHS = [
  "project.md",
  "AGENTS.md",
  "materials/000.story-ledger.md",
  ".novelgainker/project.json",
] as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function isCount(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function isSafeDisplayName(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.trim().length > 0 &&
    value.length <= 80 &&
    [...value].every((character) => {
      const codePoint = character.codePointAt(0) ?? 0;
      return codePoint > 31 && codePoint !== 127;
    }) &&
    !/[\\/]/u.test(value)
  );
}

function isSafeRelativePath(value: unknown): value is string {
  return (
    typeof value === "string" &&
    SAFE_RELATIVE_PATH.test(value) &&
    !value.startsWith("/") &&
    !value.includes("\\") &&
    !value.includes(":") &&
    value
      .split("/")
      .every((component) => component !== "" && component !== "." && component !== "..")
  );
}

function parsePlannedEntry(value: unknown): PlannedEntry {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, PLANNED_ENTRY_KEYS) ||
    !isSafeRelativePath(value.relativePath) ||
    (value.kind !== "file" && value.kind !== "directory")
  ) {
    throw new Error("Invalid planned project entry");
  }
  return value as PlannedEntry;
}

function parseConflict(value: unknown): ProjectConflict {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, CONFLICT_KEYS) ||
    !isSafeRelativePath(value.relativePath) ||
    !["existingTarget", "wrongType", "symbolicLink", "notMarkdownProject"].includes(
      value.kind as string,
    )
  ) {
    throw new Error("Invalid project conflict");
  }
  return value as ProjectConflict;
}
function parseProjectCreateFilePreview(value: unknown): ProjectCreateFilePreview {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, CREATE_FILE_PREVIEW_KEYS) ||
    !isSafeRelativePath(value.relativePath) ||
    !isCount(value.byteLength) ||
    typeof value.contentSha256 !== "string" ||
    !SHA256.test(value.contentSha256) ||
    typeof value.content !== "string" ||
    value.content.startsWith("\uFEFF") ||
    value.content.includes("\r") ||
    new TextEncoder().encode(value.content).length !== value.byteLength ||
    value.byteLength > MAX_PREVIEW_FILE_BYTES
  ) {
    throw new Error("Invalid project create file preview");
  }
  return value as ProjectCreateFilePreview;
}

export function parseProjectCreatePreview(value: unknown): ProjectCreatePreview {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, CREATE_PREVIEW_KEYS) ||
    value.schemaVersion !== 1 ||
    typeof value.previewHash !== "string" ||
    !SHA256.test(value.previewHash) ||
    typeof value.projectId !== "string" ||
    !UUID.test(value.projectId) ||
    value.writingLanguage !== "zh-CN" ||
    value.workflowPackVersion !== "1.0.0" ||
    value.defaultProviderId !== "openai" ||
    !isCount(value.existingUnrelatedEntryCount) ||
    !Array.isArray(value.directories) ||
    !Array.isArray(value.files)
  ) {
    throw new Error("Invalid project create preview contract");
  }
  if (
    value.directories.length !== CREATE_DIRECTORY_PATHS.length ||
    !value.directories.every(
      (path, index) => path === CREATE_DIRECTORY_PATHS[index] && isSafeRelativePath(path),
    )
  ) {
    throw new Error("Invalid project create directory plan");
  }
  const files = value.files.map(parseProjectCreateFilePreview);
  if (
    files.length !== CREATE_FILE_PATHS.length ||
    !files.every((file, index) => file.relativePath === CREATE_FILE_PATHS[index])
  ) {
    throw new Error("Invalid project create file plan");
  }
  return {
    schemaVersion: 1,
    previewHash: value.previewHash,
    projectId: value.projectId,
    writingLanguage: "zh-CN",
    workflowPackVersion: "1.0.0",
    defaultProviderId: "openai",
    existingUnrelatedEntryCount: value.existingUnrelatedEntryCount,
    directories: [...value.directories],
    files,
  };
}

export function parseProjectCandidate(value: unknown): ProjectCandidate | null {
  if (value === null) {
    return null;
  }
  if (
    !isRecord(value) ||
    !hasExactKeys(value, CANDIDATE_KEYS) ||
    typeof value.candidateId !== "string" ||
    !UUID.test(value.candidateId) ||
    (value.mode !== "create" && value.mode !== "open") ||
    !isSafeDisplayName(value.displayName) ||
    !isCount(value.existingEntryCount) ||
    !isCount(value.markdownFileCount) ||
    typeof value.hasProjectMetadata !== "boolean" ||
    typeof value.requiresMetadataConfirmation !== "boolean" ||
    typeof value.canConfirm !== "boolean" ||
    !Array.isArray(value.plannedEntries) ||
    !Array.isArray(value.conflicts)
  ) {
    throw new Error("Invalid project candidate contract");
  }

  return {
    candidateId: value.candidateId,
    mode: value.mode,
    displayName: value.displayName,
    existingEntryCount: value.existingEntryCount,
    markdownFileCount: value.markdownFileCount,
    hasProjectMetadata: value.hasProjectMetadata,
    requiresMetadataConfirmation: value.requiresMetadataConfirmation,
    canConfirm: value.canConfirm,
    plannedEntries: value.plannedEntries.map(parsePlannedEntry),
    conflicts: value.conflicts.map(parseConflict),
  };
}

export function parseProjectSession(value: unknown): ProjectSession {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, SESSION_KEYS) ||
    typeof value.sessionId !== "string" ||
    !UUID.test(value.sessionId) ||
    typeof value.projectId !== "string" ||
    !UUID.test(value.projectId) ||
    !isSafeDisplayName(value.displayName) ||
    value.schemaVersion !== 1 ||
    value.writingLanguage !== "zh-CN" ||
    !isCount(value.markdownFileCount) ||
    (value.accessMode !== "writer" && value.accessMode !== "readOnly") ||
    !["ready", "recovered", "required", "notChecked"].includes(value.recoveryStatus as string) ||
    !isCount(value.recoveredTransactionCount) ||
    !isCount(value.requiredTransactionCount) ||
    (value.restoredCurrentFile !== null && !isSafeRelativePath(value.restoredCurrentFile)) ||
    !isRecord(value.capabilities) ||
    !hasExactKeys(value.capabilities, CAPABILITY_KEYS) ||
    value.capabilities.canRead !== true ||
    value.capabilities.canBrowseFiles !== true ||
    typeof value.capabilities.canWrite !== "boolean" ||
    typeof value.capabilities.canSaveFiles !== "boolean"
  ) {
    throw new Error("Invalid project session contract");
  }

  const writerCanSave = value.accessMode === "writer" && value.recoveryStatus !== "required";
  const recoveryCountsAreValid =
    (value.recoveryStatus === "ready" &&
      value.recoveredTransactionCount === 0 &&
      value.requiredTransactionCount === 0) ||
    (value.recoveryStatus === "recovered" &&
      (value.recoveredTransactionCount as number) > 0 &&
      value.requiredTransactionCount === 0) ||
    (value.recoveryStatus === "required" && (value.requiredTransactionCount as number) > 0) ||
    (value.recoveryStatus === "notChecked" &&
      value.accessMode === "readOnly" &&
      value.recoveredTransactionCount === 0 &&
      value.requiredTransactionCount === 0);
  if (
    value.capabilities.canWrite !== writerCanSave ||
    value.capabilities.canSaveFiles !== writerCanSave ||
    (value.accessMode === "writer" && value.recoveryStatus === "notChecked") ||
    (value.accessMode === "readOnly" && value.recoveryStatus !== "notChecked") ||
    !recoveryCountsAreValid
  ) {
    throw new Error("Invalid project session state");
  }
  return value as ProjectSession;
}

export function parseProjectCloseResult(value: unknown): ProjectCloseResult {
  if (!isRecord(value) || !hasExactKeys(value, ["closed"]) || typeof value.closed !== "boolean") {
    throw new Error("Invalid project close contract");
  }
  return { closed: value.closed };
}

const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);

async function invokeProject<T>(
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

export function requestProjectScan(
  mode: ProjectScanMode,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProjectCandidate | null> {
  return invokeProject(PROJECT_SCAN_COMMAND, { mode }, parseProjectCandidate, invokeCommand);
}

export function requestProjectCreatePreview(
  candidateId: string,
  input: ProjectCreateInputV1,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProjectCreatePreview> {
  return invokeProject(
    PROJECT_CREATE_PREVIEW_COMMAND,
    { candidateId, input },
    parseProjectCreatePreview,
    invokeCommand,
  );
}

export function requestProjectCreate(
  candidateId: string,
  previewHash: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProjectSession> {
  return invokeProject(
    PROJECT_CREATE_COMMAND,
    { candidateId, previewHash, confirmed: true },
    parseProjectSession,
    invokeCommand,
  );
}

export function requestProjectOpen(
  candidateId: string,
  confirmMetadata: boolean,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProjectSession> {
  return invokeProject(
    PROJECT_OPEN_COMMAND,
    { candidateId, confirmMetadata },
    parseProjectSession,
    invokeCommand,
  );
}

export function requestProjectOpenRecent(
  projectId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProjectSession> {
  return invokeProject(
    PROJECT_OPEN_RECENT_COMMAND,
    { projectId },
    parseProjectSession,
    invokeCommand,
  );
}

export function requestProjectClose(
  sessionId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProjectCloseResult> {
  return invokeProject(
    PROJECT_CLOSE_COMMAND,
    { sessionId },
    parseProjectCloseResult,
    invokeCommand,
  );
}
