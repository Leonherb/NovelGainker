import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PublicationView } from "./PublicationView";
import * as api from "../lib/tauri/publication";
const id = "10000000-0000-4000-8000-000000000001";
const receipt: api.PublicationReceipt = {
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
describe("NR-505 inline publication facts", () => {
  afterEach(() => vi.restoreAllMocks());
  it("shows generated fact and only reads detailed evidence on explicit expansion", async () => {
    const load = vi.spyOn(api, "requestPublicationDetail").mockResolvedValue({
      receipt,
      inputs: [],
      validationScope: "本地结构检查；不是第三方认证。",
    });
    render(<PublicationView projectSessionId={id} receipt={receipt} />);
    expect(screen.getByText("EPUB 已生成（本地产物，未上架）")).toBeInTheDocument();
    expect(screen.getByText("1 章 · 无封面 EPUB")).toBeInTheDocument();
    const button = screen.getByRole("button", { name: "查看出版依据" });
    expect(button).toHaveAttribute("aria-expanded", "false");
    expect(load).not.toHaveBeenCalled();
    fireEvent.click(button);
    await screen.findByText(receipt.relativePath!);
    expect(screen.getByRole("button", { name: "收起出版依据" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(load).toHaveBeenCalledOnce();
  });
  it("replaces current-valid presentation with stale evidence without claiming an overwrite", async () => {
    vi.spyOn(api, "requestPublicationDetail").mockResolvedValue({
      receipt: { ...receipt, status: "stale", issue: "source_changed" },
      inputs: [],
      validationScope: "仅本地验证",
    });
    render(<PublicationView projectSessionId={id} receipt={receipt} />);
    fireEvent.click(screen.getByRole("button", { name: "查看出版依据" }));
    await screen.findByText("EPUB 输入已过期，不能作为当前版本");
    expect(screen.queryByText("EPUB 已生成（本地产物，未上架）")).not.toBeInTheDocument();
    expect(screen.getByText(receipt.relativePath!)).toBeInTheDocument();
  });
  it("keeps prior receipt on query failure and offers read-only recheck, not rebuild", async () => {
    const load = vi
      .spyOn(api, "requestPublicationDetail")
      .mockRejectedValue(new Error("offline fixture"));
    render(<PublicationView projectSessionId={id} receipt={receipt} />);
    fireEvent.click(screen.getByRole("button", { name: "查看出版依据" }));
    await screen.findByText(/以上是先前回执/);
    fireEvent.click(screen.getByRole("button", { name: "重新核对出版依据" }));
    await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
    expect(screen.getByText("EPUB 已生成（本地产物，未上架）")).toBeInTheDocument();
  });
});
