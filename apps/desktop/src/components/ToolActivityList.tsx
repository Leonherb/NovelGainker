import type { TaskActivity, TaskStatus } from "../lib/tauri/conversation";

const STATUS: Record<TaskStatus, string> = {
  queued: "等待执行",
  running: "正在执行",
  succeeded: "执行完成",
  failed: "执行失败",
  cancelled: "已中止",
};

const ERRORS: Record<string, string> = {
  "agent.tool_arguments_invalid": "工具参数格式不正确，未通过校验。",
  "agent.tool_arguments_too_large": "工具参数超过大小限制。",
  "canon.change_invalid": "变更参数不符合要求。",
  "canon.change_method_not_read": "缺少本轮有效的方法读取回执。",
  "canon.change_source_invalid": "变更来源与当前对话不匹配。",
  "canon.change_incomplete_read": "目标文件尚未完整读取。",
  "canon.change_source_changed": "原文件已变化，需要重新读取。",
  "canon.change_target_invalid": "变更目标不符合要求。",
  "canon.change_candidate_invalid": "候选文档格式或内容校验未通过。",
  "canon.change_budget_exceeded": "本轮工具次数、内容大小或安全预留已达上限，不代表 API 余额不足。",
  "canon.change_recovery_required": "写入结果需要核对，请查看文件保存回执。",
  "canon.change_writer_busy": "工程当前正在进行其他写入。",
  "agent.tool_timeout": "工具执行超时。",
  "agent.tool_cancelled": "工具操作已取消。",
  "runtime.interrupted": "应用中断后恢复了这条操作记录。",
};

function recordedTime(timestamp: number) {
  const date = new Date(timestamp);
  return Number.isNaN(date.getTime())
    ? "时间不可用"
    : date.toLocaleString("zh-CN", { hour12: false });
}

export function ToolActivityList({ tasks }: { tasks: TaskActivity[] }) {
  if (tasks.length === 0) return null;
  // Revision orders updates of one task, not different tool calls.
  const ordered = [...tasks].sort((a, b) => a.startedAtMs - b.startedAtMs);
  return (
    <section className="tool-activity-list" aria-label="Gainker 工具操作记录">
      <header>
        <strong>Gainker 工具记录</strong>
        <span>{tasks.length} 次操作 · 本地留痕</span>
      </header>
      <ol>
        {ordered.map((task) => (
          <li key={task.taskId}>
            <details className="tool-activity" data-status={task.status}>
              <summary>
                <span className="tool-activity-label">{task.displayLabel}</span>
                <span className="tool-activity-status">{STATUS[task.status]}</span>
              </summary>
              <div className="tool-activity-detail">
                <dl>
                  <div>
                    <dt>工具</dt>
                    <dd>
                      <code>{task.toolName}</code> · v{task.toolVersion}
                    </dd>
                  </div>
                  <div>
                    <dt>记录创建</dt>
                    <dd>{recordedTime(task.startedAtMs)}</dd>
                  </div>
                  <div>
                    <dt>最近更新</dt>
                    <dd>{recordedTime(task.updatedAtMs)}</dd>
                  </div>
                  <div>
                    <dt>记录编号</dt>
                    <dd>
                      <code>{task.taskId}</code>
                    </dd>
                  </div>
                </dl>
                {task.errorCode ? (
                  <p className="tool-activity-error">
                    {ERRORS[task.errorCode] ?? "操作未完成，可根据错误码复查。"}
                    <code>{task.errorCode}</code>
                  </p>
                ) : null}
                {task.toolName === "novelist.propose_change" ? (
                  <p>文件是否保存，以本轮的文件保存回执为准。</p>
                ) : null}
                {task.toolName === "novelist.publish" ? (
                  <p>出版结果以本轮的 EPUB 回执为准。</p>
                ) : null}
              </div>
            </details>
          </li>
        ))}
      </ol>
    </section>
  );
}
