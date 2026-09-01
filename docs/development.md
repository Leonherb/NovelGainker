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
pnpm quality
```

`pnpm quality` is the authoritative local gate. It runs formatting, lint, TypeScript, React tests, recursive release/IPC boundaries, NR-001 through NR-006 evidence, Rust format/Clippy/tests, the offline editor round trip, debug and release Tauri builds, Windows desktop smoke, and the Canon/recovery safety kernel.

## Runtime and IPC surface

Production contains 24 Tauri commands. Conversation owns exactly:

- `conversation_session_load`
- `conversation_turn_start`
- `conversation_turn_cancel`

The stream event is `conversation-turn-event`. When changing IPC, update the Rust manifest, Tauri handler registration, generated permissions, main-window capability, all production TypeScript clients, and boundary tests together.

Project Metadata is currently `schemaVersion = 2`. Schema v1 is a one-time disposable-state cutover, not a migration API. Do not add a compatibility reader, converter, alias, Workflow-version replacement field, or feature flag.

## Evidence

Generated evidence is written below `target/qa/`:

- `nr-001/`: retirement and Canon baseline
- `nr-002/`: Project Metadata v1→v2 clean-slate tests and source hash
- `nr-003/`: Conversation Store persistence
- `nr-004/`: dual-Provider Loopback plus derived Provider/Skill/Tool/Canon call counts
- `nr-005/`: natural conversation, R01 Store reconciliation, and close lifecycle
- `nr-006/`: recursive release scan, five-layer IPC closure, upstream evidence hashes, negative fixtures, and NR-G1 manifest

The Release desktop screenshot is written below `target/nr-g1-evidence/`. Generated evidence is not the source-of-truth specification; the current technical report is `docs/reviews/novelist-agent-runtime-nr-g1-technical-submission.zh-CN.md`.

## Test boundaries

- Never run automated tests against a real manuscript or credential.
- Provider tests use deterministic local Loopback servers and make no external requests.
- Do not run real Provider Live Smoke for NR-G1.
- Manuscript Markdown and assets must remain outside project-state reset targets.
- `.novelgainker/**` stays outside manuscript indexing and publication inputs.
- A new production TS/TSX or Rust file is automatically included in the R03 release scan; tests and historical Story documents are intentionally not counted as production references.
- NR-G1 is product-approved, but NR-1 Draft 1 is review-only; Memory, Skill, Tool, Canon-context, and AI-write capabilities are not implemented.

Current architecture is documented in [architecture.zh-CN.md](./architecture.zh-CN.md). The implemented remediation boundary is [NR-G1 Remediation Draft 2 Final](./stories/novelist-agent-runtime-nr-g1-remediation-story-packet-draft-2-final.zh-CN.md). The unapproved planning boundary is [NR-1 Story Packet Draft 1](./stories/novelist-agent-runtime-nr1-history-memory-story-packet-draft-1.zh-CN.md).
