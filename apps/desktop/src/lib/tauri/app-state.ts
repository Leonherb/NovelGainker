import { invoke } from "@tauri-apps/api/core";
import { normalizeAppError, type AppError } from "./health";

export const APP_STATE_LOAD_COMMAND = "app_state_load";
export const APP_STATE_SAVE_UI_COMMAND = "app_state_save_ui";
export const APP_STATE_SET_CURRENT_FILE_COMMAND = "app_state_set_current_file";

export type UiPreferences = {
  leftCollapsed: boolean;
  rightCollapsed: boolean;
  apiCardExpanded: boolean;
  rightPanelWidth: number | null;
};

export type RecentProject = {
  projectId: string;
  displayName: string;
};

export type ProjectFilePreference = {
  projectId: string;
  currentRelativeFile: string;
};

export type AppStateSnapshot = {
  schemaVersion: 3;
  ui: UiPreferences;
  recentProjects: RecentProject[];
  projectFiles: ProjectFilePreference[];
  recoveredWithDefaults: boolean;
};

export type AppStateWriteResult = {
  saved: true;
};

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu;
const SAFE_RELATIVE_PATH = /^[\p{L}\p{N} ._/-]{1,180}$/u;
const UI_KEYS = ["apiCardExpanded", "leftCollapsed", "rightCollapsed", "rightPanelWidth"] as const;
const SNAPSHOT_KEYS = [
  "projectFiles",
  "recentProjects",
  "recoveredWithDefaults",
  "schemaVersion",
  "ui",
] as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function isSafeDisplayName(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.trim().length > 0 &&
    value.length <= 80 &&
    !/[\\/]/u.test(value) &&
    [...value].every((character) => {
      const codePoint = character.codePointAt(0) ?? 0;
      return codePoint > 31 && codePoint !== 127;
    })
  );
}

function isSafeRelativePath(value: unknown): value is string {
  return (
    typeof value === "string" &&
    SAFE_RELATIVE_PATH.test(value) &&
    !value.startsWith("/") &&
    !value.includes("\\") &&
    !value.includes(":") &&
    value
      .split("/")
      .every((component) => component !== "" && component !== "." && component !== "..")
  );
}

function parseUiPreferences(value: unknown): UiPreferences {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, UI_KEYS) ||
    typeof value.leftCollapsed !== "boolean" ||
    typeof value.rightCollapsed !== "boolean" ||
    typeof value.apiCardExpanded !== "boolean" ||
    !(
      value.rightPanelWidth === null ||
      (Number.isSafeInteger(value.rightPanelWidth) &&
        (value.rightPanelWidth as number) >= 278 &&
        (value.rightPanelWidth as number) <= 1_600)
    )
  ) {
    throw new Error("Invalid UI preferences contract");
  }
  return value as UiPreferences;
}

function parseRecentProject(value: unknown): RecentProject {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["displayName", "projectId"]) ||
    typeof value.projectId !== "string" ||
    !UUID.test(value.projectId) ||
    !isSafeDisplayName(value.displayName)
  ) {
    throw new Error("Invalid recent project contract");
  }
  return value as RecentProject;
}

function parseProjectFile(value: unknown): ProjectFilePreference {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["currentRelativeFile", "projectId"]) ||
    typeof value.projectId !== "string" ||
    !UUID.test(value.projectId) ||
    !isSafeRelativePath(value.currentRelativeFile)
  ) {
    throw new Error("Invalid project file preference contract");
  }
  return value as ProjectFilePreference;
}

export function parseAppStateSnapshot(value: unknown): AppStateSnapshot {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, SNAPSHOT_KEYS) ||
    value.schemaVersion !== 3 ||
    typeof value.recoveredWithDefaults !== "boolean" ||
    !Array.isArray(value.recentProjects) ||
    value.recentProjects.length > 12 ||
    !Array.isArray(value.projectFiles) ||
    value.projectFiles.length > 12
  ) {
    throw new Error("Invalid App State snapshot contract");
  }
  const recentProjects = value.recentProjects.map(parseRecentProject);
  const projectFiles = value.projectFiles.map(parseProjectFile);
  const projectIds = new Set(recentProjects.map((project) => project.projectId));
  if (
    projectIds.size !== recentProjects.length ||
    new Set(projectFiles.map((file) => file.projectId)).size !== projectFiles.length ||
    projectFiles.some((file) => !projectIds.has(file.projectId))
  ) {
    throw new Error("Invalid App State project relationships");
  }
  return {
    schemaVersion: 3,
    ui: parseUiPreferences(value.ui),
    recentProjects,
    projectFiles,
    recoveredWithDefaults: value.recoveredWithDefaults,
  };
}

function parseWriteResult(value: unknown): AppStateWriteResult {
  if (!isRecord(value) || !hasExactKeys(value, ["saved"]) || value.saved !== true) {
    throw new Error("Invalid App State write contract");
  }
  return { saved: true };
}

const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);

async function invokeAppState<T>(
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

export function requestAppState(
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<AppStateSnapshot> {
  return invokeAppState(APP_STATE_LOAD_COMMAND, {}, parseAppStateSnapshot, invokeCommand);
}

export function requestSaveUiPreferences(
  preferences: UiPreferences,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<AppStateWriteResult> {
  return invokeAppState(
    APP_STATE_SAVE_UI_COMMAND,
    { preferences },
    parseWriteResult,
    invokeCommand,
  );
}

export function requestSetCurrentFile(
  sessionId: string,
  relativePath: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<AppStateWriteResult> {
  return invokeAppState(
    APP_STATE_SET_CURRENT_FILE_COMMAND,
    { sessionId, relativePath },
    parseWriteResult,
    invokeCommand,
  );
}
