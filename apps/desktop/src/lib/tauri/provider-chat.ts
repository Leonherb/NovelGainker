import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { normalizeAppError, parseAppError, type AppError } from "./health";
import type { ProviderId } from "./provider";

export const PROVIDER_CHAT_START_COMMAND = "provider_chat_start";
export const PROVIDER_CHAT_CANCEL_COMMAND = "provider_chat_cancel";
export const PROVIDER_RUN_EVENT_CHANNEL = "provider-run-event";

export type ProviderChatMessage = {
  role: "user" | "assistant";
  content: string;
};

export type ProviderRunStarted = {
  runId: string;
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

type ProviderRunEventBase = ProviderRunStarted & {
  sequence: number;
};

export type ProviderRunEvent =
  | (ProviderRunEventBase & { kind: "textDelta"; delta: string })
  | (ProviderRunEventBase & { kind: "usage"; usage: ProviderUsage })
  | (ProviderRunEventBase & {
      kind: "completed";
      finishReason: ProviderFinishReason;
    })
  | (ProviderRunEventBase & { kind: "failed"; error: AppError })
  | (ProviderRunEventBase & { kind: "cancelled" });

export type ProviderRunCancelResult = {
  cancelRequested: true;
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
const FINISH_REASONS: readonly ProviderFinishReason[] = [
  "stop",
  "length",
  "contentFilter",
  "toolCall",
  "resourceLimit",
  "unknown",
];
const STARTED_KEYS = ["model", "providerId", "runId"] as const;
const BASE_EVENT_KEYS = ["kind", "model", "providerId", "runId", "sequence"] as const;
const USAGE_KEYS = ["inputTokens", "outputTokens", "totalTokens"] as const;
const RUN_ID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function isProviderId(value: unknown): value is ProviderId {
  return value === "deepseek" || value === "openai";
}

function isSafeRunId(value: unknown): value is string {
  return typeof value === "string" && RUN_ID_PATTERN.test(value);
}

function isSafeSequence(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) > 0;
}

function isSafeCount(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0;
}

function hasValidFrozenIdentity(value: Record<string, unknown>): boolean {
  return (
    isSafeRunId(value.runId) &&
    isProviderId(value.providerId) &&
    value.model === PROVIDER_MODELS[value.providerId]
  );
}

export function parseProviderRunStarted(value: unknown): ProviderRunStarted {
  if (!isRecord(value) || !hasExactKeys(value, STARTED_KEYS) || !hasValidFrozenIdentity(value)) {
    throw new Error("Invalid Provider run start contract");
  }
  return value as ProviderRunStarted;
}

function parseUsage(value: unknown): ProviderUsage | null {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, USAGE_KEYS) ||
    !isSafeCount(value.inputTokens) ||
    !isSafeCount(value.outputTokens) ||
    !isSafeCount(value.totalTokens) ||
    value.totalTokens < value.inputTokens ||
    value.totalTokens < value.outputTokens
  ) {
    return null;
  }
  return value as ProviderUsage;
}

export function parseProviderRunEvent(value: unknown): ProviderRunEvent {
  if (
    !isRecord(value) ||
    !hasValidFrozenIdentity(value) ||
    !isSafeSequence(value.sequence) ||
    typeof value.kind !== "string"
  ) {
    throw new Error("Invalid Provider run event contract");
  }

  if (
    value.kind === "textDelta" &&
    hasExactKeys(value, [...BASE_EVENT_KEYS, "delta"]) &&
    typeof value.delta === "string" &&
    value.delta.length > 0
  ) {
    return value as ProviderRunEvent;
  }

  if (value.kind === "usage" && hasExactKeys(value, [...BASE_EVENT_KEYS, "usage"])) {
    const usage = parseUsage(value.usage);
    if (usage) {
      return { ...(value as ProviderRunEventBase), kind: "usage", usage };
    }
  }

  if (
    value.kind === "completed" &&
    hasExactKeys(value, [...BASE_EVENT_KEYS, "finishReason"]) &&
    typeof value.finishReason === "string" &&
    FINISH_REASONS.includes(value.finishReason as ProviderFinishReason)
  ) {
    return value as ProviderRunEvent;
  }

  if (value.kind === "failed" && hasExactKeys(value, [...BASE_EVENT_KEYS, "error"])) {
    const error = parseAppError(value.error);
    if (error) {
      return { ...(value as ProviderRunEventBase), kind: "failed", error };
    }
  }

  if (value.kind === "cancelled" && hasExactKeys(value, BASE_EVENT_KEYS)) {
    return value as ProviderRunEvent;
  }

  throw new Error("Invalid Provider run event contract");
}

export function parseProviderRunCancelResult(value: unknown): ProviderRunCancelResult {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["cancelRequested"]) ||
    value.cancelRequested !== true
  ) {
    throw new Error("Invalid Provider run cancellation contract");
  }
  return { cancelRequested: true };
}

const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);
const listenTauri: ListenEvent = (channel, handler) => listen<unknown>(channel, handler);

async function invokeProviderChat<T>(
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

export function requestProviderChatStart(
  expectedProviderId: ProviderId,
  messages: ProviderChatMessage[],
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProviderRunStarted> {
  return invokeProviderChat(
    PROVIDER_CHAT_START_COMMAND,
    { expectedProviderId, messages },
    parseProviderRunStarted,
    invokeCommand,
  );
}

export function requestProviderChatCancel(
  runId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProviderRunCancelResult> {
  return invokeProviderChat(
    PROVIDER_CHAT_CANCEL_COMMAND,
    { runId },
    parseProviderRunCancelResult,
    invokeCommand,
  );
}

export async function listenProviderRunEvents(
  onEvent: (event: ProviderRunEvent) => void,
  listenEvent: ListenEvent = listenTauri,
): Promise<UnlistenFn> {
  return listenEvent(PROVIDER_RUN_EVENT_CHANNEL, ({ payload }) => {
    try {
      onEvent(parseProviderRunEvent(payload));
    } catch {
      // Strict boundary: malformed or forward-incompatible events never enter UI state.
    }
  });
}
