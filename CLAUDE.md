# NovelGainker Workspace Rules

## Product scope

NovelGainker is a Windows-first, local-first Tauri 2 desktop application for Chinese novel writing. Markdown project files and assets are the only canonical manuscript data. Amendment A-001 approves one persistent `ProjectAgentSession` backed by a bundled, version-locked, license-reviewed Novelist-compatible Skill Pack. The release must remain self-contained: never load external Novelist/Codex plugin caches, user `SKILL.md` files, Python sidecars, or an external Agent host at runtime. AM2 mounts one unified Agent Timeline with nine embedded Skills, four bounded project-read tools, two governed Markdown write tools, visible diffs, bounded transactions, risk-gated confirmation, and protected undo; the G3～G5 chat/router/workflow backend remains only as an unmounted regression layer. JSON-object Providers must receive the exact `AgentActionPayloadV1` field shapes. Asynchronous Agent failures must survive session reload, preserve a stable safe error code, restore the submitted input, and execute zero tools or writes when output is invalid. An orphaned legacy `Generating` checkpoint may be retired before Agent start only when no real Provider run owns the channel; persist the terminal legacy state, never write Canon during reconciliation, and retain rejected user input. A restored legacy `RecoveryRequired` checkpoint is inert compatibility history: preserve it without automatic discard or Canon writes, but do not let it block project or Provider actions and do not surface it in the startup page or unified Agent. Real active Provider and legacy execution states remain blocking.

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
- App State is rebuildable, project-isolated machine-local data. The approved target may store bounded session summaries, message indexes, Turn/Skill/Tool metadata, authorization policy, transaction metadata, and undo records. It must never contain API keys, unrestricted manuscript copies, hidden Provider state, or raw Provider responses; full chat transcripts require an explicit future Story and user opt-in.
- Sprint 2 credentials use fixed DeepSeek/OpenAI Windows Credential Manager targets behind the Rust CredentialVault. IPC may expose only status, one-way save, confirmed delete, explicit local Provider selection, and the bounded NG-203～NG-207 Provider run/connection commands; never read, reveal, export, mask, suffix, fingerprint, or credential_ref. Provider network targets, protocols, models, redirects, retries, payloads, streaming, cancellation, fallback behavior, and evidence must remain fixed by the approved Sprint 2 Story Packet.
- Provider, key, Vault, Agent tool, AI apply, persistence, and network capabilities require an explicitly authorized Story Packet.

## Change discipline

- Preserve user changes and unrelated worktree state.
- Use deterministic synthetic fixtures under verified temporary roots; never test against a real user manuscript or credential.
- Keep UI and default writing language in Simplified Chinese and do not add Korean-specific branches.
- Do not start a later Sprint or broaden product scope without its BMAD gate authorization.
- Sprint 6 is paused. Do not start a later Agent Migration stage until its Story Packet has closed its questions and Review Gate and the implementation has been explicitly authorized.

## Authoritative documents

- Current architecture: `docs/architecture.zh-CN.md`
- Approved Agent architecture review: `docs/architecture-unified-novelist-agent-review.zh-CN.md`
- Delivery sequence and gates: `docs/sprint-plan.zh-CN.md`
- Current review packet: `docs/stories/agent-migration-am2-story-packet.zh-CN.md`. GA1 is product-approved; NG-A201 through NG-A206 and the GA2 revision 6 technical submission have passed with GA2-R001 through GA2-R005 resolved. Stop at GA2: product approval, Sprint 6, real Provider Live Smoke, and any broader runtime work remain unauthorized.
- Build, test, and command-registration guide: `docs/development.md`
