import { describe, expect, it } from "vitest";
import { ZH_CN } from "./zh-CN";

function collectStrings(value: unknown): string[] {
  if (typeof value === "string") {
    return [value];
  }

  if (Array.isArray(value)) {
    return value.flatMap(collectStrings);
  }

  if (typeof value === "object" && value !== null) {
    return Object.values(value).flatMap(collectStrings);
  }

  return [];
}

describe("NG-202 简体中文资源", () => {
  it("集中提供工作台、Key-only 状态和恢复文案", () => {
    const inventory = collectStrings(ZH_CN);

    expect(ZH_CN.locale).toBe("zh-CN");
    expect(ZH_CN.conversation.currentProvider).toBe("当前 Provider");
    expect(ZH_CN.api.safety).toContain("Windows 凭据库");
    expect(ZH_CN.api.safety).toContain("无法读回");
    expect(ZH_CN.files.editor.saveBoundary).toContain("Ctrl+S");
    expect(ZH_CN.errors.unknown.recovery).toContain("请重新检查");
    expect(inventory.length).toBeGreaterThan(70);
  });

  it("资源清单中不存在韩语字符", () => {
    expect(collectStrings(ZH_CN).join("\n")).not.toMatch(/[\uac00-\ud7af]/u);
  });
});
