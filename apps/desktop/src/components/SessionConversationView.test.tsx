import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SessionConversationView } from "./SessionConversationView";
import { DEFAULT_PROVIDER_STATE } from "./provider-settings-model";
import * as sessions from "../lib/tauri/conversation-sessions";
import * as conversation from "../lib/tauri/conversation";
import * as publication from "../lib/tauri/publication";
import * as changes from "../lib/tauri/canon-change";
import * as management from "../lib/tauri/session-management";
const project = "10000000-0000-4000-8000-000000000001";
const first = "20000000-0000-4000-8000-000000000002";
const second = "30000000-0000-4000-8000-000000000003";
const turn = "40000000-0000-4000-8000-000000000004";
const providerState = {
  ...DEFAULT_PROVIDER_STATE,
  providers: DEFAULT_PROVIDER_STATE.providers.map((p) => ({ ...p, configured: true })),
};
let page: sessions.SessionPage;
let listeners: Array<(event: conversation.ConversationTurnEvent) => void>;
beforeEach(() => {
  vi.restoreAllMocks();
  listeners = [];
  page = {
    schemaVersion: 1,
    revision: 1,
    activeSessionId: first,
    defaultSessionId: first,
    items: [first, second].map((id, index) => ({
      sessionId: id,
      name: index === 0 ? "最初的会话" : "第二段聊天",
      archived: false,
      createdAtMs: 1,
      updatedAtMs: 1,
      creationRequestId: null,
    })),
    nonArchivedCount: 2,
    canMutate: true,
    totalCount: 2,
    nextCursor: null,
    activeRun: null,
  };
  vi.spyOn(sessions, "requestSessions").mockImplementation(async () => structuredClone(page));
  vi.spyOn(sessions, "requestSessionActivate").mockImplementation(async (_project, id) => {
    page = { ...page, revision: page.revision + 1, activeSessionId: id };
    return structuredClone(page);
  });
  vi.spyOn(conversation, "requestConversationSession").mockImplementation(async (_project, id) => ({
    schemaVersion: 1,
    conversation: { schemaVersion: 1, projectId: project, sessionId: id!, turns: [] },
    taskActivities: [],
  }));
  vi.spyOn(conversation, "listenConversationTurnEvents").mockImplementation(async (handler) => {
    listeners.push(handler);
    return () => undefined;
  });
  vi.spyOn(publication, "requestPublications").mockResolvedValue({
    schemaVersion: 1,
    items: [],
    nextCursor: null,
  });
  vi.spyOn(changes, "requestCanonChangesPage").mockResolvedValue({
    schemaVersion: 1,
    items: [],
    nextCursor: null,
    candidates: [],
  });
});
describe("NR-601 session conversation view", () => {
  it("侧栏会话管理独立挂载，折叠后保留命名输入和聊天草稿", async () => {
    const user = userEvent.setup();
    const host = document.createElement("aside");
    document.body.append(host);
    const view = render(
      <SessionConversationView
        projectSessionId={project}
        providerState={providerState}
        managementHost={host}
      />,
    );
    try {
      const toolbar = await within(host).findByRole("region", { name: "会话管理" });
      expect(within(view.container).queryByRole("region", { name: "会话管理" })).toBeNull();
      const draft = await screen.findByRole("textbox", { name: "对话输入" });
      await waitFor(() => expect(draft).toBeEnabled());
      await user.type(draft, "保留的小说构思");
      await user.click(within(toolbar).getByRole("button", { name: "新建会话" }));
      const name = within(host).getByRole("textbox", { name: "会话名称" });
      expect(name).toHaveFocus();
      await user.type(name, "另一条剧情");
      host.hidden = true;
      expect(draft).toHaveValue("保留的小说构思");
      host.hidden = false;
      expect(within(host).getByRole("textbox", { name: "会话名称" })).toHaveValue("另一条剧情");
      expect(within(host).getByRole("region", { name: "会话管理" })).toBe(toolbar);
    } finally {
      view.unmount();
      host.remove();
    }
  });

  it("NR-602 keeps the original clear request after a lost acknowledgement changes active session", async () => {
    const user = userEvent.setup();
    const manage = vi
      .spyOn(management, "requestSessionManage")
      .mockImplementationOnce(async () => {
        page = {
          ...page,
          revision: 2,
          activeSessionId: second,
          items: [page.items[1]],
          nonArchivedCount: 1,
        };
        throw { code: "runtime.unavailable", messageKey: "errors.runtimeUnavailable" };
      })
      .mockImplementation(async (_project, operation) => ({
        schemaVersion: 1,
        action: operation.action,
        sourceSessionId: first,
        targetSessionId: second,
        changed: false,
        catalog: page,
      }));
    render(<SessionConversationView projectSessionId={project} providerState={providerState} />);
    await screen.findByRole("textbox", { name: "对话输入" });
    await user.click(screen.getByText("更多会话操作"));
    await user.click(screen.getByRole("button", { name: "清空当前聊天…" }));
    expect(manage).not.toHaveBeenCalled();
    expect(screen.getByText(/清空会归档旧会话/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "归档旧聊天并清空" }));
    await screen.findByRole("alert");
    expect(screen.getByRole("button", { name: "发送" })).toBeDisabled();
    expect(screen.getByLabelText("当前会话")).toHaveValue(first);
    expect(screen.getByText(/原会话状态待核对/)).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "归档旧聊天并清空" })).toBeEnabled(),
    );
    await user.click(screen.getByRole("button", { name: "归档旧聊天并清空" }));
    await waitFor(() => expect(manage).toHaveBeenCalledTimes(2));
    const a = manage.mock.calls[0][1],
      b = manage.mock.calls[1][1];
    expect(a.conversationSessionId).toBe(first);
    expect(b.conversationSessionId).toBe(first);
    expect(a.action).toBe("clear");
    expect(b.action).toBe("clear");
    if (a.action === "clear" && b.action === "clear") expect(a.requestId).toBe(b.requestId);
    await screen.findByText(/已打开空会话/);
  });
  it("NR-602 read-only switching stays local and cannot send or clear", async () => {
    page.canMutate = false;
    const start = vi.spyOn(conversation, "requestConversationTurnStart");
    const user = userEvent.setup();
    render(<SessionConversationView projectSessionId={project} providerState={providerState} />);
    const box = await screen.findByRole("textbox", { name: "对话输入" });
    await waitFor(() => expect(box).toBeEnabled());
    await user.type(box, "只做草稿{Enter}");
    expect(screen.getByRole("button", { name: "发送" })).toBeDisabled();
    expect(start).not.toHaveBeenCalled();
    await user.selectOptions(screen.getByLabelText("当前会话"), second);
    await waitFor(() =>
      expect(conversation.requestConversationSession).toHaveBeenCalledWith(project, second),
    );
    expect(sessions.requestSessionActivate).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "新建会话" })).toBeDisabled();
  });
  it("NR-602 cancelled export does not commit and successful export uses only opaque token", async () => {
    const prepare = vi
      .spyOn(management, "requestConversationExportPrepare")
      .mockResolvedValueOnce(null)
      .mockResolvedValue({
        schemaVersion: 1,
        token: turn,
        expiresAtMs: Date.now() + 300000,
        bytes: 80,
      });
    const commit = vi.spyOn(management, "requestConversationExportCommit").mockResolvedValue({
      schemaVersion: 1,
      fileName: "聊天.md",
      bytes: 80,
      sha256: "a".repeat(64),
    });
    const user = userEvent.setup();
    render(<SessionConversationView projectSessionId={project} providerState={providerState} />);
    await screen.findByRole("textbox", { name: "对话输入" });
    await user.click(screen.getByText("更多会话操作"));
    await user.click(screen.getByRole("button", { name: "导出可见聊天…" }));
    await screen.findByText("已取消导出，没有写入文件。");
    expect(commit).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "导出可见聊天…" }));
    await screen.findByText("已导出 聊天.md（80 字节）。");
    expect(prepare).toHaveBeenCalledWith(project, first, "markdown");
    expect(commit).toHaveBeenCalledWith(project, turn);
  });
  it("NR-602 archived selection is read-only without activating or replaying tools", async () => {
    const archived = { ...page.items[1], archived: true, name: "旧聊天" };
    page = { ...page, items: [page.items[0]], nonArchivedCount: 1 };
    vi.mocked(sessions.requestSessions).mockImplementation(async (_id, archives) => ({
      ...page,
      items: archives ? [archived] : page.items,
    }));
    const user = userEvent.setup();
    render(<SessionConversationView projectSessionId={project} providerState={providerState} />);
    await screen.findByRole("textbox", { name: "对话输入" });
    await user.click(screen.getByText("更多会话操作"));
    await user.click(screen.getByRole("button", { name: "查看归档" }));
    await user.click(await screen.findByRole("button", { name: "旧聊天（只读查看）" }));
    await waitFor(() =>
      expect(conversation.requestConversationSession).toHaveBeenCalledWith(project, second),
    );
    expect(screen.getByRole("button", { name: "发送" })).toBeDisabled();
    expect(sessions.requestSessionActivate).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "恢复此会话" })).toBeEnabled();
  });
  it("switches explicit session IDs, retains drafts and ignores old conversation events", async () => {
    const user = userEvent.setup();
    const start = vi.spyOn(conversation, "requestConversationTurnStart");
    render(<SessionConversationView projectSessionId={project} providerState={providerState} />);
    const box = await screen.findByRole("textbox", { name: "对话输入" });
    await waitFor(() => expect(box).toBeEnabled());
    await user.type(box, "尚未发送的想法");
    await user.selectOptions(screen.getByLabelText("当前会话"), second);
    await waitFor(() =>
      expect(conversation.requestConversationSession).toHaveBeenCalledWith(project, second),
    );
    await waitFor(() => expect(screen.getByRole("textbox", { name: "对话输入" })).toHaveValue(""));
    act(() =>
      listeners.forEach((handler) =>
        handler({
          sessionId: project,
          conversationSessionId: first,
          attemptId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
          attemptNumber: 1,
          turnId: turn,
          providerId: "deepseek",
          model: "deepseek-v4-pro",
          sequence: 1,
          kind: "textDelta",
          delta: "不应出现在第二段聊天",
        }),
      ),
    );
    expect(screen.queryByText("不应出现在第二段聊天")).not.toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText("当前会话"), first);
    await waitFor(() =>
      expect(screen.getByRole("textbox", { name: "对话输入" })).toHaveValue("尚未发送的想法"),
    );
    expect(start).not.toHaveBeenCalled();
  });
  it("stops the actual background turn and blocks only sending, not switching or typing", async () => {
    page.activeSessionId = second;
    page.activeRun = { reservationId: second, sessionId: first, turnId: turn };
    const cancel = vi
      .spyOn(conversation, "requestConversationTurnCancel")
      .mockResolvedValue({ cancelRequested: true });
    const start = vi.spyOn(conversation, "requestConversationTurnStart");
    const user = userEvent.setup();
    render(<SessionConversationView projectSessionId={project} providerState={providerState} />);
    await screen.findByRole("button", { name: "停止原会话回复" });
    await waitFor(() => expect(screen.getByRole("textbox", { name: "对话输入" })).toBeEnabled());
    await user.type(screen.getByRole("textbox", { name: "对话输入" }), "先记下想法{Enter}");
    expect(screen.getByRole("button", { name: "发送" })).toBeDisabled();
    expect(screen.getByLabelText("当前会话")).toBeEnabled();
    expect(start).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "停止原会话回复" }));
    expect(cancel).toHaveBeenCalledWith(project, turn);
  });
  it("retains field values on conflict and retries creation with the same request ID", async () => {
    const create = vi
      .spyOn(sessions, "requestSessionCreate")
      .mockRejectedValueOnce({
        code: "sessions.revision_conflict",
        messageKey: "errors.runtimeUnavailable",
      })
      .mockImplementation(async () => ({ ...page, revision: 2 }));
    const user = userEvent.setup();
    render(<SessionConversationView projectSessionId={project} providerState={providerState} />);
    await waitFor(() => expect(screen.getByRole("button", { name: "新建会话" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "新建会话" }));
    await user.type(screen.getByLabelText("会话名称"), "人物讨论");
    await user.click(screen.getByRole("button", { name: "保存名称" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("状态已变化");
    expect(screen.getByLabelText("会话名称")).toHaveValue("人物讨论");
    expect(create).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "保存名称" }));
    await waitFor(() => expect(create).toHaveBeenCalledTimes(2));
    expect(create.mock.calls[0][1]).toEqual(create.mock.calls[1][1]);
  });
});
