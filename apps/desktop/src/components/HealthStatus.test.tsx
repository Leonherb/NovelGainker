import { describe, expect, it } from "vitest";
import { presentHealthStatus } from "../presentation/health";

describe("NG-005 健康检查成功呈现", () => {
  it("以稳定简体中文摘要呈现类型化成功状态", () => {
    expect(
      presentHealthStatus({
        application: "NovelGainker",
        version: "0.1.0",
        platform: "windows",
        status: "ready",
        contractVersion: 1,
      }),
    ).toMatchInlineSnapshot(`
      {
        "detail": "NovelGainker v0.1.0 · windows · 契约 v1",
        "title": "系统核心已就绪",
        "topLine": "核心就绪 · v0.1.0 · 契约 v1",
      }
    `);
  });
});
