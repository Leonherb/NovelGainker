import { invoke } from "@tauri-apps/api/core";
import { normalizeAppError, type AppError } from "./health";

export const WORKFLOW_REGISTRY_LOAD_COMMAND = "workflow_registry_load";
export const WORKFLOW_REGISTRY_SCHEMA_VERSION = 1;
export const CHARACTER_CREATE_WORKFLOW_ID = "novelgainker.character-create";
export const PROJECT_CREATE_WORKFLOW_ID = "novelgainker.project-create";
export const SETTING_CREATE_WORKFLOW_ID = "novelgainker.setting-create";
export const MATERIAL_CREATE_WORKFLOW_ID = "novelgainker.material-create";
export const PLOT_CREATE_WORKFLOW_ID = "novelgainker.plot-create";
export const STORYTELLING_WORKFLOW_ID = "novelgainker.storytelling";

export type CharacterCreateWorkflowCatalogEntry = {
  id: typeof CHARACTER_CREATE_WORKFLOW_ID;
  version: 2;
  displayName: string;
  description: string;
  mode: "interactive-write";
  executionAvailable: true;
  routable: true;
};

export type ProjectCreateWorkflowCatalogEntry = {
  id: typeof PROJECT_CREATE_WORKFLOW_ID;
  version: 1;
  displayName: string;
  description: string;
  mode: "deterministic-write";
  executionAvailable: true;
  routable: true;
};

export type SettingCreateWorkflowCatalogEntry = {
  id: typeof SETTING_CREATE_WORKFLOW_ID;
  version: 1;
  displayName: string;
  description: string;
  mode: "interactive-write";
  executionAvailable: true;
  routable: true;
};

export type MaterialCreateWorkflowCatalogEntry = {
  id: typeof MATERIAL_CREATE_WORKFLOW_ID;
  version: 1;
  displayName: string;
  description: string;
  mode: "interactive-write";
  executionAvailable: true;
  routable: true;
};
export type PlotCreateWorkflowCatalogEntry = {
  id: typeof PLOT_CREATE_WORKFLOW_ID;
  version: 1;
  displayName: string;
  description: string;
  mode: "interactive-write";
  executionAvailable: true;
  routable: true;
};

export type StorytellingWorkflowCatalogEntry = {
  id: typeof STORYTELLING_WORKFLOW_ID;
  version: 1;
  displayName: string;
  description: string;
  mode: "interactive-write";
  executionAvailable: false;
  routable: false;
};

export type WorkflowCatalogEntry =
  | CharacterCreateWorkflowCatalogEntry
  | ProjectCreateWorkflowCatalogEntry
  | SettingCreateWorkflowCatalogEntry
  | MaterialCreateWorkflowCatalogEntry
  | PlotCreateWorkflowCatalogEntry
  | StorytellingWorkflowCatalogEntry;

export type WorkflowRegistrySnapshot = {
  schemaVersion: typeof WORKFLOW_REGISTRY_SCHEMA_VERSION;
  workflows: [
    CharacterCreateWorkflowCatalogEntry,
    ProjectCreateWorkflowCatalogEntry,
    SettingCreateWorkflowCatalogEntry,
    MaterialCreateWorkflowCatalogEntry,
    PlotCreateWorkflowCatalogEntry,
    StorytellingWorkflowCatalogEntry,
  ];
};

type InvokeCommand = (command: string) => Promise<unknown>;

const ENTRY_KEYS = [
  "description",
  "displayName",
  "executionAvailable",
  "id",
  "mode",
  "routable",
  "version",
] as const;
const SNAPSHOT_KEYS = ["schemaVersion", "workflows"] as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function isSafeDisplayText(value: unknown, maximumCharacters: number): value is string {
  return (
    typeof value === "string" &&
    value === value.trim() &&
    value.length > 0 &&
    [...value].length <= maximumCharacters &&
    ![...value].some((character) => {
      const codePoint = character.codePointAt(0) ?? 0;
      return codePoint <= 0x1f || codePoint === 0x7f || character === "<" || character === ">";
    })
  );
}

export function parseWorkflowCatalogEntry(value: unknown): WorkflowCatalogEntry {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ENTRY_KEYS) ||
    !isSafeDisplayText(value.displayName, 64) ||
    !isSafeDisplayText(value.description, 200) ||
    typeof value.executionAvailable !== "boolean" ||
    typeof value.routable !== "boolean"
  ) {
    throw new Error("Invalid workflow catalog entry contract");
  }
  if (
    value.id === CHARACTER_CREATE_WORKFLOW_ID &&
    value.version === 2 &&
    value.mode === "interactive-write" &&
    value.executionAvailable === true &&
    value.routable === true
  ) {
    return value as CharacterCreateWorkflowCatalogEntry;
  }
  if (
    value.id === SETTING_CREATE_WORKFLOW_ID &&
    value.version === 1 &&
    value.mode === "interactive-write" &&
    value.executionAvailable === true &&
    value.routable === true
  ) {
    return value as SettingCreateWorkflowCatalogEntry;
  }
  if (
    value.id === MATERIAL_CREATE_WORKFLOW_ID &&
    value.version === 1 &&
    value.mode === "interactive-write" &&
    value.executionAvailable === true &&
    value.routable === true
  ) {
    return value as MaterialCreateWorkflowCatalogEntry;
  }
  if (
    value.id === PLOT_CREATE_WORKFLOW_ID &&
    value.version === 1 &&
    value.mode === "interactive-write" &&
    value.executionAvailable === true &&
    value.routable === true
  ) {
    return value as PlotCreateWorkflowCatalogEntry;
  }
  if (
    value.id === PROJECT_CREATE_WORKFLOW_ID &&
    value.version === 1 &&
    value.mode === "deterministic-write" &&
    value.executionAvailable === true &&
    value.routable === true
  ) {
    return value as ProjectCreateWorkflowCatalogEntry;
  }
  if (
    value.id === STORYTELLING_WORKFLOW_ID &&
    value.version === 1 &&
    value.mode === "interactive-write" &&
    value.executionAvailable === false &&
    value.routable === false
  ) {
    return value as StorytellingWorkflowCatalogEntry;
  }
  throw new Error("Invalid workflow catalog identity");
}

export function parseWorkflowRegistrySnapshot(value: unknown): WorkflowRegistrySnapshot {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, SNAPSHOT_KEYS) ||
    value.schemaVersion !== WORKFLOW_REGISTRY_SCHEMA_VERSION ||
    !Array.isArray(value.workflows) ||
    value.workflows.length !== 6
  ) {
    throw new Error("Invalid workflow registry contract");
  }
  const character = parseWorkflowCatalogEntry(value.workflows[0]);
  const project = parseWorkflowCatalogEntry(value.workflows[1]);
  const setting = parseWorkflowCatalogEntry(value.workflows[2]);
  const material = parseWorkflowCatalogEntry(value.workflows[3]);
  const plot = parseWorkflowCatalogEntry(value.workflows[4]);
  const storytelling = parseWorkflowCatalogEntry(value.workflows[5]);
  if (
    character.id !== CHARACTER_CREATE_WORKFLOW_ID ||
    project.id !== PROJECT_CREATE_WORKFLOW_ID ||
    setting.id !== SETTING_CREATE_WORKFLOW_ID ||
    material.id !== MATERIAL_CREATE_WORKFLOW_ID ||
    plot.id !== PLOT_CREATE_WORKFLOW_ID ||
    storytelling.id !== STORYTELLING_WORKFLOW_ID
  ) {
    throw new Error("Invalid workflow registry order");
  }
  return {
    schemaVersion: WORKFLOW_REGISTRY_SCHEMA_VERSION,
    workflows: [character, project, setting, material, plot, storytelling],
  };
}

const invokeTauri: InvokeCommand = (command) => invoke<unknown>(command);

export async function requestWorkflowRegistry(
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<WorkflowRegistrySnapshot> {
  let response: unknown;
  try {
    response = await invokeCommand(WORKFLOW_REGISTRY_LOAD_COMMAND);
  } catch (error: unknown) {
    throw normalizeAppError(error);
  }
  try {
    return parseWorkflowRegistrySnapshot(response);
  } catch {
    throw {
      code: "contract.invalid_response",
      messageKey: "errors.invalidRuntimeResponse",
    } satisfies AppError;
  }
}
