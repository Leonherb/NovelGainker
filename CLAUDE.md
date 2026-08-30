# NovelGainker Workspace Rules

## Product scope

NovelGainker is a Windows-first, local-first Tauri 2 desktop application for Chinese novel writing. Markdown project files and assets are the only canonical manuscript data. The approved RB0 target is a chat-first, three-column workspace backed by a bundled, version-locked, license-reviewed Novelist-compatible Skill Pack. Conversation, Control, Content, Canon, and Persistence are separate protocol planes. The Host owns paths, permissions, budgets, history validation, conflict gates, transactions, and writes; Provider text never grants authority. Full visible chat, sourced Host memory, and Context Marks belong to project metadata under `.novelgainker/agent/`, are portable with the project, and are excluded from Canon search, ordinary Skill read/write, and publication input. The release must remain self-contained: never load external Novelist/Codex plugin caches, user `SKILL.md` files, Python sidecars, or an external Agent host at runtime. The current production Agent path remains the GAR1 read-only V2 path until NG-R208 cutover: 28 production commands, 7 V2 Agent IPC commands, and zero Agent Canon writes. NG-R101 through NG-R106 and the full local quality gate completed on 2026-08-30; the GAR1 technical submission remains `gar1Approved=false`. AM-R2 Draft 1 was explicitly approved without modification on 2026-08-30, with continuous implementation authorization for NG-R201 through NG-R208 and a mandatory stop at the GAR2 technical submission. Do not start AM-R3, Sprint 6, a real Provider Live Smoke, or treat GAR1/GAR2 as product-approved without later explicit authorization.

## Required checks

- Run `pnpm quality` before declaring an implementation Story or Gate technically complete.
- Use `pnpm e2e:sprint1` for the standalone no-key, offline editor round trip.
- Keep Cargo verification locked and offline after dependencies are prepared.
- Do not weaken lint, Clippy, capability, path, recovery, or secret-leak checks to make a gate pass.

## Security and data boundaries

- Frontend filesystem operations use typed Tauri clients, opaque project/session identifiers, and project-relative paths.
- Keep Tauri command manifest, handler registration, generated permissions, capability JSON, frontend client, and contract tests synchronized.
- Never expose arbitrary filesystem, URL, Shell, Process, or Opener execution.
- Canon writes require scoped path and Schema validation, source hashes, single-writer coordination, recoverable atomic transactions, visible Turn diffs, bounded undo, and stable redacted errors. The approved target supports `plan-only`, `review-every-write`, and default `auto-apply-safe`; protected-file overwrite, whole-chapter replacement, chapter-state/Story-Ledger changes, outside-project import/read, and publish replacement always require separate confirmation.
- App State is rebuildable, project-isolated machine-local data. Complete visible chat and sourced Session Memory are stored in project-local `.novelgainker/agent/` records with Manifest, Schema, project binding, sequence, Hash Chain, quarantine, sync-fork, quota, and atomic-replacement checks. App State and project metadata must never contain API keys, hidden Provider state, raw Provider responses, hidden prompts, unrestricted tool results, or absolute paths. Canon tools, indexing, Skills, and publication must exclude `.novelgainker/agent/**`.
- Sprint 2 credentials use fixed DeepSeek/OpenAI Windows Credential Manager targets behind the Rust CredentialVault. IPC may expose only status, one-way save, confirmed delete, explicit local Provider selection, and the bounded NG-203～NG-207 Provider run/connection commands; never read, reveal, export, mask, suffix, fingerprint, or credential_ref. Provider network targets, protocols, models, redirects, retries, payloads, streaming, cancellation, fallback behavior, and evidence must remain fixed by the approved Sprint 2 Story Packet.
- Provider, key, Vault, Agent tool, AI apply, persistence, and network capabilities require an explicitly authorized Story Packet.

## Change discipline

- Preserve user changes and unrelated worktree state.
- Use deterministic synthetic fixtures under verified temporary roots; never test against a real user manuscript or credential.
- Keep UI and default writing language in Simplified Chinese and do not add Korean-specific branches.
- Do not start a later Sprint or broaden product scope without its BMAD gate authorization.
- GRB0, Agent Protocol V2, AM-R1 Draft 2, and AM-R2 Draft 1 are product-approved; NG-R101 through NG-R106 are technically complete. AM-R2 is authorized for continuous NG-R201 → NG-R208 implementation, with a mandatory stop at the GAR2 technical submission. Do not start AM-R3, Sprint 6, a real Provider Live Smoke, or claim GAR1/GAR2 product approval without later explicit authorization.

## Authoritative documents

- Current architecture: `docs/architecture.zh-CN.md`
- Approved product rebaseline: `docs/stories/agent-conversation-authoring-rebaseline-plan.zh-CN.md`
- Approved Agent Protocol V2 and Context Mark specification: `docs/stories/agent-protocol-v2-context-mark-review.zh-CN.md`
- Delivery sequence and gates: `docs/sprint-plan.zh-CN.md`
- Completed execution packet: `docs/stories/agent-conversation-authoring-am-r1-story-packet.zh-CN.md`. NG-R101 through NG-R106 are complete, and the GAR1 technical submission is at `target/qa/gar1/`.
- Active approved execution packet: `docs/stories/agent-conversation-authoring-am-r2-story-packet.zh-CN.md`. Implement NG-R201 through NG-R208 continuously and stop after generating the GAR2 technical submission; AM-R3 remains unauthorized.
- Build, test, and command-registration guide: `docs/development.md`
