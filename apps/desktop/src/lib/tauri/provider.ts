import { invoke } from "@tauri-apps/api/core";
import { normalizeAppError, type AppError } from "./health";

export const PROVIDER_STATUS_LOAD_COMMAND = "provider_status_load";
export const PROVIDER_KEY_SAVE_COMMAND = "provider_key_save";
export const PROVIDER_KEY_DELETE_COMMAND = "provider_key_delete";
export const PROVIDER_SELECT_COMMAND = "provider_select";
export const PROVIDER_CONNECTION_TEST_COMMAND = "provider_connection_test";

export type ProviderId = "deepseek" | "openai";

export type ProviderConnectionStatus =
  | "unconfigured"
  | "untested"
  | "connected"
  | "authenticationFailed"
  | "unreachable"
  | "vaultUnavailable"
  | "recoveryRequired";

export type ProviderStatus = {
  providerId: ProviderId;
  configured: boolean;
  connectionStatus: ProviderConnectionStatus;
  presetVersion: 1;
  selected: boolean;
};

export type ProviderStateSnapshot = {
  schemaVersion: 2;
  providers: ProviderStatus[];
  selectedProviderId: ProviderId;
};

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const PROVIDER_IDS: readonly ProviderId[] = ["deepseek", "openai"];
const CONNECTION_STATUSES: readonly ProviderConnectionStatus[] = [
  "unconfigured",
  "untested",
  "connected",
  "authenticationFailed",
  "unreachable",
  "vaultUnavailable",
  "recoveryRequired",
];
const STATUS_KEYS = [
  "configured",
  "connectionStatus",
  "presetVersion",
  "providerId",
  "selected",
] as const;
const SNAPSHOT_KEYS = ["providers", "schemaVersion", "selectedProviderId"] as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function isProviderId(value: unknown): value is ProviderId {
  return typeof value === "string" && PROVIDER_IDS.includes(value as ProviderId);
}

function isConnectionStatus(value: unknown): value is ProviderConnectionStatus {
  return (
    typeof value === "string" && CONNECTION_STATUSES.includes(value as ProviderConnectionStatus)
  );
}

export function parseProviderStatus(value: unknown): ProviderStatus {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, STATUS_KEYS) ||
    !isProviderId(value.providerId) ||
    typeof value.configured !== "boolean" ||
    !isConnectionStatus(value.connectionStatus) ||
    value.presetVersion !== 1 ||
    typeof value.selected !== "boolean"
  ) {
    throw new Error("Invalid Provider status contract");
  }
  const configuredStatuses: readonly ProviderConnectionStatus[] = [
    "untested",
    "connected",
    "authenticationFailed",
    "unreachable",
  ];
  if (value.configured !== configuredStatuses.includes(value.connectionStatus)) {
    throw new Error("Invalid Provider configured state");
  }
  return value as ProviderStatus;
}

export function parseProviderStateSnapshot(value: unknown): ProviderStateSnapshot {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, SNAPSHOT_KEYS) ||
    value.schemaVersion !== 2 ||
    !isProviderId(value.selectedProviderId) ||
    !Array.isArray(value.providers) ||
    value.providers.length !== 2
  ) {
    throw new Error("Invalid Provider state contract");
  }
  const providers = value.providers.map(parseProviderStatus);
  const ids = providers.map((provider) => provider.providerId);
  if (
    new Set(ids).size !== 2 ||
    !PROVIDER_IDS.every((providerId) => ids.includes(providerId)) ||
    providers.filter((provider) => provider.selected).length !== 1 ||
    !providers.some(
      (provider) => provider.selected && provider.providerId === value.selectedProviderId,
    )
  ) {
    throw new Error("Invalid Provider state relationships");
  }
  return {
    schemaVersion: 2,
    providers,
    selectedProviderId: value.selectedProviderId,
  };
}

const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);

async function invokeProvider<T>(
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

export function requestProviderStatus(
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProviderStateSnapshot> {
  return invokeProvider(
    PROVIDER_STATUS_LOAD_COMMAND,
    {},
    parseProviderStateSnapshot,
    invokeCommand,
  );
}

export function requestSaveProviderKey(
  providerId: ProviderId,
  key: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProviderStatus> {
  return invokeProvider(
    PROVIDER_KEY_SAVE_COMMAND,
    { providerId, key },
    parseProviderStatus,
    invokeCommand,
  );
}

export function requestSelectProvider(
  providerId: ProviderId,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProviderStatus> {
  return invokeProvider(
    PROVIDER_SELECT_COMMAND,
    { providerId },
    parseProviderStatus,
    invokeCommand,
  );
}

export function requestProviderConnectionTest(
  providerId: ProviderId,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProviderStatus> {
  return invokeProvider(
    PROVIDER_CONNECTION_TEST_COMMAND,
    { providerId },
    parseProviderStatus,
    invokeCommand,
  );
}

export function requestDeleteProviderKey(
  providerId: ProviderId,
  confirmed: boolean,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<ProviderStatus> {
  return invokeProvider(
    PROVIDER_KEY_DELETE_COMMAND,
    { providerId, confirmed },
    parseProviderStatus,
    invokeCommand,
  );
}
