import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import * as api from "../lib/tauri/publication";
import { usePublicationRecords } from "./use-publication-history";
afterEach(() => vi.restoreAllMocks());
it("NR-505 discards an old project's delayed publication response", async () => {
  let complete: ((value: api.PublicationPage) => void) | undefined;
  vi.spyOn(api, "requestPublications").mockImplementation(async (owner) =>
    owner === "old"
      ? new Promise((resolve) => {
          complete = resolve;
        })
      : { schemaVersion: 1, items: [], nextCursor: null },
  );
  const view = renderHook(({ owner }) => usePublicationRecords(owner, "stable"), {
    initialProps: { owner: "old" },
  });
  await waitFor(() => expect(complete).toBeDefined());
  view.rerender({ owner: "new" });
  await waitFor(() => expect(view.result.current.busy).toBe(false));
  await act(async () =>
    complete?.({ schemaVersion: 1, items: [], nextCursor: "a".repeat(64) + ":20" }),
  );
  expect(view.result.current.more).toBe(false);
  expect(view.result.current.failed).toBe(false);
});
