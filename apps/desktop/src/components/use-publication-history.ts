import { useEffect, useState } from "react";
import { requestPublications, type PublicationReceipt } from "../lib/tauri/publication";

export function usePublicationRecords(projectSessionId: string | null, refreshKey: string) {
  const [attempt, setAttempt] = useState(0);
  const [pages, setPages] = useState(1);
  const [state, setState] = useState<{
    owner: string | null;
    items: PublicationReceipt[];
    more: boolean;
    failed: boolean;
    busy: boolean;
  }>({ owner: null, items: [], more: false, failed: false, busy: false });
  useEffect(() => {
    let disposed = false;
    const owner = projectSessionId;
    queueMicrotask(async () => {
      if (!owner || disposed) return;
      setState((previous) =>
        previous.owner === owner
          ? { ...previous, busy: true }
          : { owner, items: [], more: false, failed: false, busy: true },
      );
      try {
        let page = await requestPublications(owner);
        const items = [...page.items];
        const cursors = new Set<string>();
        for (let index = 1; index < pages && page.nextCursor !== null; index += 1) {
          if (disposed) return;
          if (cursors.has(page.nextCursor)) throw new Error("Repeated publication cursor");
          cursors.add(page.nextCursor);
          page = await requestPublications(owner, page.nextCursor);
          items.push(...page.items);
        }
        if (
          items.length > 256 ||
          new Set(items.map((item) => item.artifactId)).size !== items.length
        )
          throw new Error("Invalid publication history");
        if (!disposed)
          setState({ owner, items, more: page.nextCursor !== null, failed: false, busy: false });
      } catch {
        if (!disposed) setState((previous) => ({ ...previous, owner, failed: true, busy: false }));
      }
    });
    return () => {
      disposed = true;
    };
  }, [projectSessionId, refreshKey, attempt, pages]);
  return {
    items: state.owner === projectSessionId ? state.items : [],
    failed: state.owner === projectSessionId && state.failed,
    more: state.owner === projectSessionId && state.more,
    busy: state.owner === projectSessionId && state.busy,
    refresh: () => setAttempt((value) => value + 1),
    loadMore: () => setPages((value) => Math.min(13, value + 1)),
  };
}
