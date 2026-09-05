import { useState } from "react";
import { normalizeAppError } from "../lib/tauri/health";
import {
  requestCanonChangeDecision,
  requestCanonChangeDetail,
  type ChangeActivity,
  type ChangeDetail,
} from "../lib/tauri/canon-change";

export function CanonChangeView({
  sessionId,
  activity,
  onChanged,
}: {
  sessionId: string;
  activity: ChangeActivity;
  onChanged: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const [detail, setDetail] = useState<ChangeDetail | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const receipt = activity.receipt;
  const label =
    receipt.status === "committed"
      ? `已保存 ${receipt.files.length} 项资料`
      : receipt.status === "pending_confirmation"
        ? "待你确认，尚未保存"
        : receipt.status === "no_change"
          ? "资料没有变化，无需保存"
          : receipt.status === "recovery_required"
            ? "部分写入待恢复，请先核对"
            : receipt.status === "undone"
              ? "已撤销本次更改"
              : receipt.status === "applying" || receipt.status === "prepared"
                ? "正在核对保存结果"
                : "本次更改未保存";
  const showDiff = async () => {
    if (expanded) {
      setExpanded(false);
      return;
    }
    setExpanded(true);
    setBusy(true);
    setError(null);
    try {
      setDetail(await requestCanonChangeDetail(sessionId, receipt.changeId));
    } catch (failure: unknown) {
      setError(normalizeAppError(failure).code);
    } finally {
      setBusy(false);
    }
  };
  const decide = async (decision: "confirm" | "discard" | "undo" | "retry_recovery") => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await requestCanonChangeDecision(sessionId, receipt, decision);
      setExpanded(false);
      setDetail(null);
    } catch (failure: unknown) {
      setError(normalizeAppError(failure).code);
    } finally {
      setBusy(false);
      // A failed inverse transaction may have entered recovery_required.
      // Always reconcile durable truth, not just successful acknowledgements.
      onChanged();
    }
  };
  return (
    <section className="canon-change" aria-label="资料保存结果">
      <p className="canon-change-status">{label}</p>
      <div className="canon-change-actions">
        <button
          type="button"
          aria-expanded={expanded}
          disabled={busy}
          onClick={() => void showDiff()}
        >
          {expanded ? "收起更改" : "查看更改"}
        </button>
        {receipt.status === "pending_confirmation" ? (
          <>
            <button type="button" disabled={busy} onClick={() => void decide("confirm")}>
              确认保存
            </button>
            <button type="button" disabled={busy} onClick={() => void decide("discard")}>
              放弃
            </button>
          </>
        ) : null}
        {receipt.status === "committed" ? (
          <button type="button" disabled={busy} onClick={() => void decide("undo")}>
            撤销本次更改
          </button>
        ) : null}
        {receipt.status === "recovery_required" ? (
          <button type="button" disabled={busy} onClick={() => void decide("retry_recovery")}>
            重试安全恢复
          </button>
        ) : null}
        {busy ? <span role="status">正在核对…</span> : null}
      </div>
      {error ? (
        <p role="alert">
          {error === "canon.change_store_full"
            ? "变更库配额不足，本次操作未执行；不会自动删除记录或作者文件来腾空间。"
            : error === "canon.change_io" || error === "canon.change_recovery_required"
              ? "变更记录或快照暂不可用，无法确认撤销是否可执行；请保留文件与记录并核对保存状态。"
              : null}
          本次操作未完成。若原文或编辑内容已变化，请先保存或放弃手工编辑，再让 AI
          重新读取并准备更改；不会强制覆盖。
          <small>{error}</small>
        </p>
      ) : null}
      {receipt.status === "committed" ? (
        <p>撤销仅在全部目标仍为本次保存版本时执行；更新恢复原文，新建移除本次创建的文件。</p>
      ) : null}
      {receipt.status === "recovery_required" ? (
        <p>
          不会覆盖后来的外部修改。请先核对差异并保留当前文件；恢复完成后重新打开工程刷新写入状态。
        </p>
      ) : null}
      {expanded && detail ? (
        <div className="canon-change-diff">
          <p>
            以下差异来自 Host
            留存的原文与候选内容；是否已保存以上方状态为准。“−”为移除，“＋”为新增。
          </p>
          {detail.files.map((file) => (
            <div key={file.relativePath}>
              <h4>{file.relativePath}</h4>
              <pre aria-label={`${file.relativePath} 的更改`}>
                {file.lines.map((line, index) => (
                  <span key={index} data-diff={line.kind}>
                    {line.kind === "added" ? "+" : line.kind === "removed" ? "−" : " "}
                    {line.text}
                  </span>
                ))}
              </pre>
            </div>
          ))}
        </div>
      ) : null}
    </section>
  );
}
