import { useState } from "react";
import {
  requestCanonChangeDecision,
  type ChapterCandidateActivity,
} from "../lib/tauri/canon-change";

export function ChapterCandidateView({
  sessionId,
  candidate,
  conversationSessionId = null,
  onChanged,
}: {
  sessionId: string;
  candidate: ChapterCandidateActivity;
  conversationSessionId?: string | null;
  onChanged: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [discarded, setDiscarded] = useState(false);
  const [failed, setFailed] = useState(false);
  const discard = async () => {
    if (busy || discarded) return;
    setBusy(true);
    setFailed(false);
    try {
      const receipt = await requestCanonChangeDecision(
        sessionId,
        {
          schemaVersion: 1,
          changeId: candidate.candidateId,
          revision: 1,
          status: "pending_confirmation",
          files: [],
        },
        "discard",
        undefined,
        conversationSessionId,
      );
      if (
        receipt.changeId !== candidate.candidateId ||
        receipt.status !== "rejected" ||
        receipt.revision !== 2 ||
        receipt.files.length !== 0
      )
        throw new Error("Invalid candidate discard");
      setDiscarded(true);
      onChanged();
    } catch {
      setFailed(true);
    } finally {
      setBusy(false);
    }
  };
  return (
    <section className="canon-change-view" aria-label="未保存的章节候选">
      <p>
        {discarded
          ? "候选已放弃，聊天内容仍保留；未修改章节文件。"
          : "这是未保存的章节候选。可以继续讨论，或明确告诉 AI 保存这一版。"}
      </p>
      {!discarded && (
        <button type="button" disabled={busy} onClick={() => void discard()}>
          {busy ? "正在放弃候选…" : "放弃此候选"}
        </button>
      )}
      {failed && <p role="alert">未能确认放弃结果，聊天内容和章节文件均保留；可稍后重试。</p>}
    </section>
  );
}
