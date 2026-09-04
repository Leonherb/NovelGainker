import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { normalizeAppError, parseAppError, type AppError } from "./health";
import type { ProviderId } from "./provider";

export const CONVERSATION_SESSION_LOAD_COMMAND = "conversation_session_load";
export const CONVERSATION_TURN_START_COMMAND = "conversation_turn_start";
export const CONVERSATION_TURN_CANCEL_COMMAND = "conversation_turn_cancel";
export const CONVERSATION_TURN_EVENT_CHANNEL = "conversation-turn-event";

export type ConversationRole = "user" | "assistant";
export type ConversationTurnStatus =
  "running" | "completed" | "cancelled" | "failed" | "not_generated";

export type ConversationMessage = {
  messageId: string;
  role: ConversationRole;
  content: string;
  createdAtMs: number;
};

export type ConversationTurn = {
  turnId: string;
  providerId: ProviderId;
  model: string;
  status: ConversationTurnStatus;
  startedAtMs: number;
  updatedAtMs: number;
  messages: ConversationMessage[];
  errorCode?: string;
};

export type ConversationSession = {
  schemaVersion: 1;
  projectId: string;
  sessionId: string;
  turns: ConversationTurn[];
};

export type TaskStatus = "queued" | "running" | "succeeded" | "failed" | "cancelled";

export type TaskActivity = {
  taskId: string;
  sessionId: string;
  turnId: string;
  status: TaskStatus;
  displayLabel: string;
  taskRevision: number;
};

export function mergeTaskActivity(current: TaskActivity[], incoming: TaskActivity): TaskActivity[] {
  const index = current.findIndex((activity) => activity.taskId === incoming.taskId);
  if (index < 0) return [...current, incoming];
  const existing = current[index];
  if (incoming.taskRevision <= existing.taskRevision) return current;
  return current.map((activity, activityIndex) => (activityIndex === index ? incoming : activity));
}

export type ConversationRuntimeView = {
  schemaVersion: 1;
  conversation: ConversationSession;
  taskActivities: TaskActivity[];
};

export type ConversationTurnStarted = {
  sessionId: string;
  conversationSessionId: string;
  turnId: string;
  providerId: ProviderId;
  model: string;
};

export type ProviderUsage = {
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
};

export type ProviderFinishReason =
  "stop" | "length" | "contentFilter" | "toolCall" | "resourceLimit" | "unknown";

type ConversationEventBase = {
  sessionId: string;
  turnId: string;
  providerId: ProviderId;
  model: string;
  sequence: number;
};

export type ConversationTurnEvent =
  | (ConversationEventBase & { kind: "textDelta"; delta: string })
  | (ConversationEventBase & { kind: "usage"; usage: ProviderUsage })
  | (ConversationEventBase & { kind: "taskActivity"; taskActivity: TaskActivity })
  | (ConversationEventBase & {
      kind: "completed";
      finishReason: ProviderFinishReason;
    })
  | (ConversationEventBase & { kind: "failed"; error: AppError })
  | (ConversationEventBase & { kind: "cancelled" });

export type ConversationTurnCancelResult = {
  cancelRequested: boolean;
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
const TURN_STATUSES: readonly ConversationTurnStatus[] = [
  "running",
  "completed",
  "cancelled",
  "failed",
  "not_generated",
];
const TASK_STATUSES: readonly TaskStatus[] = [
  "queued",
  "running",
  "succeeded",
  "failed",
  "cancelled",
];
const FINISH_REASONS: readonly ProviderFinishReason[] = [
  "stop",
  "length",
  "contentFilter",
  "toolCall",
  "resourceLimit",
  "unknown",
];
const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;
const SAFE_ERROR_CODE = /^[a-z0-9._-]{1,96}$/u;
const MESSAGE_KEYS = ["content", "createdAtMs", "messageId", "role"] as const;
const TURN_REQUIRED_KEYS = [
  "messages",
  "model",
  "providerId",
  "startedAtMs",
  "status",
  "turnId",
  "updatedAtMs",
] as const;
const SESSION_KEYS = ["projectId", "schemaVersion", "sessionId", "turns"] as const;
const RUNTIME_VIEW_KEYS = ["conversation", "schemaVersion", "taskActivities"] as const;
const TASK_ACTIVITY_KEYS = [
  "displayLabel",
  "sessionId",
  "status",
  "taskId",
  "taskRevision",
  "turnId",
] as const;
const START_KEYS = ["conversationSessionId", "model", "providerId", "sessionId", "turnId"] as const;
const EVENT_BASE_KEYS = ["kind", "model", "providerId", "sequence", "sessionId", "turnId"] as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function hasTurnKeys(value: Record<string, unknown>): boolean {
  const keys = Object.keys(value);
  return (
    TURN_REQUIRED_KEYS.every((key) => keys.includes(key)) &&
    keys.every((key) => key === "errorCode" || TURN_REQUIRED_KEYS.includes(key as never))
  );
}

function isUuid(value: unknown): value is string {
  return typeof value === "string" && UUID_PATTERN.test(value);
}

function isProviderId(value: unknown): value is ProviderId {
  return value === "deepseek" || value === "openai";
}

function hasProviderIdentity(value: Record<string, unknown>): boolean {
  return isProviderId(value.providerId) && value.model === PROVIDER_MODELS[value.providerId];
}

function isSafeTime(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) > 0;
}

function isSafeSequence(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) > 0;
}

function parseMessage(value: unknown): ConversationMessage {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, MESSAGE_KEYS) ||
    !isUuid(value.messageId) ||
    (value.role !== "user" && value.role !== "assistant") ||
    typeof value.content !== "string" ||
    value.content.trim().length === 0 ||
    !isSafeTime(value.createdAtMs)
  ) {
    throw new Error("Invalid Conversation message contract");
  }
  return value as ConversationMessage;
}

function parseTurn(value: unknown): ConversationTurn {
  if (
    !isRecord(value) ||
    !hasTurnKeys(value) ||
    !isUuid(value.turnId) ||
    !hasProviderIdentity(value) ||
    typeof value.status !== "string" ||
    !TURN_STATUSES.includes(value.status as ConversationTurnStatus) ||
    !isSafeTime(value.startedAtMs) ||
    !isSafeTime(value.updatedAtMs) ||
    value.updatedAtMs < value.startedAtMs ||
    !Array.isArray(value.messages) ||
    (value.errorCode !== undefined &&
      (typeof value.errorCode !== "string" || !SAFE_ERROR_CODE.test(value.errorCode)))
  ) {
    throw new Error("Invalid Conversation turn contract");
  }
  const messages = value.messages.map(parseMessage);
  if (
    messages[0]?.role !== "user" ||
    (value.status === "running" && messages.length !== 1) ||
    (value.status !== "running" && (messages.length !== 2 || messages[1]?.role !== "assistant"))
  ) {
    throw new Error("Invalid Conversation turn relationship");
  }
  return { ...(value as ConversationTurn), messages };
}

export function parseConversationSession(value: unknown): ConversationSession {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, SESSION_KEYS) ||
    value.schemaVersion !== 1 ||
    !isUuid(value.projectId) ||
    !isUuid(value.sessionId) ||
    !Array.isArray(value.turns)
  ) {
    throw new Error("Invalid Conversation session contract");
  }
  return { ...(value as ConversationSession), turns: value.turns.map(parseTurn) };
}

export function parseTaskActivity(value: unknown): TaskActivity {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, TASK_ACTIVITY_KEYS) ||
    !isUuid(value.taskId) ||
    !isUuid(value.sessionId) ||
    !isUuid(value.turnId) ||
    typeof value.status !== "string" ||
    !TASK_STATUSES.includes(value.status as TaskStatus) ||
    typeof value.displayLabel !== "string" ||
    value.displayLabel.trim().length === 0 ||
    value.displayLabel.length > 160 ||
    /[\0\r\n]/u.test(value.displayLabel) ||
    !isSafeSequence(value.taskRevision)
  ) {
    throw new Error("Invalid Task activity contract");
  }
  return value as TaskActivity;
}

export function parseConversationRuntimeView(value: unknown): ConversationRuntimeView {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, RUNTIME_VIEW_KEYS) ||
    value.schemaVersion !== 1 ||
    !Array.isArray(value.taskActivities)
  ) {
    throw new Error("Invalid Conversation runtime view contract");
  }
  const conversation = parseConversationSession(value.conversation);
  const taskActivities = value.taskActivities.map(parseTaskActivity);
  if (
    taskActivities.some(
      (activity) =>
        activity.sessionId !== conversation.sessionId ||
        !conversation.turns.some((turn) => turn.turnId === activity.turnId),
    ) ||
    new Set(taskActivities.map((activity) => activity.taskId)).size !== taskActivities.length
  ) {
    throw new Error("Invalid Conversation runtime view relationship");
  }
  return { schemaVersion: 1, conversation, taskActivities };
}

export function parseConversationTurnStarted(value: unknown): ConversationTurnStarted {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, START_KEYS) ||
    !isUuid(value.sessionId) ||
    !isUuid(value.conversationSessionId) ||
    !isUuid(value.turnId) ||
    !hasProviderIdentity(value)
  ) {
    throw new Error("Invalid Conversation turn start contract");
  }
  return value as ConversationTurnStarted;
}

function parseUsage(value: unknown): ProviderUsage | null {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["inputTokens", "outputTokens", "totalTokens"]) ||
    !Number.isSafeInteger(value.inputTokens) ||
    Number(value.inputTokens) < 0 ||
    !Number.isSafeInteger(value.outputTokens) ||
    Number(value.outputTokens) < 0 ||
    !Number.isSafeInteger(value.totalTokens) ||
    Number(value.totalTokens) < 0
  ) {
    return null;
  }
  return value as ProviderUsage;
}

export function parseConversationTurnEvent(value: unknown): ConversationTurnEvent {
  if (
    !isRecord(value) ||
    !isUuid(value.sessionId) ||
    !isUuid(value.turnId) ||
    !hasProviderIdentity(value) ||
    !isSafeSequence(value.sequence) ||
    typeof value.kind !== "string"
  ) {
    throw new Error("Invalid Conversation event contract");
  }
  if (
    value.kind === "textDelta" &&
    hasExactKeys(value, [...EVENT_BASE_KEYS, "delta"]) &&
    typeof value.delta === "string" &&
    value.delta.length > 0
  ) {
    return value as ConversationTurnEvent;
  }
  if (value.kind === "usage" && hasExactKeys(value, [...EVENT_BASE_KEYS, "usage"])) {
    const usage = parseUsage(value.usage);
    if (usage) return { ...(value as ConversationEventBase), kind: "usage", usage };
  }
  if (value.kind === "taskActivity" && hasExactKeys(value, [...EVENT_BASE_KEYS, "taskActivity"])) {
    const taskActivity = parseTaskActivity(value.taskActivity);
    if (taskActivity.turnId === value.turnId) {
      return { ...(value as ConversationEventBase), kind: "taskActivity", taskActivity };
    }
  }
  if (
    value.kind === "completed" &&
    hasExactKeys(value, [...EVENT_BASE_KEYS, "finishReason"]) &&
    typeof value.finishReason === "string" &&
    FINISH_REASONS.includes(value.finishReason as ProviderFinishReason)
  ) {
    return value as ConversationTurnEvent;
  }
  if (value.kind === "failed" && hasExactKeys(value, [...EVENT_BASE_KEYS, "error"])) {
    const error = parseAppError(value.error);
    if (error) return { ...(value as ConversationEventBase), kind: "failed", error };
  }
  if (value.kind === "cancelled" && hasExactKeys(value, EVENT_BASE_KEYS)) {
    return value as ConversationTurnEvent;
  }
  throw new Error("Invalid Conversation event contract");
}

export function parseConversationTurnCancelResult(value: unknown): ConversationTurnCancelResult {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["cancelRequested"]) ||
    typeof value.cancelRequested !== "boolean"
  ) {
    throw new Error("Invalid Conversation cancellation contract");
  }
  return { cancelRequested: value.cancelRequested };
}

const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);
const listenTauri: ListenEvent = (channel, handler) => listen<unknown>(channel, handler);

async function invokeConversation<T>(
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

export function requestConversationSession(
  sessionId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ConversationRuntimeView> {
  return invokeConversation(
    CONVERSATION_SESSION_LOAD_COMMAND,
    { sessionId },
    parseConversationRuntimeView,
    invokeCommand,
  );
}

export function requestConversationTurnStart(
  sessionId: string,
  message: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ConversationTurnStarted> {
  return invokeConversation(
    CONVERSATION_TURN_START_COMMAND,
    { sessionId, message },
    parseConversationTurnStarted,
    invokeCommand,
  );
}

export function requestConversationTurnCancel(
  sessionId: string,
  turnId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ConversationTurnCancelResult> {
  return invokeConversation(
    CONVERSATION_TURN_CANCEL_COMMAND,
    { sessionId, turnId },
    parseConversationTurnCancelResult,
    invokeCommand,
  );
}

export function listenConversationTurnEvents(
  onEvent: (event: ConversationTurnEvent) => void,
  listenEvent: ListenEvent = listenTauri,
): Promise<UnlistenFn> {
  return listenEvent(CONVERSATION_TURN_EVENT_CHANNEL, ({ payload }) => {
    try {
      onEvent(parseConversationTurnEvent(payload));
    } catch {
      // Strict boundary: malformed native events never enter visible conversation state.
    }
  });
}
