import { describe, expect, it, vi } from "vitest";
import {
  parseChangeActivities,
  parseChangeDetail,
  parseChangeReceipt,
  parseChangePage,
  requestCanonChangesPage,
  requestCanonChangeDecision,
  requestEditorState,
  type ChangeActivity,
} from "./canon-change";

const id = "10000000-0000-4000-8000-000000000001";
const activity: ChangeActivity = {
  conversationSessionId: id,
  turnId: id,
  receipt: {
    schemaVersion: 1,
    changeId: id,
    revision: 1,
    status: "pending_confirmation",
    files: [{ relativePath: "characters/主角.md", beforeHash: null, afterHash: "a".repeat(64) }],
  },
};
describe("NR-404 closed local change contracts", () => {
  it("accepts only bounded Host receipts and exact diff targets", () => {
    expect(parseChangeActivities([activity])).toEqual([activity]);
    expect(() => parseChangeActivities([activity, activity])).toThrow();
    expect(() => parseChangeReceipt({ ...activity.receipt, rawResult: "secret" })).toThrow();
    expect(() => parseChangeReceipt({ ...activity.receipt, status: "saved" })).toThrow();
    expect(() => parseChangeReceipt({ ...activity.receipt, revision: 0 })).toThrow();
    for (const relativePath of [
      "../x.md",
      "chapters/nested/x.md",
      "chapters/CON.md",
      "characters/../x.md",
      "C:/x.md",
    ]) {
      expect(() =>
        parseChangeReceipt({
          ...activity.receipt,
          files: [{ ...activity.receipt.files[0], relativePath }],
        }),
      ).toThrow();
    }
    const detail = {
      activity,
      files: [
        {
          relativePath: "characters/主角.md",
          lines: [{ kind: "added", text: "<script>not markup</script>\n" }],
        },
      ],
    };
    expect(parseChangeDetail(detail)).toEqual(detail);
    expect(() => parseChangeDetail({ ...detail, files: [] })).toThrow();
    expect(() =>
      parseChangeDetail({
        ...detail,
        files: [{ ...detail.files[0], lines: [{ kind: "added", text: "文".repeat(24000) }] }],
      }),
    ).toThrow();
  });
  it("NR-502 reads bounded chapter pages with closed nullable selectors", async () => {
    const chapter = {
      ...activity,
      receipt: {
        ...activity.receipt,
        files: [{ ...activity.receipt.files[0], relativePath: "chapters/第一章.md" }],
      },
    };
    const page = { schemaVersion: 1, items: [chapter], nextCursor: "opaque_next" };
    expect(parseChangePage(page)).toEqual(page);
    for (const invalid of [
      { ...page, nextCursor: "../bad" },
      { ...page, extra: true },
      { ...page, items: [] },
      { ...page, items: Array.from({ length: 101 }, () => chapter) },
    ])
      expect(() => parseChangePage(invalid)).toThrow();
    const call = vi.fn().mockResolvedValue(page);
    await expect(requestCanonChangesPage(id, null, null, call)).resolves.toEqual(page);
    expect(call).toHaveBeenCalledExactlyOnceWith("canon_change_list", {
      request: { projectSessionId: id, turnId: null, cursor: null, limit: 100 },
    });
  });
  it("sends only opaque identifiers and a CAS decision, not candidate bytes", async () => {
    const call = vi.fn().mockResolvedValue(activity.receipt);
    await requestCanonChangeDecision(id, activity.receipt, "confirm", call);
    expect(call).toHaveBeenCalledExactlyOnceWith("canon_change_decide", {
      request: { projectSessionId: id, changeId: id, revision: 1, decision: "confirm" },
    });
    call.mockResolvedValue({ ...activity.receipt, extra: true });
    await expect(
      requestCanonChangeDecision(id, activity.receipt, "discard", call),
    ).rejects.toMatchObject({ code: "contract.invalid_response" });
  });
  it("requires exact Host acknowledgement before editor acquisition", async () => {
    const call = vi.fn().mockResolvedValue(2);
    await expect(requestEditorState(id, id, 2, true, call)).resolves.toBe(2);
    expect(call).toHaveBeenCalledExactlyOnceWith("canon_editor_state", {
      request: { projectSessionId: id, fileId: id, revision: 2, dirty: true },
    });
    await expect(requestEditorState(id, id, 3, false, call)).rejects.toMatchObject({
      code: "contract.invalid_response",
    });
  });
});
