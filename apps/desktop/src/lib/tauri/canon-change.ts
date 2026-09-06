import { invoke } from "@tauri-apps/api/core";
import { normalizeAppError } from "./health";

export const CANON_CHANGE_LIST_COMMAND = "canon_change_list";
export const CANON_CHANGE_DETAIL_COMMAND = "canon_change_detail";
export const CANON_CHANGE_DECIDE_COMMAND = "canon_change_decide";
export const CANON_EDITOR_STATE_COMMAND = "canon_editor_state";

export type ChangeStatus =
  | "prepared"
  | "pending_confirmation"
  | "applying"
  | "committed"
  | "no_change"
  | "rejected"
  | "cancelled"
  | "recovery_required"
  | "undone";
export type ChangeReceipt = {
  schemaVersion: 1;
  changeId: string;
  revision: number;
  status: ChangeStatus;
  files: { relativePath: string; beforeHash: string | null; afterHash: string }[];
};
export type ChangeActivity = {
  conversationSessionId: string;
  turnId: string;
  receipt: ChangeReceipt;
  chapterSourceState?: "current" | "stale" | "unavailable";
};
export type ChangeDetail = {
  activity: ChangeActivity;
  files: {
    relativePath: string;
    lines: { kind: "context" | "removed" | "added"; text: string }[];
  }[];
};
export type ChapterCandidateActivity = { candidateId: string; turnId: string; contentHash: string };
export type ChangePage = {
  schemaVersion: 1;
  items: ChangeActivity[];
  nextCursor: string | null;
  candidates?: ChapterCandidateActivity[];
};
type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
const nativeInvoke: Invoke = (command, args) => invoke(command, args);
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;
const hash = /^[0-9a-f]{64}$/u;
const statuses: ChangeStatus[] = [
  "prepared",
  "pending_confirmation",
  "applying",
  "committed",
  "no_change",
  "rejected",
  "cancelled",
  "recovery_required",
  "undone",
];
function object(value: unknown, keys: string[]): asserts value is Record<string, unknown> {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    Object.keys(value).sort().join(",") !== [...keys].sort().join(",")
  )
    throw new Error("Invalid change shape");
}
function id(value: unknown): value is string {
  return typeof value === "string" && uuid.test(value);
}
function revision(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) > 0;
}
function path(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length <= 512 &&
    /^(characters|world|materials|plot|outlines|chapters)\/[\p{L}\p{N} _./-]+\.md$/u.test(value) &&
    (!value.startsWith("chapters/") || value.split("/").length === 2) &&
    !value
      .split("/")
      .some(
        (part) =>
          !part ||
          part === "." ||
          part === ".." ||
          /[. ]$/u.test(part) ||
          /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/iu.test(part),
      )
  );
}
export function parseChangeReceipt(value: unknown): ChangeReceipt {
  object(value, ["schemaVersion", "changeId", "revision", "status", "files"]);
  if (
    value.schemaVersion !== 1 ||
    !id(value.changeId) ||
    !revision(value.revision) ||
    !statuses.includes(value.status as ChangeStatus) ||
    !Array.isArray(value.files) ||
    value.files.length > 4
  )
    throw new Error("Invalid change receipt");
  const paths = new Set<string>();
  for (const file of value.files) {
    object(file, ["relativePath", "beforeHash", "afterHash"]);
    if (
      !path(file.relativePath) ||
      paths.has(file.relativePath.toLowerCase()) ||
      (file.beforeHash !== null &&
        (typeof file.beforeHash !== "string" || !hash.test(file.beforeHash))) ||
      typeof file.afterHash !== "string" ||
      !hash.test(file.afterHash)
    )
      throw new Error("Invalid change file");
    paths.add(file.relativePath.toLowerCase());
  }
  return value as ChangeReceipt;
}
export function parseChangeActivity(value: unknown): ChangeActivity {
  const hasSourceState =
    typeof value === "object" && value !== null && "chapterSourceState" in value;
  object(
    value,
    hasSourceState
      ? ["conversationSessionId", "turnId", "receipt", "chapterSourceState"]
      : ["conversationSessionId", "turnId", "receipt"],
  );
  if (!id(value.conversationSessionId) || !id(value.turnId))
    throw new Error("Invalid change context");
  if (
    hasSourceState &&
    !["current", "stale", "unavailable"].includes(String(value.chapterSourceState))
  )
    throw new Error("Invalid chapter source state");
  return {
    conversationSessionId: value.conversationSessionId,
    turnId: value.turnId,
    receipt: parseChangeReceipt(value.receipt),
    ...(hasSourceState
      ? { chapterSourceState: value.chapterSourceState as "current" | "stale" | "unavailable" }
      : {}),
  };
}
export function parseChangeActivities(value: unknown): ChangeActivity[] {
  if (!Array.isArray(value) || value.length > 100) throw new Error("Invalid change list");
  const entries = value.map(parseChangeActivity);
  if (new Set(entries.map((entry) => entry.receipt.changeId)).size !== entries.length)
    throw new Error("Duplicate change");
  return entries;
}
export function parseChangePage(value: unknown): ChangePage {
  const hasCandidates = typeof value === "object" && value !== null && "candidates" in value;
  object(
    value,
    hasCandidates
      ? ["schemaVersion", "items", "nextCursor", "candidates"]
      : ["schemaVersion", "items", "nextCursor"],
  );
  if (
    value.schemaVersion !== 1 ||
    (value.nextCursor !== null &&
      (typeof value.nextCursor !== "string" || !/^[A-Za-z0-9_-]{1,1024}$/u.test(value.nextCursor)))
  )
    throw new Error("Invalid change page");
  const items = parseChangeActivities(value.items);
  if (value.nextCursor !== null && items.length === 0) throw new Error("Empty cursor page");
  if (hasCandidates) {
    if (!Array.isArray(value.candidates) || value.candidates.length > 8)
      throw new Error("Invalid candidates");
    const seen = new Set<string>();
    for (const candidate of value.candidates) {
      object(candidate, ["candidateId", "turnId", "contentHash"]);
      if (
        !id(candidate.candidateId) ||
        !id(candidate.turnId) ||
        typeof candidate.contentHash !== "string" ||
        !hash.test(candidate.contentHash) ||
        seen.has(candidate.candidateId)
      )
        throw new Error("Invalid candidate");
      seen.add(candidate.candidateId);
    }
  }
  return {
    schemaVersion: 1,
    items,
    nextCursor: value.nextCursor as string | null,
    ...(hasCandidates ? { candidates: value.candidates as ChapterCandidateActivity[] } : {}),
  };
}
export function parseChangeDetail(value: unknown): ChangeDetail {
  object(value, ["activity", "files"]);
  const activity = parseChangeActivity(value.activity);
  if (!Array.isArray(value.files) || value.files.length !== activity.receipt.files.length)
    throw new Error("Invalid diff files");
  let bytes = 0;
  const files = value.files.map((file, index) => {
    object(file, ["relativePath", "lines"]);
    if (
      file.relativePath !== activity.receipt.files[index].relativePath ||
      !Array.isArray(file.lines) ||
      file.lines.length > (String(file.relativePath).startsWith("chapters/") ? 65536 : 16384)
    )
      throw new Error("Invalid diff target");
    const lines = file.lines.map((line) => {
      object(line, ["kind", "text"]);
      if (
        !["context", "removed", "added"].includes(String(line.kind)) ||
        typeof line.text !== "string"
      )
        throw new Error("Invalid diff line");
      bytes += new TextEncoder().encode(line.text).length;
      return line as { kind: "context" | "removed" | "added"; text: string };
    });
    return { relativePath: file.relativePath as string, lines };
  });
  if (bytes > 64 * 1024) throw new Error("Oversized diff");
  return { activity, files };
}
async function request<T>(
  command: string,
  args: Record<string, unknown>,
  parse: (value: unknown) => T,
  call: Invoke,
): Promise<T> {
  let value: unknown;
  try {
    value = await call(command, args);
  } catch (error: unknown) {
    throw normalizeAppError(error);
  }
  try {
    return parse(value);
  } catch {
    throw { code: "contract.invalid_response", messageKey: "errors.invalidRuntimeResponse" };
  }
}
export function requestCanonChanges(sessionId: string, call: Invoke = nativeInvoke) {
  return requestCanonChangesPage(sessionId, null, null, call).then((page) => page.items);
}
export function requestCanonChangesPage(
  sessionId: string,
  cursor: string | null = null,
  turnId: string | null = null,
  call: Invoke = nativeInvoke,
  conversationSessionId: string | null = null,
) {
  return request(
    CANON_CHANGE_LIST_COMMAND,
    {
      request: { projectSessionId: sessionId, turnId, cursor, limit: 100 },
      ...(conversationSessionId === null ? {} : { conversationSessionId }),
    },
    parseChangePage,
    call,
  );
}
export function requestCanonChangeDetail(
  sessionId: string,
  changeId: string,
  call: Invoke = nativeInvoke,
  conversationSessionId: string | null = null,
) {
  return request(
    CANON_CHANGE_DETAIL_COMMAND,
    { sessionId, changeId, ...(conversationSessionId === null ? {} : { conversationSessionId }) },
    parseChangeDetail,
    call,
  );
}
export function requestCanonChangeDecision(
  sessionId: string,
  receipt: ChangeReceipt,
  decision: "confirm" | "discard" | "undo" | "retry_recovery",
  call: Invoke = nativeInvoke,
  conversationSessionId: string | null = null,
) {
  return request(
    CANON_CHANGE_DECIDE_COMMAND,
    {
      ...(conversationSessionId === null ? {} : { conversationSessionId }),
      request: {
        projectSessionId: sessionId,
        changeId: receipt.changeId,
        revision: receipt.revision,
        decision,
      },
    },
    parseChangeReceipt,
    call,
  );
}
export function requestEditorState(
  sessionId: string,
  fileId: string,
  nextRevision: number,
  dirty: boolean,
  call: Invoke = nativeInvoke,
) {
  return request(
    CANON_EDITOR_STATE_COMMAND,
    { request: { projectSessionId: sessionId, fileId, revision: nextRevision, dirty } },
    (value) => {
      if (value !== nextRevision || !revision(value)) throw new Error("Invalid editor revision");
      return value;
    },
    call,
  );
}
