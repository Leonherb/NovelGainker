import { useCallback, useEffect, useRef, useState } from "react";
import { ConversationView, type ConversationViewProps } from "./ConversationView";
import { requestConversationTurnCancel } from "../lib/tauri/conversation";
import { normalizeAppError } from "../lib/tauri/health";
import {
  requestSessions,
  requestSessionCreate,
  requestSessionRename,
  requestSessionActivate,
  type SessionPage,
  type SessionEntry,
} from "../lib/tauri/conversation-sessions";
import {
  requestSessionManage,
  requestConversationExportPrepare,
  requestConversationExportCommit,
  type SessionManagement,
  type ExportFormat,
} from "../lib/tauri/session-management";

const sessionError = (failure: unknown) => {
  const code = normalizeAppError(failure).code;
  if (code === "sessions.full") return "会话数量或本地存储已达上限，现有聊天仍保留。";
  if (code === "sessions.revision_conflict") return "会话状态已变化，已重新读取；请确认后再操作。";
  if (code === "sessions.invalid_name") return "请输入 1–60 个字符的会话名称。";
  if (code === "project.writer_required")
    return "另一实例正在使用此工程；当前只能查看。关闭该实例后重新打开工程再操作。";
  if (code === "sessions.export_exists")
    return "目标文件已存在，请选择新文件名；现有文件不会被覆盖。";
  if (code === "sessions.export_busy")
    return "已有导出正在准备或保存，请先完成或取消保存对话框；未提交的导出凭证会在五分钟后过期。";
  if (code === "sessions.export_boundary")
    return "请选择小说工程之外的现有普通文件夹，并使用所选格式的扩展名。";
  if (code === "sessions.export_stale" || code === "sessions.export_expired")
    return "导出快照已变化或过期，请重新选择导出。";
  if (code === "sessions.run_active") return "请等待当前回复结束再操作，不会自动停止生成。";
  return "会话操作未完成，现有内容未被清除。请重试。";
};

// The parent keys this component by project. The child is keyed by conversation,
// so a late load/event from an old view cannot replace the selected conversation.
export function SessionConversationView(props: ConversationViewProps) {
  const { projectSessionId, onRunActiveChange } = props;
  const [page, setPage] = useState<SessionPage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState<"create" | "rename" | "branch" | "clear" | null>(null);
  const [viewed, setViewed] = useState<string | null>(null);
  const [archives, setArchives] = useState<SessionEntry[]>([]);
  const [archiveCursor, setArchiveCursor] = useState<string | null>(null);
  const [archiveRevision, setArchiveRevision] = useState(0);
  const [showArchives, setShowArchives] = useState(false);
  const [boundary, setBoundary] = useState<string | null>(null);
  const [format, setFormat] = useState<ExportFormat>("markdown");
  const [name, setName] = useState("");
  const [notice, setNotice] = useState("");
  const [localRun, setLocalRun] = useState(false);
  const alive = useRef(true);
  const loading = useRef(false);
  const mutating = useRef(false);
  const creation = useRef<{ id: string; name: string } | null>(null);
  const currentPage = useRef<SessionPage | null>(null);
  const [drafts, setDrafts] = useState<Record<string, string>>({});

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  const accept = useCallback((incoming: SessionPage) => {
    if (!alive.current || (currentPage.current && incoming.revision < currentPage.current.revision))
      return;
    currentPage.current = incoming;
    setPage(incoming);
  }, []);
  const refresh = useCallback(async () => {
    if (!projectSessionId || loading.current) return;
    loading.current = true;
    try {
      accept(await requestSessions(projectSessionId));
    } catch (failure: unknown) {
      if (alive.current) setError(sessionError(failure));
    } finally {
      loading.current = false;
    }
  }, [accept, projectSessionId]);
  useEffect(() => {
    void refresh();
  }, [refresh]);

  const runActive = localRun || page?.activeRun != null;
  useEffect(() => {
    onRunActiveChange?.(runActive);
  }, [runActive, onRunActiveChange]);
  useEffect(() => {
    if (!runActive) return;
    const timer = setInterval(() => {
      void refresh();
    }, 1_000);
    return () => clearInterval(timer);
  }, [refresh, runActive]);
  const recordRun = useCallback(
    (active: boolean) => {
      setLocalRun(active);
      void refresh();
    },
    [refresh],
  );

  const mutate = async (operation: () => Promise<SessionPage>, success: string) => {
    if (mutating.current) return;
    mutating.current = true;
    setBusy(true);
    setError(null);
    setNotice("");
    try {
      const result = await operation();
      accept(result);
      if (alive.current) {
        setEditing(null);
        setNotice(success);
        creation.current = null;
      }
    } catch (failure: unknown) {
      if (alive.current) setError(sessionError(failure));
      await refresh();
    } finally {
      mutating.current = false;
      if (alive.current) setBusy(false);
    }
  };
  const selected = viewed ?? page?.activeSessionId ?? null;
  const recordDraft = useCallback(
    (text: string) => {
      if (selected)
        setDrafts((current) =>
          current[selected] === text ? current : { ...current, [selected]: text },
        );
    },
    [selected],
  );
  const selectedEntry =
    page?.items.find((item) => item.sessionId === selected) ??
    archives.find((item) => item.sessionId === selected);
  const canMutate = page?.canMutate === true;
  const archived = selectedEntry?.archived === true;
  const selectionUncertain = selected !== null && !selectedEntry;
  const otherRun = page?.activeRun && page.activeRun.sessionId !== selected ? page.activeRun : null;
  const saveName = () => {
    if (!projectSessionId || !page || !editing) return;
    const trimmed = name.trim();
    if (
      trimmed.length === 0 ||
      [...trimmed].length > 60 ||
      new TextEncoder().encode(trimmed).length > 180
    ) {
      setError("请输入 1–60 个字符的会话名称。");
      return;
    }
    if (editing === "create") {
      if (!creation.current || creation.current.name !== trimmed)
        creation.current = { id: crypto.randomUUID(), name: trimmed };
      const request = creation.current;
      void mutate(async () => {
        const result = await requestSessionCreate(
          projectSessionId,
          request.id,
          request.name,
          page.revision,
        );
        setViewed(null);
        return result;
      }, "已新建会话。");
    } else if ((editing === "branch" || editing === "clear") && selected) {
      if (editing === "branch" && !boundary) return;
      const fingerprint = `${editing}:${selected}:${boundary ?? ""}:${trimmed}`;
      if (!creation.current || creation.current.name !== fingerprint)
        creation.current = { id: crypto.randomUUID(), name: fingerprint };
      const common = {
        conversationSessionId: selected,
        expectedRevision: page.revision,
        requestId: creation.current.id,
        name: trimmed,
      };
      void manage(
        editing === "branch"
          ? { ...common, action: "branch", boundaryTurnId: boundary! }
          : { ...common, action: "clear" },
      );
    } else if (selected) {
      void mutate(
        () => requestSessionRename(projectSessionId, selected, trimmed, page.revision),
        "会话已重命名。",
      );
    }
  };

  const loadArchives = async (next = false) => {
    if (!projectSessionId || mutating.current) return;
    mutating.current = true;
    setBusy(true);
    setError(null);
    try {
      const result = await requestSessions(projectSessionId, true, next ? archiveCursor : null);
      if (!alive.current) return;
      if (next && result.revision !== archiveRevision) throw new Error("stale archive page");
      setArchives((old) => (next ? [...old, ...result.items] : result.items));
      setArchiveCursor(result.nextCursor);
      setArchiveRevision(result.revision);
      setShowArchives(true);
    } catch (failure: unknown) {
      if (alive.current) setError(sessionError(failure));
    } finally {
      mutating.current = false;
      if (alive.current) setBusy(false);
    }
  };
  const manage = async (operation: SessionManagement) => {
    if (!projectSessionId || !canMutate || runActive) return;
    await mutate(
      async () => {
        const result = await requestSessionManage(projectSessionId, operation);
        if (alive.current) {
          setViewed(operation.action === "archive" ? null : result.targetSessionId);
          setArchives([]);
          setShowArchives(false);
          setArchiveCursor(null);
        }
        return result.catalog;
      },
      operation.action === "clear"
        ? "已打开空会话。旧聊天在归档中，共享资料未改变。"
        : operation.action === "branch"
          ? "已创建可见聊天分支，不会重放旧工具；共享资料保持当前版本。"
          : operation.action === "restore"
            ? "已恢复会话，没有重新执行旧操作。"
            : "已归档，可从归档列表恢复。资料未改变。",
    );
  };
  const exportVisible = async () => {
    if (!projectSessionId || !selected || mutating.current || runActive) return;
    mutating.current = true;
    setBusy(true);
    setError(null);
    setNotice("请选择工程之外的新文件名…");
    try {
      const prepared = await requestConversationExportPrepare(projectSessionId, selected, format);
      if (!alive.current) return;
      if (!prepared) {
        setNotice("已取消导出，没有写入文件。");
        return;
      }
      const receipt = await requestConversationExportCommit(projectSessionId, prepared.token);
      if (alive.current) setNotice(`已导出 ${receipt.fileName}（${receipt.bytes} 字节）。`);
    } catch (failure: unknown) {
      if (alive.current) {
        setError(sessionError(failure));
        setNotice("");
      }
    } finally {
      mutating.current = false;
      if (alive.current) setBusy(false);
    }
  };

  if (!projectSessionId) return <ConversationView {...props} />;
  return (
    <div className="session-conversation">
      <section className="session-toolbar" aria-label="会话管理" aria-busy={busy}>
        <div className="session-controls">
          <label htmlFor="current-conversation">当前会话</label>
          <select
            id="current-conversation"
            value={selected ?? ""}
            disabled={busy || !page || editing !== null}
            onChange={(event) => {
              const id = event.target.value;
              if (!canMutate) {
                setViewed(id);
                return;
              }
              if (page && id)
                void mutate(async () => {
                  const result = await requestSessionActivate(projectSessionId, id, page.revision);
                  setViewed(id);
                  return result;
                }, "已切换会话。");
            }}
          >
            {!selected ? (
              <option value="">{page ? "没有非归档会话" : "正在读取会话…"}</option>
            ) : null}
            {archived && selectedEntry ? (
              <option value={selectedEntry.sessionId}>{selectedEntry.name}（归档只读）</option>
            ) : null}
            {selectionUncertain ? (
              <option value={selected!}>原会话（状态待核对，只读）</option>
            ) : null}
            {page?.items.map((item) => (
              <option value={item.sessionId} key={item.sessionId}>
                {item.name}
              </option>
            ))}
          </select>
          <button
            type="button"
            disabled={busy || !canMutate || !page || page.nonArchivedCount >= 32}
            onClick={() => {
              setEditing("create");
              setName("");
              setError(null);
            }}
          >
            新建会话
          </button>
          <button
            type="button"
            disabled={busy || !canMutate || !selectedEntry || archived}
            onClick={() => {
              setEditing("rename");
              setName(selectedEntry?.name ?? "");
              setError(null);
            }}
          >
            重命名
          </button>
        </div>
        {editing ? (
          <form
            className="session-controls"
            onSubmit={(event) => {
              event.preventDefault();
              saveName();
            }}
          >
            <label htmlFor="conversation-name">会话名称</label>
            <input
              id="conversation-name"
              value={name}
              onChange={(event) => setName(event.target.value)}
              disabled={busy}
              aria-describedby="session-name-help session-feedback"
            />
            <button type="submit" disabled={busy}>
              {busy
                ? "正在保存…"
                : editing === "clear"
                  ? "归档旧聊天并清空"
                  : editing === "branch"
                    ? "创建分支"
                    : "保存名称"}
            </button>
            <button type="button" disabled={busy} onClick={() => setEditing(null)}>
              取消
            </button>
            <small id="session-name-help">1–60 个字符；各会话独立聊天，共享当前工程资料。</small>
            {editing === "clear" ? (
              <p>清空会归档旧会话并打开一个空会话；旧聊天可以恢复，不删除或回滚小说资料。</p>
            ) : null}
            {editing === "branch" ? (
              <p>只复制截至所选轮次的可见文本，不复制旧工具权限，也不会回滚共享资料。</p>
            ) : null}
          </form>
        ) : null}
        {!canMutate && page ? (
          <p role="status">
            当前实例只读，可查看及导出聊天；关闭其他写入实例后重新打开工程，才能继续发送或管理会话。
          </p>
        ) : null}
        {archived ? (
          <p role="status">
            归档会话只读；恢复后才能发送及确认旧待保存操作。共享资料仍是当前版本。
          </p>
        ) : null}
        {selectionUncertain ? (
          <p role="status">
            原会话状态待核对，暂时只读。可重试原操作，或切换到列表中的会话继续聊天。
          </p>
        ) : null}
        <details>
          <summary>更多会话操作</summary>
          <div className="session-controls">
            <button type="button" disabled={busy || !page} onClick={() => void loadArchives()}>
              查看归档
            </button>
            <button
              type="button"
              disabled={busy || !canMutate || !selected || archived || runActive}
              onClick={() => {
                setEditing("clear");
                setViewed(selected);
                setName("新的会话");
                setBoundary(null);
                setError(null);
              }}
            >
              清空当前聊天…
            </button>
            <button
              type="button"
              disabled={busy || !canMutate || !selected || archived || runActive}
              onClick={() => {
                if (page && selected)
                  void manage({
                    action: "archive",
                    conversationSessionId: selected,
                    expectedRevision: page.revision,
                  });
              }}
            >
              归档当前会话
            </button>
            {archived ? (
              <button
                type="button"
                disabled={busy || !canMutate || runActive || !page || page.nonArchivedCount >= 32}
                onClick={() => {
                  if (page && selected)
                    void manage({
                      action: "restore",
                      conversationSessionId: selected,
                      expectedRevision: page.revision,
                    });
                }}
              >
                恢复此会话
              </button>
            ) : null}
          </div>
          {showArchives ? (
            <div>
              <p>
                归档只释放会话名额，仍占用本地存储；共 {page?.nonArchivedCount ?? 0}/32
                个非归档会话。
              </p>
              {archives.length === 0 ? (
                <p>暂无归档会话。</p>
              ) : (
                <ul>
                  {archives.map((entry) => (
                    <li key={entry.sessionId}>
                      <button
                        type="button"
                        disabled={busy || editing !== null}
                        onClick={() => setViewed(entry.sessionId)}
                      >
                        {entry.name}（只读查看）
                      </button>
                    </li>
                  ))}
                </ul>
              )}
              {archiveCursor ? (
                <button type="button" disabled={busy} onClick={() => void loadArchives(true)}>
                  更多归档
                </button>
              ) : null}
            </div>
          ) : null}
          <div className="session-controls">
            <label htmlFor="conversation-export-format">导出格式</label>
            <select
              id="conversation-export-format"
              value={format}
              disabled={busy}
              onChange={(event) => setFormat(event.target.value as ExportFormat)}
            >
              <option value="markdown">Markdown</option>
              <option value="json">JSON</option>
            </select>
            <button
              type="button"
              disabled={busy || !selected || runActive}
              onClick={() => void exportVisible()}
            >
              导出可见聊天…
            </button>
            <p>
              仅导出可见文本。请先检查自己在聊天中输入的隐私内容；不包含隐藏提示或工具原始数据，不覆盖已有文件。
            </p>
          </div>
        </details>
        {otherRun ? (
          <div className="session-controls">
            <span role="status">另一会话正在回复，可切换查看；结束后才能发送新消息。</span>
            <button
              type="button"
              disabled={busy || !canMutate || !otherRun.turnId}
              onClick={() => {
                const turnId = otherRun.turnId;
                if (!turnId) return;
                void mutate(async () => {
                  await requestConversationTurnCancel(projectSessionId, turnId);
                  return requestSessions(projectSessionId);
                }, "已请求停止原会话的回复，已生成内容会保留。");
              }}
            >
              停止原会话回复
            </button>
          </div>
        ) : null}
        <div id="session-feedback">
          {error ? (
            <p role="alert">
              {error}{" "}
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  setError(null);
                  void refresh();
                }}
              >
                重新读取会话
              </button>
            </p>
          ) : notice ? (
            <p role="status">{notice}</p>
          ) : null}
        </div>
      </section>
      {selected ? (
        <ConversationView
          {...props}
          key={selected}
          conversationSessionId={selected}
          otherTurnActive={Boolean(otherRun)}
          readOnly={!canMutate || archived || selectionUncertain}
          onBranch={
            canMutate && !selectionUncertain && !busy && !runActive
              ? (turnId) => {
                  setBoundary(turnId);
                  setViewed(selected);
                  setEditing("branch");
                  setName("聊天分支");
                  setError(null);
                }
              : undefined
          }
          onRunActiveChange={recordRun}
          initialDraft={drafts[selected] ?? ""}
          onDraftChange={recordDraft}
        />
      ) : (
        <p role="status">
          {page
            ? "没有非归档会话，可新建会话或查看归档。"
            : "正在读取当前会话。读取失败时可重试，不会清除聊天。"}
        </p>
      )}
    </div>
  );
}
