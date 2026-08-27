import { invoke } from "@tauri-apps/api/core";
import { normalizeAppError, type AppError } from "./health";
import type { StoryContextRequestPreviewInput } from "./story-context";

export const STORY_CONTINUITY_CHECK_COMMAND = "story_continuity_check";
export const STORY_CONTINUITY_STATUS_LOAD_COMMAND = "story_continuity_status_load";
export const STORY_CONTINUITY_WARNING_CONFIRM_COMMAND = "story_continuity_warning_confirm";
export const STORY_CONTINUITY_SCHEMA_VERSION = 1;

export type ContinuitySeverity = "blocking" | "warning" | "unknown" | "passed";
export type ContinuityRuleResult = {
  ruleId: string;
  severity: ContinuitySeverity;
  title: string;
  explanation: string;
  evidence: string[];
  sourceIds: string[];
  suggestedAction: string;
};
export type StoryContinuityReport = {
  schemaVersion: 1;
  reportHash: string;
  proposalHash: string;
  sourceManifestHash: string;
  currentManifestHash: string | null;
  generationTargetHash: string | null;
  currentTargetHash: string | null;
  checkedAt: number;
  localOnly: true;
  results: ContinuityRuleResult[];
  summary: { blocking: number; warning: number; unknown: number; passed: number };
  acknowledgedWarningRuleIds: string[];
  acknowledgedUnknownRuleIds: string[];
  warningsConfirmed: boolean;
  unknownsConfirmed: boolean;
  reviewReady: boolean;
  writeEnabled: false;
};
export type StoryContinuityCheckInput = StoryContextRequestPreviewInput & {
  runId: string;
  proposalHash: string;
};

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
const SHA256 = /^[0-9a-f]{64}$/u;
const TOKEN = /^[a-zA-Z0-9._-]{1,160}$/u;
const SEVERITIES = new Set<ContinuitySeverity>(["blocking", "warning", "unknown", "passed"]);
const RESULT_KEYS = [
  "evidence",
  "explanation",
  "ruleId",
  "severity",
  "sourceIds",
  "suggestedAction",
  "title",
];
const REPORT_KEYS = [
  "acknowledgedUnknownRuleIds",
  "acknowledgedWarningRuleIds",
  "checkedAt",
  "currentManifestHash",
  "currentTargetHash",
  "generationTargetHash",
  "localOnly",
  "proposalHash",
  "reportHash",
  "results",
  "reviewReady",
  "schemaVersion",
  "sourceManifestHash",
  "summary",
  "unknownsConfirmed",
  "warningsConfirmed",
  "writeEnabled",
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function exact(value: Record<string, unknown>, keys: string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}
function safeText(value: unknown, limit = 4_096): value is string {
  return (
    typeof value === "string" && value.length > 0 && value.length <= limit && value === value.trim()
  );
}
function hashOrNull(value: unknown): value is string | null {
  return value === null || (typeof value === "string" && SHA256.test(value));
}
function stringList(value: unknown, limit: number): value is string[] {
  return (
    Array.isArray(value) &&
    value.length <= limit &&
    value.every((item) => safeText(item, 2_048)) &&
    new Set(value).size === value.length
  );
}

function parseResult(value: unknown): ContinuityRuleResult {
  if (
    !isRecord(value) ||
    !exact(value, RESULT_KEYS) ||
    typeof value.ruleId !== "string" ||
    !TOKEN.test(value.ruleId) ||
    !SEVERITIES.has(value.severity as ContinuitySeverity) ||
    !safeText(value.title, 240) ||
    !safeText(value.explanation) ||
    !stringList(value.evidence, 128) ||
    !stringList(value.sourceIds, 128) ||
    !safeText(value.suggestedAction)
  )
    throw new Error("Invalid continuity rule result");
  return value as ContinuityRuleResult;
}

export function parseStoryContinuityReport(value: unknown): StoryContinuityReport {
  if (
    !isRecord(value) ||
    !exact(value, REPORT_KEYS) ||
    value.schemaVersion !== 1 ||
    typeof value.reportHash !== "string" ||
    !SHA256.test(value.reportHash) ||
    typeof value.proposalHash !== "string" ||
    !SHA256.test(value.proposalHash) ||
    typeof value.sourceManifestHash !== "string" ||
    !SHA256.test(value.sourceManifestHash) ||
    !hashOrNull(value.currentManifestHash) ||
    !hashOrNull(value.generationTargetHash) ||
    !hashOrNull(value.currentTargetHash) ||
    !Number.isSafeInteger(value.checkedAt) ||
    Number(value.checkedAt) < 0 ||
    value.localOnly !== true ||
    value.writeEnabled !== false ||
    typeof value.warningsConfirmed !== "boolean" ||
    typeof value.unknownsConfirmed !== "boolean" ||
    typeof value.reviewReady !== "boolean" ||
    !stringList(value.acknowledgedWarningRuleIds, 64) ||
    !stringList(value.acknowledgedUnknownRuleIds, 64) ||
    !Array.isArray(value.results) ||
    value.results.length !== 17 ||
    !isRecord(value.summary)
  )
    throw new Error("Invalid continuity report");
  const summary = value.summary;
  if (!exact(summary, ["blocking", "passed", "unknown", "warning"]))
    throw new Error("Invalid continuity summary");
  const results = value.results.map(parseResult);
  const ruleIds = results.map((item) => item.ruleId);
  if (new Set(ruleIds).size !== ruleIds.length) throw new Error("Duplicate continuity rule");
  const counts = { blocking: 0, warning: 0, unknown: 0, passed: 0 };
  for (const result of results) counts[result.severity] += 1;
  if (
    ["blocking", "warning", "unknown", "passed"].some(
      (key) =>
        !Number.isSafeInteger(summary[key]) ||
        Number(summary[key]) !== counts[key as keyof typeof counts],
    )
  )
    throw new Error("Invalid continuity summary");
  const warningIds = results
    .filter((item) => item.severity === "warning")
    .map((item) => item.ruleId);
  const unknownIds = results
    .filter((item) => item.severity === "unknown")
    .map((item) => item.ruleId);
  if (
    value.warningsConfirmed !==
      (warningIds.length === 0 ||
        warningIds.every((id) => (value.acknowledgedWarningRuleIds as string[]).includes(id))) ||
    value.unknownsConfirmed !==
      (unknownIds.length === 0 ||
        unknownIds.every((id) => (value.acknowledgedUnknownRuleIds as string[]).includes(id))) ||
    value.reviewReady !==
      (counts.blocking === 0 && value.warningsConfirmed && value.unknownsConfirmed)
  )
    throw new Error("Invalid continuity confirmation state");
  return { ...value, results, summary: counts } as StoryContinuityReport;
}

export function parseStoryContinuityStatus(value: unknown): {
  schemaVersion: 1;
  report: StoryContinuityReport | null;
} {
  if (!isRecord(value) || !exact(value, ["report", "schemaVersion"]) || value.schemaVersion !== 1)
    throw new Error("Invalid continuity status");
  return {
    schemaVersion: 1,
    report: value.report === null ? null : parseStoryContinuityReport(value.report),
  };
}

function requestPayload(input: StoryContinuityCheckInput) {
  return {
    schemaVersion: 1,
    runId: input.runId,
    proposalHash: input.proposalHash,
    request: {
      schemaVersion: 1,
      context: {
        schemaVersion: 2,
        action: input.action,
        chapterNumber: input.chapterNumber,
        chapterTitle: input.chapterTitle.trim(),
        selectedSourcePaths: [...input.selectedSourcePaths].sort(),
      },
      reviewedManifestHash: input.reviewedManifestHash,
      readerMentalModel: { ...input.readerMentalModel },
    },
  };
}
const invokeTauri: InvokeCommand = (command, args) => invoke<unknown>(command, args);
function contractError(): AppError {
  return { code: "contract.invalid_response", messageKey: "errors.invalidRuntimeResponse" };
}

export async function requestStoryContinuityCheck(
  sessionId: string,
  input: StoryContinuityCheckInput,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<StoryContinuityReport> {
  if (
    !TOKEN.test(input.runId) ||
    !SHA256.test(input.proposalHash) ||
    !SHA256.test(input.reviewedManifestHash)
  )
    throw {
      code: "workflow.input_invalid",
      messageKey: "errors.workflowInputInvalid",
    } satisfies AppError;
  try {
    const report = parseStoryContinuityReport(
      await invokeCommand(STORY_CONTINUITY_CHECK_COMMAND, {
        sessionId,
        input: requestPayload(input),
      }),
    );
    if (report.proposalHash !== input.proposalHash) throw new Error("Continuity binding mismatch");
    return report;
  } catch (error: unknown) {
    if (error instanceof Error) throw contractError();
    throw normalizeAppError(error);
  }
}

export async function requestStoryContinuityStatus(
  sessionId: string,
  invokeCommand: InvokeCommand = invokeTauri,
) {
  try {
    return parseStoryContinuityStatus(
      await invokeCommand(STORY_CONTINUITY_STATUS_LOAD_COMMAND, { sessionId }),
    );
  } catch (error: unknown) {
    if (error instanceof Error) throw contractError();
    throw normalizeAppError(error);
  }
}

export async function requestStoryContinuityWarningConfirm(
  sessionId: string,
  runId: string,
  report: StoryContinuityReport,
  invokeCommand: InvokeCommand = invokeTauri,
): Promise<StoryContinuityReport> {
  const warningRuleIds = report.results
    .filter((item) => item.severity === "warning")
    .map((item) => item.ruleId);
  const unknownRuleIds = report.results
    .filter((item) => item.severity === "unknown")
    .map((item) => item.ruleId);
  try {
    return parseStoryContinuityReport(
      await invokeCommand(STORY_CONTINUITY_WARNING_CONFIRM_COMMAND, {
        sessionId,
        input: {
          schemaVersion: 1,
          runId,
          reportHash: report.reportHash,
          warningRuleIds,
          unknownRuleIds,
          confirmed: true,
        },
      }),
    );
  } catch (error: unknown) {
    if (error instanceof Error) throw contractError();
    throw normalizeAppError(error);
  }
}
