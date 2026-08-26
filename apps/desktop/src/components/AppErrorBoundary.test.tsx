import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AppErrorBoundary } from "./AppErrorBoundary";

function BrokenWorkspace(): never {
  throw new Error("C:\\Users\\Leon\\secret-project\\draft.md api_key=sk-secret");
}

describe("NG-005 React Error Boundary", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("捕获未知渲染异常并显示安全中文恢复界面", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const reload = vi.fn();
    const user = userEvent.setup();

    render(
      <AppErrorBoundary onReload={reload}>
        <BrokenWorkspace />
      </AppErrorBoundary>,
    );

    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "工作台遇到意外错误" })).toBeInTheDocument();
    expect(screen.getByText("错误代码：ui.render_failed")).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("Leon");
    expect(document.body.textContent).not.toContain("draft.md");
    expect(document.body.textContent).not.toContain("sk-secret");

    await user.click(screen.getByRole("button", { name: "重新加载应用" }));
    expect(reload).toHaveBeenCalledOnce();
  });
});
