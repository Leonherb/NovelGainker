import { describe, expect, it, vi } from "vitest";
import appErrorFixture from "../../../../../contracts/ipc/app-error.v1.json";
import healthFixture from "../../../../../contracts/ipc/health-status.v1.json";
import {
  HEALTH_CHECK_COMMAND,
  normalizeAppError,
  parseAppError,
  parseHealthStatus,
  requestHealthStatus,
} from "./health";

describe("NG-003 Tauri IPC 契约", () => {
  it("解析与 Rust 共用的 HealthStatus v1 夹具", () => {
    expect(parseHealthStatus(healthFixture)).toEqual(healthFixture);
  });

  it("拒绝契约版本变化和任何多余字段", () => {
    expect(() => parseHealthStatus({ ...healthFixture, contractVersion: 2 })).toThrow(
      "Invalid health contract",
    );
    expect(() => parseHealthStatus({ ...healthFixture, apiKey: "must-not-cross-ipc" })).toThrow(
      "Invalid health contract",
    );
  });

  it("只调用无参数 health_check", async () => {
    const invokeCommand = vi.fn().mockResolvedValue(healthFixture);

    await expect(requestHealthStatus(invokeCommand)).resolves.toEqual(healthFixture);
    expect(invokeCommand).toHaveBeenCalledOnce();
    expect(invokeCommand).toHaveBeenCalledWith(HEALTH_CHECK_COMMAND);
  });

  it("识别 Rust AppError 共享夹具", async () => {
    const invokeCommand = vi.fn().mockRejectedValue(appErrorFixture);

    expect(parseAppError(appErrorFixture)).toEqual(appErrorFixture);
    await expect(requestHealthStatus(invokeCommand)).rejects.toEqual(appErrorFixture);
  });

  it("把未知错误替换为不含原始信息的安全错误", () => {
    const normalized = normalizeAppError(new Error("C:\\Users\\Leon\\secret-project\\draft.md"));

    expect(normalized).toEqual({
      code: "runtime.unknown",
      messageKey: "errors.unknown",
    });
    expect(JSON.stringify(normalized)).not.toContain("Leon");
    expect(JSON.stringify(normalized)).not.toContain("draft.md");
  });

  it("把无效成功响应转换为受控契约错误", async () => {
    const invokeCommand = vi.fn().mockResolvedValue({ status: "ready" });

    await expect(requestHealthStatus(invokeCommand)).rejects.toEqual({
      code: "contract.invalid_response",
      messageKey: "errors.invalidRuntimeResponse",
    });
  });
});
