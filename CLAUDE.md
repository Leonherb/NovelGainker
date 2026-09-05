# NovelGainker Workspace Rules

## Scope and current authorization

NovelGainker is a Windows-first, local-first Tauri 2 Simplified-Chinese novel-writing app. Markdown and assets are the only manuscript Canon. Keep the three-column workspace: project/files/provider, natural conversation, real Markdown editor.

The user approved NR-4-D1 / NR4-D1-R01～R04 and sequential implementation NR-401 → NR-406. All six Stories passed their individual full quality gates; NR-G5 is technically submitted. Stop here and await explicit product Gate approval. NR-G4 stays passed. Do not restore Pre-NR-4, reopen NR-G4, or form/implement NR-5～NR-7 without separate authorization.

The minimal read-only Provider Live Smoke is explicitly waived with compatibility risk accepted; do not run real credentials/network testing. NR-6 full validation remains planned. Independent formal chapter-source maintenance is deferred to NR-5; explicit user tasks referencing chapters may maintain the five NR-4 data types, not chapter files.

## Required checks and change discipline

- Run `pnpm quality` before declaring any implementation Story or Gate technically complete. `pnpm e2e:sprint1` is the standalone offline editor round trip.
- Cargo verification remains `--locked --offline`. Do not weaken lint, Clippy, schemas, capabilities, path, recovery, evidence or secret checks.
- Preserve user changes and unrelated worktree state. Use verified isolated synthetic fixtures, never a real manuscript or credential.
- Keep UI/default writing language Simplified Chinese; no Korean-specific branches.
- Keep Manifest, Handler, generated Permission, Capability JSON and every production TypeScript client synchronized. Current approved surface is 28 commands, including exactly 3 Conversation and 4 Canon-change commands.
- Historical Gate machine claims/technical manifests stay frozen. Later-stage regression evidence must be separate; current facts must not masquerade as the old 24-command/read-only snapshot.
- `AGENTS.md` is the intentional pointer to this authoritative file; do not duplicate rules or replace the pointer merely to satisfy a generic symlink convention.

## Runtime and data boundaries

- Embed the self-contained Novelist projection (currently 1.1.0, nine capabilities/37 resources); never load external plugin caches, user SKILL.md, Python sidecars or an external runtime host.
- AI chooses methods/tools semantically. Do not add keyword routing, Anchor, Control JSON, user-visible Skill selection or a separate Task Center.
- A write-enabled project registers only `novelist.read_context` and `novelist.propose_change`. Mutations target only characters/world/materials/plot/outlines. Method resources do not imply executable chapter, image or publication capability.
- Require trustworthy persisted message references, delivered method/full-read receipts, source hashes, strict schemas, scoped paths, writer coordination and durable transactions. Source hashes validate provenance, not model intent; do not claim offline scripts prove semantic authorization.
- Local confirmation/Undo APIs accept opaque identifiers, revision and closed actions, never arbitrary replacement bytes or execution paths. Editor protection must be acknowledged by Host before editing; unsaved manual content cannot be overwritten by AI.
- Only a durable Committed change receipt means saved. Task succeeded and model prose are not save evidence. Commit precedes final reply; a later Provider/Task failure cannot deny it.
- Undo requires all targets to match recorded afterHash. Preserve immutable before/after snapshots and inverse intent; unresolved external edits keep recovery_required and block new writes, not chat. Do not clear recovery records to bypass uncertainty.
- No arbitrary filesystem, URL, Shell, Process or Opener capability. Manual writes retain relative-path validation, source hashes, writer lease, atomic replacement and recovery.
- Visible History: `.novelgainker/conversation/`; derived Memory: `.novelgainker/memory/`; Task: `.novelgainker/tasks/`; Change: `.novelgainker/changes/`. All are non-Canon and excluded from indexing/publication.
- Task stores bounded hashes, byte counts, controlled status/labels only. Change stores normalized snapshots/receipts and source references, not raw Provider requests/results. Hidden prompts, Provider Continuation, credentials and absolute paths must not enter project state, IPC, logs or evidence.
- Project Metadata is strict schemaVersion 2. A v1 reset targets exact `.novelgainker/**` only under writer lease, canonical containment and recursive symlink/reparse rejection, generates Project ID once and preserves Canon/assets/settings/credentials/unrelated projects. No migration reader, converter, alias, Workflow-version field or feature flag.

## Provider, conversation and close

- Credentials use fixed Windows Credential Manager targets through Rust CredentialVault. IPC may expose configuration status, one-way save, confirmed delete, local selection and bounded run/connection commands, never read/export/mask/suffix/fingerprint keys.
- User-initiated production conversation may use the configured key only inside Rust. Automated evidence remains offline with loopback synthetic keys.
- Persist the user message and running before Provider invocation, not every delta. Preserve all nonempty AI text; abnormal terminal appends `本次内容未生成完成。`, zero text shows `本次回复未生成。`.
- Event is low-latency; Store is terminal truth. Listener readiness gates sending, active reconciliation is non-overlapping and at most 2000 ms; synchronization retry must never call Provider or duplicate messages.
- Only unsaved manual Markdown may block closing with 保存/放弃/取消. Do not cancel AI before that decision. Committed close cancels the active Turn and exits without waiting for Provider, flush or project cleanup. Pending AI proposals add no close blocker.
- NR-4 allows two reads/one mutation, four Agent + one Compactor and 17,408 requested output tokens; four targets/8 KiB total candidate, 160 KiB cumulative Continuation, 256 KiB actual Provider body. Check exact serialized boundaries and reserve final-result space before writing; no automatic retries or parallel tools.

## Authoritative documents

- Architecture: `docs/architecture.zh-CN.md`
- Delivery/current Gate: `docs/sprint-plan.zh-CN.md`, `bmad/workflow-status.yaml`, `bmad/sprint-status.yaml`
- Development/recovery/evidence: `docs/development.md`
- Current product view: `docs/product-current-state-report.zh-CN.md`
- Approved scope: `docs/stories/novelist-agent-runtime-nr4-canon-mutation-story-packet-draft-1.zh-CN.md`
- Approval review: `docs/reviews/novelist-agent-runtime-nr4-story-packet-draft-1-review.zh-CN.md`
- Implementation evidence: `docs/reviews/novelist-agent-runtime-nr4-implementation-log.zh-CN.md`
- Passed NR-G4: `docs/reviews/novelist-agent-runtime-nr-g4-gate-rereview.zh-CN.md`
- Current Gate submission: `docs/reviews/novelist-agent-runtime-nr-g5-technical-submission.zh-CN.md`; earlier NR-G1–G4 reports in the same directory remain historical.
- Approved phased roadmap: `docs/novelist-agent-runtime-nr2-nr7-capability-delivery-plan-draft-2.zh-CN.md`
