import { describe, expect, it, vi } from "vitest";
import {
  parseProviderStateSnapshot,
  parseProviderStatus,
  requestDeleteProviderKey,
  requestProviderConnectionTest,
  requestProviderStatus,
  requestSelectProvider,
  requestSaveProviderKey,
  type ProviderStateSnapshot,
  type ProviderStatus,
} from "./provider";

const unconfiguredDeepseek: ProviderStatus = {
  providerId: "deepseek",
  configured: false,
  connectionStatus: "unconfigured",
  presetVersion: 1,
  selected: false,
};

const unconfiguredOpenai: ProviderStatus = {
  providerId: "openai",
  configured: false,
  connectionStatus: "unconfigured",
  presetVersion: 1,
  selected: true,
};

const snapshot: ProviderStateSnapshot = {
  schemaVersion: 2,
  providers: [unconfiguredDeepseek, unconfiguredOpenai],
  selectedProviderId: "openai",
};

describe("NG-201 Provider IPC contract", () => {
  it("accepts only the two exact provider statuses and one selected provider", () => {
    expect(parseProviderStateSnapshot(snapshot)).toEqual(snapshot);
    expect(() =>
      parseProviderStatus({
        ...unconfiguredDeepseek,
        maskedKey: "not-allowed",
      }),
    ).toThrow();
    expect(() =>
      parseProviderStateSnapshot({
        ...snapshot,
        providers: [unconfiguredDeepseek, unconfiguredDeepseek],
      }),
    ).toThrow();
    expect(() =>
      parseProviderStatus({
        ...unconfiguredDeepseek,
        configured: true,
      }),
    ).toThrow();
  });

  it("invokes only status, save and confirmed-delete commands", async () => {
    const configuredDeepseek: ProviderStatus = {
      ...unconfiguredDeepseek,
      configured: true,
      connectionStatus: "untested",
    };
    const invoke = vi
      .fn()
      .mockResolvedValueOnce(snapshot)
      .mockResolvedValueOnce(configuredDeepseek)
      .mockResolvedValueOnce(unconfiguredDeepseek);
    const syntheticInput = "ng201-synthetic-input";

    const loaded = await requestProviderStatus(invoke);
    const saved = await requestSaveProviderKey("deepseek", syntheticInput, invoke);
    const deleted = await requestDeleteProviderKey("deepseek", true, invoke);

    expect(invoke.mock.calls[0]).toEqual(["provider_status_load", {}]);
    expect(invoke.mock.calls[1]).toEqual([
      "provider_key_save",
      { providerId: "deepseek", key: syntheticInput },
    ]);
    expect(invoke.mock.calls[2]).toEqual([
      "provider_key_delete",
      { providerId: "deepseek", confirmed: true },
    ]);
    expect(JSON.stringify([loaded, saved, deleted])).not.toContain(syntheticInput);
    expect(saved).not.toHaveProperty("maskedKey");
  });

  it("rejects malformed responses and normalizes raw failures without input readback", async () => {
    const syntheticInput = "ng201-synthetic-error-input";
    await expect(
      requestSaveProviderKey(
        "openai",
        syntheticInput,
        vi.fn().mockResolvedValue({
          ...unconfiguredOpenai,
          configured: true,
          connectionStatus: "untested",
          keySuffix: syntheticInput,
        }),
      ),
    ).rejects.toEqual({
      code: "contract.invalid_response",
      messageKey: "errors.invalidRuntimeResponse",
    });

    const rawError = new Error(`api_key=${syntheticInput} C:\\Private`);
    const failure = requestSaveProviderKey(
      "openai",
      syntheticInput,
      vi.fn().mockRejectedValue(rawError),
    );
    await expect(failure).rejects.toEqual({
      code: "runtime.unknown",
      messageKey: "errors.unknown",
    });
  });

  it("selects one fixed provider without a key or network parameters", async () => {
    const selectedDeepseek: ProviderStatus = {
      ...unconfiguredDeepseek,
      selected: true,
    };
    const invoke = vi.fn().mockResolvedValue(selectedDeepseek);

    await expect(requestSelectProvider("deepseek", invoke)).resolves.toEqual(selectedDeepseek);
    expect(invoke).toHaveBeenCalledWith("provider_select", {
      providerId: "deepseek",
    });
    expect(JSON.stringify(invoke.mock.calls)).not.toMatch(/key|url|model|header|timeout/iu);
  });

  it("runs the bounded connection test with only the fixed provider id", async () => {
    const connectedOpenai: ProviderStatus = {
      ...unconfiguredOpenai,
      configured: true,
      connectionStatus: "connected",
    };
    const invoke = vi.fn().mockResolvedValue(connectedOpenai);

    await expect(requestProviderConnectionTest("openai", invoke)).resolves.toEqual(connectedOpenai);
    expect(invoke).toHaveBeenCalledWith("provider_connection_test", {
      providerId: "openai",
    });
    expect(JSON.stringify(invoke.mock.calls)).not.toMatch(/key|url|model|header|timeout/iu);
  });
});
