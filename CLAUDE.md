# NovelGainker Workspace Rules

## Scope and current authorization

NovelGainker is a Windows-first, local-first Tauri 2 Simplified-Chinese novel-writing app. Markdown and assets are the only manuscript Canon. Keep the three-column workspace: project/files/provider, natural conversation, real Markdown editor.

NR-401 → NR-406 passed their individual quality gates and the user approved NR-G5. The remaining packet contains only NR-5/NR-6 (12 Stories). R04 product behaviors (shared-Canon multi-session, branch/recoverable clear/export, conservative explicit retry) are accepted; R05 real validation is now multi-user application testing, not sole-owner testing or automated Live batches. See docs/testing/novelist-agent-runtime-nr6-multi-user-test-plan.zh-CN.md. R01～R03 and unconfirmed engineering limits/batch arrangements remain pending; no later Story is implemented or tested. The user explicitly has no cloud-sync requirement: exclude it from the entire plan, not as a deferred/conditional phase or backlog. The final planned Gate is NR-G7. Do not infer approval of new budgets, assistant use of Live credentials/network, user recruitment/build distribution or future Gates. Release NR-501 only after its packet decisions are resolved. Do not restore Pre-NR-4 or reopen NR-G4/G5.

The minimal read-only Provider Live Smoke is explicitly waived with compatibility risk accepted; do not run real credentials/network testing. NR-6 full validation remains planned. Independent formal chapter-source maintenance is deferred to NR-5; explicit user tasks referencing chapters may maintain the five NR-4 data types, not chapter files.

The user additionally accepted NG-F01–NG-F05 plan repairs: version-bound finalization invalidated by edits, full creation-to-EPUB user tests, inline expandable Host tool evidence, 32 non-archived sessions (archives still consume bounded storage), and clean install/old-project upgrade/final-build impact retesting. These are approved design/acceptance changes in the existing Stories, not implementation/Gate/distribution authorization. Do not re-request the 32 non-archived session product decision; remaining aggregate storage/catalog/evidence DTO bounds must be settled before the responsible implementation.

## Current execution authorization (supersedes pending-design statements above)

The user explicitly agreed to continuous NR-501→506→601→606 implementation under the revised packet, including R01/R02 and NG-F01–NG-F05. See docs/reviews/novelist-agent-runtime-nr5-nr6-implementation-approval.zh-CN.md. R03 now permits NR-6 entry immediately after NR-G6 technical checks/evidence pass, without a separate human wait; never pre-mark technical approval. In-scope engineering parameters may be settled, recorded and tested by the implementer. Ask again only for expanded scope/permissions/costs or required external resources. Each Story must pass full quality. Multi-user testing still needs the user's participants/arrangements; no assistant Live keys/batches or automatic distribution. NR-G7 remains the user's final decision. Current runtime restrictions below describe production NR-4 until the authorized responsible Story safely extends them.

## Required checks and change discipline

- Run `pnpm quality` before declaring any implementation Story or Gate technically complete. `pnpm e2e:sprint1` is the standalone offline editor round trip.
- Cargo verification remains `--locked --offline`. Do not weaken lint, Clippy, schemas, capabilities, path, recovery, evidence or secret checks.
- Preserve user changes and unrelated worktree state. Use verified isolated synthetic fixtures, never a real manuscript or credential.
- Keep UI/default writing language Simplified Chinese; no Korean-specific branches.
- Keep Manifest, Handler, generated Permission, Capability JSON and every production TypeScript client synchronized. NR-505 passed full quality with 30 commands: the frozen 28-command NR-504 baseline plus exactly publication_list/publication_detail under the current approval. Exactly 3 Conversation and 4 Canon-change commands remain; NR-506 is in progress.
- Historical Gate machine claims/technical manifests stay frozen. Later-stage regression evidence must be separate; current facts must not masquerade as the old 24-command/read-only snapshot.
- Current product approval is in BMAD `current_gate`; historical `gate`/`nr4_packet` fields retain submission scope for old checkers. NR-501 must separate historical validation from current approval with negative fixtures, not hide approval or weaken checks.
- `AGENTS.md` is the intentional pointer to this authoritative file; do not duplicate rules or replace the pointer merely to satisfy a generic symlink convention.

## Runtime and data boundaries

- Embed the self-contained Novelist projection (currently 1.4.0, nine capabilities/37 resources); never load external plugin caches, user SKILL.md, Python sidecars or an external runtime host.
- AI chooses methods/tools semantically. Do not add keyword routing, Anchor, Control JSON, user-visible Skill selection or a separate Task Center.
- A write-enabled project registers only `novelist.read_context` and `novelist.propose_change`. NR-502/503 passed full quality: v2 supports five data types plus one chapter per Turn, strict current source/full-read evidence and 32 KiB chapters; Host binds current committed chapters for five-type maintenance and separately displays stale dependencies. NR-504 is in progress, mounting bounded nonlinear ordering and the approved increased budget; its directed tests pass but full quality remains required. Publication/image execution remains unavailable; the maximum-budget test uses a cfg(test)-only publication stand-in, not an EPUB executor.
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
- Active NR-504 extends only the NR-5 profile to four reads/two mutations/one publication slot, eight Agent + one Compactor and 33,792 requested output tokens. The publication slot is not registered until NR-505. Orders are semantic and serial; one chapter per Turn, seventh tool at most, eighth Agent tool-less. Arguments/results 64 KiB, cumulative native Continuation 512 KiB, actual native Body 768 KiB; reserve double-escaped full results before effects. Failed/pending tasks do not undo independent committed work or erase visible text. NR-4's historical limits remain separately tested.

## NR-601 current implementation facts (supersede older runtime availability notes above)

NR-501 through NR-506, NR-G6 technical submission, NR-601 (32 steps), NR-602 (33 steps), and NR-603 (34 steps) passed individual full quality: 9 of 12 Stories completed. Production has exactly 38 IPCs, one explicit retry, durable attempt identity, eight Agent plus one Compactor / 33792 cumulative tokens, and no automatic Provider retry or native server-cursor continuation. NR-603 passed 151 frontend / 294 app-core tests plus directed evidence and Debug/Release build/start. NR-604 entry awaits the product-owner-confirmed trial batch; tool evidence details, test packages and install/upgrade preparation are still unimplemented, and all real user validation is unexecuted. See nr604-entry-readiness and the NR-601/602/603 contracts. Do not mark NR-604/605/606 completed or distribute automatically. Frozen NR-G6 initial human-approved claim stays false; continuous NR-6 entry was separately authorized. Final NR-G7 still requires the user.

NR-501 through NR-505 passed individual full quality. The current NR-5 registry contains novelist.read_context, novelist.propose_change and novelist.publish; NR-4 remains read/change only. Chapter writes and five-type maintenance retain strict source/full-read evidence, version-bound final authority and 32 KiB chapters. NR-505 has real local EPUB preflight/build, durable receipts and two inline read IPCs. NR-506 exercises actual desktop commands and worker through a test-only Tauri MockRuntime bridge with isolated synthetic vault and loopback HTTP; this is not native GUI or real Provider verification. Task snapshots and complete local updates are coordinated per project to avoid Windows replacement races, without holding locks across HTTP or tool execution. Image generation remains unavailable. Historical NR-504 maximum-budget fixtures stay test-only stand-ins; separate NR-505 maximum loops execute actual publication. The budget is unchanged: four reads/two mutations/one publication, eight Agent plus one Compactor, 33792 tokens.

## Authoritative documents

- Architecture: `docs/architecture.zh-CN.md`
- Delivery/current Gate: `docs/sprint-plan.zh-CN.md`, `bmad/workflow-status.yaml`, `bmad/sprint-status.yaml`
- Development/recovery/evidence: `docs/development.md`
- Current product view: `docs/product-current-state-report.zh-CN.md`
- Approved scope: `docs/stories/novelist-agent-runtime-nr4-canon-mutation-story-packet-draft-1.zh-CN.md`
- Approval review: `docs/reviews/novelist-agent-runtime-nr4-story-packet-draft-1-review.zh-CN.md`
- Implementation evidence: `docs/reviews/novelist-agent-runtime-nr4-implementation-log.zh-CN.md`
- Passed NR-G4: `docs/reviews/novelist-agent-runtime-nr-g4-gate-rereview.zh-CN.md`
- Current Gate approval: `docs/reviews/novelist-agent-runtime-nr-g5-product-approval.zh-CN.md`; NR-G5 technical submission and earlier reports remain frozen.
- Next packet/review: `docs/stories/novelist-agent-runtime-nr5-nr6-story-packet-draft-1.zh-CN.md`, `docs/reviews/novelist-agent-runtime-nr5-nr6-story-packet-draft-1-review.zh-CN.md`.
- Approved phased roadmap: `docs/novelist-agent-runtime-nr2-nr6-capability-delivery-plan-draft-2.zh-CN.md`
