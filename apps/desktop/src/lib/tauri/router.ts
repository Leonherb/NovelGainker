import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { normalizeAppError, parseAppError, type AppError } from "./health";
import type { ProviderId } from "./provider";
import type { ProviderRunCancelResult, ProviderUsage } from "./provider-chat";
import { PROJECT_CREATE_WORKFLOW_ID } from "./workflow";
import type { WorkflowId } from "./workflow-run";

export const ROUTER_RESOLVE_LOCAL_COMMAND = "router_resolve_local";
export const ROUTER_RUN_START_COMMAND = "router_run_start";
export const ROUTER_RUN_CANCEL_COMMAND = "router_run_cancel";
export const ROUTER_RUN_EVENT_CHANNEL = "router-run-event";

export type RoutableWorkflowId = WorkflowId | typeof PROJECT_CREATE_WORKFLOW_ID;
export type RouterEntryMode = "default" | "character-create" | "character-enhance";
export type LocalRouterOutcome = "direct" | "needsProvider" | "multiple" | "unsupported";
export type RouterConfidence = "high" | "medium" | "low";

export type LocalRouterDecision = {
  schemaVersion: 1;
  outcome: LocalRouterOutcome;
  workflowId: RoutableWorkflowId | null;
  entryMode: RouterEntryMode | null;
};

export type RouterDecision = {
  schemaVersion: 1;
  workflowId: RoutableWorkflowId;
  confidence: RouterConfidence;
};

export type RouterRunStarted = {
  runId: string;
  providerId: ProviderId;
  model: string;
};

type RouterRunIdentity = RouterRunStarted;
export type RouterRunEvent =
  | (RouterRunIdentity & {
      kind: "completed";
      decision: RouterDecision;
      usage: ProviderUsage | null;
    })
  | (RouterRunIdentity & { kind: "failed"; error: AppError })
  | (RouterRunIdentity & { kind: "cancelled" });

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
type ListenEvent = (
  channel: string,
  handler: (event: { payload: unknown }) => void,
) => Promise<UnlistenFn>;

const WORKFLOW_IDS: readonly RoutableWorkflowId[] = [
  "novelgainker.character-create",
  "novelgainker.project-create",
  "novelgainker.setting-create",
  "novelgainker.material-create",
  "novelgainker.plot-create",
];
const OUTCOMES: readonly LocalRouterOutcome[] = [
  "direct",
  "needsProvider",
  "multiple",
  "unsupported",
];
const ENTRY_MODES: readonly RouterEntryMode[] = [
  "default",
  "character-create",
  "character-enhance",
];
const CONFIDENCES: readonly RouterConfidence[] = ["high", "medium", "low"];
const PROVIDER_MODELS: Record<ProviderId, string> = {
  deepseek: "deepseek-v4-pro",
  openai: "gpt-5.6-terra",
};
const RUN_ID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function isWorkflowId(value: unknown): value is RoutableWorkflowId {
  return typeof value === "string" && WORKFLOW_IDS.includes(value as RoutableWorkflowId);
}

function isProviderId(value: unknown): value is ProviderId {
  return value === "deepseek" || value === "openai";
}

function hasValidIdentity(value: Record<string, unknown>): boolean {
  return (
    typeof value.runId === "string" &&
    RUN_ID_PATTERN.test(value.runId) &&
    isProviderId(value.providerId) &&
    value.model === PROVIDER_MODELS[value.providerId]
  );
}

function parseUsage(value: unknown): ProviderUsage | null | undefined {
  if (value === null) return null;
  if (!isRecord(value) || !hasExactKeys(value, ["inputTokens", "outputTokens", "totalTokens"])) {
    return undefined;
  }
  const values = [value.inputTokens, value.outputTokens, value.totalTokens];
  if (
    values.some((count) => !Number.isSafeInteger(count) || Number(count) < 0) ||
    Number(value.totalTokens) < Number(value.inputTokens) ||
    Number(value.totalTokens) < Number(value.outputTokens)
  ) {
    return undefined;
  }
  return value as ProviderUsage;
}

export function parseLocalRouterDecision(value: unknown): LocalRouterDecision {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["entryMode", "outcome", "schemaVersion", "workflowId"]) ||
    value.schemaVersion !== 1 ||
    typeof value.outcome !== "string" ||
    !OUTCOMES.includes(value.outcome as LocalRouterOutcome)
  ) {
    throw new Error("Invalid local Router decision");
  }
  if (value.outcome === "direct") {
    if (
      !isWorkflowId(value.workflowId) ||
      typeof value.entryMode !== "string" ||
      !ENTRY_MODES.includes(value.entryMode as RouterEntryMode)
    ) {
      throw new Error("Invalid direct Router decision");
    }
  } else if (value.workflowId !== null || value.entryMode !== null) {
    throw new Error("Invalid non-direct Router decision");
  }
  return value as LocalRouterDecision;
}

export function parseRouterDecision(value: unknown): RouterDecision {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["confidence", "schemaVersion", "workflowId"]) ||
    value.schemaVersion !== 1 ||
    !isWorkflowId(value.workflowId) ||
    typeof value.confidence !== "string" ||
    !CONFIDENCES.includes(value.confidence as RouterConfidence)
  ) {
    throw new Error("Invalid Router decision");
  }
  return value as RouterDecision;
}

export function parseRouterRunStarted(value: unknown): RouterRunStarted {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["model", "providerId", "runId"]) ||
    !hasValidIdentity(value)
  ) {
    throw new Error("Invalid Router run start");
  }
  return value as RouterRunStarted;
}

export function parseRouterRunEvent(value: unknown): RouterRunEvent {
  if (!isRecord(value) || !hasValidIdentity(value) || typeof value.kind !== "string") {
    throw new Error("Invalid Router run event");
  }
  const baseKeys = ["kind", "model", "providerId", "runId"] as const;
  if (value.kind === "completed" && hasExactKeys(value, [...baseKeys, "decision", "usage"])) {
    const decision = parseRouterDecision(value.decision);
    const usage = parseUsage(value.usage);
    if (usage !== undefined) {
      return { ...(value as RouterRunIdentity), kind: "completed", decision, usage };
    }
  }
  if (value.kind === "failed" && hasExactKeys(value, [...baseKeys, "error"])) {
    const error = parseAppError(value.error);
    if (error) return { ...(value as RouterRunIdentity), kind: "failed", error };
  }
  if (value.kind === "cancelled" && hasExactKeys(value, baseKeys)) {
    return value as RouterRunEvent;
  }
  throw new Error("Invalid Router run event");
}

function parseCancel(value: unknown): ProviderRunCancelResult {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["cancelRequested"]) ||
    value.cancelRequested !== true
  ) {
    throw new Error("Invalid Router cancellation");
  }
  return { cancelRequested: true };
}

const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);
const listenTauri: ListenEvent = (channel, handler) => listen<unknown>(channel, handler);

async function invokeRouter<T>(
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

export function requestLocalRouterDecision(
  text: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<LocalRouterDecision> {
  return invokeRouter(
    ROUTER_RESOLVE_LOCAL_COMMAND,
    { text },
    parseLocalRouterDecision,
    invokeCommand,
  );
}

export function requestRouterRunStart(
  expectedProviderId: ProviderId,
  text: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<RouterRunStarted> {
  return invokeRouter(
    ROUTER_RUN_START_COMMAND,
    { expectedProviderId, text },
    parseRouterRunStarted,
    invokeCommand,
  );
}

export function requestRouterRunCancel(
  runId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProviderRunCancelResult> {
  return invokeRouter(ROUTER_RUN_CANCEL_COMMAND, { runId }, parseCancel, invokeCommand);
}

export async function listenRouterRunEvents(
  onEvent: (event: RouterRunEvent) => void,
  listenEvent: ListenEvent = listenTauri,
): Promise<UnlistenFn> {
  return listenEvent(ROUTER_RUN_EVENT_CHANNEL, ({ payload }) => {
    try {
      onEvent(parseRouterRunEvent(payload));
    } catch {
      // Strict boundary: malformed events never enter UI state.
    }
  });
}
