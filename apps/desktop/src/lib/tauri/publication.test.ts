import { describe, expect, it, vi } from "vitest";
import {
  parsePublicationDetail,
  parsePublicationPage,
  parsePublicationReceipt,
  requestPublicationDetail,
  requestPublications,
  type PublicationReceipt,
} from "./publication";
const id = "10000000-0000-4000-8000-000000000001";
export const receipt: PublicationReceipt = {
  schemaVersion: 1,
  artifactId: id,
  sessionId: id,
  turnId: id,
  status: "generated",
  chapterCount: 1,
  skippedDrafts: 0,
  withoutCover: true,
  relativePath: `published/${id}.epub`,
  outputHash: "a".repeat(64),
  sizeBytes: 2048,
  issue: null,
};
const detail = {
  receipt,
  inputs: [
    {
      relativePath: "chapters/第一章.md",
      contentHash: "b".repeat(64),
      sizeBytes: 1024,
      confirmationId: id,
      confirmationRevision: 1,
    },
    {
      relativePath: "project.md",
      contentHash: "c".repeat(64),
      sizeBytes: 128,
      confirmationId: null,
      confirmationRevision: null,
    },
  ],
  validationScope: "本地结构检查，不是第三方认证。",
};
describe("NR-505 strict publication contracts", () => {
  it("requires every nullable field and separates generated from preflight or failure", () => {
    expect(parsePublicationReceipt(receipt)).toEqual(receipt);
    for (const key of Object.keys(receipt)) {
      const bad = { ...receipt } as Record<string, unknown>;
      delete bad[key];
      expect(() => parsePublicationReceipt(bad)).toThrow();
    }
    for (const bad of [
      { ...receipt, status: "completed" },
      { ...receipt, outputHash: null },
      { ...receipt, relativePath: "C:/outside.epub" },
      { ...receipt, sizeBytes: 112 * 1024 * 1024 + 1 },
      { ...receipt, status: "validation_failed" },
      { ...receipt, hiddenPrompt: "forbidden" },
      { ...receipt, chapterCount: 0 },
      { ...receipt, status: "preflight_passed" },
    ])
      expect(() => parsePublicationReceipt(bad)).toThrow();
  });
  it("rejects duplicate pages, private input paths and mismatched confirmation pairs", () => {
    expect(
      parsePublicationPage({ schemaVersion: 1, items: [receipt], nextCursor: null }).items,
    ).toHaveLength(1);
    expect(() =>
      parsePublicationPage({ schemaVersion: 1, items: [receipt, receipt], nextCursor: null }),
    ).toThrow();
    expect(parsePublicationDetail(detail).inputs).toHaveLength(2);
    expect(() =>
      parsePublicationDetail({ ...detail, inputs: detail.inputs.slice(0, 1) }),
    ).toThrow();
    for (const relativePath of [
      ".novelgainker/tasks/session.json",
      "../outside.md",
      "assets/CON.png",
      "assets/a.svg",
      "chapters/../secret.md",
      "chapters/sub/a.md",
    ])
      expect(() =>
        parsePublicationDetail({ ...detail, inputs: [{ ...detail.inputs[0], relativePath }] }),
      ).toThrow();
    expect(() =>
      parsePublicationDetail({
        ...detail,
        inputs: [{ ...detail.inputs[0], confirmationId: null }],
      }),
    ).toThrow();
  });
  it("uses only closed read IPC and checks returned artifact identity", async () => {
    const call = vi
      .fn()
      .mockResolvedValue({ schemaVersion: 1, items: [receipt], nextCursor: null });
    await requestPublications(id, null, call);
    expect(call).toHaveBeenCalledWith("publication_list", { sessionId: id, cursor: null });
    call.mockResolvedValue(detail);
    await requestPublicationDetail(id, id, call);
    expect(call).toHaveBeenLastCalledWith("publication_detail", { sessionId: id, artifactId: id });
    await expect(
      requestPublicationDetail(id, "20000000-0000-4000-8000-000000000002", call),
    ).rejects.toMatchObject({ code: "contract.invalid_response" });
  });
});
