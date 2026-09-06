import { afterEach, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import * as api from "../lib/tauri/canon-change";
import { ChapterCandidateView } from "./ChapterCandidateView";

const candidate: api.ChapterCandidateActivity = {
  candidateId: "11111111-1111-4111-8111-111111111111",
  turnId: "22222222-2222-4222-8222-222222222222",
  contentHash: "a".repeat(64),
};
afterEach(() => vi.restoreAllMocks());

it("discards only the opaque candidate and never offers save or arbitrary bytes", async () => {
  const call = vi.spyOn(api, "requestCanonChangeDecision").mockResolvedValue({
    schemaVersion: 1,
    changeId: candidate.candidateId,
    revision: 2,
    status: "rejected",
    files: [],
  });
  const changed = vi.fn();
  render(
    <ChapterCandidateView sessionId="project-session" candidate={candidate} onChanged={changed} />,
  );
  expect(screen.queryByRole("button", { name: /保存/ })).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "放弃此候选" }));
  expect(
    await screen.findByText("候选已放弃，聊天内容仍保留；未修改章节文件。"),
  ).toBeInTheDocument();
  expect(call).toHaveBeenCalledWith(
    "project-session",
    {
      schemaVersion: 1,
      changeId: candidate.candidateId,
      revision: 1,
      status: "pending_confirmation",
      files: [],
    },
    "discard",
    undefined,
    null,
  );
  expect(changed).toHaveBeenCalledOnce();
});

it("does not claim discard on failure or a mismatched saved receipt", async () => {
  vi.spyOn(api, "requestCanonChangeDecision").mockResolvedValue({
    schemaVersion: 1,
    changeId: candidate.candidateId,
    revision: 2,
    status: "committed",
    files: [],
  });
  const changed = vi.fn();
  render(
    <ChapterCandidateView sessionId="project-session" candidate={candidate} onChanged={changed} />,
  );
  await userEvent.click(screen.getByRole("button", { name: "放弃此候选" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("未能确认放弃结果");
  expect(screen.getByRole("button", { name: "放弃此候选" })).toBeEnabled();
  expect(changed).not.toHaveBeenCalled();
});

it("rejects unknown, duplicated, oversized and unscoped candidate DTOs", () => {
  const page = { schemaVersion: 1, items: [], nextCursor: null, candidates: [candidate] };
  expect(api.parseChangePage(page).candidates).toEqual([candidate]);
  for (const candidates of [
    [{ ...candidate, path: "chapters/a.md" }],
    [candidate, candidate],
    Array(9).fill(candidate),
    [{ ...candidate, turnId: "bad" }],
    [{ ...candidate, contentHash: "bad" }],
  ]) {
    expect(() => api.parseChangePage({ ...page, candidates })).toThrow();
  }
});
