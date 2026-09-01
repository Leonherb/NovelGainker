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
pnpm quality
```

`pnpm quality` is the authoritative local gate. It runs formatting, lint, TypeScript, React tests, recursive release/IPC boundaries, NR-001 through NR-006 evidence, NR-106 scale and NR-G2 evidence, Rust format/Clippy/tests, the offline editor round trip, debug and release Tauri builds, Windows desktop smoke, and the Canon/recovery safety kernel.

## Runtime and IPC surface

Production contains 24 Tauri commands. Conversation owns exactly:

- `conversation_session_load`
- `conversation_turn_start`
- `conversation_turn_cancel`

The stream event is `conversation-turn-event`. When changing IPC, update the Rust manifest, Tauri handler registration, generated permissions, main-window capability, all production TypeScript clients, and boundary tests together.

NR-1 adds no IPC command or UI panel. The Host stores complete visible History in `.novelgainker/conversation/session.json` and discardable derived Memory in `.novelgainker/memory/state.json`. The fixed production profile lives in `crates/app-core/src/memory.rs`; Provider one-attempt primitives live in `provider_core.rs`; the Compactor/main state machine lives in `conversation_runtime.rs`.

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

The historical NR-G1 report is `docs/reviews/novelist-agent-runtime-nr-g1-technical-submission.zh-CN.md`. The current technical report is `docs/reviews/novelist-agent-runtime-nr-g2-technical-submission.zh-CN.md`. Generated evidence is not the source-of-truth specification.

## Test boundaries

- Never run automated tests against a real manuscript or credential.
- Provider tests use deterministic `127.0.0.1` Loopback servers; `pnpm e2e:nr106` removes Provider/secret environment variables and enables Cargo Offline.
- Do not run real Provider Live Smoke without separate explicit authorization.
- Manuscript Markdown and assets remain outside Memory and project-state reset targets.
- `.novelgainker/**` stays outside manuscript indexing and publication inputs.
- Conversation Turn code may call only the zero-retry `complete_once_cancellable` and `stream_once_cancellable` paths. Retrying Provider methods remain limited to non-Conversation connection checks.
- The frozen requested-output rule is `1024 + 4096 <= 5120`; Provider Usage is observation only, while response bytes have an independent 8 MiB ceiling.
- A new production TS/TSX or Rust file is automatically included in the release scan; tests and historical Story documents are intentionally not counted as production references.
- NR-G2 is only a technical submission. `nrG2Approved = false`; Skill, Tool, Canon-context, AI Markdown writes, real Provider Live Smoke, and NR-2 remain unauthorized.

Current architecture is documented in [architecture.zh-CN.md](./architecture.zh-CN.md). The approved implementation boundary is [NR-1 Story Packet Draft 2 Final](./stories/novelist-agent-runtime-nr1-history-memory-story-packet-draft-2-final.zh-CN.md), and the current review target is [NR-G2 technical submission](./reviews/novelist-agent-runtime-nr-g2-technical-submission.zh-CN.md).
