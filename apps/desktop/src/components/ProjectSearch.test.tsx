import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ProjectSearch } from "./ProjectSearch";

describe("NG-105 offline search UI", () => {
  it("submits a literal query and navigates without exposing a write action", async () => {
    const user = userEvent.setup();
    const onQueryChange = vi.fn();
    const onSearch = vi.fn();
    const onNavigate = vi.fn();
    const { rerender } = render(
      <ProjectSearch
        busy={false}
        caseSensitive={false}
        error={null}
        onCaseSensitiveChange={vi.fn()}
        onNavigate={onNavigate}
        onQueryChange={onQueryChange}
        onSearch={onSearch}
        query=""
        report={null}
      />,
    );

    await user.type(screen.getByRole("searchbox", { name: "离线搜索" }), "月光");
    expect(onQueryChange).toHaveBeenNthCalledWith(1, "月");
    expect(onQueryChange).toHaveBeenNthCalledWith(2, "光");

    rerender(
      <ProjectSearch
        busy={false}
        caseSensitive={false}
        error={null}
        onCaseSensitiveChange={vi.fn()}
        onNavigate={onNavigate}
        onQueryChange={onQueryChange}
        onSearch={onSearch}
        query="月光"
        report={{
          matches: [
            {
              relativePath: "chapters/001.md",
              line: 8,
              column: 3,
              snippet: "月光落在旧书页上。",
            },
          ],
          scannedFileCount: 2,
          skippedFileCount: 0,
          scannedBytes: 1024,
          truncated: false,
        }}
      />,
    );
    await user.click(screen.getByRole("button", { name: "搜索" }));
    expect(onSearch).toHaveBeenCalledOnce();
    await user.click(screen.getByRole("button", { name: /chapters\/001\.md/u }));
    expect(onNavigate).toHaveBeenCalledWith("chapters/001.md", 8);
    expect(screen.queryByRole("button", { name: /保存/u })).not.toBeInTheDocument();
  });
});
