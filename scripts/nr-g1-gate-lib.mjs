import { createHash } from "node:crypto";
import {
  cpSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  currentNr5Authorization,
  currentNr6Authorization,
  currentNr602Authorization,
  currentNr603Authorization,
} from "./nr-stage-state.mjs";
export const NR602_SESSION_COMMANDS = [
  "conversation_session_manage",
  "conversation_export_prepare",
  "conversation_export_commit",
];
export const NR603_RETRY_COMMANDS = ["conversation_turn_retry"];

export const NR601_SESSION_COMMANDS = [
  "conversation_session_list",
  "conversation_session_create",
  "conversation_session_rename",
  "conversation_session_activate",
];

const NR5_PUBLICATION_COMMANDS = ["publication_list", "publication_detail"];
export function hasApprovedPublicationIpc(status) {
  return currentNr5Authorization(status);
}

export const CONVERSATION_COMMANDS = [
  "conversation_session_load",
  "conversation_turn_start",
  "conversation_turn_cancel",
];

export const NR4_COMMANDS = [
  "canon_change_list",
  "canon_change_detail",
  "canon_change_decide",
  "canon_editor_state",
];
const BASELINE_COMMANDS = [
  ...CONVERSATION_COMMANDS,
  "health_check",
  "app_state_load",
  "app_state_save_ui",
  "provider_status_load",
  "provider_key_save",
  "provider_key_delete",
  "provider_select",
  "provider_connection_test",
  "project_scan",
  "project_create_preview",
  "project_create",
  "project_open",
  "project_open_recent",
  "project_list_files",
  "project_read_file",
  "app_state_set_current_file",
  "project_watch_file",
  "project_save_file",
  "project_validate_draft",
  "project_search",
  "project_close",
].sort();

export function hasApprovedNr4Ipc(status) {
  const packet = status.match(/^nr4_packet:\r?\n((?:[ \t].*(?:\r?\n|$)|\r?\n)*)/mu)?.[1] ?? "";
  return [
    /^\s+product_approved: true\s*$/mu,
    /^\s+implementation_authorized: true\s*$/mu,
    /^\s+decisions_approved: "NR4-D1-R01～R04"\s*$/mu,
    /^\s+live_smoke_decision: "explicitly_waived_risk_accepted"\s*$/mu,
    /^\s+chapter_commit_source_staging: "approved_deferred_to_nr5"\s*$/mu,
  ].every((pattern) => pattern.test(packet));
}

const RETIRED_PRODUCT_PATTERNS = [
  /AgentConversation/gu,
  /ConversationPlaceholder/gu,
  /RouterDecision/gu,
  /agent[_-](?:turn_start|session_load|context_mark|write_plan|conflict_resolution)/giu,
  /agent-(?:v2|action|governance)/giu,
  /control[_ -]?json/giu,
  /skill[_ -]?selection/giu,
  /workflow[_-](?:catalog|run|prepare|registry)/giu,
  /proposal[_ -]?json/giu,
];

const toPosix = (value) => value.split(path.sep).join("/");

function walkFiles(directory, accept = () => true) {
  if (!existsSync(directory)) return [];
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...walkFiles(absolute, accept));
    else if (entry.isFile() && accept(absolute)) files.push(absolute);
  }
  return files;
}

function isProductionTypeScript(absolute) {
  const relative = toPosix(absolute);
  return (
    /\.(?:ts|tsx)$/u.test(relative) &&
    !/\.(?:test|spec)\.(?:ts|tsx)$/u.test(relative) &&
    !/(?:^|\/)(?:__tests__|fixtures)(?:\/|$)/u.test(relative)
  );
}

function configuredProductionResources(repositoryRoot) {
  const configPath = path.join(repositoryRoot, "apps/desktop/src-tauri/tauri.conf.json");
  if (!existsSync(configPath)) return [];
  const config = JSON.parse(readFileSync(configPath, "utf8"));
  const configured = config.bundle?.resources;
  if (configured === undefined) return [];
  const entries =
    typeof configured === "string"
      ? [configured]
      : Array.isArray(configured)
        ? configured
        : Object.keys(configured);
  const files = [];
  for (const entry of entries) {
    if (/[*?{}[\]]/u.test(entry)) {
      throw new Error(`NR-G1 production resource glob requires explicit gate support: ${entry}`);
    }
    const absolute = path.resolve(path.dirname(configPath), entry);
    if (!existsSync(absolute))
      throw new Error(`NR-G1 configured production resource is missing: ${entry}`);
    files.push(...(statSync(absolute).isDirectory() ? walkFiles(absolute) : [absolute]));
  }
  return files;
}

export function discoverReleaseSurface(repositoryRoot) {
  const files = [
    ...walkFiles(path.join(repositoryRoot, "apps/desktop/src"), isProductionTypeScript),
    ...walkFiles(path.join(repositoryRoot, "apps/desktop/src-tauri/src")),
    ...walkFiles(path.join(repositoryRoot, "contracts")),
    ...configuredProductionResources(repositoryRoot),
  ];
  const cratesRoot = path.join(repositoryRoot, "crates");
  if (existsSync(cratesRoot)) {
    for (const crate of readdirSync(cratesRoot, { withFileTypes: true })) {
      if (crate.isDirectory()) files.push(...walkFiles(path.join(cratesRoot, crate.name, "src")));
    }
  }
  return [...new Set(files.map((file) => path.resolve(file)))].sort();
}

export function scanRetiredProductReferences(repositoryRoot, files) {
  const findings = [];
  for (const absolute of files) {
    const source = readFileSync(absolute, "utf8");
    for (const pattern of RETIRED_PRODUCT_PATTERNS) {
      pattern.lastIndex = 0;
      for (const match of source.matchAll(pattern)) {
        findings.push({
          relativePath: toPosix(path.relative(repositoryRoot, absolute)),
          token: match[0],
        });
      }
    }
  }
  return findings;
}

function uniqueSorted(values, label) {
  const unique = [...new Set(values)].sort();
  if (unique.length !== values.length)
    throw new Error(`NR-G1 ${label} contains duplicate commands`);
  return unique;
}

function assertSameCommands(expected, actual, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`NR-G1 ${label} command set drift: ${JSON.stringify(actual)}`);
  }
}

export function inspectIpcClosure(repositoryRoot) {
  const read = (relative) => readFileSync(path.join(repositoryRoot, relative), "utf8");
  const manifestSource = read("apps/desktop/src-tauri/src/command_manifest.rs");
  const manifest = uniqueSorted(
    [...manifestSource.matchAll(/"([a-z][a-z0-9_]*)"/gu)].map((match) => match[1]),
    "manifest",
  );

  const handlerSource = read("apps/desktop/src-tauri/src/lib.rs");
  const handlerBlock = handlerSource.match(/tauri::generate_handler!\[([\s\S]*?)\]\s*\)/u);
  if (!handlerBlock) throw new Error("NR-G1 Tauri handler registration was not found");
  const handlers = uniqueSorted(
    [...handlerBlock[1].matchAll(/commands::([a-z][a-z0-9_]*)/gu)].map((match) => match[1]),
    "handler",
  );

  const permissionRoot = path.join(
    repositoryRoot,
    "apps/desktop/src-tauri/permissions/autogenerated",
  );
  const permissions = uniqueSorted(
    readdirSync(permissionRoot)
      .filter((name) => name.endsWith(".toml"))
      .map((name) => name.slice(0, -5)),
    "permission",
  );

  const capability = JSON.parse(read("apps/desktop/src-tauri/capabilities/default.json"));
  const capabilityCommands = uniqueSorted(
    capability.permissions
      .filter(
        (permission) =>
          typeof permission === "string" &&
          permission.startsWith("allow-") &&
          !permission.includes(":"),
      )
      .map((permission) => permission.slice(6).replaceAll("-", "_")),
    "capability",
  );

  const clientFiles = walkFiles(
    path.join(repositoryRoot, "apps/desktop/src"),
    isProductionTypeScript,
  );
  const clients = uniqueSorted(
    clientFiles
      .flatMap((file) => [
        ...readFileSync(file, "utf8").matchAll(
          /export const [A-Z][A-Z0-9_]*_COMMAND\s*=\s*"([a-z][a-z0-9_]*)"/gu,
        ),
      ])
      .map((match) => match[1]),
    "TypeScript client",
  );

  const nr4 = manifest.some((command) => NR4_COMMANDS.includes(command));
  const publication = manifest.some((command) => NR5_PUBLICATION_COMMANDS.includes(command));
  const sessions = manifest.some((command) => NR601_SESSION_COMMANDS.includes(command));
  const management = manifest.some((command) => NR602_SESSION_COMMANDS.includes(command));
  const retry = manifest.some((command) => NR603_RETRY_COMMANDS.includes(command));
  if (retry) {
    if (
      !management ||
      !["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"].every((file) =>
        currentNr603Authorization(read(file)),
      )
    ) {
      throw new Error("NR-603 requires frozen NR-602 full quality and current authorization");
    }
    for (const [file, hash] of [
      [
        "target/qa/nr-602/quality.json",
        "ea1421aa835117bc8ea9fb8aa8c1877f24fe24c03fe562d8b31c096b38b75001",
      ],
      [
        "target/qa/nr-602/validation.json",
        "5b510cf6f0406db3a86435ac3562aa609df5a43f9fc498e3ef164c0b0e2b7f06",
      ],
    ])
      if (
        createHash("sha256")
          .update(readFileSync(path.join(repositoryRoot, file)))
          .digest("hex") !== hash
      )
        throw new Error("Frozen NR-602 evidence drift");
  }
  if (management) {
    if (
      !sessions ||
      !["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"].every((relative) =>
        currentNr602Authorization(read(relative)),
      )
    )
      throw new Error("NR-602 IPC requires frozen NR-601 full quality and current authorization");
    for (const [file, expected] of [
      [
        "target/qa/nr-601/quality.json",
        "9f617a5518518de20ca66d87597ddde50f31aa1c1a302668c8cb73cc8be1d773",
      ],
      [
        "target/qa/nr-601/validation.json",
        "fb22fd0dd3455a2ed56252ad626ceb55881ddad525e6bcc1da0376a75e831c3e",
      ],
    ])
      if (
        createHash("sha256")
          .update(readFileSync(path.join(repositoryRoot, file)))
          .digest("hex") !== expected
      )
        throw new Error("Frozen NR-601 evidence drift");
  }
  if (sessions) {
    if (
      !publication ||
      !["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"].every((relative) =>
        currentNr6Authorization(read(relative)),
      )
    )
      throw new Error("Session IPC requires approved continuous entry after NR-G6 technical pass");
    for (const [file, expected] of [
      [
        "docs/reviews/novelist-agent-runtime-nr-g6-technical-submission.zh-CN.md",
        "bdf59cb8982cc4f783b5e569133265dce7a189153d9de49eda7d1f997364e1d8",
      ],
      [
        "target/qa/nr-506/nr-g6-manifest.json",
        "166e2ef11c69230329ee3eba07d478098cfd40adb1db3cb3f15ddf24b83325d1",
      ],
      [
        "target/qa/nr-506/quality.json",
        "098ca628049e23eb684692bb42968f7489f28fd4cfc3a90f89ae1a0044683541",
      ],
    ]) {
      if (
        createHash("sha256")
          .update(readFileSync(path.join(repositoryRoot, file)))
          .digest("hex") !== expected
      )
        throw new Error("Frozen NR-G6 technical evidence drift");
    }
  }
  if (
    publication &&
    !["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"].every((relative) =>
      hasApprovedPublicationIpc(read(relative)),
    )
  ) {
    throw new Error(
      "Publication IPC requires the existing explicit NR-5/NR-6 implementation approval",
    );
  }
  if (
    nr4 &&
    !["bmad/workflow-status.yaml", "bmad/sprint-status.yaml"].every((relative) =>
      hasApprovedNr4Ipc(read(relative)),
    )
  ) {
    throw new Error(
      "NR-4 IPC requires explicit packet implementation approval in both status files",
    );
  }
  assertSameCommands(
    [
      ...BASELINE_COMMANDS,
      ...(nr4 ? NR4_COMMANDS : []),
      ...(publication ? NR5_PUBLICATION_COMMANDS : []),
      ...(sessions ? NR601_SESSION_COMMANDS : []),
      ...(management ? NR602_SESSION_COMMANDS : []),
      ...(retry ? NR603_RETRY_COMMANDS : []),
    ].sort(),
    manifest,
    "approved manifest",
  );
  for (const [label, commands] of [
    ["handler", handlers],
    ["permission", permissions],
    ["capability", capabilityCommands],
    ["TypeScript client", clients],
  ]) {
    assertSameCommands(manifest, commands, label);
  }
  const conversation = manifest.filter((command) => CONVERSATION_COMMANDS.includes(command));
  assertSameCommands([...CONVERSATION_COMMANDS].sort(), conversation, "Conversation");
  return {
    manifest,
    handlers,
    permissions,
    capability: capabilityCommands,
    clients,
    conversation,
    baselineManifest: BASELINE_COMMANDS,
    nr4Commands: nr4 ? [...NR4_COMMANDS].sort() : [],
    publicationCommands: publication ? [...NR5_PUBLICATION_COMMANDS].sort() : [],
    sessionCommands: [
      ...(sessions ? NR601_SESSION_COMMANDS : []),
      ...(management ? NR602_SESSION_COMMANDS : []),
      ...(retry ? NR603_RETRY_COMMANDS : []),
    ].sort(),
    sessionCatalogCommands: sessions ? [...NR601_SESSION_COMMANDS].sort() : [],
    sessionManagementCommands: management ? [...NR602_SESSION_COMMANDS].sort() : [],
    retryCommands: retry ? [...NR603_RETRY_COMMANDS].sort() : [],
    allConversationCommands: manifest.filter((command) => command.startsWith("conversation_")),
  };
}

export function releaseScopeSha256(repositoryRoot, files) {
  const hash = createHash("sha256");
  for (const absolute of files) {
    hash.update(toPosix(path.relative(repositoryRoot, absolute)));
    hash.update("\0");
    hash.update(readFileSync(absolute));
    hash.update("\0");
  }
  return hash.digest("hex");
}

export function proveReleaseScannerRejectsNewProductionFile(repositoryRoot) {
  const fixtureRoot = mkdtempSync(path.join(tmpdir(), "novelgainker-nrg1-release-"));
  const safeParent = path.resolve(path.dirname(fixtureRoot)) === path.resolve(tmpdir());
  const safeName = path.basename(fixtureRoot).startsWith("novelgainker-nrg1-release-");
  if (!safeParent || !safeName) throw new Error("NR-G1 negative-fixture cleanup boundary failed");
  try {
    const copiedRoot = path.join(fixtureRoot, "apps/desktop/src");
    cpSync(path.join(repositoryRoot, "apps/desktop/src"), copiedRoot, { recursive: true });
    writeFileSync(
      path.join(copiedRoot, "retired-production-entry.ts"),
      'export const retiredCommand = "agent_turn_start";\n',
      "utf8",
    );
    const findings = scanRetiredProductReferences(fixtureRoot, discoverReleaseSurface(fixtureRoot));
    if (!findings.some((finding) => finding.relativePath.endsWith("retired-production-entry.ts"))) {
      throw new Error("NR-G1 release scanner accepted a new retired production file");
    }
    return true;
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
}

export function parseReportClaims(report) {
  const match = report.match(
    /<!-- NR-G1-MANIFEST-CLAIMS:START -->\s*```json\s*([\s\S]*?)\s*```\s*<!-- NR-G1-MANIFEST-CLAIMS:END -->/u,
  );
  if (!match) throw new Error("NR-G1 report machine claims are missing");
  return JSON.parse(match[1]);
}

export function assertReportClaims(manifest, claims) {
  const keys = [
    "status",
    "nrG1Approved",
    "productionCommandCount",
    "conversationCommandCount",
    "providerCallsPerTurn",
    "skillCallsPerTurn",
    "toolCallsPerTurn",
    "canonOperationsPerTurn",
    "sessionReconciliationMaximumIntervalMs",
    "releaseConversationPathCount",
    "retiredProductReferences",
    "conversationStartsBlank",
    "automatedExternalRequests",
    "realProviderLiveSmokeExecuted",
  ];
  for (const key of keys) {
    if (claims[key] !== manifest[key]) {
      throw new Error(`NR-G1 report drift for ${key}: ${JSON.stringify(claims[key])}`);
    }
  }
  return true;
}
