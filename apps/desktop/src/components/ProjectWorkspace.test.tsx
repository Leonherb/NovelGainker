import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ProjectWorkspace } from "./ProjectWorkspace";
import * as analysisApi from "../lib/tauri/analysis";
import * as appStateApi from "../lib/tauri/app-state";
import * as projectApi from "../lib/tauri/project";
import type { CloseRequestEvent, WindowClosePort } from "../lib/tauri/window-close";
import * as workspaceApi from "../lib/tauri/workspace";

const candidate: projectApi.ProjectCandidate = {
  candidateId: "10000000-0000-4000-8000-000000000001",
  mode: "create",
  displayName: "我的小说",
  existingEntryCount: 0,
  markdownFileCount: 0,
  hasProjectMetadata: false,
  requiresMetadataConfirmation: false,
  canConfirm: true,
  plannedEntries: [
    { relativePath: "project.md", kind: "file" },
    { relativePath: ".novelgainker/project.json", kind: "file" },
  ],
  conflicts: [],
};

const scaffoldContents = [
  ["project.md", '---\ntitle: "雾港来信"\nwritingLanguage: zh-CN\n---\n\n# 雾港来信\n'],
  ["AGENTS.md", "# 项目约定\n"],
  ["materials/000.story-ledger.md", "# 故事状态账本\n"],
  [
    ".novelgainker/project.json",
    '{\n  "schemaVersion": 1,\n  "projectId": "30000000-0000-4000-8000-000000000003"\n}\n',
  ],
] as const;

const createPreview: projectApi.ProjectCreatePreview = {
  schemaVersion: 1,
  previewHash: "a".repeat(64),
  projectId: "30000000-0000-4000-8000-000000000003",
  writingLanguage: "zh-CN",
  workflowPackVersion: "1.0.0",
  defaultProviderId: "openai",
  existingUnrelatedEntryCount: 0,
  directories: [
    "characters",
    "materials",
    "macguffins",
    "plot",
    "outlines",
    "world",
    "style",
    "assets",
    "chapters",
    "published",
    ".novelgainker",
  ],
  files: scaffoldContents.map(([relativePath, content]) => ({
    relativePath,
    content,
    byteLength: new TextEncoder().encode(content).length,
    contentSha256: "b".repeat(64),
  })),
};
const session: projectApi.ProjectSession = {
  sessionId: "20000000-0000-4000-8000-000000000002",
  projectId: "30000000-0000-4000-8000-000000000003",
  displayName: "我的小说",
  schemaVersion: 1,
  writingLanguage: "zh-CN",
  markdownFileCount: 3,
  accessMode: "writer",
  recoveryStatus: "ready",
  recoveredTransactionCount: 0,
  requiredTransactionCount: 0,
  restoredCurrentFile: null,
  capabilities: {
    canRead: true,
    canWrite: true,
    canBrowseFiles: true,
    canSaveFiles: true,
  },
};

const fileIndex: workspaceApi.ProjectFileIndex = {
  maxEditableFileBytes: workspaceApi.MAX_EDITABLE_FILE_BYTES,
  nodes: [
    {
      name: "draft.md",
      relativePath: "draft.md",
      kind: "markdown",
      sizeBytes: 15,
      readOnly: false,
      children: [],
    },
    {
      name: "chapters",
      relativePath: "chapters",
      kind: "directory",
      sizeBytes: null,
      readOnly: false,
      children: [
        {
          name: "01.md",
          relativePath: "chapters/01.md",
          kind: "markdown",
          sizeBytes: 13,
          readOnly: false,
          children: [],
        },
      ],
    },
    {
      name: ".novelgainker",
      relativePath: ".novelgainker",
      kind: "directory",
      sizeBytes: null,
      readOnly: true,
      children: [
        {
          name: "project.json",
          relativePath: ".novelgainker/project.json",
          kind: "metadata",
          sizeBytes: 40,
          readOnly: true,
          children: [],
        },
      ],
    },
  ],
};

const draftDocument: workspaceApi.ProjectFileDocument = {
  relativePath: "draft.md",
  content: "# 草稿\n\n第一段。",
  contentHash: "a".repeat(64),
  modifiedTimeMs: 1_700_000_000_000,
  sizeBytes: 22,
  kind: "markdown",
  readOnly: false,
  bom: "none",
  lineEnding: "lf",
};

const chapterDocument: workspaceApi.ProjectFileDocument = {
  ...draftDocument,
  relativePath: "chapters/01.md",
  content: "# 第一章\r\n\r\n正文。",
  contentHash: "b".repeat(64),
  lineEnding: "crlf",
};

const validValidation: analysisApi.ValidationReport = {
  relativePath: "draft.md",
  contentHash: "d".repeat(64),
  diagnostics: [],
  eligibleForAiApply: true,
  eligibleForFinal: true,
  eligibleForEpub: true,
};

const saveResult: workspaceApi.ProjectFileSaveResult = {
  relativePath: "draft.md",
  contentHash: "d".repeat(64),
  modifiedTimeMs: 1_700_000_000_100,
  sizeBytes: 28,
  bom: "none",
  lineEnding: "lf",
  diagnosticsTriggered: true,
  diagnosticState: "ready",
  validation: validValidation,
};
function createWindowCloseHarness() {
  let handler: ((event: CloseRequestEvent) => void | Promise<void>) | undefined;
  const dispose = vi.fn();
  const port: WindowClosePort = {
    onCloseRequested: vi.fn(async (nextHandler) => {
      handler = nextHandler;
      return dispose;
    }),
    close: vi.fn(async () => undefined),
  };
  return {
    port,
    dispose,
    requestClose: async () => {
      if (!handler) {
        throw new Error("close handler not installed");
      }
      const event = { preventDefault: vi.fn() };
      await act(async () => {
        await handler?.(event);
      });
      return event;
    },
  };
}

async function createProject(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "选择目录创建" }));
  expect(await screen.findByText("创建前预检")).toBeInTheDocument();
  await user.type(screen.getByRole("textbox", { name: "项目标题（必填）" }), "雾港来信");
  await user.click(screen.getByRole("button", { name: "生成完整脚手架预览" }));
  expect(await screen.findByText("目前仍为零写入")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "最终确认并创建" }));
  expect(await screen.findByText("项目已打开")).toBeInTheDocument();
}

describe("NG-105 project workspace", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(projectApi, "requestProjectScan").mockResolvedValue(candidate);
    vi.spyOn(projectApi, "requestProjectCreate").mockResolvedValue(session);
    vi.spyOn(projectApi, "requestProjectCreatePreview").mockResolvedValue(createPreview);
    vi.spyOn(appStateApi, "requestSetCurrentFile").mockResolvedValue({ saved: true });
    vi.spyOn(workspaceApi, "requestProjectFileIndex").mockResolvedValue(fileIndex);
    vi.spyOn(workspaceApi, "requestProjectFileWatch").mockImplementation(
      async (_sessionId, relativePath, expectedHash) => ({
        relativePath,
        state: "unchanged",
        observedHash: expectedHash,
        modifiedTimeMs: draftDocument.modifiedTimeMs,
      }),
    );
    vi.spyOn(workspaceApi, "requestProjectFileSave").mockResolvedValue(saveResult);
    vi.spyOn(analysisApi, "requestProjectDraftValidation").mockResolvedValue({
      ...validValidation,
      contentHash: "c".repeat(64),
    });
    vi.spyOn(analysisApi, "requestProjectSearch").mockResolvedValue({
      matches: [],
      scannedFileCount: 3,
      skippedFileCount: 0,
      scannedBytes: 100,
      truncated: false,
    });
  });

  it("previews the complete scaffold offline and invalidates confirmation after input changes", async () => {
    const user = userEvent.setup();
    const preview = vi.mocked(projectApi.requestProjectCreatePreview);
    const create = vi.mocked(projectApi.requestProjectCreate);
    render(<ProjectWorkspace desktopRuntime />);

    await user.click(screen.getByRole("button", { name: "选择目录创建" }));
    expect(await screen.findByText("创建前预检")).toBeInTheDocument();
    await user.type(screen.getByRole("textbox", { name: "项目标题（必填）" }), "雾港来信");
    await user.type(screen.getByRole("textbox", { name: "作者显示名（可选）" }), "林舟");
    await user.type(screen.getByRole("textbox", { name: "类型（可选）" }), "悬疑");
    await user.type(screen.getByRole("textbox", { name: "一句话前提（可选）" }), "迟到十年的信。");

    const confirm = screen.getByRole("button", { name: "最终确认并创建" });
    expect(confirm).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "生成完整脚手架预览" }));
    expect(await screen.findByText("目前仍为零写入")).toBeInTheDocument();
    expect(preview).toHaveBeenCalledWith(candidate.candidateId, {
      schemaVersion: 1,
      title: "雾港来信",
      authorDisplayName: "林舟",
      genre: "悬疑",
      premise: "迟到十年的信。",
    });
    expect(confirm).toBeEnabled();
    expect(screen.getByText(createPreview.previewHash)).toBeInTheDocument();
    for (const [relativePath] of scaffoldContents) {
      expect(screen.getAllByText(relativePath).length).toBeGreaterThan(0);
    }

    await user.type(screen.getByRole("textbox", { name: "项目标题（必填）" }), "续");
    expect(screen.queryByText("目前仍为零写入")).not.toBeInTheDocument();
    expect(confirm).toBeDisabled();
    expect(create).not.toHaveBeenCalled();
  });

  it("reports a real active workflow when it blocks project selection", async () => {
    const user = userEvent.setup();
    vi.mocked(projectApi.requestProjectScan).mockRejectedValue({
      code: "workflow.run_active",
      messageKey: "errors.workflowRunActive",
    });
    render(<ProjectWorkspace desktopRuntime />);

    await user.click(screen.getByRole("button", { name: "打开现有项目" }));

    expect(await screen.findByText("已有工作流正在执行")).toBeInTheDocument();
    expect(screen.getByText("当前工作流仍在占用运行通道，本次操作尚未执行。")).toBeInTheDocument();
    expect(
      screen.getByText("请等待当前工作流结束，或在对应运行界面取消后重试。"),
    ).toBeInTheDocument();
    expect(screen.queryByText(/本轮 Agent/u)).not.toBeInTheDocument();
  });

  it("reopens an opaque recent project and restores its relative current file", async () => {
    const user = userEvent.setup();
    const harness = createWindowCloseHarness();
    const changed = vi.fn();
    const recentProjectId = "30000000-0000-4000-8000-000000000003";
    const openRecent = vi.spyOn(projectApi, "requestProjectOpenRecent").mockResolvedValue({
      ...session,
      restoredCurrentFile: "chapters/01.md",
    });
    const read = vi.spyOn(workspaceApi, "requestProjectFile").mockResolvedValue(chapterDocument);

    render(
      <ProjectWorkspace
        desktopRuntime
        onAppStateChanged={changed}
        recentProjects={[{ projectId: recentProjectId, displayName: "Novel One" }]}
        windowClosePort={harness.port}
      />,
    );

    await user.click(screen.getByRole("button", { name: /Novel One/u }));

    expect(openRecent).toHaveBeenCalledWith(recentProjectId);
    expect(read).toHaveBeenCalledWith(session.sessionId, "chapters/01.md");
    expect(appStateApi.requestSetCurrentFile).toHaveBeenCalledWith(
      session.sessionId,
      "chapters/01.md",
    );
    expect(await screen.findByRole("textbox", { name: /Markdown/u })).toHaveValue(
      chapterDocument.content.replace(/\r\n/gu, "\n"),
    );
    expect(changed).toHaveBeenCalled();
  });

  it("confirms project creation, indexes the whitelist and edits a real source buffer", async () => {
    const user = userEvent.setup();
    const harness = createWindowCloseHarness();
    const read = vi.spyOn(workspaceApi, "requestProjectFile").mockResolvedValue(draftDocument);
    const getItem = vi.spyOn(Storage.prototype, "getItem");
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    render(<ProjectWorkspace desktopRuntime windowClosePort={harness.port} />);

    await createProject(user);
    expect(screen.getByRole("button", { name: "draft.md" })).toBeInTheDocument();
    expect(screen.queryByText("published")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "draft.md" }));
    const editor = await screen.findByRole("textbox", { name: "项目 Markdown 编辑器" });
    expect(editor).toHaveValue(draftDocument.content);
    expect(screen.getByText("已保存")).toBeInTheDocument();

    await user.clear(editor);
    await user.type(editor, "# 新稿\n\n手动修改。");
    const nextContent = "# 新稿\n\n手动修改。";
    expect(screen.getByText("未保存")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存" })).toBeEnabled();
    await user.keyboard("{Control>}s{/Control}");
    await waitFor(() =>
      expect(workspaceApi.requestProjectFileSave).toHaveBeenCalledWith(
        session.sessionId,
        "draft.md",
        nextContent,
        draftDocument.contentHash,
      ),
    );
    expect(await screen.findByText("已安全保存；新文件 Hash 的结构检查通过。")).toBeInTheDocument();
    expect(screen.getByText("已保存")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
    await user.type(editor, "继续修改");
    expect(await screen.findByText("诊断已过期")).toBeInTheDocument();
    expect(screen.getAllByText(/· 暂停$/u)).toHaveLength(3);
    expect(read).toHaveBeenCalledWith(session.sessionId, "draft.md");
    expect(getItem).not.toHaveBeenCalled();
    expect(setItem).not.toHaveBeenCalled();
  });

  it("shows saving state and blocks re-entry until the atomic save completes", async () => {
    const user = userEvent.setup();
    const harness = createWindowCloseHarness();
    let completeSave: ((result: workspaceApi.ProjectFileSaveResult) => void) | undefined;
    vi.spyOn(workspaceApi, "requestProjectFile").mockResolvedValue(draftDocument);
    vi.mocked(workspaceApi.requestProjectFileSave).mockReturnValue(
      new Promise((resolve) => {
        completeSave = resolve;
      }),
    );
    render(<ProjectWorkspace desktopRuntime windowClosePort={harness.port} />);
    await createProject(user);

    await user.click(screen.getByRole("button", { name: "draft.md" }));
    const editor = await screen.findByRole("textbox", { name: "项目 Markdown 编辑器" });
    await user.type(editor, "保存中测试");
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(screen.getAllByText("保存中").length).toBeGreaterThan(0);
    expect(editor).toHaveAttribute("readonly");
    expect(screen.getByRole("button", { name: "保存中" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "01.md" })).toBeDisabled();

    await act(async () => {
      completeSave?.(saveResult);
    });
    expect(await screen.findByText("已保存")).toBeInTheDocument();
  });
  it("keeps the dirty editor content when the save button sees a stale source hash", async () => {
    const user = userEvent.setup();
    const harness = createWindowCloseHarness();
    vi.spyOn(workspaceApi, "requestProjectFile").mockResolvedValue(draftDocument);
    vi.mocked(workspaceApi.requestProjectFileSave).mockRejectedValue({
      code: "file.source_changed",
      messageKey: "errors.fileSourceChanged",
    });
    render(<ProjectWorkspace desktopRuntime windowClosePort={harness.port} />);
    await createProject(user);

    await user.click(screen.getByRole("button", { name: "draft.md" }));
    const editor = await screen.findByRole("textbox", { name: "项目 Markdown 编辑器" });
    const memoryDraft = `${draftDocument.content}外部冲突前的内存内容`;
    fireEvent.change(editor, { target: { value: memoryDraft } });
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(await screen.findByText("磁盘文件已经变化")).toBeInTheDocument();
    expect(screen.getByText("保存失败")).toBeInTheDocument();
    expect(editor).toHaveValue(memoryDraft);
    expect(screen.getByRole("button", { name: "保存" })).toBeEnabled();
    expect(screen.getByText(/当前编辑内容仍保留/u)).toBeInTheDocument();
  });
  it("detects an external edit, blocks overwrite, and offers all three recovery actions", async () => {
    const user = userEvent.setup();
    const harness = createWindowCloseHarness();
    const externalDocument: workspaceApi.ProjectFileDocument = {
      ...draftDocument,
      content: "# 外部版本\n",
      contentHash: "e".repeat(64),
      modifiedTimeMs: draftDocument.modifiedTimeMs + 10,
      sizeBytes: 16,
    };
    let reportExternalChange: ((status: workspaceApi.ProjectFileWatchStatus) => void) | undefined;
    const firstWatch = new Promise<workspaceApi.ProjectFileWatchStatus>((resolve) => {
      reportExternalChange = resolve;
    });
    vi.spyOn(workspaceApi, "requestProjectFile")
      .mockResolvedValueOnce(draftDocument)
      .mockResolvedValueOnce(externalDocument);
    vi.mocked(workspaceApi.requestProjectFileWatch).mockImplementation(
      async (_sessionId, relativePath, expectedHash) => {
        if (expectedHash === draftDocument.contentHash) {
          return firstWatch;
        }
        return {
          relativePath,
          state: "unchanged",
          observedHash: expectedHash,
          modifiedTimeMs: externalDocument.modifiedTimeMs,
        };
      },
    );
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    render(<ProjectWorkspace desktopRuntime windowClosePort={harness.port} />);
    await createProject(user);

    await user.click(screen.getByRole("button", { name: "draft.md" }));
    const editor = await screen.findByRole("textbox", { name: "项目 Markdown 编辑器" });
    await user.type(editor, "内存新增");
    const memoryDraft = `${draftDocument.content}内存新增`;
    await act(async () => {
      reportExternalChange?.({
        relativePath: "draft.md",
        state: "modified",
        observedHash: externalDocument.contentHash,
        modifiedTimeMs: externalDocument.modifiedTimeMs,
      });
    });

    expect(await screen.findByText("磁盘文件已在外部变化")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
    expect(workspaceApi.requestProjectFileSave).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "复制内存文本" }));
    expect(writeText).toHaveBeenCalledWith(memoryDraft);
    expect(await screen.findByText("内存文本已复制到剪贴板。")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "保留内存文本" }));
    expect(editor).toHaveValue(memoryDraft);
    expect(await screen.findByText(/仍不能保存/u)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "重新载入磁盘版本" }));
    expect(await screen.findByRole("textbox", { name: "项目 Markdown 编辑器" })).toHaveValue(
      externalDocument.content,
    );
    expect(screen.queryByText("磁盘文件已在外部变化")).not.toBeInTheDocument();
  });

  it("opens a second instance as read-only without transferring the writer lock", async () => {
    const user = userEvent.setup();
    const harness = createWindowCloseHarness();
    vi.mocked(projectApi.requestProjectCreate).mockResolvedValue({
      ...session,
      accessMode: "readOnly",
      recoveryStatus: "notChecked",
      recoveredTransactionCount: 0,
      requiredTransactionCount: 0,
      capabilities: {
        canRead: true,
        canWrite: false,
        canBrowseFiles: true,
        canSaveFiles: false,
      },
    });
    vi.spyOn(workspaceApi, "requestProjectFile").mockResolvedValue(draftDocument);
    render(<ProjectWorkspace desktopRuntime windowClosePort={harness.port} />);
    await createProject(user);

    expect(screen.getByText("第二实例 · 只读会话")).toBeInTheDocument();
    expect(screen.getByText(/本窗口只能浏览/u)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "draft.md" }));
    const editor = await screen.findByRole("textbox", { name: "项目 Markdown 编辑器" });
    expect(editor).toHaveAttribute("readonly");
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  });

  it("shows startup recovery results and blocks writes when manual recovery is required", async () => {
    const user = userEvent.setup();
    const harness = createWindowCloseHarness();
    vi.mocked(projectApi.requestProjectCreate).mockResolvedValue({
      ...session,
      recoveryStatus: "required",
      recoveredTransactionCount: 0,
      requiredTransactionCount: 1,
      capabilities: {
        canRead: true,
        canWrite: false,
        canBrowseFiles: true,
        canSaveFiles: false,
      },
    });
    vi.spyOn(workspaceApi, "requestProjectFile").mockResolvedValue(draftDocument);
    render(<ProjectWorkspace desktopRuntime windowClosePort={harness.port} />);
    await createProject(user);

    expect(screen.getByRole("alert")).toHaveTextContent("发现无法自动判定的保存事务");
    await user.click(screen.getByRole("button", { name: "draft.md" }));
    expect(await screen.findByRole("textbox", { name: "项目 Markdown 编辑器" })).toHaveAttribute(
      "readonly",
    );
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  });

  it("refreshes and opens an applied character without discarding a dirty manual draft", async () => {
    const user = userEvent.setup();
    const harness = createWindowCloseHarness();
    const characterDocument: workspaceApi.ProjectFileDocument = {
      ...draftDocument,
      relativePath: "characters/林遥.md",
      content: '---\ntitle: "林遥"\nwritingLanguage: zh-CN\n---\n\n# 林遥\n',
      contentHash: "e".repeat(64),
      sizeBytes: 68,
    };
    const refreshedIndex: workspaceApi.ProjectFileIndex = {
      ...fileIndex,
      nodes: [
        ...fileIndex.nodes,
        {
          name: "characters",
          relativePath: "characters",
          kind: "directory",
          sizeBytes: null,
          readOnly: false,
          children: [
            {
              name: "林遥.md",
              relativePath: "characters/林遥.md",
              kind: "markdown",
              sizeBytes: characterDocument.sizeBytes,
              readOnly: false,
              children: [],
            },
          ],
        },
      ],
    };
    const read = vi
      .spyOn(workspaceApi, "requestProjectFile")
      .mockImplementation(async (_sessionId, relativePath) =>
        relativePath === characterDocument.relativePath ? characterDocument : draftDocument,
      );
    const { rerender } = render(<ProjectWorkspace desktopRuntime windowClosePort={harness.port} />);
    await createProject(user);
    await user.click(screen.getByRole("button", { name: "draft.md" }));
    const editor = await screen.findByRole("textbox", { name: "项目 Markdown 编辑器" });
    fireEvent.change(editor, { target: { value: `${draftDocument.content}手动未保存` } });
    vi.mocked(workspaceApi.requestProjectFileIndex).mockResolvedValueOnce(refreshedIndex);

    rerender(
      <ProjectWorkspace
        desktopRuntime
        externalOpenPath="characters/林遥.md"
        externalRefreshToken={1}
        windowClosePort={harness.port}
      />,
    );

    expect(await screen.findByRole("button", { name: "林遥.md" })).toBeInTheDocument();
    expect(screen.getByRole("alertdialog", { name: "放弃未保存修改？" })).toBeInTheDocument();
    expect(editor).toHaveValue(`${draftDocument.content}手动未保存`);
    expect(read).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole("button", { name: "放弃修改并继续" }));
    expect(await screen.findByRole("textbox", { name: "项目 Markdown 编辑器" })).toHaveValue(
      characterDocument.content,
    );
    expect(read).toHaveBeenLastCalledWith(session.sessionId, "characters/林遥.md");
    expect(appStateApi.requestSetCurrentFile).toHaveBeenLastCalledWith(
      session.sessionId,
      "characters/林遥.md",
    );
  });

  it("protects a dirty buffer before switching files", async () => {
    const user = userEvent.setup();
    const harness = createWindowCloseHarness();
    const read = vi
      .spyOn(workspaceApi, "requestProjectFile")
      .mockResolvedValueOnce(draftDocument)
      .mockResolvedValueOnce(chapterDocument);
    render(<ProjectWorkspace desktopRuntime windowClosePort={harness.port} />);
    await createProject(user);

    await user.click(screen.getByRole("button", { name: "draft.md" }));
    const editor = await screen.findByRole("textbox", { name: "项目 Markdown 编辑器" });
    fireEvent.change(editor, {
      target: { value: `${draftDocument.content}新增` },
    });
    await user.click(screen.getByRole("button", { name: "01.md" }));

    expect(screen.getByRole("alertdialog", { name: "放弃未保存修改？" })).toBeInTheDocument();
    expect(read).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "继续编辑" }));
    expect(editor).toHaveValue(`${draftDocument.content}新增`);

    await user.click(screen.getByRole("button", { name: "01.md" }));
    await user.click(screen.getByRole("button", { name: "放弃修改并继续" }));
    expect(await screen.findByRole("textbox", { name: "项目 Markdown 编辑器" })).toHaveValue(
      chapterDocument.content.replace(/\r\n/gu, "\n"),
    );
    expect(read).toHaveBeenCalledTimes(2);
  });

  it("protects dirty edits before closing the project", async () => {
    const user = userEvent.setup();
    const harness = createWindowCloseHarness();
    const close = vi.spyOn(projectApi, "requestProjectClose").mockResolvedValue({ closed: true });
    vi.spyOn(workspaceApi, "requestProjectFile").mockResolvedValue(draftDocument);
    render(<ProjectWorkspace desktopRuntime windowClosePort={harness.port} />);
    await createProject(user);

    await user.click(screen.getByRole("button", { name: "draft.md" }));
    await user.type(await screen.findByRole("textbox", { name: "项目 Markdown 编辑器" }), "修改");
    await user.click(screen.getByRole("button", { name: "关闭项目" }));
    expect(close).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "继续编辑" }));
    expect(screen.getByText("未保存")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "关闭项目" }));
    await user.click(screen.getByRole("button", { name: "放弃修改并继续" }));
    await waitFor(() => expect(close).toHaveBeenCalledWith(session.sessionId));
    expect(screen.getByText("未打开项目")).toBeInTheDocument();
  });

  it("prevents a window close until the user explicitly discards dirty edits", async () => {
    const user = userEvent.setup();
    const harness = createWindowCloseHarness();
    vi.spyOn(workspaceApi, "requestProjectFile").mockResolvedValue(draftDocument);
    render(<ProjectWorkspace desktopRuntime windowClosePort={harness.port} />);
    await createProject(user);
    await waitFor(() => expect(harness.port.onCloseRequested).toHaveBeenCalledOnce());

    await user.click(screen.getByRole("button", { name: "draft.md" }));
    await user.type(await screen.findByRole("textbox", { name: "项目 Markdown 编辑器" }), "修改");
    const closeEvent = await harness.requestClose();

    expect(closeEvent.preventDefault).toHaveBeenCalledOnce();
    expect(
      screen.getByText("继续关闭 NovelGainker 会丢弃当前编辑缓冲区中的修改。"),
    ).toBeInTheDocument();
    expect(harness.port.close).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "放弃修改并继续" }));
    expect(harness.port.close).toHaveBeenCalledOnce();
  });

  it("shows controlled read failures and keeps metadata read-only", async () => {
    const user = userEvent.setup();
    const harness = createWindowCloseHarness();
    const metadataDocument: workspaceApi.ProjectFileDocument = {
      ...draftDocument,
      relativePath: ".novelgainker/project.json",
      content: '{"schemaVersion":1}',
      contentHash: "c".repeat(64),
      kind: "metadata",
      readOnly: true,
      lineEnding: "none",
    };
    vi.spyOn(workspaceApi, "requestProjectFile")
      .mockResolvedValueOnce(metadataDocument)
      .mockRejectedValueOnce({
        code: "file.too_large",
        messageKey: "errors.fileTooLarge",
      });
    render(<ProjectWorkspace desktopRuntime windowClosePort={harness.port} />);
    await createProject(user);

    await user.click(screen.getByRole("button", { name: /project\.json/u }));
    expect(await screen.findByRole("textbox", { name: "项目 Markdown 编辑器" })).toHaveAttribute(
      "readonly",
    );
    expect(screen.getAllByText("只读").length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: "draft.md" }));
    expect(await screen.findByText("文件过大，无法在编辑器中打开")).toBeInTheDocument();
    expect(screen.getAllByText("draft.md").length).toBeGreaterThan(0);
    expect(document.body.textContent).not.toMatch(/[A-Z]:\\/u);
  });

  it("retains NG-101 conflict blocking and safe cancellation", async () => {
    const user = userEvent.setup();
    const harness = createWindowCloseHarness();
    const create = vi.mocked(projectApi.requestProjectCreate);
    vi.mocked(projectApi.requestProjectScan)
      .mockResolvedValueOnce({
        ...candidate,
        canConfirm: false,
        conflicts: [{ relativePath: "project.md", kind: "existingTarget" }],
      })
      .mockResolvedValueOnce(null);
    render(<ProjectWorkspace desktopRuntime windowClosePort={harness.port} />);

    await user.click(screen.getByRole("button", { name: "选择目录创建" }));
    expect(await screen.findByText("目标已存在，不会覆盖")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "最终确认并创建" })).toBeDisabled();
    expect(create).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "返回" }));
    await user.click(screen.getByRole("button", { name: "选择目录创建" }));
    expect(await screen.findByText("已取消目录选择，未写入任何文件。")).toBeInTheDocument();
  });
});
