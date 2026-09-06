import { invoke } from "@tauri-apps/api/core";
import { normalizeAppError, type AppError } from "./health";

export const PROJECT_VALIDATE_DRAFT_COMMAND = "project_validate_draft";
export const PROJECT_SEARCH_COMMAND = "project_search";
export const MAX_SEARCH_QUERY_CHARS = 128;
export const MAX_SEARCH_RESULTS = 100;
export const MAX_SEARCH_SNIPPET_CHARS = 162;
const MAX_SEARCH_FILES = 500;
const MAX_SEARCH_BYTES = 32 * 1024 * 1024;

export type SourceDiagnostic = {
  code: string;
  severity: "error";
  relativePath: string;
  line: number;
  column: number;
  message: string;
  recovery: string;
};

export type ValidationReport = {
  relativePath: string;
  contentHash: string;
  diagnostics: SourceDiagnostic[];
  eligibleForAiApply: boolean;
  eligibleForFinal: boolean;
  eligibleForEpub: boolean;
};

export type SearchMatch = {
  relativePath: string;
  line: number;
  column: number;
  snippet: string;
};

export type SearchReport = {
  matches: SearchMatch[];
  scannedFileCount: number;
  skippedFileCount: number;
  scannedBytes: number;
  truncated: boolean;
};

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const HASH = /^[0-9a-f]{64}$/u;
const SAFE_PATH_CHARACTERS = /^[\p{L}\p{N} ._/-]{1,512}$/u;
const DIAGNOSTIC_CODE = /^(?:markdown\.[a-z_]+|frontmatter\.[a-z_]+|chapter\.[a-z_]+)$/u;
const REPORT_KEYS = [
  "contentHash",
  "diagnostics",
  "eligibleForAiApply",
  "eligibleForEpub",
  "eligibleForFinal",
  "relativePath",
] as const;
const DIAGNOSTIC_KEYS = [
  "code",
  "column",
  "line",
  "message",
  "recovery",
  "relativePath",
  "severity",
] as const;
const SEARCH_REPORT_KEYS = [
  "matches",
  "scannedBytes",
  "scannedFileCount",
  "skippedFileCount",
  "truncated",
] as const;
const SEARCH_MATCH_KEYS = ["column", "line", "relativePath", "snippet"] as const;

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

function isPositivePosition(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 1 && (value as number) <= 10_000_000;
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

function isBoundedDisplayText(value: unknown, maximumCharacters: number): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    [...value].length <= maximumCharacters &&
    [...value].every((character) => {
      const codePoint = character.codePointAt(0) ?? 0;
      return codePoint >= 32 && codePoint !== 127;
    })
  );
}

function parseDiagnostic(value: unknown, relativePath: string): SourceDiagnostic {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, DIAGNOSTIC_KEYS) ||
    typeof value.code !== "string" ||
    !DIAGNOSTIC_CODE.test(value.code) ||
    value.severity !== "error" ||
    value.relativePath !== relativePath ||
    !isPositivePosition(value.line) ||
    !isPositivePosition(value.column) ||
    !isBoundedDisplayText(value.message, 240) ||
    !isBoundedDisplayText(value.recovery, 320)
  ) {
    throw new Error("Invalid source diagnostic contract");
  }
  return value as SourceDiagnostic;
}

export function parseValidationReport(value: unknown): ValidationReport {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, REPORT_KEYS) ||
    !isSafeRelativePath(value.relativePath) ||
    typeof value.contentHash !== "string" ||
    !HASH.test(value.contentHash) ||
    !Array.isArray(value.diagnostics) ||
    value.diagnostics.length > 64 ||
    typeof value.eligibleForAiApply !== "boolean" ||
    typeof value.eligibleForFinal !== "boolean" ||
    typeof value.eligibleForEpub !== "boolean"
  ) {
    throw new Error("Invalid validation report contract");
  }
  const diagnostics = value.diagnostics.map((item) =>
    parseDiagnostic(item, value.relativePath as string),
  );
  const eligible = diagnostics.length === 0;
  if (
    value.eligibleForAiApply !== eligible ||
    value.eligibleForFinal !== eligible ||
    value.eligibleForEpub !== false
  ) {
    throw new Error("Invalid validation eligibility");
  }
  return { ...(value as ValidationReport), diagnostics };
}

export function parseSearchReport(value: unknown): SearchReport {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, SEARCH_REPORT_KEYS) ||
    !Array.isArray(value.matches) ||
    value.matches.length > MAX_SEARCH_RESULTS ||
    !isCount(value.scannedFileCount) ||
    value.scannedFileCount > MAX_SEARCH_FILES ||
    !isCount(value.skippedFileCount) ||
    value.skippedFileCount > value.scannedFileCount ||
    !isCount(value.scannedBytes) ||
    value.scannedBytes > MAX_SEARCH_BYTES ||
    typeof value.truncated !== "boolean"
  ) {
    throw new Error("Invalid search report contract");
  }
  const matches = value.matches.map((item) => {
    if (
      !isRecord(item) ||
      !hasExactKeys(item, SEARCH_MATCH_KEYS) ||
      !isSafeRelativePath(item.relativePath) ||
      !isPositivePosition(item.line) ||
      !isPositivePosition(item.column) ||
      !isBoundedDisplayText(item.snippet, MAX_SEARCH_SNIPPET_CHARS)
    ) {
      throw new Error("Invalid search match contract");
    }
    return item as SearchMatch;
  });
  return { ...(value as SearchReport), matches };
}

const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);

async function invokeAnalysis<T>(
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

export function requestProjectDraftValidation(
  sessionId: string,
  relativePath: string,
  content: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ValidationReport> {
  return invokeAnalysis(
    PROJECT_VALIDATE_DRAFT_COMMAND,
    { sessionId, relativePath, content },
    parseValidationReport,
    invokeCommand,
  );
}

export function requestProjectSearch(
  sessionId: string,
  query: string,
  caseSensitive: boolean,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<SearchReport> {
  return invokeAnalysis(
    PROJECT_SEARCH_COMMAND,
    { sessionId, query, caseSensitive },
    parseSearchReport,
    invokeCommand,
  );
}
