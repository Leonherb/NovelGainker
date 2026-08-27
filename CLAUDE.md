# NovelGainker Workspace Rules

## Product scope

NovelGainker is a Windows-first, local-first Tauri 2 desktop application for Chinese novel writing. Markdown project files and assets are the only canonical manuscript data. The current implementation is independent of the Novelist plugin and must not load plugin caches, Python sidecars, or SKILL.md files at runtime.

## Required checks

- Run `pnpm quality` before declaring an implementation Story or Gate technically complete.
- Use `pnpm e2e:sprint1` for the standalone no-key, offline editor round trip.
- Keep Cargo verification locked and offline after dependencies are prepared.
- Do not weaken lint, Clippy, capability, path, recovery, or secret-leak checks to make a gate pass.

## Security and data boundaries

- Frontend filesystem operations use typed Tauri clients, opaque project/session identifiers, and project-relative paths.
- Keep Tauri command manifest, handler registration, generated permissions, capability JSON, frontend client, and contract tests synchronized.
- Never expose arbitrary filesystem, URL, Shell, Process, or Opener execution.
- Canon writes require scoped path validation, source hashes, single-writer coordination, recoverable atomic replacement, and stable redacted errors.
- App State is rebuildable machine-local data. It must not contain manuscript bodies, API keys, prompts, search queries, or Provider responses.
- Sprint 2 credentials use fixed DeepSeek/OpenAI Windows Credential Manager targets behind the Rust CredentialVault. IPC may expose only status, one-way save, confirmed delete, explicit local Provider selection, and the bounded NG-203～NG-207 Provider run/connection commands; never read, reveal, export, mask, suffix, fingerprint, or credential_ref. Provider network targets, protocols, models, redirects, retries, payloads, streaming, cancellation, fallback behavior, and evidence must remain fixed by the approved Sprint 2 Story Packet.
- Provider, key, Vault, AI apply, and network capabilities require an explicitly authorized Story Packet.

## Change discipline

- Preserve user changes and unrelated worktree state.
- Use deterministic synthetic fixtures under verified temporary roots; never test against a real user manuscript or credential.
- Keep UI and default writing language in Simplified Chinese and do not add Korean-specific branches.
- Do not start a later Sprint or broaden product scope without its BMAD gate authorization.

## Authoritative documents

- Current architecture: `docs/architecture.zh-CN.md`
- Delivery sequence and gates: `docs/sprint-plan.zh-CN.md`
- Current execution packet: `docs/stories/sprint-6-story-packet.zh-CN.md` (created; S6-Q001 through S6-Q007 and the Story Packet Review Gate are pending; NG-601 is not authorized)
- Build, test, and command-registration guide: `docs/development.md`
