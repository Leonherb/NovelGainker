import { invoke } from "@tauri-apps/api/core";

export const HEALTH_CHECK_COMMAND = "health_check";
export const HEALTH_CONTRACT_VERSION = 1;

export type HealthPlatform = "windows" | "android";

export type HealthStatus = {
  application: "NovelGainker";
  version: string;
  platform: HealthPlatform;
  status: "ready";
  contractVersion: typeof HEALTH_CONTRACT_VERSION;
};

export type AppError = {
  code: string;
  messageKey: string;
  diagnostic?: string;
};

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const HEALTH_STATUS_KEYS = [
  "application",
  "contractVersion",
  "platform",
  "status",
  "version",
] as const;
const APP_ERROR_REQUIRED_KEYS = ["code", "messageKey"] as const;
const APP_ERROR_KEYS = [...APP_ERROR_REQUIRED_KEYS, "diagnostic"] as const;
const CONTRACT_TOKEN = /^[a-zA-Z0-9._-]{1,80}$/u;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasOnlyKeys(value: Record<string, unknown>, allowedKeys: readonly string[]): boolean {
  const keys = Object.keys(value).sort();
  return (
    keys.length === allowedKeys.length &&
    keys.every((key, index) => key === [...allowedKeys].sort()[index])
  );
}

function hasOnlyOptionalDiagnostic(value: Record<string, unknown>): boolean {
  const keys = Object.keys(value);
  return (
    APP_ERROR_REQUIRED_KEYS.every((key) => keys.includes(key)) &&
    keys.every((key) => APP_ERROR_KEYS.includes(key as (typeof APP_ERROR_KEYS)[number]))
  );
}

function isSafeContractToken(value: unknown): value is string {
  return typeof value === "string" && CONTRACT_TOKEN.test(value);
}

export function parseHealthStatus(value: unknown): HealthStatus {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, HEALTH_STATUS_KEYS) ||
    value.application !== "NovelGainker" ||
    typeof value.version !== "string" ||
    value.version.length === 0 ||
    (value.platform !== "windows" && value.platform !== "android") ||
    value.status !== "ready" ||
    value.contractVersion !== HEALTH_CONTRACT_VERSION
  ) {
    throw new Error("Invalid health contract");
  }

  return value as HealthStatus;
}

export function parseAppError(value: unknown): AppError | null {
  if (
    !isRecord(value) ||
    !hasOnlyOptionalDiagnostic(value) ||
    !isSafeContractToken(value.code) ||
    !isSafeContractToken(value.messageKey) ||
    (value.diagnostic !== undefined && !isSafeContractToken(value.diagnostic))
  ) {
    return null;
  }

  return {
    code: value.code,
    messageKey: value.messageKey,
    ...(value.diagnostic === undefined ? {} : { diagnostic: value.diagnostic as string }),
  };
}

export function normalizeAppError(value: unknown): AppError {
  return (
    parseAppError(value) ?? {
      code: "runtime.unknown",
      messageKey: "errors.unknown",
    }
  );
}

const invokeTauri: InvokeCommand = (command) => invoke<unknown>(command);

export async function requestHealthStatus(
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<HealthStatus> {
  let response: unknown;

  try {
    response = await invokeCommand(HEALTH_CHECK_COMMAND);
  } catch (error: unknown) {
    throw normalizeAppError(error);
  }

  try {
    return parseHealthStatus(response);
  } catch {
    throw {
      code: "contract.invalid_response",
      messageKey: "errors.invalidRuntimeResponse",
    } satisfies AppError;
  }
}

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
