import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { DEFAULT_PROVIDER_STATE } from "./components/provider-settings-model";
import { ZH_CN } from "./i18n/zh-CN";
import * as appStateApi from "./lib/tauri/app-state";
import * as healthApi from "./lib/tauri/health";
import * as providerApi from "./lib/tauri/provider";

function installMatchMedia(matches: boolean) {
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: vi.fn().mockImplementation(
      (query: string) =>
        ({
          addEventListener: vi.fn(),
          addListener: vi.fn(),
          dispatchEvent: vi.fn(),
          matches,
          media: query,
          onchange: null,
          removeEventListener: vi.fn(),
          removeListener: vi.fn(),
        }) as unknown as MediaQueryList,
    ),
    writable: true,
  });
}

describe("NovelGainker Sprint 0 工作台", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    installMatchMedia(false);
    Object.defineProperty(window, "innerWidth", { configurable: true, value: 1_024 });
    vi.spyOn(healthApi, "isTauriRuntime").mockReturnValue(false);
    vi.spyOn(providerApi, "requestProviderStatus").mockResolvedValue(DEFAULT_PROVIDER_STATE);
  });

  it("默认显示 API、对话和文件三栏", () => {
    render(<App />);

    expect(
      screen.getByRole("complementary", { name: ZH_CN.providers.ariaLabel }),
    ).toBeInTheDocument();
    expect(screen.getByRole("main", { name: "对话工作区" })).toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: ZH_CN.files.ariaLabel })).toBeInTheDocument();
    expect(screen.getAllByText("DeepSeek").length).toBeGreaterThan(0);
    expect(screen.getAllByText("OpenAI").length).toBeGreaterThan(0);
    expect(screen.getByRole("region", { name: "小说创作对话消息" })).toBeInTheDocument();
  });

  it("未打开工程时阻塞显示项目选择弹窗，并让右栏只承载文件内容", () => {
    render(<App />);

    const left = screen.getByRole("complementary", { name: ZH_CN.providers.ariaLabel });
    const right = screen.getByRole("complementary", { name: ZH_CN.files.ariaLabel });
    const dialog = screen.getByRole("dialog", { name: ZH_CN.files.project.startupTitle });
    const projectTree = within(dialog).getByRole("region", { name: ZH_CN.files.projectFiles });

    expect(projectTree).toBeInTheDocument();
    expect(within(left).queryByRole("region", { name: ZH_CN.files.projectFiles })).toBeNull();
    expect(within(right).queryByRole("region", { name: ZH_CN.files.projectFiles })).toBeNull();
    expect(
      within(right).getByRole("region", { name: ZH_CN.files.editor.heading }),
    ).toBeInTheDocument();
  });

  it("可以独立折叠 API 卡、左栏和右栏并恢复", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "折叠 OpenAI API Key 填写" }));
    expect(screen.queryByLabelText("OpenAI API Key")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "展开 OpenAI API Key 填写" }));
    expect(screen.getByLabelText("OpenAI API Key")).toBeDisabled();

    await user.click(screen.getByRole("button", { name: ZH_CN.panels.collapseApi }));
    expect(
      screen.queryByRole("complementary", { name: ZH_CN.providers.ariaLabel }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("main", { name: "对话工作区" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: ZH_CN.panels.expandApi }));
    expect(
      screen.getByRole("complementary", { name: ZH_CN.providers.ariaLabel }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: ZH_CN.panels.collapseFiles }));
    expect(
      screen.queryByRole("complementary", { name: ZH_CN.files.ariaLabel }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: ZH_CN.panels.expandFiles }));
    expect(screen.getByRole("complementary", { name: ZH_CN.files.ariaLabel })).toBeInTheDocument();
  });

  it("折叠右栏时保持项目工作区挂载且不使用浏览器存储", async () => {
    const user = userEvent.setup();
    const getItem = vi.spyOn(Storage.prototype, "getItem");
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    render(<App />);

    const sidebar = document.querySelector<HTMLElement>("#file-sidebar");
    expect(sidebar).not.toBeNull();
    expect(sidebar).not.toHaveAttribute("hidden");

    await user.click(screen.getByRole("button", { name: ZH_CN.panels.collapseFiles }));
    expect(sidebar).toHaveAttribute("hidden");
    await user.click(screen.getByRole("button", { name: ZH_CN.panels.expandFiles }));
    expect(sidebar).not.toHaveAttribute("hidden");
    expect(document.querySelector("#file-sidebar")).toBe(sidebar);
    expect(getItem).not.toHaveBeenCalled();
    expect(setItem).not.toHaveBeenCalled();
  });

  it("右侧文件栏可拖动或用键盘放大，并可恢复默认宽度", async () => {
    const user = userEvent.setup();
    render(<App />);

    const shell = document.querySelector<HTMLElement>(".app-shell");
    const workspace = document.querySelector<HTMLElement>(".workspace");
    const separator = screen.getByRole("separator", { name: ZH_CN.panels.resizeFiles });
    expect(shell).not.toBeNull();
    expect(workspace).not.toBeNull();
    expect(shell?.style.getPropertyValue("--right-panel-width")).toBe("");

    separator.focus();
    await user.keyboard("{ArrowLeft}");
    expect(shell?.style.getPropertyValue("--right-panel-width")).toBe("336px");
    await user.keyboard("{ArrowRight}");
    expect(shell?.style.getPropertyValue("--right-panel-width")).toBe("304px");
    await user.keyboard("{Home}");
    expect(shell?.style.getPropertyValue("--right-panel-width")).toBe("");

    vi.spyOn(workspace as HTMLElement, "getBoundingClientRect").mockReturnValue({
      bottom: 800,
      height: 800,
      left: 0,
      right: 1_400,
      top: 0,
      width: 1_400,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    });
    fireEvent.pointerDown(separator, { button: 0, clientX: 900, pointerId: 7 });
    expect(shell).toHaveAttribute("data-right-resizing", "true");
    expect(shell?.style.getPropertyValue("--right-panel-width")).toBe("500px");
    fireEvent.pointerMove(separator, { clientX: 800, pointerId: 7 });
    expect(shell?.style.getPropertyValue("--right-panel-width")).toBe("600px");
    fireEvent.pointerUp(separator, { clientX: 800, pointerId: 7 });
    expect(shell).toHaveAttribute("data-right-resizing", "false");
  });
  it("从本机״̬恢复布局、最近项Ŀ并继续持久化界面ƫ好", async () => {
    const user = userEvent.setup();
    Object.defineProperty(window, "innerWidth", { configurable: true, value: 1_920 });
    vi.mocked(healthApi.isTauriRuntime).mockReturnValue(true);
    vi.spyOn(healthApi, "requestHealthStatus").mockReturnValue(new Promise(() => undefined));
    vi.spyOn(appStateApi, "requestAppState").mockResolvedValue({
      schemaVersion: 3,
      ui: {
        leftCollapsed: true,
        rightCollapsed: false,
        apiCardExpanded: false,
        rightPanelWidth: 920,
      },
      recentProjects: [
        {
          projectId: "30000000-0000-4000-8000-000000000003",
          displayName: "Novel One",
        },
      ],
      projectFiles: [],
      recoveredWithDefaults: false,
    });
    const save = vi.spyOn(appStateApi, "requestSaveUiPreferences").mockResolvedValue({
      saved: true,
    });

    render(<App />);

    await waitFor(() =>
      expect(
        screen.queryByRole("complementary", { name: ZH_CN.providers.ariaLabel }),
      ).not.toBeInTheDocument(),
    );
    expect(screen.queryByLabelText(ZH_CN.api.deepSeekLabel)).not.toBeInTheDocument();
    expect(screen.getByText("Novel One")).toBeInTheDocument();
    expect(
      document
        .querySelector<HTMLElement>(".app-shell")
        ?.style.getPropertyValue("--right-panel-width"),
    ).toBe("920px");
    await waitFor(() =>
      expect(save).toHaveBeenLastCalledWith({
        leftCollapsed: true,
        rightCollapsed: false,
        apiCardExpanded: false,
        rightPanelWidth: 920,
      }),
    );
    await user.click(screen.getByRole("button", { name: ZH_CN.panels.expandApi }));
    expect(
      screen.getByRole("dialog", { name: ZH_CN.files.project.startupTitle }),
    ).toBeInTheDocument();
  });

  it("本机״̬损坏ʱ显ʾ安ȫĬ认提ʾ且ֹͣƫ好д入", async () => {
    vi.mocked(healthApi.isTauriRuntime).mockReturnValue(true);
    vi.spyOn(healthApi, "requestHealthStatus").mockReturnValue(new Promise(() => undefined));
    vi.spyOn(appStateApi, "requestAppState").mockResolvedValue({
      schemaVersion: 3,
      ui: {
        leftCollapsed: false,
        rightCollapsed: false,
        apiCardExpanded: true,
        rightPanelWidth: null,
      },
      recentProjects: [],
      projectFiles: [],
      recoveredWithDefaults: true,
    });
    const save = vi.spyOn(appStateApi, "requestSaveUiPreferences");

    render(<App />);

    expect(await screen.findByText(ZH_CN.appState.recoveredDefaults)).toBeInTheDocument();
    expect(save).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: ZH_CN.files.project.open })).toBeEnabled();
  });

  it("窄窗口自动保护中栏，并以互斥抽屉打开侧栏", async () => {
    installMatchMedia(true);
    const user = userEvent.setup();
    render(<App />);

    const shell = document.querySelector(".app-shell");
    expect(shell).toHaveAttribute("data-layout", "narrow");
    expect(
      screen.queryByRole("separator", { name: ZH_CN.panels.resizeFiles }),
    ).not.toBeInTheDocument();
    await waitFor(() => {
      expect(
        screen.queryByRole("complementary", { name: ZH_CN.providers.ariaLabel }),
      ).not.toBeInTheDocument();
      expect(
        screen.queryByRole("complementary", { name: ZH_CN.files.ariaLabel }),
      ).not.toBeInTheDocument();
    });
    expect(screen.getByRole("main", { name: "对话工作区" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: ZH_CN.panels.expandApi }));
    expect(
      screen.getByRole("complementary", { name: ZH_CN.providers.ariaLabel }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: ZH_CN.panels.expandFiles }));
    expect(
      screen.queryByRole("complementary", { name: ZH_CN.providers.ariaLabel }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: ZH_CN.files.ariaLabel })).toBeInTheDocument();
  });

  it("折叠按钮支持键盘操作，过渡中栏不会发起网络请求", async () => {
    const user = userEvent.setup();
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    render(<App />);

    await user.tab();
    expect(screen.getByRole("button", { name: ZH_CN.panels.collapseApi })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(screen.getByRole("button", { name: ZH_CN.panels.expandApi })).toBeInTheDocument();

    expect(screen.getByRole("button", { name: "发送" })).toBeDisabled();
    expect(screen.getByRole("region", { name: "小说创作对话消息" })).toHaveTextContent(
      "请先新建或打开一个小说工程。",
    );
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(document.body.textContent).not.toMatch(/[\uac00-\ud7af]/u);
  });

  it("健康检查进行中时显示清晰中文状态并禁用重复操作", async () => {
    vi.mocked(healthApi.isTauriRuntime).mockReturnValue(true);
    vi.spyOn(healthApi, "requestHealthStatus").mockReturnValue(new Promise(() => undefined));

    render(<App />);

    const action = await screen.findByRole("button", { name: "运行系统健康检查" });
    expect(action).toBeDisabled();
    expect(action).toHaveTextContent("检查中");
    expect(screen.getByText("正在执行只读健康检查，请稍候。")).toBeInTheDocument();
  });

  it("在 Tauri 运行时通过真实客户端入口显示类型化健康状态", async () => {
    vi.mocked(healthApi.isTauriRuntime).mockReturnValue(true);
    const request = vi.spyOn(healthApi, "requestHealthStatus").mockResolvedValue({
      application: "NovelGainker",
      version: "0.1.0",
      platform: "windows",
      status: "ready",
      contractVersion: 1,
    });

    render(<App />);

    expect(await screen.findByText("系统核心已就绪")).toBeInTheDocument();
    expect(screen.getByText("NovelGainker v0.1.0 · windows · 契约 v1")).toBeInTheDocument();
    expect(request).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "运行系统健康检查" })).toHaveTextContent("重新检查");
  });

  it("普通 UI 将受控 AppError 映射为中文且不显示诊断", async () => {
    vi.mocked(healthApi.isTauriRuntime).mockReturnValue(true);
    vi.spyOn(healthApi, "requestHealthStatus").mockRejectedValue({
      code: "runtime.unavailable",
      messageKey: "errors.runtimeUnavailable",
      diagnostic: "safe-diagnostic",
    });

    render(<App />);

    expect(await screen.findByText("本地核心暂时不可用")).toBeInTheDocument();
    expect(screen.getByText("NovelGainker 无法连接本地 Rust 核心。")).toBeInTheDocument();
    expect(screen.getByText("错误代码：runtime.unavailable")).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("safe-diagnostic");
  });

  it("未知健康检查错误显示通用恢复信息且不泄露路径", async () => {
    vi.mocked(healthApi.isTauriRuntime).mockReturnValue(true);
    vi.spyOn(healthApi, "requestHealthStatus").mockRejectedValue(
      new Error("C:\\Users\\Leon\\secret-project\\draft.md api_key=sk-secret"),
    );

    render(<App />);

    expect(await screen.findByText("系统检查未完成")).toBeInTheDocument();
    expect(screen.getByText("发生未识别的本地错误。")).toBeInTheDocument();
    expect(screen.getByText("错误代码：runtime.unknown")).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("Leon");
    expect(document.body.textContent).not.toContain("draft.md");
    expect(document.body.textContent).not.toContain("sk-secret");
  });

  it("仅在显式开发诊断模式呈现已脱敏诊断 token", async () => {
    vi.mocked(healthApi.isTauriRuntime).mockReturnValue(true);
    vi.spyOn(healthApi, "requestHealthStatus").mockRejectedValue({
      code: "runtime.unavailable",
      messageKey: "errors.runtimeUnavailable",
      diagnostic: "tauri-runtime-unavailable",
    });

    render(<App diagnosticsEnabled />);

    expect(await screen.findByText("开发诊断（已脱敏）")).toBeInTheDocument();
    expect(screen.getByText("tauri-runtime-unavailable")).toBeInTheDocument();
  });
});
