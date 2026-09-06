import { describe, expect, it, vi } from "vitest";
import {
  requestSessionManage,
  requestConversationExportPrepare,
  requestConversationExportCommit,
} from "./session-management";
const project = "10000000-0000-4000-8000-000000000001";
const session = "20000000-0000-4000-8000-000000000002";
const token = "30000000-0000-4000-8000-000000000003";
describe("NR-602 closed session management DTOs", () => {
  it("requires a scoped truthful action receipt and rejects extra hidden fields", async () => {
    const operation = {
      action: "archive" as const,
      conversationSessionId: session,
      expectedRevision: 1,
    };
    const response = {
      schemaVersion: 1,
      action: "archive",
      sourceSessionId: session,
      targetSessionId: session,
      changed: true,
      catalog: {
        schemaVersion: 1,
        canMutate: true,
        revision: 2,
        activeSessionId: null,
        defaultSessionId: session,
        items: [],
        nonArchivedCount: 0,
        totalCount: 1,
        nextCursor: null,
        activeRun: null,
      },
    };
    const call = vi.fn().mockResolvedValue(response);
    expect(await requestSessionManage(project, operation, call)).toEqual(response);
    expect(call).toHaveBeenCalledWith("conversation_session_manage", {
      request: { projectSessionId: project, operation },
    });
    for (const bad of [
      { ...response, action: "delete" },
      { ...response, sourceSessionId: token },
      { ...response, targetSessionId: token },
      { ...response, hiddenPrompt: "not allowed" },
      { ...response, catalog: { ...response.catalog, canMutate: undefined } },
    ]) {
      await expect(
        requestSessionManage(project, operation, vi.fn().mockResolvedValue(bad)),
      ).rejects.toMatchObject({ code: "contract.invalid_response" });
    }
  });
  it("prepare returns only bounded opaque token and permits explicit native cancellation", async () => {
    const response = { schemaVersion: 1, token, expiresAtMs: 300001, bytes: 300 };
    const call = vi.fn().mockResolvedValue(response);
    expect(await requestConversationExportPrepare(project, session, "json", call)).toEqual(
      response,
    );
    expect(call).toHaveBeenCalledWith("conversation_export_prepare", {
      request: { projectSessionId: project, conversationSessionId: session, format: "json" },
    });
    expect(
      await requestConversationExportPrepare(
        project,
        session,
        "json",
        vi.fn().mockResolvedValue(null),
      ),
    ).toBeNull();
    for (const bad of [
      { ...response, path: "D:/private.json" },
      { ...response, bytes: 33554433 },
      { ...response, token: "path" },
      { ...response, expiresAtMs: 0 },
    ])
      await expect(
        requestConversationExportPrepare(project, session, "json", vi.fn().mockResolvedValue(bad)),
      ).rejects.toMatchObject({ code: "contract.invalid_response" });
  });
  it("commit accepts only bounded safe filename, hash and count; errors do not leak diagnostics", async () => {
    const response = { schemaVersion: 1, fileName: "聊天.json", sha256: "a".repeat(64), bytes: 90 };
    const call = vi.fn().mockResolvedValue(response);
    expect(await requestConversationExportCommit(project, token, call)).toEqual(response);
    expect(call).toHaveBeenCalledWith("conversation_export_commit", {
      request: { projectSessionId: project, token },
    });
    for (const fileName of [
      "../chat.json",
      "C:/chat.json",
      "chat:stream.json",
      "chat.exe",
      "chat\n.json",
    ])
      await expect(
        requestConversationExportCommit(
          project,
          token,
          vi.fn().mockResolvedValue({ ...response, fileName }),
        ),
      ).rejects.toMatchObject({ code: "contract.invalid_response" });
    await expect(
      requestConversationExportCommit(
        project,
        token,
        vi.fn().mockResolvedValue({ ...response, hiddenResult: "private" }),
      ),
    ).rejects.toMatchObject({ code: "contract.invalid_response" });
  });
});
