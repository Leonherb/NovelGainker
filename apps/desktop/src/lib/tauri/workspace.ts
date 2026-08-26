import { invoke } from "@tauri-apps/api/core";
import { normalizeAppError, type AppError } from "./health";
import { parseValidationReport, type ValidationReport } from "./analysis";

export const PROJECT_LIST_FILES_COMMAND = "project_list_files";
export const PROJECT_READ_FILE_COMMAND = "project_read_file";
export const PROJECT_WATCH_FILE_COMMAND = "project_watch_file";
export const PROJECT_SAVE_FILE_COMMAND = "project_save_file";
export const MAX_EDITABLE_FILE_BYTES = 4 * 1024 * 1024;
const MAX_INDEX_ENTRIES = 5_000;
const MAX_INDEX_DEPTH = 16;

export type ProjectFileKind = "directory" | "markdown" | "metadata";

export type ProjectFileNode = {
  name: string;
  relativePath: string;
  kind: ProjectFileKind;
  sizeBytes: number | null;
  readOnly: boolean;
  children: ProjectFileNode[];
};

export type ProjectFileIndex = {
  nodes: ProjectFileNode[];
  maxEditableFileBytes: typeof MAX_EDITABLE_FILE_BYTES;
};

export type ProjectFileDocument = {
  relativePath: string;
  content: string;
  contentHash: string;
  modifiedTimeMs: number;
  sizeBytes: number;
  kind: "markdown" | "metadata";
  readOnly: boolean;
  bom: "none" | "utf8";
  lineEnding: "none" | "lf" | "crlf" | "mixed";
};

export type ProjectFileSaveResult = {
  relativePath: string;
  contentHash: string;
  modifiedTimeMs: number;
  sizeBytes: number;
  bom: "none" | "utf8";
  lineEnding: "none" | "lf" | "crlf";
  diagnosticsTriggered: true;
  diagnosticState: "ready";
  validation: ValidationReport;
};

export type ProjectFileWatchStatus = {
  relativePath: string;
  state: "unchanged" | "modified" | "missing";
  observedHash: string | null;
  modifiedTimeMs: number;
};
type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu;
const HASH = /^[0-9a-f]{64}$/u;
const SAFE_PATH_CHARACTERS = /^[\p{L}\p{N} ._/-]{1,512}$/u;
const NODE_KEYS = ["children", "kind", "name", "readOnly", "relativePath", "sizeBytes"] as const;
const INDEX_KEYS = ["maxEditableFileBytes", "nodes"] as const;
const SAVE_RESULT_KEYS = [
  "bom",
  "contentHash",
  "diagnosticState",
  "diagnosticsTriggered",
  "lineEnding",
  "modifiedTimeMs",
  "relativePath",
  "sizeBytes",
  "validation",
] as const;
const WATCH_STATUS_KEYS = ["modifiedTimeMs", "observedHash", "relativePath", "state"] as const;
const DOCUMENT_KEYS = [
  "bom",
  "content",
  "contentHash",
  "kind",
  "lineEnding",
  "modifiedTimeMs",
  "readOnly",
  "relativePath",
  "sizeBytes",
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

function isSafeName(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= 255 &&
    !value.includes("/") &&
    !value.includes("\\") &&
    [...value].every((character) => {
      const codePoint = character.codePointAt(0) ?? 0;
      return codePoint > 31 && codePoint !== 127;
    })
  );
}

function isSafeRelativePath(value: unknown): value is string {
  return (
    typeof value === "string" &&
    SAFE_PATH_CHARACTERS.test(value) &&
    !value.startsWith("/") &&
    !value.includes("\\") &&
    !value.includes(":") &&
    value
      .split("/")
      .every((component) => component !== "" && component !== "." && component !== "..")
  );
}

function parseNode(
  value: unknown,
  parentPath: string | null,
  depth: number,
  budget: { count: number },
): ProjectFileNode {
  if (
    depth > MAX_INDEX_DEPTH ||
    !isRecord(value) ||
    !hasExactKeys(value, NODE_KEYS) ||
    !isSafeName(value.name) ||
    !isSafeRelativePath(value.relativePath) ||
    !["directory", "markdown", "metadata"].includes(value.kind as string) ||
    typeof value.readOnly !== "boolean" ||
    !Array.isArray(value.children)
  ) {
    throw new Error("Invalid project file node");
  }
  if (
    (parentPath === null && value.relativePath.includes("/")) ||
    (parentPath !== null && !value.relativePath.startsWith(parentPath + "/"))
  ) {
    throw new Error("Invalid project file hierarchy");
  }

  const kind = value.kind as ProjectFileKind;
  if (
    (kind === "directory" && value.sizeBytes !== null) ||
    (kind !== "directory" && !isCount(value.sizeBytes)) ||
    (kind !== "directory" && value.children.length !== 0) ||
    (kind === "metadata" &&
      (value.relativePath !== ".novelgainker/project.json" || value.readOnly !== true))
  ) {
    throw new Error("Invalid project file node shape");
  }

  budget.count += 1;
  if (budget.count > MAX_INDEX_ENTRIES) {
    throw new Error("Project file index exceeds client limit");
  }

  return {
    name: value.name,
    relativePath: value.relativePath,
    kind,
    sizeBytes: value.sizeBytes as number | null,
    readOnly: value.readOnly,
    children: value.children.map((child) =>
      parseNode(child, value.relativePath as string, depth + 1, budget),
    ),
  };
}

export function parseProjectFileIndex(value: unknown): ProjectFileIndex {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, INDEX_KEYS) ||
    value.maxEditableFileBytes !== MAX_EDITABLE_FILE_BYTES ||
    !Array.isArray(value.nodes)
  ) {
    throw new Error("Invalid project file index contract");
  }
  const budget = { count: 0 };
  return {
    maxEditableFileBytes: MAX_EDITABLE_FILE_BYTES,
    nodes: value.nodes.map((node) => parseNode(node, null, 0, budget)),
  };
}

export function parseProjectFileDocument(value: unknown): ProjectFileDocument {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, DOCUMENT_KEYS) ||
    !isSafeRelativePath(value.relativePath) ||
    typeof value.content !== "string" ||
    typeof value.contentHash !== "string" ||
    !HASH.test(value.contentHash) ||
    !isCount(value.modifiedTimeMs) ||
    !isCount(value.sizeBytes) ||
    (value.kind !== "markdown" && value.kind !== "metadata") ||
    typeof value.readOnly !== "boolean" ||
    (value.kind === "metadata" && value.readOnly !== true) ||
    (value.bom !== "none" && value.bom !== "utf8") ||
    !["none", "lf", "crlf", "mixed"].includes(value.lineEnding as string)
  ) {
    throw new Error("Invalid project file document contract");
  }
  return value as ProjectFileDocument;
}

export function parseProjectFileSaveResult(value: unknown): ProjectFileSaveResult {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, SAVE_RESULT_KEYS) ||
    !isSafeRelativePath(value.relativePath) ||
    typeof value.contentHash !== "string" ||
    !HASH.test(value.contentHash) ||
    !isCount(value.modifiedTimeMs) ||
    !isCount(value.sizeBytes) ||
    (value.bom !== "none" && value.bom !== "utf8") ||
    !["none", "lf", "crlf"].includes(value.lineEnding as string) ||
    value.diagnosticsTriggered !== true ||
    value.diagnosticState !== "ready"
  ) {
    throw new Error("Invalid project file save result contract");
  }
  const validation = parseValidationReport(value.validation);
  if (
    validation.relativePath !== value.relativePath ||
    validation.contentHash !== value.contentHash
  ) {
    throw new Error("Save validation does not match saved source");
  }
  return { ...(value as ProjectFileSaveResult), validation };
}

export function parseProjectFileWatchStatus(value: unknown): ProjectFileWatchStatus {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, WATCH_STATUS_KEYS) ||
    !isSafeRelativePath(value.relativePath) ||
    !["unchanged", "modified", "missing"].includes(value.state as string) ||
    !isCount(value.modifiedTimeMs) ||
    (value.state === "missing"
      ? value.observedHash !== null || value.modifiedTimeMs !== 0
      : typeof value.observedHash !== "string" || !HASH.test(value.observedHash))
  ) {
    throw new Error("Invalid project file watch status contract");
  }
  return value as ProjectFileWatchStatus;
}
const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);

async function invokeWorkspace<T>(
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

export function requestProjectFileIndex(
  sessionId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProjectFileIndex> {
  return invokeWorkspace(
    PROJECT_LIST_FILES_COMMAND,
    { sessionId },
    parseProjectFileIndex,
    invokeCommand,
  );
}

export function requestProjectFile(
  sessionId: string,
  relativePath: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProjectFileDocument> {
  return invokeWorkspace(
    PROJECT_READ_FILE_COMMAND,
    { sessionId, relativePath },
    parseProjectFileDocument,
    invokeCommand,
  );
}

export function requestProjectFileWatch(
  sessionId: string,
  relativePath: string,
  expectedHash: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProjectFileWatchStatus> {
  return invokeWorkspace(
    PROJECT_WATCH_FILE_COMMAND,
    { sessionId, relativePath, expectedHash },
    parseProjectFileWatchStatus,
    invokeCommand,
  );
}

export function requestProjectFileSave(
  sessionId: string,
  relativePath: string,
  content: string,
  sourceHash: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProjectFileSaveResult> {
  return invokeWorkspace(
    PROJECT_SAVE_FILE_COMMAND,
    { sessionId, relativePath, content, sourceHash },
    parseProjectFileSaveResult,
    invokeCommand,
  );
}
export function isProjectSessionId(value: string): boolean {
  return UUID.test(value);
}
