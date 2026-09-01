# NovelGainker Workspace Rules

## Product scope

NovelGainker is a Windows-first, local-first Tauri 2 desktop application for Chinese novel writing. Markdown project files and assets are the only canonical manuscript data. The interface is a persistent three-column workspace: project/files/provider on the left, natural conversation in the center, and the real Markdown editor on the right.

The implemented baseline is NR-1. It preserves complete project-bound visible conversation and maintains a discardable derived Memory before bounded long-context replies. A turn uses one main Provider HTTP attempt and may use one preceding Compactor attempt at the byte threshold; both are cancellable and zero-retry. It does not load Novelist Skills, read Canon into model context, call tools, or let AI write Markdown. NR-101 through NR-106 and NR-G2-R01 through NR-G2-R03 are implemented, and the product owner has approved the NR-G2 technical submission. Work is stopped at the passed NR-G2 Gate; real Provider Live Smoke and NR-2 remain unauthorized. The release must remain self-contained: never load external plugin caches, user `SKILL.md` files, Python sidecars, or an external runtime host.

## Required checks

- Run `pnpm quality` before declaring an implementation Story or Gate technically complete.
- Use `pnpm e2e:sprint1` for the standalone no-key, offline editor round trip.
- Keep Cargo verification locked and offline after dependencies are prepared.
- Do not weaken lint, Clippy, capability, path, recovery, evidence, or secret-leak checks to make a gate pass.

## Security and data boundaries

- Frontend filesystem operations use typed Tauri clients, opaque project/session identifiers, and project-relative paths.
- Keep Tauri command manifest, handler registration, generated permissions, capability JSON, and every production TypeScript client synchronized.
- Never expose arbitrary filesystem, URL, Shell, Process, or Opener execution.
- Manual Markdown writes require scoped relative paths, source hashes, single-writer coordination, atomic replacement, recovery, and stable redacted errors. NR-0 has no AI Markdown-write path.
- App State is rebuildable, project-isolated machine-local data. Visible Conversation lives in `.novelgainker/conversation/` with project binding, stable order, visible text, and terminal state. Derived Memory lives in `.novelgainker/memory/`, is invalidatable against History, and is never Canon or user-visible Conversation. App State and project metadata must never contain API keys, hidden Provider state, raw Provider responses, hidden prompts, unrestricted results, or absolute paths. Indexing and publication must exclude `.novelgainker/**`.
- Current Project Metadata is strict `schemaVersion = 2`. A v1 project may reset exact `.novelgainker/**` only under a writer lease, canonical-root containment, and recursive symlink/reparse rejection. The cutover generates a new Project ID once; it must preserve manuscript Markdown, assets, Provider settings, UI preferences, unrelated projects, and Windows credentials. Do not add migration, converter, compatibility reader, alias, replacement Workflow-version field, or feature flag.
- DeepSeek/OpenAI credentials use fixed Windows Credential Manager targets behind the Rust CredentialVault. IPC may expose only status, one-way save, confirmed delete, explicit local Provider selection, and bounded run/connection commands; never read, reveal, export, mask, suffix, fingerprint, or persist credential references.
- A user-initiated production Conversation may use the configured Provider key only inside Rust. Do not add credential-read/export IPC or expose keys to the frontend, logs, project data, tests, reviews, or desktop Smoke; automated evidence remains offline.

## Change discipline

- Preserve user changes and unrelated worktree state.
- Use deterministic synthetic fixtures under verified temporary roots; never test against a real user manuscript or credential.
- Keep UI and default writing language in Simplified Chinese and do not add Korean-specific branches.
- Do not start a later delivery stage or broaden product scope without its BMAD gate authorization.
- NR-001～NR-006, NR-G1-R01～NR-G1-R03, NR-101～NR-106, and NR-G2-R01～NR-G2-R03 are implemented, and NR-G2 is approved and passed. Stop at NR-G2; do not execute real Provider Live Smoke, form or enter NR-2, or enter NR-3～NR-6 without separate explicit approval.
- Preserve user Markdown Canon and assets while retiring obsolete runtime data and product paths.
- Persist the user message and `running` before Provider invocation, but do not persist every stream delta. Any nonempty AI text is preserved; abnormal completion adds `本次内容未生成完成。`, while zero-text failure shows `本次回复未生成。`.
- Event is a low-latency path; Conversation Store is terminal truth. Listener readiness gates sending, active Turns use one non-overlapping reconciliation at no more than 2000ms, and synchronization retry must never call Provider or duplicate the user message.
- Window close may be blocked only by unsaved manual Markdown, using `保存 / 放弃 / 取消`. A raw close request must not cancel the AI before this decision. Once close is committed, cancel any active AI Turn and exit without waiting for Provider, Conversation flush, or project cleanup; a leftover `running` becomes `本次回复未生成。` on next open.
- The NR-G1 release gate scans production surfaces only; tests and historical Story records are not production-reference evidence. Keep the technical-submission Manifest and machine-claim block frozen, while the Report Gate Decision and BMAD current state consistently record the later product approval.

## Authoritative documents

- Current architecture: `docs/architecture.zh-CN.md`
- Approved NR-0 Packet: `docs/stories/novelist-agent-runtime-nr0-clean-slate-story-packet.zh-CN.md`
- Implemented Remediation Packet: `docs/stories/novelist-agent-runtime-nr-g1-remediation-story-packet-draft-2-final.zh-CN.md`
- Current delivery sequence and gate: `docs/sprint-plan.zh-CN.md`
- Approved NR-1 Packet: `docs/stories/novelist-agent-runtime-nr1-history-memory-story-packet-draft-2-final.zh-CN.md`
- Historical NR-G1 technical submission: `docs/reviews/novelist-agent-runtime-nr-g1-technical-submission.zh-CN.md`
- Current NR-G2 technical submission: `docs/reviews/novelist-agent-runtime-nr-g2-technical-submission.zh-CN.md`
- Current NR-G2 Remediation Draft: `docs/stories/novelist-agent-runtime-nr-g2-remediation-story-packet-draft-2.zh-CN.md`
- Current BMAD state: `bmad/workflow-status.yaml` and `bmad/sprint-status.yaml`
- Build, test, and command-registration guide: `docs/development.md`
