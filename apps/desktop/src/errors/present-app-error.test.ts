import { describe, expect, it } from "vitest";
import { presentAppError } from "./present-app-error";

describe("NG-005 AppError 中文映射", () => {
  it("把已知稳定错误对映射为安全中文恢复信息", () => {
    expect(
      presentAppError({
        code: "runtime.unavailable",
        messageKey: "errors.runtimeUnavailable",
        diagnostic: "tauri-runtime-unavailable",
      }),
    ).toMatchInlineSnapshot(`
      {
        "code": "runtime.unavailable",
        "diagnostic": "tauri-runtime-unavailable",
        "message": "NovelGainker 无法连接本地 Rust 核心。",
        "recovery": "请重新检查；若问题持续，请重启应用。",
        "title": "本地核心暂时不可用",
      }
    `);
  });

  it("为无效响应提供稳定中文错误", () => {
    expect(
      presentAppError({
        code: "contract.invalid_response",
        messageKey: "errors.invalidRuntimeResponse",
      }),
    ).toMatchInlineSnapshot(`
      {
        "code": "contract.invalid_response",
        "message": "本地核心返回了无法识别的数据。",
        "recovery": "请重新启动应用；若问题持续，请更新 NovelGainker。",
        "title": "系统响应版本不兼容",
      }
    `);
  });

  it("呈现 NG-104 的只读与恢复阻断错误", () => {
    expect(
      presentAppError({
        code: "project.writer_required",
        messageKey: "errors.projectWriterRequired",
      }),
    ).toMatchObject({
      code: "project.writer_required",
      title: "当前项目会话为只读",
    });
    expect(
      presentAppError({
        code: "project.recovery_required",
        messageKey: "errors.projectRecoveryRequired",
      }),
    ).toMatchObject({
      code: "project.recovery_required",
      title: "项目需要人工恢复",
    });
  });

  it("呈现 NG-105 搜索错误但不回显查询正文", () => {
    const presented = presentAppError({
      code: "search.invalid_query",
      messageKey: "errors.searchInvalidQuery",
    });
    expect(presented).toMatchObject({
      code: "search.invalid_query",
      title: "搜索关键词无效",
    });
    expect(JSON.stringify(presented)).not.toContain("查询正文");
  });

  it("呈现 NG-201 的凭据库失败且不回显原始诊断", () => {
    const presented = presentAppError({
      code: "provider.vault_unavailable",
      messageKey: "errors.providerVaultUnavailable",
      diagnostic: "api_key=synthetic-input",
    });
    expect(presented).toMatchObject({
      code: "provider.vault_unavailable",
      title: "系统凭据库暂不可用",
    });
    expect(presented).not.toHaveProperty("diagnostic");
    expect(JSON.stringify(presented)).not.toContain("synthetic-input");
  });

  it("呈现 NG-402 的设定来源边界且不回显路径或正文", () => {
    const disallowed = presentAppError({
      code: "workflow.source_not_allowed",
      messageKey: "errors.workflowSourceNotAllowed",
      diagnostic: String.raw`C:\private\secret.md`,
    });
    expect(disallowed).toMatchObject({
      code: "workflow.source_not_allowed",
      title: "所选设定来源不在允许范围",
    });
    expect(JSON.stringify(disallowed)).not.toContain("secret.md");

    expect(
      presentAppError({
        code: "workflow.source_limit_exceeded",
        messageKey: "errors.workflowSourceLimitExceeded",
      }),
    ).toMatchObject({
      code: "workflow.source_limit_exceeded",
      title: "设定上下文超过固定上限",
    });
  });

  it("未知或错配错误只返回通用错误且丢弃原始信息", () => {
    const rawPath = "C:\\Users\\Leon\\secret-project\\draft.md";
    const presented = presentAppError({
      code: "runtime.unavailable",
      messageKey: "errors.unknown",
      diagnostic: rawPath,
    });

    expect(presented).toMatchInlineSnapshot(`
      {
        "code": "runtime.unknown",
        "message": "发生未识别的本地错误。",
        "recovery": "请重新检查；若问题持续，请重启应用。",
        "title": "系统检查未完成",
      }
    `);
    expect(JSON.stringify(presented)).not.toContain("Leon");
    expect(JSON.stringify(presented)).not.toContain("draft.md");
  });

  it("即使错误对已知也不会呈现不安全诊断", () => {
    const presented = presentAppError({
      code: "runtime.unavailable",
      messageKey: "errors.runtimeUnavailable",
      diagnostic: "prompt=用户输入&api_key=sk-secret",
    });

    expect(presented).not.toHaveProperty("diagnostic");
    expect(JSON.stringify(presented)).not.toContain("sk-secret");
  });
});
