import { invoke } from "@tauri-apps/api/core";
import { normalizeAppError } from "./health";

export const PUBLICATION_LIST_COMMAND = "publication_list";
export const PUBLICATION_DETAIL_COMMAND = "publication_detail";
const issues = [
  "metadata_required",
  "no_final_chapters",
  "final_confirmation_required",
  "invalid_chapter",
  "unsupported_markdown",
  "invalid_asset",
  "unsafe_path",
  "limit_exceeded",
  "source_changed",
  "local_validation_failed",
  "store_unavailable",
  "source_required",
  "cancelled",
] as const;
export type PublicationIssue = (typeof issues)[number];
export type PublicationReceipt = {
  schemaVersion: 1;
  artifactId: string;
  sessionId: string;
  turnId: string;
  status: "preflight_passed" | "generated" | "validation_failed" | "stale";
  chapterCount: number;
  skippedDrafts: number;
  withoutCover: boolean;
  relativePath: string | null;
  outputHash: string | null;
  sizeBytes: number | null;
  issue: PublicationIssue | null;
};
export type PublicationInput = {
  relativePath: string;
  contentHash: string;
  sizeBytes: number;
  confirmationId: string | null;
  confirmationRevision: number | null;
};
export type PublicationDetail = {
  receipt: PublicationReceipt;
  inputs: PublicationInput[];
  validationScope: string;
};
export type PublicationPage = {
  schemaVersion: 1;
  items: PublicationReceipt[];
  nextCursor: string | null;
};
type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
const nativeInvoke: Invoke = (command, args) => invoke(command, args);
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;
const hash = /^[0-9a-f]{64}$/u;
function id(value: unknown): value is string {
  return typeof value === "string" && uuid.test(value);
}
function integer(value: unknown, max: number): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0 && Number(value) <= max;
}
function object(value: unknown, keys: string[]): asserts value is Record<string, unknown> {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    Object.keys(value).sort().join(",") !== [...keys].sort().join(",")
  )
    throw new Error("Invalid publication shape");
}
export function parsePublicationReceipt(value: unknown): PublicationReceipt {
  object(value, [
    "schemaVersion",
    "artifactId",
    "sessionId",
    "turnId",
    "status",
    "chapterCount",
    "skippedDrafts",
    "withoutCover",
    "relativePath",
    "outputHash",
    "sizeBytes",
    "issue",
  ]);
  const output = value.relativePath !== null;
  if (
    value.schemaVersion !== 1 ||
    !id(value.artifactId) ||
    !id(value.sessionId) ||
    !id(value.turnId) ||
    !["preflight_passed", "generated", "validation_failed", "stale"].includes(
      String(value.status),
    ) ||
    !integer(value.chapterCount, 500) ||
    !integer(value.skippedDrafts, 500) ||
    Number(value.chapterCount) + Number(value.skippedDrafts) > 500 ||
    typeof value.withoutCover !== "boolean" ||
    output !== (value.outputHash !== null) ||
    output !== (value.sizeBytes !== null) ||
    (output &&
      (value.relativePath !== `published/${value.artifactId}.epub` ||
        typeof value.outputHash !== "string" ||
        !hash.test(value.outputHash) ||
        !integer(value.sizeBytes, 112 * 1024 * 1024) ||
        value.sizeBytes === 0)) ||
    (value.issue !== null && !issues.includes(value.issue as PublicationIssue)) ||
    (value.status === "generated" &&
      (!output || value.issue !== null || value.chapterCount === 0)) ||
    (value.status === "preflight_passed" &&
      (output || value.issue !== null || value.chapterCount === 0)) ||
    (["validation_failed", "stale"].includes(String(value.status)) && value.issue === null)
  )
    throw new Error("Invalid publication receipt");
  return value as PublicationReceipt;
}
export function parsePublicationPage(value: unknown): PublicationPage {
  object(value, ["schemaVersion", "items", "nextCursor"]);
  if (
    value.schemaVersion !== 1 ||
    !Array.isArray(value.items) ||
    value.items.length > 20 ||
    (value.nextCursor !== null &&
      (typeof value.nextCursor !== "string" ||
        !/^[0-9a-f]{64}:[0-9]{1,3}$/u.test(value.nextCursor)))
  )
    throw new Error("Invalid publication page");
  const items = value.items.map(parsePublicationReceipt);
  if (new Set(items.map((item) => item.artifactId)).size !== items.length)
    throw new Error("Duplicate publication");
  return { schemaVersion: 1, items, nextCursor: value.nextCursor as string | null };
}
function inputPath(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length <= 512 &&
    (value === "project.md" ||
      /^chapters\/[^/]+\.md$/u.test(value) ||
      /^assets\/.+\.(png|jpe?g)$/iu.test(value)) &&
    !/[\\:?%#]/u.test(value) &&
    !Array.from(value).some(
      (character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127,
    ) &&
    !value
      .split("/")
      .some(
        (part) =>
          !part ||
          part.startsWith(".") ||
          /[. ]$/u.test(part) ||
          part.trim() !== part ||
          /^(con|prn|aux|nul|conin\$|conout\$|com[1-9]|lpt[1-9])(?:\.|$)/iu.test(part),
      )
  );
}
export function parsePublicationDetail(value: unknown): PublicationDetail {
  object(value, ["receipt", "inputs", "validationScope"]);
  const receipt = parsePublicationReceipt(value.receipt);
  if (
    !Array.isArray(value.inputs) ||
    value.inputs.length > 701 ||
    typeof value.validationScope !== "string" ||
    value.validationScope.length > 512 ||
    new TextEncoder().encode(JSON.stringify(value)).length > 512 * 1024
  )
    throw new Error("Invalid publication detail");
  const inputs = value.inputs.map((input) => {
    object(input, [
      "relativePath",
      "contentHash",
      "sizeBytes",
      "confirmationId",
      "confirmationRevision",
    ]);
    if (
      !inputPath(input.relativePath) ||
      typeof input.contentHash !== "string" ||
      !hash.test(input.contentHash) ||
      !integer(
        input.sizeBytes,
        input.relativePath.startsWith("assets/") ? 16 * 1024 * 1024 : 32 * 1024,
      ) ||
      (input.confirmationId !== null) !== (input.confirmationRevision !== null) ||
      (input.confirmationId !== null &&
        (!id(input.confirmationId) ||
          !input.relativePath.startsWith("chapters/") ||
          !integer(input.confirmationRevision, Number.MAX_SAFE_INTEGER) ||
          input.confirmationRevision === 0))
    )
      throw new Error("Invalid publication input");
    return input as PublicationInput;
  });
  if (new Set(inputs.map((input) => input.relativePath)).size !== inputs.length)
    throw new Error("Duplicate input");
  const chapters = inputs.filter((input) => input.relativePath.startsWith("chapters/"));
  const assets = inputs.filter((input) => input.relativePath.startsWith("assets/"));
  if (
    chapters.length > 500 ||
    assets.length > 200 ||
    chapters.reduce((total, input) => total + input.sizeBytes, 0) > 8 * 1024 * 1024 ||
    assets.reduce((total, input) => total + input.sizeBytes, 0) > 64 * 1024 * 1024 ||
    ((receipt.relativePath !== null || receipt.status === "preflight_passed") &&
      (inputs.filter((input) => input.relativePath === "project.md").length !== 1 ||
        chapters.length !== receipt.chapterCount + receipt.skippedDrafts ||
        chapters.filter((input) => input.confirmationId !== null).length !== receipt.chapterCount))
  )
    throw new Error("Invalid publication input totals or confirmation evidence");
  return { receipt, inputs, validationScope: value.validationScope };
}
async function request<T>(
  command: string,
  args: Record<string, unknown>,
  parse: (value: unknown) => T,
  call: Invoke,
): Promise<T> {
  let value: unknown;
  try {
    value = await call(command, args);
  } catch (error: unknown) {
    throw normalizeAppError(error);
  }
  try {
    return parse(value);
  } catch {
    throw { code: "contract.invalid_response", messageKey: "errors.invalidRuntimeResponse" };
  }
}
export function requestPublications(
  sessionId: string,
  cursor: string | null = null,
  call: Invoke = nativeInvoke,
) {
  return request(PUBLICATION_LIST_COMMAND, { sessionId, cursor }, parsePublicationPage, call);
}
export function requestPublicationDetail(
  sessionId: string,
  artifactId: string,
  call: Invoke = nativeInvoke,
) {
  return request(
    PUBLICATION_DETAIL_COMMAND,
    { sessionId, artifactId },
    (value) => {
      const detail = parsePublicationDetail(value);
      if (detail.receipt.artifactId !== artifactId) throw new Error("Mismatched artifact");
      return detail;
    },
    call,
  );
}
