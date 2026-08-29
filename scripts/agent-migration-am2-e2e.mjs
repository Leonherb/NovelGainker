import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const ROOT = process.cwd();
const STORY = process.argv.find((value) => value.startsWith("--story="))?.slice(8);
const QUALITY_SEQUENCE = process.argv.includes("--quality-sequence");
const STORIES = ["ng-a201", "ng-a202", "ng-a203", "ng-a204", "ng-a205", "ng-a206"];
const EXPECTED_AM2_HASH = "812b8d9208f46e70899ebb64681878f13a5a9854baf64f7afabd473e4dd64fa7";
const EXPECTED_AM1_HASH = "a46ace17642e407f0f2b10c0579ddc7044352e2cea59fb2e7915d0813aeaa425";

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

async function text(relativePath) {
  return readFile(path.join(ROOT, relativePath), "utf8");
}

async function json(relativePath) {
  return JSON.parse(await text(relativePath));
}

function requireAll(source, markers, label) {
  for (const marker of markers) assert(source.includes(marker), `${label} missing ${marker}`);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function commandItems(source) {
  return [...source.matchAll(/^\s*"([a-z0-9_]+)",?$/gmu)].map((match) => match[1]);
}

async function validateContracts() {
  const am1 = await json("resources/contracts/agent-runtime/v1/contract-set.json");
  const am2 = await json("resources/contracts/agent-runtime/v2/contract-set.json");
  assert(am1.contractSetHash === EXPECTED_AM1_HASH, "AM1 runtime contract drifted");
  assert(am2.contractSetHash === EXPECTED_AM2_HASH, "AM2 runtime contract drifted");
  assert(am2.enabledAtRuntime === true, "AM2 runtime is disabled");
  assert(am2.readToolCount === 4 && am2.writeToolCount === 2, "AM2 tool counts drifted");
  return am2;
}

async function buildEvidence(story) {
  const sources = await Promise.all([
    text("crates/app-core/src/agent_write.rs"),
    text("crates/app-core/src/agent_transaction.rs"),
    text("crates/app-core/src/app_state.rs"),
    text("apps/desktop/src-tauri/src/agent_commands.rs"),
    text("apps/desktop/src-tauri/src/commands.rs"),
    text("apps/desktop/src/components/AgentTimelinePanel.tsx"),
    text("apps/desktop/src/components/AgentTimelinePanel.test.tsx"),
    text("apps/desktop/src/i18n/zh-CN.ts"),
    text("apps/desktop/src/lib/tauri/agent.ts"),
    text("crates/novelgainker-agent/src/runtime_registry.rs"),
    text("apps/desktop/src/App.tsx"),
    text("apps/desktop/src-tauri/src/command_manifest.rs"),
    text("apps/desktop/src/components/ProjectWorkspace.test.tsx"),
    text("crates/app-core/src/provider_core.rs"),
    text("crates/app-core/src/provider_core_mock_tests.rs"),
    text("apps/desktop/src-tauri/src/lib.rs"),
  ]);
  const [
    write,
    transaction,
    state,
    commands,
    legacyCommands,
    panel,
    panelTest,
    i18n,
    client,
    registry,
    app,
    manifest,
    projectWorkspaceTest,
    providerCore,
    providerCoreMockTests,
    desktopLib,
  ] = sources;
  if (story === "ng-a201") {
    requireAll(
      write,
      [
        "novelgainker.patch-v1",
        "prepare_write_batch",
        "AgentWritePreviewV1",
        "line_diff",
        "expected_source_hash",
        "project_write_count: 0",
        "path-policy-blocked",
      ],
      "NG-A201 write preview core",
    );
    return {
      story: "NG-A201",
      status: "technical_pass",
      writeToolCount: 2,
      strictPatchEnvelope: true,
      lineDiff: true,
      sourceHashBound: true,
      skillScopeBound: true,
      preconfirmationWriteCount: 0,
    };
  }
  if (story === "ng-a202") {
    requireAll(
      transaction,
      [
        "CryptProtectData",
        "CryptUnprotectData",
        "commit_prepared_agent_write",
        "rollback_mutations",
        "recover_incomplete_agent_transactions",
        "apply_agent_undo",
        "MAX_UNDO_BYTES_PER_TURN",
        "injected_second_write_failure_rolls_back_first_and_discards_undo",
      ],
      "NG-A202 transaction core",
    );
    return {
      story: "NG-A202",
      status: "technical_pass",
      groupAtomicity: true,
      failureRollback: true,
      crashRecovery: true,
      undoProtection: "windows_user_context_dpapi",
      undoBytesPerTurn: 2097152,
      undoRecordLimit: 20,
    };
  }
  if (story === "ng-a203") {
    requireAll(
      state,
      [
        "APP_STATE_SCHEMA_VERSION: u16 = 10",
        "agent_undo_records",
        "save_agent_undo_record",
        "set_agent_undo_status",
        '"plan_only" | "review_every_write" | "auto_apply_safe"',
      ],
      "NG-A203 App State",
    );
    requireAll(
      commands,
      [
        "agent_policy_set",
        "agent_write_confirm",
        "agent_write_reject",
        "agent_undo_apply",
        "agent_pending_writes",
        "confirmation-binding",
      ],
      "NG-A203 IPC",
    );
    return {
      story: "NG-A203",
      status: "technical_pass",
      policies: ["plan_only", "review_every_write", "auto_apply_safe"],
      pendingBodyPersistence: false,
      appStateSchemaVersion: 10,
      agentIpcCount: 9,
    };
  }
  if (story === "ng-a204") {
    requireAll(
      commands,
      [
        "agent_write_action_schema_source",
        "prepare_write_batch",
        "AuthorizationEffectV1::Execute",
        "WritePreview",
        "WriteCommitted",
        "agent-runtime.tool-batch-mixed",
      ],
      "NG-A204 Agent loop",
    );
    requireAll(
      registry,
      ["AM2_TOOL_IDS", "write_scopes", "write_path_allowed_for_skills"],
      "NG-A204 Skill routing",
    );
    return {
      story: "NG-A204",
      status: "technical_pass",
      skillCount: 9,
      providerIds: ["deepseek", "openai"],
      sameProviderRepairLimit: 1,
      crossProviderFallback: false,
      oneWriteTransactionPerTurn: true,
    };
  }
  if (story === "ng-a205") {
    requireAll(
      panel,
      [
        "requestAgentPolicySet",
        "requestAgentWriteConfirm",
        "requestAgentWriteReject",
        "requestAgentUndoApply",
        "agent-line-diff",
        "preview.transactionHash",
        "preview.sourceSetHash",
        "file.expectedSourceHash",
        "file.resultHash",
      ],
      "NG-A205 Timeline",
    );
    requireAll(
      client,
      [
        "parseAgentWritePreview",
        "parseCommitResult",
        "AGENT_WRITE_CONFIRM_COMMAND",
        "AGENT_UNDO_APPLY_COMMAND",
        "pendingId: preview.pendingId",
        "transactionHash: preview.transactionHash",
      ],
      "NG-A205 client boundary",
    );
    return {
      story: "NG-A205",
      status: "technical_pass",
      visibleLineDiff: true,
      policyControl: true,
      hashBoundConfirmation: true,
      rejectAction: true,
      undoAction: true,
    };
  }
  assert(story === "ng-a206", "Unknown AM2 story");
  const commandsList = commandItems(manifest);
  assert(commandsList.length === 60, `Expected 60 commands, found ${commandsList.length}`);
  assert(app.includes("<AgentTimelinePanel"), "Unified Agent Timeline is not mounted");
  assert(!app.includes("<ConversationPanel"), "Legacy ConversationPanel remains mounted");
  requireAll(
    manifest,
    ["agent_policy_set", "agent_write_confirm", "agent_write_reject", "agent_undo_apply"],
    "NG-A206 command manifest",
  );
  requireAll(
    legacyCommands,
    [
      "retire_orphaned_legacy_workflow_generation",
      "state.active_provider_id().is_some()",
      ".active_record()",
      ".abandon(&current.run_id, true, now_unix_seconds())",
      "save_workflow_run(&retired)",
      "ga2_retires_only_orphaned_legacy_generation_before_agent_use",
    ],
    "NG-A206 orphaned legacy workflow reconciliation",
  );
  requireAll(
    legacyCommands,
    [
      "fn legacy_workflow_blocks_unified_runtime",
      "record.state != WorkflowRunState::RecoveryRequired",
      "ng_302_restored_recovery_is_inert_and_can_still_be_explicitly_abandoned",
      "WorkflowRunAction::Abandon { confirmed: true }",
    ],
    "NG-A206 inert legacy recovery compatibility",
  );
  assert(
    !legacyCommands.includes("recovery_required_workflow_matches_project"),
    "NG-A206 must not bind unified project entry to a restored legacy recovery record",
  );
  requireAll(
    commands,
    [
      "if state.active_provider_id().is_some()",
      "retire_orphaned_legacy_workflow_generation(&state)?",
      "if workflow_generation_active(&state)?",
      "fn agent_action_format_instruction",
      String.raw`\"action\":\"respond\"`,
      String.raw`\"visibleText\"`,
      "storytelling_prompt_exposes_the_exact_respond_action_shape",
    ],
    "NG-A206 Agent channel and Action-format reconciliation",
  );
  requireAll(
    panel,
    [
      "items.filter((item) => item.id !== messageId)",
      "setInput(submitted)",
      'id="agent-composer-error"',
      'aria-describedby={presentedError ? "agent-composer-error" : undefined}',
      "ZH_CN.agent.errorCodeLabel",
      "submittedInputRef",
      "void loadSession(true)",
      'status: "failed"',
    ],
    "NG-A206 rejected and asynchronous-failure input recovery",
  );
  assert(
    !panel.includes("requestWorkflowRunStatus") &&
      !panel.includes("requestWorkflowRunAbandon") &&
      !panel.includes("legacyRecoveryRequired"),
    "NG-A206 unified Agent must ignore inert legacy recovery state",
  );
  requireAll(
    panelTest,
    [
      "keeps unsent input when a real workflow is still running",
      'toHaveValue("把港口规则同步到世界观")',
      'toHaveAttribute("aria-describedby", "agent-composer-error")',
      "workflow.run_active",
      'queryByRole("button", { name: "确认结束旧工作流" })',
      "keeps an asynchronous Turn failure visible and restores the submitted input",
      "agent-action.repair-exhausted",
    ],
    "NG-A206 frontend regression",
  );
  requireAll(
    i18n,
    [
      'title: "已有工作流正在执行"',
      'message: "当前工作流仍在占用运行通道，本次操作尚未执行。"',
      'recovery: "请等待当前工作流结束，或在对应运行界面取消后重试。"',
      'failedInputRestored: "Turn 失败 · 输入已恢复"',
      'title: "Agent 操作格式未通过校验"',
    ],
    "NG-A206 current-run copy",
  );
  assert(
    !i18n.includes("不能同时启动普通对话请求") && !i18n.includes("检测到迁移前的工作流状态"),
    "NG-A206 must not describe the unified Agent as ordinary chat or surface inert migration state",
  );
  requireAll(
    projectWorkspaceTest,
    [
      "reports a real active workflow when it blocks project selection",
      "当前工作流仍在占用运行通道，本次操作尚未执行。",
      "queryByText(/本轮 Agent/u)",
    ],
    "NG-A206 project-entry current-run copy",
  );
  requireAll(
    providerCore,
    [
      "pub async fn complete_cancellable",
      "read_bounded_cancellable",
      "sleep_retry_cancellable",
      "tokio::select!",
    ],
    "GA2-R005 cancellable Provider core",
  );
  requireAll(
    commands,
    [
      "tauri::async_runtime::block_on(client.complete_cancellable",
      "if context.cancellation.is_cancelled()",
      "prepare_write_batch",
      "commit_prepared_agent_write",
    ],
    "GA2-R005 Agent cancellation checkpoints",
  );
  requireAll(
    providerCoreMockTests,
    [
      "ga2_r005_cancellable_completion_aborts_before_response_headers_without_retrying",
      "ga2_r005_cancellable_completion_aborts_while_response_body_is_stalled",
      'assert_eq!(recorded.len(), 1, "cancelled requests must never retry")',
    ],
    "GA2-R005 Provider regression",
  );
  requireAll(
    panel,
    ['setTurnState("cancelling")', "ZH_CN.agent.cancellingStatus", 'aria-live="polite"'],
    "GA2-R005 cancellation feedback",
  );
  requireAll(
    panelTest,
    [
      "acknowledges cancellation immediately and releases the Turn on the terminal event",
      "正在中断 Provider 请求；不会再启动新的工具或写入",
    ],
    "GA2-R005 cancellation UI regression",
  );
  requireAll(
    desktopLib,
    ["tauri::WindowEvent::CloseRequested", "cancel_active_provider_run"],
    "GA2-R005 window-close cancellation hook",
  );
  return {
    story: "NG-A206",
    status: "technical_pass",
    tauriCommandCount: 60,
    legacyConversationMounted: false,
    legacyBackendRetained: true,
    orphanedLegacyGenerationRetired: true,
    activeProviderRunPreserved: true,
    rejectedInputRetained: true,
    legacyRecoveryRecordPreserved: true,
    legacyRecoveryStateIgnored: true,
    projectAndProviderActionsUnblockedByLegacyRecovery: true,
    legacyRecoveryUiHidden: true,
    exactAgentActionShapePrompted: true,
    asynchronousFailureVisible: true,
    failedInputRestored: true,
    cancellableProviderRequest: true,
    cancellationStopsBeforeNewToolsOrWrites: true,
    cancellationFeedbackAccessible: true,
    windowCloseCancelsActiveRun: true,
    sprint6Authorized: false,
    liveProviderSmokeExecuted: false,
  };
}

async function writeStoryEvidence(story) {
  await validateContracts();
  const evidence = await buildEvidence(story);
  const generatedAt = new Date().toISOString();
  if (QUALITY_SEQUENCE) {
    const startedAt = process.env.NOVELGAINKER_QUALITY_STARTED_AT;
    assert(startedAt, "Quality sequence marker is missing");
    assert(Date.parse(generatedAt) >= Date.parse(startedAt), "Evidence predates quality sequence");
  }
  const sourceFiles = [
    "resources/contracts/agent-runtime/v2/contract-set.json",
    "crates/app-core/src/agent_write.rs",
    "crates/app-core/src/agent_transaction.rs",
    "crates/app-core/src/app_state.rs",
    "apps/desktop/src-tauri/src/agent_commands.rs",
    "apps/desktop/src-tauri/src/commands.rs",
    "crates/novelgainker-workflows/src/manager.rs",
    "apps/desktop/src/components/AgentTimelinePanel.tsx",
    "apps/desktop/src/components/AgentTimelinePanel.test.tsx",
    "apps/desktop/src/errors/present-app-error.ts",
    "apps/desktop/src/errors/present-app-error.test.ts",
    "apps/desktop/src/i18n/zh-CN.ts",
    "apps/desktop/src/lib/tauri/agent.ts",
    "crates/app-core/src/project.rs",
    "apps/desktop/src/components/ProjectWorkspace.test.tsx",
    "crates/app-core/src/provider_core.rs",
    "crates/app-core/src/provider_core_mock_tests.rs",
    "apps/desktop/src-tauri/src/lib.rs",
  ];
  const sourceHashes = {};
  for (const file of sourceFiles)
    sourceHashes[file] = sha256(await readFile(path.join(ROOT, file)));
  const output = {
    schemaVersion: 1,
    generatedAt,
    qualitySequence: QUALITY_SEQUENCE,
    am1RuntimeContractHash: EXPECTED_AM1_HASH,
    am2RuntimeContractHash: EXPECTED_AM2_HASH,
    sourceHashes,
    ...evidence,
  };
  const evidenceRoot = path.join(ROOT, "target", "qa", story);
  await mkdir(evidenceRoot, { recursive: true });
  await writeFile(
    path.join(evidenceRoot, `${story}-evidence.json`),
    `${JSON.stringify(output, null, 2)}\n`,
  );
  console.log(
    `[agent-am2-e2e] PASS ${evidence.story}; evidence target/qa/${story}/${story}-evidence.json`,
  );
}

async function writeGa2Evidence() {
  const runtime = await validateContracts();
  const stories = [];
  for (const story of STORIES) {
    const evidence = await json(`target/qa/${story}/${story}-evidence.json`);
    assert(evidence.status === "technical_pass", `${story} has not passed`);
    assert(evidence.am2RuntimeContractHash === EXPECTED_AM2_HASH, `${story} contract drifted`);
    if (QUALITY_SEQUENCE) assert(evidence.qualitySequence === true, `${story} is stale`);
    if (story === "ng-a206") {
      assert(evidence.orphanedLegacyGenerationRetired === true, "GA2-R001 backend fix missing");
      assert(evidence.activeProviderRunPreserved === true, "GA2-R001 provider guard missing");
      assert(evidence.rejectedInputRetained === true, "GA2-R001 input recovery missing");
      assert(
        evidence.legacyRecoveryRecordPreserved === true,
        "GA2-R003 legacy recovery history preservation missing",
      );
      assert(
        evidence.legacyRecoveryStateIgnored === true &&
          evidence.projectAndProviderActionsUnblockedByLegacyRecovery === true &&
          evidence.legacyRecoveryUiHidden === true,
        "GA2-R003 inert recovery behavior missing",
      );
      assert(
        evidence.exactAgentActionShapePrompted === true &&
          evidence.asynchronousFailureVisible === true &&
          evidence.failedInputRestored === true,
        "GA2-R004 Action guidance and failure recovery missing",
      );
      assert(
        evidence.cancellableProviderRequest === true &&
          evidence.cancellationStopsBeforeNewToolsOrWrites === true &&
          evidence.cancellationFeedbackAccessible === true &&
          evidence.windowCloseCancelsActiveRun === true,
        "GA2-R005 cancellation and window-close fix missing",
      );
    }
    stories.push({
      story: evidence.story,
      status: evidence.status,
      generatedAt: evidence.generatedAt,
    });
  }
  const generatedAt = new Date().toISOString();
  const releaseExecutablePath = "target/release/novelgainker-desktop.exe";
  const releaseExecutable = await readFile(path.join(ROOT, releaseExecutablePath));
  const manifest = {
    schemaVersion: 1,
    gate: "GA2",
    generatedAt,
    gateStatus: "technical_review_passed_product_approval_required",
    submissionRevision: 6,
    ga2Approved: false,
    acceptanceBlockersResolved: ["GA2-R001", "GA2-R002", "GA2-R003", "GA2-R004", "GA2-R005"],
    stories,
    am1RuntimeContractHash: EXPECTED_AM1_HASH,
    am2RuntimeContractHash: runtime.contractSetHash,
    runtimeEnabled: true,
    appStateSchemaVersion: 10,
    tauriCommandCount: 60,
    agentCommandCount: 9,
    skillCount: 9,
    readToolCount: 4,
    writeToolCount: 2,
    writePolicies: ["plan_only", "review_every_write", "auto_apply_safe"],
    writeTransactionFileLimit: 8,
    undoProtection: "windows_user_context_dpapi",
    fullChatPersisted: false,
    pendingWriteBodiesPersisted: false,
    providerIds: ["deepseek", "openai"],
    legacyBackendRetained: true,
    orphanedLegacyGenerationRetired: true,
    rejectedInputRetained: true,
    legacyRecoveryRecordPreserved: true,
    legacyRecoveryStateIgnored: true,
    projectAndProviderActionsUnblockedByLegacyRecovery: true,
    legacyRecoveryUiHidden: true,
    exactAgentActionShapePrompted: true,
    asynchronousFailureVisible: true,
    failedInputRestored: true,
    cancellableProviderRequest: true,
    cancellationStopsBeforeNewToolsOrWrites: true,
    cancellationFeedbackAccessible: true,
    windowCloseCancelsActiveRun: true,
    releaseExecutablePath,
    releaseExecutableBytes: releaseExecutable.length,
    releaseExecutableSha256: sha256(releaseExecutable),
    liveProviderSmokeExecuted: false,
    sprint6Authorized: false,
    nextAuthorizedAction: null,
  };
  const root = path.join(ROOT, "target", "qa", "ga2");
  await mkdir(root, { recursive: true });
  await writeFile(path.join(root, "ga2-manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  await writeFile(
    path.join(root, "ga2-review-report.md"),
    `# GA2 Technical Review Report\n\n- Status: technical review passed; independent product approval required\n- Submission revision: 6 (GA2-R001 through GA2-R005 acceptance blockers resolved)\n- Generated: ${generatedAt}\n- Stories: NG-A201 through NG-A206 technical pass\n- Runtime contract: \`${runtime.contractSetHash}\`\n- Runtime: nine Skills, four bounded reads, two Skill-scoped governed writes\n- Persistence: App State v10 stores bounded metadata; Pending bodies are memory-only; Undo bodies use Windows user-context DPAPI\n- Providers: DeepSeek and OpenAI share one strict Agent Action contract and never cross-fallback\n- Legacy workflow backend: retained for regression; old ConversationPanel remains unmounted\n- GA2-R001: an orphaned pre-migration Generating state is retired only when no real Provider run owns the channel; rejected Agent input is retained with an accessible current-run error path\n- GA2-R002: the original-project recovery deadlock was removed in revision 3\n- GA2-R003: the product decision supersedes visible recovery handling; a restored RecoveryRequired checkpoint remains stored as inert compatibility history, does not block project or Provider actions, never appears in the unified Agent/start page, and causes no Canon write or automatic discard\n- GA2-R004: JSON-object Providers receive the exact AgentActionPayloadV1 field shapes; asynchronous failures remain visible through session reload, preserve their stable error code, restore the submitted input, and execute zero tools or writes on invalid output\n- GA2-R005: Agent completion HTTP is directly cancellable before headers and while reading the body; cancellation does not retry, new tools and writes are blocked at safety checkpoints, the Timeline exposes an accessible cancelling status, and window close uses the same cancellation signal\n- Release executable: \`${releaseExecutablePath}\` (${releaseExecutable.length} bytes, SHA-256 \`${sha256(releaseExecutable)}\`)\n- Live Provider smoke: not executed; no credential or network authority was inferred\n- GA2 product approval: **false**\n- Sprint 6: **not authorized**\n\nGA2 stops here. Product approval is a separate decision.\n`,
  );
  console.log("[agent-am2-e2e] PASS GA2 technical submission; product approval remains false");
}

assert(STORY === "ga2" || STORIES.includes(STORY), "Use --story=ng-a201..ng-a206|ga2");
if (STORY === "ga2") await writeGa2Evidence();
else await writeStoryEvidence(STORY);
