import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();
const read = (relative) => readFileSync(path.join(root, relative), "utf8");
const app = read("apps/desktop/src/App.tsx");
const conversation = read("apps/desktop/src/components/ConversationView.tsx");
const workspace = read("apps/desktop/src/components/ProjectWorkspace.tsx");
const rustWindow = read("apps/desktop/src-tauri/src/lib.rs");

for (const required of [
  "<ConversationView",
  "从任何话题开始",
  "可以闲聊、构思、讨论人物，也可以直接开始写正文。",
  "requestConversationTurnStart",
  "requestConversationTurnCancel",
  "正在停止",
]) {
  if (!app.includes(required) && !conversation.includes(required)) {
    throw new Error("NR-005 conversation UI missing: " + required);
  }
}

for (const forbidden of ["ConversationPlaceholder", "History", "锚定", "Control JSON"]) {
  if (app.includes(forbidden) || conversation.includes(forbidden)) {
    throw new Error("NR-005 middle column exposes retired UI concept: " + forbidden);
  }
}

const intervalMatch = conversation.match(/SESSION_RECONCILIATION_INTERVAL_MS\s*=\s*([\d_]+)/u);
if (!intervalMatch) throw new Error("Conversation Store reconciliation interval is missing");
const sessionReconciliationMaximumIntervalMs = Number(intervalMatch[1].replaceAll("_", ""));
if (
  !Number.isSafeInteger(sessionReconciliationMaximumIntervalMs) ||
  sessionReconciliationMaximumIntervalMs <= 0 ||
  sessionReconciliationMaximumIntervalMs > 2000
) {
  throw new Error("Conversation Store reconciliation interval exceeds the R01 boundary");
}

const closeStart = workspace.indexOf("const commitWindowClose");
const closeEnd = workspace.indexOf("useEffect(() =>", closeStart);
const closeCommit = workspace.slice(closeStart, closeEnd);
if (!closeCommit.includes("windowClosePort.destroy()")) {
  throw new Error("NR-005 close commit must destroy the window immediately");
}
if (
  closeCommit.includes("requestProjectClose") ||
  closeCommit.includes("requestConversationTurnCancel")
) {
  throw new Error("raw window close must not wait for project cleanup or pre-cancel AI");
}
for (const required of [
  "event.preventDefault()",
  "dirtyRef.current",
  '{pendingAction.kind === "closeWindow" ? "取消"',
  '{busy === "save" ? "保存中" : "保存"}',
  '{pendingAction.kind === "closeWindow" ? "放弃"',
]) {
  if (!workspace.includes(required)) {
    throw new Error("NR-005 dirty Markdown close contract missing: " + required);
  }
}
if (!rustWindow.includes("WindowEvent::Destroyed")) {
  throw new Error("AI cancellation must be attached to confirmed window destruction");
}
if (rustWindow.includes("WindowEvent::CloseRequested")) {
  throw new Error("raw CloseRequested must not cancel the active AI turn");
}

const pnpmCli = process.env.npm_execpath;
if (!pnpmCli) throw new Error("NR-005 must be run through pnpm");
const testFiles = [
  "src/components/ConversationView.test.tsx",
  "src/components/ConversationView.reconciliation.test.tsx",
  "src/components/ProjectWorkspace.test.tsx",
];
const tests = spawnSync(
  process.execPath,
  [pnpmCli, "--filter", "@novelgainker/desktop", "exec", "vitest", "run", ...testFiles],
  { cwd: root, encoding: "utf8", stdio: "inherit" },
);
if (tests.status !== 0) throw new Error("NR-005/R01 UI and reconciliation tests failed");

const evidenceRoot = path.join(root, "target/qa/nr-005");
mkdirSync(evidenceRoot, { recursive: true });
writeFileSync(
  path.join(evidenceRoot, "nr-005-evidence.json"),
  JSON.stringify(
    {
      schemaVersion: 1,
      storyId: "NR-005+NR-G1-R01",
      result: "pass",
      middleColumn: "natural-conversation-only",
      historyLocation: "conversation-store",
      sessionReconciliationMaximumIntervalMs,
      reconciliationTerminalStates: ["completed", "failed", "cancelled", "not_generated"],
      reconciliationTestFiles: testFiles,
      dirtyMarkdownCloseChoices: ["保存", "放弃", "取消"],
      otherCloseBlockers: 0,
      rawCloseCancelsAi: false,
      confirmedCloseCancelsAi: true,
      closeWaitsForProjectCleanup: false,
      automatedExternalRequests: 0,
      liveProviderSmokeExecuted: false,
      uiCloseSha256: createHash("sha256")
        .update(app + conversation + workspace + rustWindow)
        .digest("hex"),
    },
    null,
    2,
  ) + "\n",
  "utf8",
);
console.log(
  `[nr-005] PASS natural conversation / Store reconciliation <= ${sessionReconciliationMaximumIntervalMs}ms / dirty-only close guard`,
);
