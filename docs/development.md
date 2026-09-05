# NovelGainker development guide

## Prerequisites

- Windows 11
- Node.js 24+
- pnpm 11+
- stable Rust toolchain with the MSVC target
- Tauri 2 system prerequisites

Install JavaScript dependencies once with `pnpm install --frozen-lockfile`. Cargo verification then runs with `--locked --offline`.

## Daily commands

```powershell
pnpm dev
pnpm typecheck
pnpm lint
pnpm test:frontend
cargo test --workspace --locked --offline
pnpm e2e:nr106
pnpm e2e:nr206
pnpm e2e:nr406
pnpm quality
```

`pnpm quality` is the authoritative local gate. It runs formatting, lint, TypeScript, React tests, recursive release/IPC boundaries, NR-001 through NR-006 evidence, NR-106/206/306 historical contract regressions, Pack integrity, NR-406 validation, Rust format/Clippy/tests, the offline editor round trip, debug and release Tauri builds, Windows desktop smoke, and the Canon/recovery safety kernel.

## Runtime and IPC surface

Production contains 28 Tauri commands. Conversation owns exactly:

- `conversation_session_load`
- `conversation_turn_start`
- `conversation_turn_cancel`

The stream event is `conversation-turn-event`. When changing IPC, update the Rust manifest, Tauri handler registration, generated permissions, main-window capability, all production TypeScript clients, and boundary tests together.

NR-4 adds four closed commands: `canon_change_list`, `canon_change_detail`, `canon_change_decide`, `canon_editor_state`. The decision request contains projectSessionId/changeId/revision and confirm/discard/undo/retry_recovery only. Editor requests contain projectSessionId/opaque fileId/revision/dirty; project_read_file returns fileId/editorRevision after Host edit reservation. These APIs accept no new arbitrary path or replacement content.

The Host stores visible History in `.novelgainker/conversation/session.json`, discardable Memory in `.novelgainker/memory/state.json`, bounded Task status in `.novelgainker/tasks/session.json`, and immutable snapshots/receipts in `.novelgainker/changes/{changeId}.json`. Write-enabled sessions register exactly read_context and propose_change. The NR-4 loop is in `conversation_change_runtime.rs`; delivered read evidence in `canon_change_read.rs`; transactions/Undo in `canon_change_store.rs` and its child modules. Only Host Committed receipts mean saved.

Project Metadata is `schemaVersion = 2`. Schema v1 is a one-time disposable-state cutover, not a migration API. Do not add a compatibility reader, converter, alias, Workflow-version replacement field, or feature flag.

## Evidence

Generated evidence is written below `target/qa/`:

- `nr-001/`: retirement and Canon baseline
- `nr-002/`: Project Metadata v1→v2 clean-slate tests and source hash
- `nr-003/`: Conversation Store persistence
- `nr-004/`: dual-Provider Loopback and NR-0 call counts
- `nr-005/`: natural conversation, Store reconciliation, and close lifecycle
- `nr-006/`: release scan, five-layer IPC closure, and historical NR-G1 manifest
- `nr-106/`: candidate profiles, frozen NR-1 budgets, scale test runs, source hashes, and NR-G2 manifest
- `nr-206/`: Agent Loop profile, bounded Tool/Task regressions, release scan, source hashes, and NR-G3 manifest
- `nr-306/`: embedded Pack and historical NR-3 read-only contract regression
- `nr-406/`: current five-directory transactions, actual dual Provider 4+1 chains, validation.json, full quality.json and NR-G5 technical manifest

NR-G1 through NR-G4 machine claims remain frozen. Current later-stage checks write separate regression manifests, recording the current IPC/Pack separately from historical claims. NR-406 validation is not product approval: after a full successful `pnpm quality`, `node scripts/nr406-nrg5-e2e.mjs --submit` requires matching fresh evidence before producing `nr-g5-manifest.json`. NR-G5 approval remains false until the user decides.

## Test boundaries

- Never run automated tests against a real manuscript or credential.
- Provider tests use deterministic `127.0.0.1` Loopback servers; `pnpm e2e:nr106` removes Provider/secret environment variables and enables Cargo Offline.
- Do not run real Provider Live Smoke without separate explicit authorization.
- Manuscript Markdown and assets remain outside Memory and project-state reset targets.
- `.novelgainker/**` stays outside manuscript indexing and publication inputs.
- Conversation Turn code may call only the zero-retry `complete_once_cancellable` and `stream_once_cancellable` paths. Retrying Provider methods remain limited to non-Conversation connection checks.
- NR-4 allows at most 4 Agent + 1 Compactor attempts and 17,408 requested output tokens; two reads and one mutation, four targets, 8 KiB total candidate, 160 KiB cumulative Continuation and 256 KiB actual native body. No automatic retry. Historical read-only profiles remain tested separately. Provider Usage is observation only; response bytes retain the independent 8 MiB ceiling.
- A new production TS/TSX or Rust file is automatically included in the release scan; tests and historical Story documents are intentionally not counted as production references.
- NR-G4 is approved; NR-4-D1 implementation is authorized through NR-406, stopping at NR-G5 technical submission. No NR-5 implementation or real Provider Live Smoke is authorized. Scripted providers prove Host/protocol behavior, not model intent or semantic quality.

## Recovery and local operation

Pending changes are not saved and do not add a close blocker. A stale revision/source/dirty editor requires a new proposal, not force overwrite. Undo requires every file to match its recorded afterHash; missing snapshots or full quotas cannot be bypassed by deleting Canon or resetting app state.

If recovery is required, preserve the current files and `.novelgainker/changes/`. Compare the stored Diff, resolve external edits explicitly, then use local safe retry or reopen; no Provider is called. Reopen after successful repair refreshes project write capabilities. Do not clear unresolved recovery data to make the warning disappear. A full store does not automatically evict Undo history; no history-cleanup UI is included in NR-4.

Current architecture is [architecture.zh-CN.md](./architecture.zh-CN.md); approved implementation scope is [NR-4-D1](./stories/novelist-agent-runtime-nr4-canon-mutation-story-packet-draft-1.zh-CN.md); current verification is [implementation log](./reviews/novelist-agent-runtime-nr4-implementation-log.zh-CN.md).
