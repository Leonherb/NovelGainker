import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { normalizeAppError, parseAppError, type AppError } from "./health";
import { parseCanonProposalView, type CanonProposalView } from "./workflow-proposal";
import {
  CHARACTER_CREATE_WORKFLOW_ID,
  MATERIAL_CREATE_WORKFLOW_ID,
  PLOT_CREATE_WORKFLOW_ID,
  SETTING_CREATE_WORKFLOW_ID,
} from "./workflow";

export const WORKFLOW_RUN_STATUS_LOAD_COMMAND = "workflow_run_status_load";
export const WORKFLOW_SOURCE_CATALOG_COMMAND = "workflow_source_catalog";
export const WORKFLOW_RUN_PREPARE_COMMAND = "workflow_run_prepare";
export const WORKFLOW_RUN_GENERATE_COMMAND = "workflow_run_generate";
export const WORKFLOW_PROPOSAL_APPLY_COMMAND = "workflow_proposal_apply";
export const WORKFLOW_RUN_CANCEL_COMMAND = "workflow_run_cancel";
export const WORKFLOW_RUN_ABANDON_COMMAND = "workflow_run_abandon";
export const WORKFLOW_RUN_EVENT_CHANNEL = "workflow-run-event";
export const WORKFLOW_RUN_SCHEMA_VERSION = 3;
export const CONTEXT_MANIFEST_SCHEMA_VERSION = 1;

export type WorkflowId =
  | typeof CHARACTER_CREATE_WORKFLOW_ID
  | typeof SETTING_CREATE_WORKFLOW_ID
  | typeof MATERIAL_CREATE_WORKFLOW_ID
  | typeof PLOT_CREATE_WORKFLOW_ID;
export type WorkflowRunState =
  | "created"
  | "routed"
  | "awaiting_input"
  | "context_ready"
  | "generating"
  | "proposal_ready"
  | "awaiting_confirmation"
  | "applying"
  | "validating"
  | "completed"
  | "failed"
  | "cancelled"
  | "recovery_required";

export type WorkflowRunStatus = {
  runId: string;
  projectId: string;
  workflowId: WorkflowId;
  workflowVersion: 1 | 2;
  state: WorkflowRunState;
  sequence: number;
  providerId: "deepseek" | "openai" | null;
  model: "deepseek-v4-pro" | "gpt-5.6-terra" | null;
  updatedAt: number;
  canCancel: boolean;
  canAbandon: boolean;
  recoveryRequired: boolean;
};

export type ContextManifestSource = {
  relativePath: string;
  contentHash: string;
  byteCount: number;
  characterCount: number;
  includedByteCount: number;
  includedCharacterCount: number;
  inclusionReason:
    | "project_overview"
    | "project_writing_constraints"
    | "selected_character_context"
    | "selected_setting_context"
    | "selected_material_context"
    | "selected_plot_context";
  trustClass: "project_canon" | "project_instruction";
  boundedPreview: string;
  truncated: boolean;
};

export type ContextManifest = {
  schemaVersion: typeof CONTEXT_MANIFEST_SCHEMA_VERSION;
  runId: string;
  projectId: string;
  workflowId: WorkflowId;
  workflowVersion: 1 | 2;
  packHash: string;
  providerId: "deepseek" | "openai";
  presetVersion: 1;
  model: "deepseek-v4-pro" | "gpt-5.6-terra";
  briefHash: string;
  briefCharacterCount: number;
  sources: ContextManifestSource[];
  manifestHash: string;
};

export type WorkflowRunStatusSnapshot = {
  schemaVersion: typeof WORKFLOW_RUN_SCHEMA_VERSION;
  available: boolean;
  hasActiveRun: boolean;
  run: WorkflowRunStatus | null;
  contextManifest: ContextManifest | null;
  proposalView: CanonProposalView | null;
};

export type WorkflowRunEvent = {
  runId: string;
  sequence: number;
  kind: "repairing" | "proposalReady" | "failed" | "cancelled";
  workflowId: WorkflowId;
  providerId: "deepseek" | "openai";
  model: "deepseek-v4-pro" | "gpt-5.6-terra";
  state: "generating" | "awaiting_confirmation" | "failed" | "cancelled";
  error: AppError | null;
};

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

export type SettingType = "world" | "location" | "rule";

export type MaterialType = "theme" | "clue" | "object" | "scene_seed";

export type WorkflowSourceCatalogEntry = {
  relativePath: string;
  sourceKind: "character" | "setting" | "world" | "characters" | "plot" | "material" | "materials";
  displayName: string;
  entityId: string | null;
  entitySchemaVersion: 1 | 2 | null;
  settingId: string | null;
  settingType: SettingType | null;
  materialId: string | null;
  materialType: MaterialType | null;
  contentHash: string;
  byteCount: number;
};

export type WorkflowSourceCatalogSnapshot = {
  schemaVersion: 1;
  workflowId:
    | typeof CHARACTER_CREATE_WORKFLOW_ID
    | typeof SETTING_CREATE_WORKFLOW_ID
    | typeof MATERIAL_CREATE_WORKFLOW_ID
    | typeof PLOT_CREATE_WORKFLOW_ID;
  sources: WorkflowSourceCatalogEntry[];
  replaceTargets: WorkflowSourceCatalogEntry[];
};

export type CharacterBriefInput = {
  schemaVersion: 2;
  mode: "create" | "enhance";
  brief: string;
};

export type SettingBriefInput = {
  schemaVersion: 1;
  displayName: string;
  settingType: SettingType;
  objective: string;
  mustInclude: string[];
  constraints: string | null;
};

export type MaterialBriefInput = {
  schemaVersion: 1;
  displayName: string;
  materialType: MaterialType;
  seed: string;
  intendedUse: string;
  constraints: string | null;
};

export type PlotBriefInput = {
  schemaVersion: 1;
  title: string;
  genre: string;
  coreQuestion: string;
  desiredEnding: string;
  mustInclude: string[];
  forbiddenDirections: string[];
};

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu;
const SHA256 = /^[0-9a-f]{64}$/u;
const RUN_KEYS = [
  "canAbandon",
  "canCancel",
  "model",
  "projectId",
  "providerId",
  "recoveryRequired",
  "runId",
  "sequence",
  "state",
  "updatedAt",
  "workflowId",
  "workflowVersion",
] as const;
const SOURCE_KEYS = [
  "boundedPreview",
  "byteCount",
  "characterCount",
  "contentHash",
  "includedByteCount",
  "includedCharacterCount",
  "inclusionReason",
  "relativePath",
  "truncated",
  "trustClass",
] as const;
const MANIFEST_KEYS = [
  "briefCharacterCount",
  "briefHash",
  "manifestHash",
  "model",
  "packHash",
  "presetVersion",
  "projectId",
  "providerId",
  "runId",
  "schemaVersion",
  "sources",
  "workflowId",
  "workflowVersion",
] as const;
const SNAPSHOT_KEYS = [
  "available",
  "contextManifest",
  "hasActiveRun",
  "proposalView",
  "run",
  "schemaVersion",
] as const;
const EVENT_KEYS = [
  "error",
  "kind",
  "model",
  "providerId",
  "runId",
  "sequence",
  "state",
  "workflowId",
] as const;
const STATES = new Set<WorkflowRunState>([
  "created",
  "routed",
  "awaiting_input",
  "context_ready",
  "generating",
  "proposal_ready",
  "awaiting_confirmation",
  "applying",
  "validating",
  "completed",
  "failed",
  "cancelled",
  "recovery_required",
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function byteLength(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function isWorkflowId(value: unknown): value is WorkflowId {
  return (
    value === CHARACTER_CREATE_WORKFLOW_ID ||
    value === SETTING_CREATE_WORKFLOW_ID ||
    value === MATERIAL_CREATE_WORKFLOW_ID ||
    value === PLOT_CREATE_WORKFLOW_ID
  );
}

function isCharacterContextMarkdownPath(value: unknown): value is string {
  return (
    value === "plot/main.md" ||
    (typeof value === "string" &&
      /^(?:characters|world|materials)\/(?!000\.)(?!\.)[^\\/:]+(?:\/[^\\/:]+)*\.md$/u.test(value) &&
      !Array.from(value).some((character) => character.charCodeAt(0) <= 0x1f) &&
      !value.split("/").some((component) => component === "." || component === ".."))
  );
}

function isCharacterTargetMarkdownPath(value: unknown): value is string {
  return (
    typeof value === "string" &&
    /^characters\/(?!\.)[^\\/:]+\.md$/u.test(value) &&
    !Array.from(value).some((character) => character.charCodeAt(0) <= 0x1f)
  );
}

function isWorldMarkdownPath(value: unknown): value is string {
  return (
    typeof value === "string" &&
    /^world\/(?!\.)[^\\/:]+(?:\/[^\\/:]+)*\.md$/u.test(value) &&
    !Array.from(value).some((character) => character.charCodeAt(0) <= 0x1f) &&
    !value.split("/").some((component) => component === "." || component === "..")
  );
}

function isMaterialContextMarkdownPath(value: unknown): value is string {
  return (
    typeof value === "string" &&
    /^(?:world|characters|plot)\/(?!\.)[^\\/:]+(?:\/[^\\/:]+)*\.md$/u.test(value) &&
    !Array.from(value).some((character) => character.charCodeAt(0) <= 0x1f) &&
    !value.split("/").some((component) => component === "." || component === "..")
  );
}

function isPlotContextMarkdownPath(value: unknown): value is string {
  return (
    typeof value === "string" &&
    /^(?:world|characters|materials)\/(?!000\.)(?!\.)[^\\/:]+(?:\/[^\\/:]+)*\.md$/u.test(value) &&
    !Array.from(value).some((character) => character.charCodeAt(0) <= 0x1f) &&
    !value.split("/").some((component) => component === "." || component === "..")
  );
}

function isPlotTargetMarkdownPath(value: unknown): value is "plot/main.md" {
  return value === "plot/main.md";
}
function isMaterialTargetMarkdownPath(value: unknown): value is string {
  return (
    typeof value === "string" &&
    /^materials\/(?!000\.)(?!\.)[^\\/:]+\.md$/u.test(value) &&
    !Array.from(value).some((character) => character.charCodeAt(0) <= 0x1f)
  );
}

function parseManifestSource(
  value: unknown,
  index: number,
  workflowId: WorkflowId,
): ContextManifestSource {
  if (!isRecord(value) || !hasExactKeys(value, SOURCE_KEYS)) {
    throw new Error("Invalid Context Manifest source");
  }
  const identityValid =
    (index === 0 &&
      value.relativePath === "project.md" &&
      value.inclusionReason === "project_overview" &&
      value.trustClass === "project_canon") ||
    (index === 1 &&
      value.relativePath === "AGENTS.md" &&
      value.inclusionReason === "project_writing_constraints" &&
      value.trustClass === "project_instruction") ||
    (workflowId === CHARACTER_CREATE_WORKFLOW_ID &&
      index > 0 &&
      isCharacterContextMarkdownPath(value.relativePath) &&
      value.inclusionReason === "selected_character_context" &&
      value.trustClass === "project_canon") ||
    (workflowId === SETTING_CREATE_WORKFLOW_ID &&
      index > 0 &&
      isWorldMarkdownPath(value.relativePath) &&
      value.inclusionReason === "selected_setting_context" &&
      value.trustClass === "project_canon") ||
    (workflowId === MATERIAL_CREATE_WORKFLOW_ID &&
      index > 0 &&
      isMaterialContextMarkdownPath(value.relativePath) &&
      value.inclusionReason === "selected_material_context" &&
      value.trustClass === "project_canon") ||
    (workflowId === PLOT_CREATE_WORKFLOW_ID &&
      index > 0 &&
      isPlotContextMarkdownPath(value.relativePath) &&
      value.inclusionReason === "selected_plot_context" &&
      value.trustClass === "project_canon");
  if (
    !identityValid ||
    typeof value.contentHash !== "string" ||
    !SHA256.test(value.contentHash) ||
    !Number.isSafeInteger(value.byteCount) ||
    (value.byteCount as number) < 1 ||
    !Number.isSafeInteger(value.characterCount) ||
    (value.characterCount as number) < 1 ||
    !Number.isSafeInteger(value.includedByteCount) ||
    (value.includedByteCount as number) < 1 ||
    (value.includedByteCount as number) > 4 * 1024 ||
    !Number.isSafeInteger(value.includedCharacterCount) ||
    (value.includedCharacterCount as number) < 1 ||
    (value.includedCharacterCount as number) > 1_600 ||
    typeof value.boundedPreview !== "string" ||
    value.boundedPreview.length === 0 ||
    byteLength(value.boundedPreview) !== value.includedByteCount ||
    [...value.boundedPreview].length !== value.includedCharacterCount ||
    (value.includedByteCount as number) > (value.byteCount as number) ||
    (value.includedCharacterCount as number) > (value.characterCount as number) ||
    value.truncated !== (value.includedCharacterCount as number) < (value.characterCount as number)
  ) {
    throw new Error("Invalid Context Manifest source");
  }
  return value as ContextManifestSource;
}

function parseContextManifest(value: unknown): ContextManifest {
  if (!isRecord(value) || !hasExactKeys(value, MANIFEST_KEYS)) {
    throw new Error("Invalid Context Manifest");
  }
  const providerSnapshotValid =
    (value.providerId === "deepseek" && value.model === "deepseek-v4-pro") ||
    (value.providerId === "openai" && value.model === "gpt-5.6-terra");
  if (
    value.schemaVersion !== CONTEXT_MANIFEST_SCHEMA_VERSION ||
    typeof value.runId !== "string" ||
    !UUID.test(value.runId) ||
    typeof value.projectId !== "string" ||
    !UUID.test(value.projectId) ||
    !isWorkflowId(value.workflowId) ||
    (value.workflowId === CHARACTER_CREATE_WORKFLOW_ID && value.workflowVersion !== 2) ||
    (value.workflowId !== CHARACTER_CREATE_WORKFLOW_ID && value.workflowVersion !== 1) ||
    typeof value.packHash !== "string" ||
    !SHA256.test(value.packHash) ||
    !providerSnapshotValid ||
    value.presetVersion !== 1 ||
    typeof value.briefHash !== "string" ||
    !SHA256.test(value.briefHash) ||
    !Number.isSafeInteger(value.briefCharacterCount) ||
    (value.briefCharacterCount as number) < 1 ||
    (value.briefCharacterCount as number) > 4_000 ||
    !Array.isArray(value.sources) ||
    value.sources.length < 1 ||
    value.sources.length >
      (value.workflowId === PLOT_CREATE_WORKFLOW_ID
        ? 10
        : value.workflowId === SETTING_CREATE_WORKFLOW_ID ||
            value.workflowId === MATERIAL_CREATE_WORKFLOW_ID
          ? 8
          : 8) ||
    typeof value.manifestHash !== "string" ||
    !SHA256.test(value.manifestHash)
  ) {
    throw new Error("Invalid Context Manifest");
  }
  const workflowId = value.workflowId as WorkflowId;
  const sources = value.sources.map((source, index) =>
    parseManifestSource(source, index, workflowId),
  );
  const paths = sources.map((source) => source.relativePath);
  const selected = sources.filter(
    (source) =>
      source.inclusionReason === "selected_character_context" ||
      source.inclusionReason === "selected_setting_context" ||
      source.inclusionReason === "selected_material_context" ||
      source.inclusionReason === "selected_plot_context",
  );
  const selectedPaths = selected.map((source) => source.relativePath);
  if (
    new Set(paths).size !== paths.length ||
    selectedPaths.length > (workflowId === PLOT_CREATE_WORKFLOW_ID ? 8 : 6) ||
    selectedPaths.some((path, index) => path !== [...selectedPaths].sort()[index]) ||
    (workflowId === CHARACTER_CREATE_WORKFLOW_ID &&
      selected.some((source) => !isCharacterContextMarkdownPath(source.relativePath))) ||
    (workflowId === SETTING_CREATE_WORKFLOW_ID &&
      selected.some((source) => !isWorldMarkdownPath(source.relativePath))) ||
    (workflowId === MATERIAL_CREATE_WORKFLOW_ID &&
      selected.some((source) => !isMaterialContextMarkdownPath(source.relativePath))) ||
    (workflowId === PLOT_CREATE_WORKFLOW_ID &&
      selected.some((source) => !isPlotContextMarkdownPath(source.relativePath))) ||
    ((workflowId === SETTING_CREATE_WORKFLOW_ID ||
      workflowId === MATERIAL_CREATE_WORKFLOW_ID ||
      workflowId === PLOT_CREATE_WORKFLOW_ID ||
      workflowId === CHARACTER_CREATE_WORKFLOW_ID) &&
      sources.reduce((total, source) => total + source.includedByteCount, 0) > 16 * 1024)
  ) {
    throw new Error("Invalid Context Manifest source order");
  }
  return {
    ...(value as Omit<ContextManifest, "sources">),
    workflowId,
    sources,
  };
}

function parseRun(value: unknown): WorkflowRunStatus {
  if (!isRecord(value) || !hasExactKeys(value, RUN_KEYS)) {
    throw new Error("Invalid Workflow Run contract");
  }
  const state = value.state as WorkflowRunState;
  const terminal = state === "completed" || state === "failed" || state === "cancelled";
  const canCancel = state === "generating";
  const canAbandon = !terminal && state !== "applying" && state !== "validating";
  const providerSnapshotValid =
    (value.providerId === null && value.model === null) ||
    (value.providerId === "deepseek" && value.model === "deepseek-v4-pro") ||
    (value.providerId === "openai" && value.model === "gpt-5.6-terra");
  if (
    typeof value.runId !== "string" ||
    !UUID.test(value.runId) ||
    typeof value.projectId !== "string" ||
    !UUID.test(value.projectId) ||
    !isWorkflowId(value.workflowId) ||
    (value.workflowId === CHARACTER_CREATE_WORKFLOW_ID && value.workflowVersion !== 2) ||
    (value.workflowId !== CHARACTER_CREATE_WORKFLOW_ID && value.workflowVersion !== 1) ||
    typeof value.state !== "string" ||
    !STATES.has(state) ||
    !Number.isSafeInteger(value.sequence) ||
    (value.sequence as number) < 0 ||
    !providerSnapshotValid ||
    !Number.isSafeInteger(value.updatedAt) ||
    (value.updatedAt as number) < 0 ||
    value.canCancel !== canCancel ||
    value.canAbandon !== canAbandon ||
    value.recoveryRequired !== (state === "recovery_required")
  ) {
    throw new Error("Invalid Workflow Run contract");
  }
  return value as WorkflowRunStatus;
}

export function parseWorkflowRunStatusSnapshot(value: unknown): WorkflowRunStatusSnapshot {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, SNAPSHOT_KEYS) ||
    value.schemaVersion !== WORKFLOW_RUN_SCHEMA_VERSION ||
    typeof value.available !== "boolean" ||
    typeof value.hasActiveRun !== "boolean" ||
    !(value.run === null || isRecord(value.run)) ||
    !(value.contextManifest === null || isRecord(value.contextManifest)) ||
    !(value.proposalView === null || isRecord(value.proposalView))
  ) {
    throw new Error("Invalid Workflow Run snapshot contract");
  }
  if (!value.available) {
    if (
      value.hasActiveRun ||
      value.run !== null ||
      value.contextManifest !== null ||
      value.proposalView !== null
    ) {
      throw new Error("Unavailable Workflow Manager cannot expose a Run");
    }
    return {
      schemaVersion: WORKFLOW_RUN_SCHEMA_VERSION,
      available: false,
      hasActiveRun: false,
      run: null,
      contextManifest: null,
      proposalView: null,
    };
  }
  const run = value.run === null ? null : parseRun(value.run);
  const contextManifest =
    value.contextManifest === null ? null : parseContextManifest(value.contextManifest);
  const proposalView =
    value.proposalView === null ? null : parseCanonProposalView(value.proposalView);
  if (run && !["completed", "failed", "cancelled"].includes(run.state) && !value.hasActiveRun) {
    throw new Error("Nonterminal Workflow Run must be active");
  }
  if (
    contextManifest &&
    (!run ||
      !["context_ready", "generating"].includes(run.state) ||
      contextManifest.runId !== run.runId ||
      contextManifest.projectId !== run.projectId ||
      contextManifest.workflowId !== run.workflowId ||
      contextManifest.workflowVersion !== run.workflowVersion ||
      contextManifest.providerId !== run.providerId ||
      contextManifest.model !== run.model)
  ) {
    throw new Error("Context Manifest does not match Workflow Run");
  }
  if (run?.state === "context_ready" && !contextManifest) {
    throw new Error("Context-ready Run requires an in-memory Manifest");
  }
  if (
    proposalView &&
    (!run ||
      !["proposal_ready", "awaiting_confirmation", "applying", "validating"].includes(run.state) ||
      proposalView.runId !== run.runId ||
      proposalView.workflowId !== run.workflowId ||
      proposalView.workflowVersion !== run.workflowVersion ||
      proposalView.providerId !== run.providerId ||
      proposalView.model !== run.model)
  ) {
    throw new Error("Proposal View does not match Workflow Run");
  }
  if (run?.state === "awaiting_confirmation" && !proposalView) {
    throw new Error("Awaiting-confirmation Run requires an in-memory Proposal View");
  }
  return {
    schemaVersion: WORKFLOW_RUN_SCHEMA_VERSION,
    available: true,
    hasActiveRun: value.hasActiveRun,
    run,
    contextManifest,
    proposalView,
  };
}

const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);

async function invokeWorkflowRun(
  command: string,
  args: Record<string, unknown>,
  invokeCommand: InvokeCommand,
): Promise<WorkflowRunStatusSnapshot> {
  let response: unknown;
  try {
    response = await invokeCommand(command, args);
  } catch (error: unknown) {
    throw normalizeAppError(error);
  }
  try {
    return parseWorkflowRunStatusSnapshot(response);
  } catch {
    throw {
      code: "contract.invalid_response",
      messageKey: "errors.invalidRuntimeResponse",
    } satisfies AppError;
  }
}

const CHARACTER_ID =
  /^character_[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;
const PLOT_ID = /^plot_[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;
const SETTING_ID =
  /^setting_[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;
const MATERIAL_ID =
  /^material_[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;
const SOURCE_CATALOG_KEYS = ["replaceTargets", "schemaVersion", "sources", "workflowId"] as const;
const SOURCE_CATALOG_ENTRY_KEYS = [
  "byteCount",
  "contentHash",
  "displayName",
  "entityId",
  "entitySchemaVersion",
  "materialId",
  "materialType",
  "relativePath",
  "settingId",
  "settingType",
  "sourceKind",
] as const;

function isSafeSettingText(
  value: unknown,
  maxCharacters: number,
  maxBytes: number,
): value is string {
  return (
    typeof value === "string" &&
    value === value.trim() &&
    value.length > 0 &&
    [...value].length <= maxCharacters &&
    byteLength(value) <= maxBytes &&
    !value.includes("<") &&
    !value.includes(">") &&
    ![...value].some(
      (character) =>
        (/\p{Cc}/u.test(character) && character !== "\n" && character !== "\t") ||
        /[\u061c\u200b-\u200f\u202a-\u202e\u2060-\u2069\ufeff\ufffd]/u.test(character),
    )
  );
}

function isMaterialType(value: unknown): value is MaterialType {
  return value === "theme" || value === "clue" || value === "object" || value === "scene_seed";
}

function parseCatalogEntry(
  value: unknown,
  workflowId:
    | typeof CHARACTER_CREATE_WORKFLOW_ID
    | typeof SETTING_CREATE_WORKFLOW_ID
    | typeof MATERIAL_CREATE_WORKFLOW_ID
    | typeof PLOT_CREATE_WORKFLOW_ID,
  purpose: "source" | "replace",
): WorkflowSourceCatalogEntry {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, SOURCE_CATALOG_ENTRY_KEYS) ||
    !isSafeSettingText(
      value.displayName,
      workflowId === PLOT_CREATE_WORKFLOW_ID ? 120 : 80,
      workflowId === PLOT_CREATE_WORKFLOW_ID ? 512 : 256,
    ) ||
    typeof value.contentHash !== "string" ||
    !SHA256.test(value.contentHash) ||
    !Number.isSafeInteger(value.byteCount) ||
    (value.byteCount as number) < 1 ||
    (value.byteCount as number) > 64 * 1024
  ) {
    throw new Error("Invalid Workflow Source Catalog entry");
  }

  if (workflowId === CHARACTER_CREATE_WORKFLOW_ID) {
    const path = value.relativePath;
    const expectedKind =
      typeof path !== "string"
        ? null
        : path.startsWith("characters/")
          ? "character"
          : path.startsWith("world/")
            ? "setting"
            : path.startsWith("materials/")
              ? "material"
              : path === "plot/main.md"
                ? "plot"
                : null;
    const identityValid =
      expectedKind === "character"
        ? (value.entitySchemaVersion === 1 && value.entityId === null) ||
          (value.entitySchemaVersion === 2 &&
            typeof value.entityId === "string" &&
            CHARACTER_ID.test(value.entityId))
        : expectedKind === "setting"
          ? value.entitySchemaVersion === 1 &&
            typeof value.entityId === "string" &&
            SETTING_ID.test(value.entityId)
          : expectedKind === "material"
            ? value.entitySchemaVersion === 1 &&
              typeof value.entityId === "string" &&
              MATERIAL_ID.test(value.entityId)
            : expectedKind === "plot"
              ? value.entitySchemaVersion === 1 &&
                typeof value.entityId === "string" &&
                PLOT_ID.test(value.entityId)
              : false;
    if (
      !isCharacterContextMarkdownPath(path) ||
      value.sourceKind !== expectedKind ||
      !identityValid ||
      (purpose === "replace" &&
        (!isCharacterTargetMarkdownPath(path) || expectedKind !== "character")) ||
      value.settingId !== null ||
      value.settingType !== null ||
      value.materialId !== null ||
      value.materialType !== null
    ) {
      throw new Error("Invalid character source entry");
    }
    return value as WorkflowSourceCatalogEntry;
  }
  if (value.entityId !== null || value.entitySchemaVersion !== null) {
    throw new Error("Unexpected generic entity identity");
  }

  if (workflowId === SETTING_CREATE_WORKFLOW_ID) {
    if (
      !isWorldMarkdownPath(value.relativePath) ||
      value.sourceKind !== "setting" ||
      typeof value.settingId !== "string" ||
      !SETTING_ID.test(value.settingId) ||
      (value.settingType !== "world" &&
        value.settingType !== "location" &&
        value.settingType !== "rule") ||
      value.materialId !== null ||
      value.materialType !== null
    ) {
      throw new Error("Invalid setting source entry");
    }
    return value as WorkflowSourceCatalogEntry;
  }

  if (workflowId === PLOT_CREATE_WORKFLOW_ID) {
    if (purpose === "source") {
      const expectedKind =
        typeof value.relativePath === "string" ? value.relativePath.split("/", 1)[0] : null;
      if (
        !isPlotContextMarkdownPath(value.relativePath) ||
        (expectedKind !== "world" &&
          expectedKind !== "characters" &&
          expectedKind !== "materials") ||
        value.sourceKind !== expectedKind ||
        value.settingId !== null ||
        value.settingType !== null ||
        value.materialId !== null ||
        value.materialType !== null
      ) {
        throw new Error("Invalid plot context source entry");
      }
      return value as WorkflowSourceCatalogEntry;
    }
    if (
      !isPlotTargetMarkdownPath(value.relativePath) ||
      value.sourceKind !== "plot" ||
      value.settingId !== null ||
      value.settingType !== null ||
      value.materialId !== null ||
      value.materialType !== null
    ) {
      throw new Error("Invalid plot replace target entry");
    }
    return value as WorkflowSourceCatalogEntry;
  }
  if (purpose === "source") {
    const expectedKind =
      typeof value.relativePath === "string" ? value.relativePath.split("/", 1)[0] : null;
    if (
      !isMaterialContextMarkdownPath(value.relativePath) ||
      (expectedKind !== "world" && expectedKind !== "characters" && expectedKind !== "plot") ||
      value.sourceKind !== expectedKind ||
      value.settingId !== null ||
      value.settingType !== null ||
      value.materialId !== null ||
      value.materialType !== null
    ) {
      throw new Error("Invalid material context source entry");
    }
    return value as WorkflowSourceCatalogEntry;
  }

  if (
    !isMaterialTargetMarkdownPath(value.relativePath) ||
    value.sourceKind !== "material" ||
    value.settingId !== null ||
    value.settingType !== null ||
    typeof value.materialId !== "string" ||
    !MATERIAL_ID.test(value.materialId) ||
    !isMaterialType(value.materialType)
  ) {
    throw new Error("Invalid material replace target entry");
  }
  return value as WorkflowSourceCatalogEntry;
}

function validateCatalogOrder(entries: WorkflowSourceCatalogEntry[]): void {
  const paths = entries.map((entry) => entry.relativePath);
  if (
    new Set(paths).size !== paths.length ||
    paths.some((path, index) => path !== [...paths].sort()[index])
  ) {
    throw new Error("Invalid Workflow Source Catalog order");
  }
}

export function parseWorkflowSourceCatalog(value: unknown): WorkflowSourceCatalogSnapshot {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, SOURCE_CATALOG_KEYS) ||
    value.schemaVersion !== 1 ||
    (value.workflowId !== CHARACTER_CREATE_WORKFLOW_ID &&
      value.workflowId !== SETTING_CREATE_WORKFLOW_ID &&
      value.workflowId !== MATERIAL_CREATE_WORKFLOW_ID &&
      value.workflowId !== PLOT_CREATE_WORKFLOW_ID) ||
    !Array.isArray(value.sources) ||
    !Array.isArray(value.replaceTargets) ||
    value.sources.length > 5_000 ||
    value.replaceTargets.length > 5_000 ||
    (value.workflowId === PLOT_CREATE_WORKFLOW_ID && value.replaceTargets.length > 1)
  ) {
    throw new Error("Invalid Workflow Source Catalog");
  }
  const workflowId = value.workflowId;
  const sources = value.sources.map((source) => parseCatalogEntry(source, workflowId, "source"));
  const replaceTargets = value.replaceTargets.map((source) =>
    parseCatalogEntry(source, workflowId, "replace"),
  );
  validateCatalogOrder(sources);
  validateCatalogOrder(replaceTargets);
  if (workflowId === CHARACTER_CREATE_WORKFLOW_ID) {
    const entityIds = sources.flatMap((source) => (source.entityId ? [source.entityId] : []));
    const sourcePaths = new Set(sources.map((source) => source.relativePath));
    if (
      new Set(entityIds).size !== entityIds.length ||
      replaceTargets.some(
        (target) => target.sourceKind !== "character" || !sourcePaths.has(target.relativePath),
      )
    ) {
      throw new Error("Invalid character entity catalog");
    }
  }
  if (
    workflowId === SETTING_CREATE_WORKFLOW_ID &&
    (sources.length !== replaceTargets.length ||
      sources.some((source, index) => source.relativePath !== replaceTargets[index]?.relativePath))
  ) {
    throw new Error("Setting source and replace catalogs must match");
  }
  return {
    schemaVersion: 1,
    workflowId,
    sources,
    replaceTargets,
  };
}

async function requestSourceCatalogForWorkflow(
  sessionId: string,
  workflowId:
    | typeof CHARACTER_CREATE_WORKFLOW_ID
    | typeof SETTING_CREATE_WORKFLOW_ID
    | typeof MATERIAL_CREATE_WORKFLOW_ID
    | typeof PLOT_CREATE_WORKFLOW_ID,
  invokeCommand: InvokeCommand,
): Promise<WorkflowSourceCatalogSnapshot> {
  let response: unknown;
  try {
    response = await invokeCommand(WORKFLOW_SOURCE_CATALOG_COMMAND, {
      sessionId,
      workflowId,
    });
  } catch (error: unknown) {
    throw normalizeAppError(error);
  }
  try {
    const parsed = parseWorkflowSourceCatalog(response);
    if (parsed.workflowId !== workflowId) {
      throw new Error("Workflow catalog identity mismatch");
    }
    return parsed;
  } catch {
    throw {
      code: "contract.invalid_response",
      messageKey: "errors.invalidRuntimeResponse",
    } satisfies AppError;
  }
}

export function requestCharacterWorkflowSourceCatalog(
  sessionId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<WorkflowSourceCatalogSnapshot> {
  return requestSourceCatalogForWorkflow(sessionId, CHARACTER_CREATE_WORKFLOW_ID, invokeCommand);
}

export function requestWorkflowSourceCatalog(
  sessionId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<WorkflowSourceCatalogSnapshot> {
  return requestSourceCatalogForWorkflow(sessionId, SETTING_CREATE_WORKFLOW_ID, invokeCommand);
}

export function requestMaterialWorkflowSourceCatalog(
  sessionId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<WorkflowSourceCatalogSnapshot> {
  return requestSourceCatalogForWorkflow(sessionId, MATERIAL_CREATE_WORKFLOW_ID, invokeCommand);
}
export function requestPlotWorkflowSourceCatalog(
  sessionId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<WorkflowSourceCatalogSnapshot> {
  return requestSourceCatalogForWorkflow(sessionId, PLOT_CREATE_WORKFLOW_ID, invokeCommand);
}

export function requestWorkflowRunStatus(
  sessionId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<WorkflowRunStatusSnapshot> {
  return invokeWorkflowRun(WORKFLOW_RUN_STATUS_LOAD_COMMAND, { sessionId }, invokeCommand);
}

export function requestCharacterWorkflowRunPrepare(
  sessionId: string,
  input: CharacterBriefInput,
  selectedSourcePaths: string[],
  replaceTargetPath: string | null,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<WorkflowRunStatusSnapshot> {
  const normalized = input.brief.replace(/\r\n?/gu, "\n").trim();
  if (
    input.schemaVersion !== 2 ||
    (input.mode !== "create" && input.mode !== "enhance") ||
    normalized.length === 0 ||
    [...normalized].length > 4_000 ||
    byteLength(normalized) > 16 * 1024 ||
    [...normalized].some(
      (character) => /\p{Cc}/u.test(character) && character !== "\n" && character !== "\t",
    )
  ) {
    return Promise.reject({
      code: "workflow.input_invalid",
      messageKey: "errors.workflowInputInvalid",
    } satisfies AppError);
  }
  if (selectedSourcePaths.length > 6) {
    return Promise.reject({
      code: "workflow.source_limit_exceeded",
      messageKey: "errors.workflowSourceLimitExceeded",
    } satisfies AppError);
  }
  if (
    new Set(selectedSourcePaths).size !== selectedSourcePaths.length ||
    selectedSourcePaths.some((path) => !isCharacterContextMarkdownPath(path)) ||
    (input.mode === "create" && replaceTargetPath !== null) ||
    (input.mode === "enhance" &&
      (replaceTargetPath === null ||
        !isCharacterTargetMarkdownPath(replaceTargetPath) ||
        !selectedSourcePaths.includes(replaceTargetPath)))
  ) {
    return Promise.reject({
      code: "workflow.source_not_allowed",
      messageKey: "errors.workflowSourceNotAllowed",
    } satisfies AppError);
  }
  return invokeWorkflowRun(
    WORKFLOW_RUN_PREPARE_COMMAND,
    {
      sessionId,
      workflowId: CHARACTER_CREATE_WORKFLOW_ID,
      input: { schemaVersion: 2, mode: input.mode, brief: normalized },
      selectedSourcePaths: [...selectedSourcePaths].sort(),
      replaceTargetPath,
    },
    invokeCommand,
  );
}

export function requestWorkflowRunPrepare(
  sessionId: string,
  brief: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<WorkflowRunStatusSnapshot> {
  return requestCharacterWorkflowRunPrepare(
    sessionId,
    { schemaVersion: 2, mode: "create", brief },
    [],
    null,
    invokeCommand,
  );
}

export function requestSettingWorkflowRunPrepare(
  sessionId: string,
  input: SettingBriefInput,
  selectedSourcePaths: string[],
  replaceTargetPath: string | null,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<WorkflowRunStatusSnapshot> {
  if (selectedSourcePaths.length > 6) {
    return Promise.reject({
      code: "workflow.source_limit_exceeded",
      messageKey: "errors.workflowSourceLimitExceeded",
    } satisfies AppError);
  }
  if (
    new Set(selectedSourcePaths).size !== selectedSourcePaths.length ||
    selectedSourcePaths.some((path) => !isWorldMarkdownPath(path)) ||
    (replaceTargetPath !== null &&
      (!isWorldMarkdownPath(replaceTargetPath) || !selectedSourcePaths.includes(replaceTargetPath)))
  ) {
    return Promise.reject({
      code: "workflow.source_not_allowed",
      messageKey: "errors.workflowSourceNotAllowed",
    } satisfies AppError);
  }
  const displayName = input.displayName.replace(/\r\n?/gu, "\n").trim();
  const objective = input.objective.replace(/\r\n?/gu, "\n").trim();
  const mustInclude = input.mustInclude.map((value) => value.replace(/\r\n?/gu, "\n").trim());
  const constraintsValue = input.constraints?.replace(/\r\n?/gu, "\n").trim() ?? "";
  const constraints = constraintsValue.length > 0 ? constraintsValue : null;
  if (
    input.schemaVersion !== 1 ||
    (input.settingType !== "world" &&
      input.settingType !== "location" &&
      input.settingType !== "rule") ||
    !isSafeSettingText(displayName, 80, 256) ||
    displayName.includes("\n") ||
    !isSafeSettingText(objective, 1_000, 4 * 1024) ||
    mustInclude.length < 1 ||
    mustInclude.length > 12 ||
    mustInclude.some((value) => !isSafeSettingText(value, 160, 512)) ||
    new Set(mustInclude).size !== mustInclude.length ||
    (constraints !== null && !isSafeSettingText(constraints, 1_000, 4 * 1024))
  ) {
    return Promise.reject({
      code: "workflow.input_invalid",
      messageKey: "errors.workflowInputInvalid",
    } satisfies AppError);
  }
  return invokeWorkflowRun(
    WORKFLOW_RUN_PREPARE_COMMAND,
    {
      sessionId,
      workflowId: SETTING_CREATE_WORKFLOW_ID,
      input: {
        schemaVersion: 1,
        displayName,
        settingType: input.settingType,
        objective,
        mustInclude,
        constraints,
      },
      selectedSourcePaths: [...selectedSourcePaths].sort(),
      replaceTargetPath,
    },
    invokeCommand,
  );
}

export function requestMaterialWorkflowRunPrepare(
  sessionId: string,
  input: MaterialBriefInput,
  selectedSourcePaths: string[],
  replaceTargetPath: string | null,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<WorkflowRunStatusSnapshot> {
  if (selectedSourcePaths.length > 6) {
    return Promise.reject({
      code: "workflow.source_limit_exceeded",
      messageKey: "errors.workflowSourceLimitExceeded",
    } satisfies AppError);
  }
  if (
    new Set(selectedSourcePaths).size !== selectedSourcePaths.length ||
    selectedSourcePaths.some((path) => !isMaterialContextMarkdownPath(path)) ||
    (replaceTargetPath !== null && !isMaterialTargetMarkdownPath(replaceTargetPath))
  ) {
    return Promise.reject({
      code: "workflow.source_not_allowed",
      messageKey: "errors.workflowSourceNotAllowed",
    } satisfies AppError);
  }

  const displayName = input.displayName.replace(/\r\n?/gu, "\n").trim();
  const seed = input.seed.replace(/\r\n?/gu, "\n").trim();
  const intendedUse = input.intendedUse.replace(/\r\n?/gu, "\n").trim();
  const constraintsValue = input.constraints?.replace(/\r\n?/gu, "\n").trim() ?? "";
  const constraints = constraintsValue.length > 0 ? constraintsValue : null;
  if (
    input.schemaVersion !== 1 ||
    !isMaterialType(input.materialType) ||
    !isSafeSettingText(displayName, 80, 256) ||
    displayName.includes("\n") ||
    !isSafeSettingText(seed, 2_000, 8 * 1024) ||
    !isSafeSettingText(intendedUse, 1_000, 4 * 1024) ||
    (constraints !== null && !isSafeSettingText(constraints, 1_000, 4 * 1024))
  ) {
    return Promise.reject({
      code: "workflow.input_invalid",
      messageKey: "errors.workflowInputInvalid",
    } satisfies AppError);
  }
  return invokeWorkflowRun(
    WORKFLOW_RUN_PREPARE_COMMAND,
    {
      sessionId,
      workflowId: MATERIAL_CREATE_WORKFLOW_ID,
      input: {
        schemaVersion: 1,
        displayName,
        materialType: input.materialType,
        seed,
        intendedUse,
        constraints,
      },
      selectedSourcePaths: [...selectedSourcePaths].sort(),
      replaceTargetPath,
    },
    invokeCommand,
  );
}
export function requestPlotWorkflowRunPrepare(
  sessionId: string,
  input: PlotBriefInput,
  selectedSourcePaths: string[],
  replaceTargetPath: string | null,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<WorkflowRunStatusSnapshot> {
  if (selectedSourcePaths.length > 8) {
    return Promise.reject({
      code: "workflow.source_limit_exceeded",
      messageKey: "errors.workflowSourceLimitExceeded",
    } satisfies AppError);
  }
  if (
    new Set(selectedSourcePaths).size !== selectedSourcePaths.length ||
    selectedSourcePaths.some((path) => !isPlotContextMarkdownPath(path)) ||
    (replaceTargetPath !== null && !isPlotTargetMarkdownPath(replaceTargetPath))
  ) {
    return Promise.reject({
      code: "workflow.source_not_allowed",
      messageKey: "errors.workflowSourceNotAllowed",
    } satisfies AppError);
  }

  const title = input.title.replace(/\r\n?/gu, "\n").trim();
  const genre = input.genre.replace(/\r\n?/gu, "\n").trim();
  const coreQuestion = input.coreQuestion.replace(/\r\n?/gu, "\n").trim();
  const desiredEnding = input.desiredEnding.replace(/\r\n?/gu, "\n").trim();
  const mustInclude = input.mustInclude.map((value) => value.replace(/\r\n?/gu, "\n").trim());
  const forbiddenDirections = input.forbiddenDirections.map((value) =>
    value.replace(/\r\n?/gu, "\n").trim(),
  );
  if (
    input.schemaVersion !== 1 ||
    !isSafeSettingText(title, 120, 512) ||
    title.includes("\n") ||
    !isSafeSettingText(genre, 80, 256) ||
    genre.includes("\n") ||
    !isSafeSettingText(coreQuestion, 1_000, 4 * 1024) ||
    !isSafeSettingText(desiredEnding, 2_000, 8 * 1024) ||
    mustInclude.length > 8 ||
    forbiddenDirections.length > 8 ||
    mustInclude.some((value) => !isSafeSettingText(value, 500, 2 * 1024)) ||
    forbiddenDirections.some((value) => !isSafeSettingText(value, 500, 2 * 1024)) ||
    new Set(mustInclude).size !== mustInclude.length ||
    new Set(forbiddenDirections).size !== forbiddenDirections.length
  ) {
    return Promise.reject({
      code: "workflow.input_invalid",
      messageKey: "errors.workflowInputInvalid",
    } satisfies AppError);
  }
  return invokeWorkflowRun(
    WORKFLOW_RUN_PREPARE_COMMAND,
    {
      sessionId,
      workflowId: PLOT_CREATE_WORKFLOW_ID,
      input: {
        schemaVersion: 1,
        title,
        genre,
        coreQuestion,
        desiredEnding,
        mustInclude,
        forbiddenDirections,
      },
      selectedSourcePaths: [...selectedSourcePaths].sort(),
      replaceTargetPath,
    },
    invokeCommand,
  );
}
export function requestWorkflowRunGenerate(
  runId: string,
  manifestHash: string,
  confirmed: boolean,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<WorkflowRunStatusSnapshot> {
  return invokeWorkflowRun(
    WORKFLOW_RUN_GENERATE_COMMAND,
    { runId, manifestHash, confirmed },
    invokeCommand,
  );
}

export function requestWorkflowProposalApply(
  runId: string,
  proposalHash: string,
  confirmed: boolean,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<WorkflowRunStatusSnapshot> {
  return invokeWorkflowRun(
    WORKFLOW_PROPOSAL_APPLY_COMMAND,
    { runId, proposalHash, confirmed },
    invokeCommand,
  );
}

export function requestWorkflowRunCancel(
  sessionId: string,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<WorkflowRunStatusSnapshot> {
  return invokeWorkflowRun(WORKFLOW_RUN_CANCEL_COMMAND, { sessionId }, invokeCommand);
}

export function requestWorkflowRunAbandon(
  sessionId: string,
  confirmed: boolean,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<WorkflowRunStatusSnapshot> {
  return invokeWorkflowRun(WORKFLOW_RUN_ABANDON_COMMAND, { sessionId, confirmed }, invokeCommand);
}

export function parseWorkflowRunEvent(value: unknown): WorkflowRunEvent {
  if (!isRecord(value) || !hasExactKeys(value, EVENT_KEYS)) {
    throw new Error("Invalid Workflow Run event");
  }
  const providerSnapshotValid =
    (value.providerId === "deepseek" && value.model === "deepseek-v4-pro") ||
    (value.providerId === "openai" && value.model === "gpt-5.6-terra");
  const error = value.error === null ? null : parseAppError(value.error);
  if (
    typeof value.runId !== "string" ||
    !UUID.test(value.runId) ||
    !Number.isSafeInteger(value.sequence) ||
    (value.sequence as number) < 1 ||
    !isWorkflowId(value.workflowId) ||
    !providerSnapshotValid ||
    !(
      (value.kind === "repairing" && value.state === "generating" && error !== null) ||
      (value.kind === "proposalReady" &&
        value.state === "awaiting_confirmation" &&
        value.error === null) ||
      (value.kind === "failed" && value.state === "failed" && error !== null) ||
      (value.kind === "cancelled" && value.state === "cancelled" && value.error === null)
    )
  ) {
    throw new Error("Invalid Workflow Run event");
  }
  return {
    runId: value.runId,
    sequence: value.sequence as number,
    kind: value.kind,
    workflowId: value.workflowId,
    providerId: value.providerId,
    model: value.model,
    state: value.state,
    error,
  } as WorkflowRunEvent;
}

export async function listenWorkflowRunEvents(
  onEvent: (event: WorkflowRunEvent) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(WORKFLOW_RUN_EVENT_CHANNEL, ({ payload }) => {
    try {
      onEvent(parseWorkflowRunEvent(payload));
    } catch {
      return;
    }
  });
}
