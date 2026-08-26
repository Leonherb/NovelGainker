import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as providerApi from "../lib/tauri/provider";
import type { ProviderStateSnapshot, ProviderStatus } from "../lib/tauri/provider";
import { ProviderSettings } from "./ProviderSettings";
import { DEFAULT_PROVIDER_STATE } from "./provider-settings-model";

const configuredDeepseek: ProviderStatus = {
  providerId: "deepseek",
  configured: true,
  connectionStatus: "untested",
  presetVersion: 1,
  selected: false,
};

const configuredOpenai: ProviderStatus = {
  providerId: "openai",
  configured: true,
  connectionStatus: "untested",
  presetVersion: 1,
  selected: true,
};

const selectedDeepseek: ProviderStatus = {
  ...configuredDeepseek,
  selected: true,
};

const selectedUnconfiguredDeepseek: ProviderStatus = {
  ...selectedDeepseek,
  configured: false,
  connectionStatus: "unconfigured",
};

const unselectedOpenai: ProviderStatus = {
  ...configuredOpenai,
  selected: false,
};

const configuredSnapshot: ProviderStateSnapshot = {
  schemaVersion: 2,
  providers: [configuredDeepseek, configuredOpenai],
  selectedProviderId: "openai",
};

const deepseekSnapshot: ProviderStateSnapshot = {
  schemaVersion: 2,
  providers: [selectedUnconfiguredDeepseek, unselectedOpenai],
  selectedProviderId: "deepseek",
};

function providerCard(input: HTMLElement): HTMLElement {
  const card = input.closest<HTMLElement>(".provider-settings-card");
  if (!card) {
    throw new Error("Provider settings card missing");
  }
  return card;
}

describe("NG-202 Key-only Provider 设置", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(providerApi, "requestProviderStatus").mockResolvedValue(DEFAULT_PROVIDER_STATE);
  });

  it("用顶部 Provider 直接选择唯一 Key 表单，不重复模型和状态说明", async () => {
    vi.mocked(providerApi.requestProviderStatus).mockResolvedValue(configuredSnapshot);
    render(<ProviderSettings desktopRuntime expanded onExpandedChange={vi.fn()} />);

    const openaiInput = await screen.findByLabelText("OpenAI API Key");
    expect(openaiInput).toHaveAttribute("type", "password");
    expect(openaiInput).toHaveAttribute("autocomplete", "new-password");
    expect(screen.queryByLabelText("DeepSeek API Key")).not.toBeInTheDocument();
    await waitFor(() => expect(openaiInput).toBeEnabled());
    expect(document.querySelectorAll('input[type="password"]')).toHaveLength(1);
    expect(screen.getByRole("button", { name: "替换" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "连接测试" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "删除" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "OpenAI，当前使用" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "DeepSeek，已保存 · 未测试" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    expect(screen.getByRole("button", { name: "折叠 OpenAI API Key 填写" })).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("deepseek-v4-pro");
    expect(document.body.textContent).not.toContain("gpt-5.6-terra");
    expect(document.body.textContent).not.toMatch(
      /Base URL|Endpoint|Header|Timeout|Temperature|Reasoning/iu,
    );
    expect(document.body.textContent).not.toMatch(/[\uac00-\ud7af]/u);
  });

  it("保存后立即清空 Key，状态和连接说明不包含任何 Key 字符", async () => {
    const user = userEvent.setup();
    const canary = "ng202-canary-deepseek-value";
    vi.mocked(providerApi.requestProviderStatus).mockResolvedValue(deepseekSnapshot);
    const save = vi
      .spyOn(providerApi, "requestSaveProviderKey")
      .mockResolvedValue(selectedDeepseek);
    const connectionTest = vi
      .spyOn(providerApi, "requestProviderConnectionTest")
      .mockResolvedValue({
        ...selectedDeepseek,
        connectionStatus: "connected",
      });
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    const getItem = vi.spyOn(Storage.prototype, "getItem");
    const setItem = vi.spyOn(Storage.prototype, "setItem");

    render(<ProviderSettings desktopRuntime expanded onExpandedChange={vi.fn()} />);

    const input = await screen.findByLabelText("DeepSeek API Key");
    const card = providerCard(input);
    await waitFor(() => expect(input).toBeEnabled());
    await user.type(input, canary);
    await user.click(within(card).getByRole("button", { name: "保存" }));

    await waitFor(() => expect(save).toHaveBeenCalledWith("deepseek", canary));
    expect(input).toHaveValue("");
    expect(await screen.findByText(/DeepSeek API Key 已安全保存/u)).toBeInTheDocument();
    expect(document.body.textContent).not.toContain(canary);
    expect(getItem).not.toHaveBeenCalled();
    expect(setItem).not.toHaveBeenCalled();
    expect(fetchSpy).not.toHaveBeenCalled();

    await user.click(within(card).getByRole("button", { name: "连接测试" }));
    const dialog = screen.getByRole("dialog", { name: "运行连接测试？" });
    expect(within(dialog).getByText("DeepSeek")).toBeInTheDocument();
    expect(within(dialog).getByText("deepseek-v4-pro")).toBeInTheDocument();
    expect(within(dialog).getByText(/极少 API 用量/u)).toBeInTheDocument();
    expect(connectionTest).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole("button", { name: "开始测试" }));
    await waitFor(() => expect(connectionTest).toHaveBeenCalledWith("deepseek"));
    expect(await screen.findByText("DeepSeek 连接测试成功。")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "DeepSeek，当前使用" }).querySelector(".provider-state"),
    ).toHaveAttribute("data-status", "connected");
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("保存失败也清空输入并只显示稳定脱敏错误", async () => {
    const user = userEvent.setup();
    const canary = "ng202-canary-vault-failure";
    vi.spyOn(providerApi, "requestSaveProviderKey").mockRejectedValue({
      code: "provider.vault_unavailable",
      messageKey: "errors.providerVaultUnavailable",
      diagnostic: canary,
    });

    render(<ProviderSettings desktopRuntime expanded onExpandedChange={vi.fn()} />);

    const input = await screen.findByLabelText("OpenAI API Key");
    await waitFor(() => expect(input).toBeEnabled());
    await user.type(input, canary);
    await user.click(within(providerCard(input)).getByRole("button", { name: "保存" }));

    expect(await screen.findByText("系统凭据库暂不可用")).toBeInTheDocument();
    expect(input).toHaveValue("");
    expect(document.body.textContent).not.toContain(canary);
    expect(screen.getByText("错误代码：provider.vault_unavailable")).toBeInTheDocument();
  });

  it("显式切换和确认删除只改变目标 Provider", async () => {
    const user = userEvent.setup();
    vi.mocked(providerApi.requestProviderStatus).mockResolvedValue(configuredSnapshot);
    const select = vi
      .spyOn(providerApi, "requestSelectProvider")
      .mockResolvedValue(selectedDeepseek);
    const remove = vi.spyOn(providerApi, "requestDeleteProviderKey").mockResolvedValue({
      ...selectedDeepseek,
      configured: false,
      connectionStatus: "unconfigured",
    });

    render(<ProviderSettings desktopRuntime expanded onExpandedChange={vi.fn()} />);

    const deepseekButton = await screen.findByRole("button", {
      name: "DeepSeek，已保存 · 未测试",
    });
    await waitFor(() => expect(deepseekButton).toBeEnabled());
    await user.click(deepseekButton);
    await waitFor(() => expect(select).toHaveBeenCalledWith("deepseek"));
    expect(screen.getByRole("button", { name: "DeepSeek，当前使用" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    const deepseekCard = providerCard(await screen.findByLabelText("DeepSeek API Key"));
    await user.click(within(deepseekCard).getByRole("button", { name: "删除" }));
    const dialog = screen.getByRole("dialog", { name: "删除 API Key？" });
    expect(within(dialog).getByRole("button", { name: "取消" })).toHaveFocus();
    await user.click(within(dialog).getByRole("button", { name: "确认删除" }));

    await waitFor(() => expect(remove).toHaveBeenCalledWith("deepseek", true));
    expect(
      screen.getByText("DeepSeek API Key 已删除；另一 Provider 不受影响。"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "OpenAI，已保存 · 未测试" })).toBeInTheDocument();
  });

  it("非桌面环境保持 Key 控件禁用且不会读取浏览器存储", () => {
    const getItem = vi.spyOn(Storage.prototype, "getItem");
    const setItem = vi.spyOn(Storage.prototype, "setItem");

    render(<ProviderSettings desktopRuntime={false} expanded onExpandedChange={vi.fn()} />);

    expect(screen.getByLabelText("OpenAI API Key")).toBeDisabled();
    expect(screen.queryByLabelText("DeepSeek API Key")).not.toBeInTheDocument();
    expect(screen.getByText(/只能在 NovelGainker Windows 桌面窗口/u)).toBeInTheDocument();
    expect(getItem).not.toHaveBeenCalled();
    expect(setItem).not.toHaveBeenCalled();
  });
  it("活动 Run 期间前端拒绝切换且切换本身不触发网络", async () => {
    vi.mocked(providerApi.requestProviderStatus).mockResolvedValue(configuredSnapshot);
    const select = vi.spyOn(providerApi, "requestSelectProvider");
    const fetchSpy = vi.spyOn(globalThis, "fetch");

    render(<ProviderSettings desktopRuntime expanded onExpandedChange={vi.fn()} runActive />);

    const switchButton = await screen.findByRole("button", {
      name: "DeepSeek，已保存 · 未测试",
    });
    await waitFor(() => expect(switchButton).toBeDisabled());
    expect(
      screen.getByText("当前 AI 请求运行中；停止或等待结束后才能切换 Provider。"),
    ).toBeInTheDocument();
    expect(select).not.toHaveBeenCalled();
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("NG-207 clears two high-entropy Key buffers and leaks no correlatable DOM fragment", async () => {
    const user = userEvent.setup();
    vi.mocked(providerApi.requestProviderStatus).mockResolvedValue(configuredSnapshot);
    const canaries = {
      deepseek: `NG207DeepSeek${crypto.randomUUID().replace(/-/gu, "")}${crypto
        .randomUUID()
        .replace(/-/gu, "")}`,
      openai: `NG207OpenAI${crypto.randomUUID().replace(/-/gu, "")}${crypto
        .randomUUID()
        .replace(/-/gu, "")}`,
    } as const;
    expect(canaries.deepseek).not.toBe(canaries.openai);
    vi.spyOn(providerApi, "requestSelectProvider").mockResolvedValue(selectedDeepseek);
    const save = vi
      .spyOn(providerApi, "requestSaveProviderKey")
      .mockImplementation(async (providerId) =>
        providerId === "deepseek" ? selectedDeepseek : configuredOpenai,
      );

    render(<ProviderSettings desktopRuntime expanded onExpandedChange={vi.fn()} />);
    const openaiInput = await screen.findByLabelText("OpenAI API Key");
    await waitFor(() => expect(openaiInput).toBeEnabled());
    await user.type(openaiInput, canaries.openai);
    expect(openaiInput).toHaveValue(canaries.openai);
    await user.click(within(providerCard(openaiInput)).getByRole("button", { name: "替换" }));
    await waitFor(() => expect(save).toHaveBeenCalledWith("openai", canaries.openai));
    expect(openaiInput).toHaveValue("");

    await user.click(
      screen.getByRole("button", {
        name: "DeepSeek，已保存 · 未测试",
      }),
    );
    const deepseekInput = await screen.findByLabelText("DeepSeek API Key");
    await user.type(deepseekInput, canaries.deepseek);
    expect(deepseekInput).toHaveValue(canaries.deepseek);
    await user.click(within(providerCard(deepseekInput)).getByRole("button", { name: "替换" }));
    await waitFor(() => expect(save).toHaveBeenCalledWith("deepseek", canaries.deepseek));
    expect(deepseekInput).toHaveValue("");
    expect(document.querySelectorAll('input[type="password"]')).toHaveLength(1);

    const serializedDom = document.documentElement.outerHTML;
    const visibleText = document.body.textContent ?? "";
    for (const canary of Object.values(canaries)) {
      expect(serializedDom).not.toContain(canary);
      expect(visibleText).not.toContain(canary);
      for (let index = 0; index <= canary.length - 16; index += 1) {
        const fragment = canary.slice(index, index + 16);
        expect(serializedDom).not.toContain(fragment);
        expect(visibleText).not.toContain(fragment);
      }
    }
  });
});
