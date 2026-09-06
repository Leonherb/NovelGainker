import { invoke } from "@tauri-apps/api/core";
import { normalizeAppError } from "./health";
import { parseSessionPage, type SessionPage } from "./conversation-sessions";

export const SESSION_MANAGE_COMMAND = "conversation_session_manage";
export const CONVERSATION_EXPORT_PREPARE_COMMAND = "conversation_export_prepare";
export const CONVERSATION_EXPORT_COMMIT_COMMAND = "conversation_export_commit";
export type SessionManagement = {
  conversationSessionId: string;
  expectedRevision: number;
} & (
  | { action: "branch"; requestId: string; boundaryTurnId: string; name: string }
  | { action: "clear"; requestId: string; name: string }
  | { action: "archive" | "restore" }
);
export type SessionManagementResult = {
  schemaVersion: 1;
  action: SessionManagement["action"];
  sourceSessionId: string;
  targetSessionId: string;
  changed: boolean;
  catalog: SessionPage;
};
export type ExportFormat = "markdown" | "json";
export type ExportPrepared = {
  schemaVersion: 1;
  token: string;
  expiresAtMs: number;
  bytes: number;
};
export type ExportReceipt = { schemaVersion: 1; fileName: string; bytes: number; sha256: string };
type Call = (command: string, args: Record<string, unknown>) => Promise<unknown>;
const call: Call = (command, args) => invoke<unknown>(command, args);
const record = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);
const exact = (v: Record<string, unknown>, keys: string[]) =>
  Object.keys(v).length === keys.length &&
  keys.every((key) => Object.prototype.hasOwnProperty.call(v, key));
const uuid = (v: unknown): v is string =>
  typeof v === "string" &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u.test(v);
const integer = (v: unknown, max: number): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v > 0 && v <= max;
const invalid = () => ({
  code: "contract.invalid_response",
  messageKey: "errors.invalidRuntimeResponse",
});
async function request(command: string, request: Record<string, unknown>, invokeCommand: Call) {
  try {
    return await invokeCommand(command, { request });
  } catch (error: unknown) {
    throw normalizeAppError(error);
  }
}
export async function requestSessionManage(
  projectSessionId: string,
  operation: SessionManagement,
  invokeCommand: Call = call,
): Promise<SessionManagementResult> {
  const value = await request(
    SESSION_MANAGE_COMMAND,
    { projectSessionId, operation },
    invokeCommand,
  );
  if (
    !record(value) ||
    !exact(value, [
      "schemaVersion",
      "action",
      "sourceSessionId",
      "targetSessionId",
      "changed",
      "catalog",
    ]) ||
    value.schemaVersion !== 1 ||
    value.action !== operation.action ||
    value.sourceSessionId !== operation.conversationSessionId ||
    !uuid(value.sourceSessionId) ||
    !uuid(value.targetSessionId) ||
    typeof value.changed !== "boolean" ||
    ((operation.action === "archive" || operation.action === "restore") &&
      value.targetSessionId !== value.sourceSessionId) ||
    ((operation.action === "branch" || operation.action === "clear") &&
      value.targetSessionId === value.sourceSessionId)
  )
    throw invalid();
  try {
    parseSessionPage(value.catalog);
  } catch {
    throw invalid();
  }
  return value as SessionManagementResult;
}
export async function requestConversationExportPrepare(
  projectSessionId: string,
  conversationSessionId: string,
  format: ExportFormat,
  invokeCommand: Call = call,
): Promise<ExportPrepared | null> {
  const value = await request(
    CONVERSATION_EXPORT_PREPARE_COMMAND,
    { projectSessionId, conversationSessionId, format },
    invokeCommand,
  );
  if (value === null) return null;
  if (
    !record(value) ||
    !exact(value, ["schemaVersion", "token", "expiresAtMs", "bytes"]) ||
    value.schemaVersion !== 1 ||
    !uuid(value.token) ||
    !integer(value.expiresAtMs, Number.MAX_SAFE_INTEGER) ||
    !integer(value.bytes, 32 * 1024 * 1024)
  )
    throw invalid();
  return value as ExportPrepared;
}
export async function requestConversationExportCommit(
  projectSessionId: string,
  token: string,
  invokeCommand: Call = call,
): Promise<ExportReceipt> {
  const value = await request(
    CONVERSATION_EXPORT_COMMIT_COMMAND,
    { projectSessionId, token },
    invokeCommand,
  );
  if (
    !record(value) ||
    !exact(value, ["schemaVersion", "fileName", "bytes", "sha256"]) ||
    value.schemaVersion !== 1 ||
    typeof value.fileName !== "string" ||
    value.fileName.length === 0 ||
    new TextEncoder().encode(value.fileName).length > 180 ||
    value.fileName.startsWith(".") ||
    /[:/\\<>|?*]/u.test(value.fileName) ||
    /[ .]$/u.test(value.fileName) ||
    [...value.fileName].some(
      (c) => c.codePointAt(0)! < 32 || (c.codePointAt(0)! >= 127 && c.codePointAt(0)! <= 159),
    ) ||
    !/\.(md|json)$/u.test(value.fileName) ||
    !integer(value.bytes, 32 * 1024 * 1024) ||
    typeof value.sha256 !== "string" ||
    !/^[0-9a-f]{64}$/u.test(value.sha256)
  )
    throw invalid();
  return value as ExportReceipt;
}
