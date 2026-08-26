import {
  CHARACTER_CREATE_WORKFLOW_ID,
  MATERIAL_CREATE_WORKFLOW_ID,
  PLOT_CREATE_WORKFLOW_ID,
  SETTING_CREATE_WORKFLOW_ID,
} from "./workflow";

export const CANON_PROPOSAL_SCHEMA_VERSION = 1;

export type CanonFileOperationKind = "create" | "replace";

export type CanonFileOperation = {
  kind: CanonFileOperationKind;
  relativePath: string;
  expectedSourceHash: "absent" | string;
  proposedContentHash: string;
  proposedByteCount: number;
};

export type ProposalValidationSummary = {
  strict: true;
  eligibleForAiApply: true;
  diagnosticCodes: string[];
};

export type ProposalDiffLine = {
  kind: "context" | "add" | "remove";
  oldLine: number | null;
  newLine: number | null;
  content: string;
};

export type CanonProposalView = {
  schemaVersion: typeof CANON_PROPOSAL_SCHEMA_VERSION;
  proposalId: string;
  proposalHash: string;
  runId: string;
  workflowId:
    | typeof CHARACTER_CREATE_WORKFLOW_ID
    | typeof SETTING_CREATE_WORKFLOW_ID
    | typeof MATERIAL_CREATE_WORKFLOW_ID
    | typeof PLOT_CREATE_WORKFLOW_ID;
  workflowVersion: 1 | 2;
  packHash: string;
  providerId: "deepseek" | "openai";
  presetVersion: 1;
  model: "deepseek-v4-pro" | "gpt-5.6-terra";
  manifestHash: string;
  operation: CanonFileOperation;
  validationSummary: ProposalValidationSummary;
  warningCodes: string[];
  fullContent: string;
  lineDiff: ProposalDiffLine[];
};

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu;
const SHA256 = /^[0-9a-f]{64}$/u;
const CODE = /^[a-z0-9._-]{1,80}$/u;
const SAFE_RELATIVE_MARKDOWN =
  /^(?!\/)(?!.*(?:^|\/)\.{1,2}(?:\/|$))(?!.*[\\:])[^/]+(?:\/[^/]+)*\.md$/u;
const VIEW_KEYS = [
  "fullContent",
  "lineDiff",
  "manifestHash",
  "model",
  "operation",
  "packHash",
  "presetVersion",
  "proposalHash",
  "proposalId",
  "providerId",
  "runId",
  "schemaVersion",
  "validationSummary",
  "warningCodes",
  "workflowId",
  "workflowVersion",
] as const;
const OPERATION_KEYS = [
  "expectedSourceHash",
  "kind",
  "proposedByteCount",
  "proposedContentHash",
  "relativePath",
] as const;
const VALIDATION_KEYS = ["diagnosticCodes", "eligibleForAiApply", "strict"] as const;
const DIFF_KEYS = ["content", "kind", "newLine", "oldLine"] as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function hasControlCharacters(value: string): boolean {
  return [...value].some((character) => character.charCodeAt(0) <= 0x1f);
}

function parseCodes(value: unknown, maximum: number): string[] {
  if (
    !Array.isArray(value) ||
    value.length > maximum ||
    value.some((code) => typeof code !== "string" || !CODE.test(code)) ||
    new Set(value).size !== value.length ||
    [...value].sort().some((code, index) => code !== value[index])
  ) {
    throw new Error("Invalid Proposal code list");
  }
  return value as string[];
}

function parseOperation(value: unknown): CanonFileOperation {
  if (!isRecord(value) || !hasExactKeys(value, OPERATION_KEYS)) {
    throw new Error("Invalid Proposal operation");
  }
  if (
    (value.kind !== "create" && value.kind !== "replace") ||
    typeof value.relativePath !== "string" ||
    value.relativePath.length > 240 ||
    !SAFE_RELATIVE_MARKDOWN.test(value.relativePath) ||
    hasControlCharacters(value.relativePath) ||
    typeof value.expectedSourceHash !== "string" ||
    (value.kind === "create"
      ? value.expectedSourceHash !== "absent"
      : !SHA256.test(value.expectedSourceHash)) ||
    typeof value.proposedContentHash !== "string" ||
    !SHA256.test(value.proposedContentHash) ||
    !Number.isSafeInteger(value.proposedByteCount) ||
    (value.proposedByteCount as number) < 1 ||
    (value.proposedByteCount as number) > 1024 * 1024
  ) {
    throw new Error("Invalid Proposal operation");
  }
  return value as CanonFileOperation;
}

function parseValidation(value: unknown): ProposalValidationSummary {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, VALIDATION_KEYS) ||
    value.strict !== true ||
    value.eligibleForAiApply !== true
  ) {
    throw new Error("Invalid Proposal validation");
  }
  const diagnosticCodes = parseCodes(value.diagnosticCodes, 64);
  if (diagnosticCodes.length !== 0) {
    throw new Error("Confirmable Proposal cannot contain blocking diagnostics");
  }
  return { strict: true, eligibleForAiApply: true, diagnosticCodes };
}

function parseDiffLine(value: unknown): ProposalDiffLine {
  if (!isRecord(value) || !hasExactKeys(value, DIFF_KEYS)) {
    throw new Error("Invalid Proposal diff line");
  }
  const validLine = (line: unknown) =>
    line === null || (Number.isSafeInteger(line) && (line as number) > 0);
  if (
    (value.kind !== "context" && value.kind !== "add" && value.kind !== "remove") ||
    !validLine(value.oldLine) ||
    !validLine(value.newLine) ||
    typeof value.content !== "string" ||
    value.content.includes("\n") ||
    (value.kind === "context" && (value.oldLine === null || value.newLine === null)) ||
    (value.kind === "add" && (value.oldLine !== null || value.newLine === null)) ||
    (value.kind === "remove" && (value.oldLine === null || value.newLine !== null))
  ) {
    throw new Error("Invalid Proposal diff line");
  }
  return value as ProposalDiffLine;
}

export function parseCanonProposalView(value: unknown): CanonProposalView {
  if (!isRecord(value) || !hasExactKeys(value, VIEW_KEYS)) {
    throw new Error("Invalid Canon Proposal view");
  }
  const providerValid =
    (value.providerId === "deepseek" && value.model === "deepseek-v4-pro") ||
    (value.providerId === "openai" && value.model === "gpt-5.6-terra");
  if (
    value.schemaVersion !== CANON_PROPOSAL_SCHEMA_VERSION ||
    typeof value.proposalId !== "string" ||
    !UUID.test(value.proposalId) ||
    typeof value.proposalHash !== "string" ||
    !SHA256.test(value.proposalHash) ||
    typeof value.runId !== "string" ||
    !UUID.test(value.runId) ||
    !(
      value.workflowId === CHARACTER_CREATE_WORKFLOW_ID ||
      value.workflowId === SETTING_CREATE_WORKFLOW_ID ||
      value.workflowId === MATERIAL_CREATE_WORKFLOW_ID ||
      value.workflowId === PLOT_CREATE_WORKFLOW_ID
    ) ||
    (value.workflowId === CHARACTER_CREATE_WORKFLOW_ID && value.workflowVersion !== 2) ||
    (value.workflowId !== CHARACTER_CREATE_WORKFLOW_ID && value.workflowVersion !== 1) ||
    typeof value.packHash !== "string" ||
    !SHA256.test(value.packHash) ||
    !providerValid ||
    value.presetVersion !== 1 ||
    typeof value.manifestHash !== "string" ||
    !SHA256.test(value.manifestHash) ||
    typeof value.fullContent !== "string" ||
    value.fullContent.length === 0 ||
    value.fullContent.includes("\r") ||
    !value.fullContent.endsWith("\n") ||
    !Array.isArray(value.lineDiff)
  ) {
    throw new Error("Invalid Canon Proposal view");
  }
  const operation = parseOperation(value.operation);
  const expectedPrefix =
    value.workflowId === CHARACTER_CREATE_WORKFLOW_ID
      ? "characters/"
      : value.workflowId === SETTING_CREATE_WORKFLOW_ID
        ? "world/"
        : value.workflowId === MATERIAL_CREATE_WORKFLOW_ID
          ? "materials/"
          : "plot/";
  if (
    (value.workflowId === MATERIAL_CREATE_WORKFLOW_ID &&
      operation.relativePath.slice("materials/".length).startsWith("000.")) ||
    (value.workflowId === PLOT_CREATE_WORKFLOW_ID && operation.relativePath !== "plot/main.md") ||
    !operation.relativePath.startsWith(expectedPrefix)
  ) {
    throw new Error("Proposal target does not match Workflow");
  }
  const validationSummary = parseValidation(value.validationSummary);
  const warningCodes = parseCodes(value.warningCodes, 32);
  const lineDiff = value.lineDiff.map(parseDiffLine);
  if (
    new TextEncoder().encode(value.fullContent).byteLength !== operation.proposedByteCount ||
    (operation.kind === "create" && lineDiff.length !== 0) ||
    (operation.kind === "replace" && lineDiff.length === 0)
  ) {
    throw new Error("Proposal preview does not match operation metadata");
  }
  return {
    ...(value as Omit<
      CanonProposalView,
      "operation" | "validationSummary" | "warningCodes" | "lineDiff"
    >),
    operation,
    validationSummary,
    warningCodes,
    lineDiff,
  };
}
