import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { SourceDiagnostics } from "./SourceDiagnostics";

describe("NG-105 source diagnostics UI", () => {
  it("marks future actions paused while keeping the structural issue navigable", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    render(
      <SourceDiagnostics
        error={null}
        onNavigate={onNavigate}
        stale={false}
        report={{
          relativePath: "chapters/001.md",
          contentHash: "a".repeat(64),
          diagnostics: [
            {
              code: "chapter.draft_heading_missing",
              severity: "error",
              relativePath: "chapters/001.md",
              line: 6,
              column: 1,
              message: "章节缺少严格的 ## Draft 标题。",
              recovery: "请添加严格标题后重试。",
            },
          ],
          eligibleForAiApply: false,
          eligibleForFinal: false,
          eligibleForEpub: false,
        }}
      />,
    );

    expect(screen.getByText("发现结构问题")).toBeInTheDocument();
    expect(screen.getAllByText(/暂停/u)).toHaveLength(3);
    expect(screen.getByText(/chapters\/001\.md:6:1/u)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /章节缺少严格/u }));
    expect(onNavigate).toHaveBeenCalledWith(6);
  });

  it("does not promote structural validity to publication readiness", () => {
    render(
      <SourceDiagnostics
        error={null}
        onNavigate={vi.fn()}
        stale={false}
        report={{
          relativePath: "project.md",
          contentHash: "b".repeat(64),
          diagnostics: [],
          eligibleForAiApply: true,
          eligibleForFinal: true,
          eligibleForEpub: false,
        }}
      />,
    );
    expect(screen.getByText("结构检查通过")).toBeInTheDocument();
    expect(screen.getAllByText(/· 可用$/u)).toHaveLength(2);
    expect(screen.getByText(/EPUB · 暂停$/u)).toBeInTheDocument();
    expect(screen.getByText(/仍需核对用户确认和当前版本/u)).toBeInTheDocument();
  });

  it("marks a previously valid report stale after content changes", () => {
    render(
      <SourceDiagnostics
        error={null}
        onNavigate={vi.fn()}
        stale
        report={{
          relativePath: "project.md",
          contentHash: "c".repeat(64),
          diagnostics: [],
          eligibleForAiApply: true,
          eligibleForFinal: true,
          eligibleForEpub: false,
        }}
      />,
    );
    expect(screen.getByText("诊断已过期")).toBeInTheDocument();
    expect(screen.getAllByText(/· 暂停$/u)).toHaveLength(3);
    expect(screen.queryByText(/· 可用$/u)).not.toBeInTheDocument();
  });
});
