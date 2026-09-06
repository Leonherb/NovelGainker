import { invoke } from "@tauri-apps/api/core";
import { normalizeAppError, type AppError } from "./health";

export const SESSION_LIST_COMMAND = "conversation_session_list";
export const SESSION_CREATE_COMMAND = "conversation_session_create";
export const SESSION_RENAME_COMMAND = "conversation_session_rename";
export const SESSION_ACTIVATE_COMMAND = "conversation_session_activate";
export type SessionEntry = {
  sessionId: string;
  name: string;
  archived: boolean;
  createdAtMs: number;
  updatedAtMs: number;
  creationRequestId: string | null;
};
export type RunReservation = { reservationId: string; sessionId: string; turnId: string | null };
export type SessionPage = {
  canMutate: boolean;
  schemaVersion: 1;
  revision: number;
  activeSessionId: string | null;
  defaultSessionId: string;
  items: SessionEntry[];
  nonArchivedCount: number;
  totalCount: number;
  nextCursor: string | null;
  activeRun: RunReservation | null;
};
type Call = (command: string, args: Record<string, unknown>) => Promise<unknown>;
const call: Call = (command, args) => invoke<unknown>(command, args);
const uuid = (v: unknown): v is string =>
  typeof v === "string" &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u.test(v);
const integer = (v: unknown, min: number, max: number): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= min && v <= max;
const record = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);
const exact = (v: Record<string, unknown>, keys: string[]) =>
  Object.keys(v).length === keys.length &&
  keys.every((key) => Object.prototype.hasOwnProperty.call(v, key));
const nullableId = (v: unknown) => v === null || uuid(v);
export function parseSessionPage(value: unknown): SessionPage {
  if (
    !record(value) ||
    !exact(value, [
      "canMutate",
      "schemaVersion",
      "revision",
      "activeSessionId",
      "defaultSessionId",
      "items",
      "nonArchivedCount",
      "totalCount",
      "nextCursor",
      "activeRun",
    ]) ||
    value.schemaVersion !== 1 ||
    typeof value.canMutate !== "boolean" ||
    !integer(value.revision, 1, Number.MAX_SAFE_INTEGER) ||
    !nullableId(value.activeSessionId) ||
    !uuid(value.defaultSessionId) ||
    !integer(value.nonArchivedCount, 0, 32) ||
    !integer(value.totalCount, 1, 256) ||
    value.nonArchivedCount > value.totalCount ||
    !Array.isArray(value.items) ||
    value.items.length > 32 ||
    !(
      value.nextCursor === null ||
      (typeof value.nextCursor === "string" &&
        value.nextCursor.length > 0 &&
        value.nextCursor.length <= 1024)
    ) ||
    (value.activeSessionId === null) !== (value.nonArchivedCount === 0)
  )
    throw new Error("Invalid session page");
  const ids = new Set<string>();
  const encoder = new TextEncoder();
  for (const entry of value.items) {
    if (
      !record(entry) ||
      !exact(entry, [
        "sessionId",
        "name",
        "archived",
        "createdAtMs",
        "updatedAtMs",
        "creationRequestId",
      ]) ||
      !uuid(entry.sessionId) ||
      ids.has(entry.sessionId) ||
      typeof entry.name !== "string" ||
      entry.name.trim() !== entry.name ||
      entry.name.length === 0 ||
      [...entry.name].length > 60 ||
      encoder.encode(entry.name).length > 180 ||
      [...entry.name].some(
        (char) =>
          char.codePointAt(0)! < 32 || (char.codePointAt(0)! >= 127 && char.codePointAt(0)! <= 159),
      ) ||
      typeof entry.archived !== "boolean" ||
      !integer(entry.createdAtMs, 1, Number.MAX_SAFE_INTEGER) ||
      !integer(entry.updatedAtMs, entry.createdAtMs, Number.MAX_SAFE_INTEGER) ||
      !nullableId(entry.creationRequestId)
    )
      throw new Error("Invalid session entry");
    ids.add(entry.sessionId);
  }
  if (value.activeRun !== null) {
    const run = value.activeRun;
    if (
      !record(run) ||
      !exact(run, ["reservationId", "sessionId", "turnId"]) ||
      !uuid(run.reservationId) ||
      !uuid(run.sessionId) ||
      !nullableId(run.turnId)
    )
      throw new Error("Invalid run reservation");
  }
  return value as SessionPage;
}
async function request(
  command: string,
  request: Record<string, unknown>,
  invokeCommand: Call,
): Promise<SessionPage> {
  let value: unknown;
  try {
    value = await invokeCommand(command, { request });
  } catch (error: unknown) {
    throw normalizeAppError(error);
  }
  try {
    return parseSessionPage(value);
  } catch {
    throw {
      code: "contract.invalid_response",
      messageKey: "errors.invalidRuntimeResponse",
    } satisfies AppError;
  }
}
export const requestSessions = (
  projectSessionId: string,
  archived = false,
  cursor: string | null = null,
  invokeCommand: Call = call,
) => request(SESSION_LIST_COMMAND, { projectSessionId, archived, cursor }, invokeCommand);
export const requestSessionCreate = (
  projectSessionId: string,
  requestId: string,
  name: string,
  expectedRevision: number,
  invokeCommand: Call = call,
) =>
  request(
    SESSION_CREATE_COMMAND,
    { projectSessionId, requestId, name, expectedRevision },
    invokeCommand,
  );
export const requestSessionRename = (
  projectSessionId: string,
  conversationSessionId: string,
  name: string,
  expectedRevision: number,
  invokeCommand: Call = call,
) =>
  request(
    SESSION_RENAME_COMMAND,
    { projectSessionId, conversationSessionId, name, expectedRevision },
    invokeCommand,
  );
export const requestSessionActivate = (
  projectSessionId: string,
  conversationSessionId: string,
  expectedRevision: number,
  invokeCommand: Call = call,
) =>
  request(
    SESSION_ACTIVATE_COMMAND,
    { projectSessionId, conversationSessionId, expectedRevision },
    invokeCommand,
  );
