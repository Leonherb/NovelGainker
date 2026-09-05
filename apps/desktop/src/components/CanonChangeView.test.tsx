import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CanonChangeView } from "./CanonChangeView";
import * as api from "../lib/tauri/canon-change";
const id = "10000000-0000-4000-8000-000000000001";
const activity: api.ChangeActivity = {
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
describe("NR-404 actual Host save feedback", () => {
  it("NR-405 invokes local Undo and refreshes durable truth even on recovery failure", async () => {
    const committed = {
      ...activity,
      receipt: { ...activity.receipt, revision: 3, status: "committed" as const },
    };
    const decide = vi.spyOn(api, "requestCanonChangeDecision").mockRejectedValue({
      code: "canon.change_recovery_required",
      messageKey: "errors.fileSaveFailed",
    });
    const changed = vi.fn();
    const view = render(
      <CanonChangeView sessionId={id} activity={committed} onChanged={changed} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "撤销本次更改" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("变更记录或快照暂不可用");
    expect(decide).toHaveBeenCalledExactlyOnceWith(id, committed.receipt, "undo");
    expect(changed).toHaveBeenCalledOnce();
    view.rerender(
      <CanonChangeView
        sessionId={id}
        activity={{
          ...activity,
          receipt: { ...activity.receipt, revision: 5, status: "recovery_required" },
        }}
        onChanged={changed}
      />,
    );
    expect(screen.getByText("部分写入待恢复，请先核对")).toBeInTheDocument();
    expect(screen.queryByText("已保存 1 项资料")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重试安全恢复" })).toBeEnabled();
  });
  beforeEach(() => vi.restoreAllMocks());
  it("does not claim pending content is saved, and renders literal local diff safely", async () => {
    const detail = vi.spyOn(api, "requestCanonChangeDetail").mockResolvedValue({
      activity,
      files: [
        {
          relativePath: "characters/主角.md",
          lines: [{ kind: "added", text: "<script>not markup</script>\n" }],
        },
      ],
    });
    render(<CanonChangeView sessionId={id} activity={activity} onChanged={vi.fn()} />);
    expect(screen.getByText("待你确认，尚未保存")).toBeInTheDocument();
    expect(screen.queryByText(/已保存 1/u)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "查看更改" }));
    expect(await screen.findByText("+<script>not markup</script>")).toBeInTheDocument();
    expect(document.querySelector("script")).toBeNull();
    expect(detail).toHaveBeenCalledExactlyOnceWith(id, id);
  });
  it("disables repeat confirmation, uses local CAS and waits for refreshed Host truth", async () => {
    let finish!: (receipt: api.ChangeReceipt) => void;
    const decide = vi.spyOn(api, "requestCanonChangeDecision").mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    const changed = vi.fn();
    const view = render(<CanonChangeView sessionId={id} activity={activity} onChanged={changed} />);
    const button = screen.getByRole("button", { name: "确认保存" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(button).toBeDisabled();
    expect(decide).toHaveBeenCalledExactlyOnceWith(id, activity.receipt, "confirm");
    const receipt = { ...activity.receipt, revision: 3, status: "committed" as const };
    await act(async () => finish(receipt));
    expect(changed).toHaveBeenCalledOnce();
    expect(screen.queryByText("已保存 1 项资料")).not.toBeInTheDocument();
    view.rerender(
      <CanonChangeView sessionId={id} activity={{ ...activity, receipt }} onChanged={changed} />,
    );
    expect(screen.getByText("已保存 1 项资料")).toBeInTheDocument();
  });
  it("keeps stale confirmation unsaved and leaves conversation controls untouched", async () => {
    vi.spyOn(api, "requestCanonChangeDecision").mockRejectedValue({
      code: "canon.source_changed",
      messageKey: "errors.fileSaveFailed",
    });
    render(<CanonChangeView sessionId={id} activity={activity} onChanged={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "确认保存" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("不会强制覆盖");
    expect(screen.getByText("待你确认，尚未保存")).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });
});
