# NovelGainker Workspace Rules

## Scope and authority

NovelGainker is a Windows-first, local-first Tauri 2 Simplified-Chinese novel-writing app. Markdown and controlled assets are the only manuscript Canon. Keep the three-column workspace: project/files/sessions/Provider, natural conversation, real Markdown editor. Cloud sync is explicitly excluded from the entire plan, including deferred or conditional backlogs.

The user approved continuous NR-501→506→601→606 implementation under the revised packet; in-scope engineering parameters may be settled, recorded and tested by the implementer. NR-G6 technical checks/evidence permit NR-6 entry without a separate human wait. NR-G7 remains the user's final decision. Do not re-request accepted R01–R03, 32 non-archived sessions or NG-F01–NG-F05 decisions. Do not reopen NR-G4/G5 or restore Pre-NR-4. See the implementation approval and current delivery status below.

Current recorded completion is NR-501～506 and NR-601～603 (9/12); NR-604～606 and final NR-G7 remain incomplete. Trial batch arrangements and external resources remain unresolved, but local preparation already within scope does not need new code permission. No automatic recruitment, build distribution, paid services or assistant use of real Provider credentials/network is authorized. Participants operate their own trials and voluntarily share redacted evidence. The minimal read-only Provider Live Smoke was explicitly waived; formal NR-6 validation was not waived.

The product decisions Q1–Q17 in `docs/product-spec.zh-CN.md` are confirmed, with the 2026-09-10 T2/T2a amendment: automatically audit relevant context on every conversational mention of entities/keywords, including aliases and contextual pronouns; formal editorial review requires an explicit author request and is never automatically scheduled at stage/book completion. Keep quality goals and author acceptance; unrequested editorial review is not an acceptance/export prerequisite. The G01–G15 directions in `docs/reviews/technical-gap-closure-proposal-2026-09-10.zh-CN.md` are now confirmed, including input priority/pause-and-archive (T3/T5), copy-based migration with old-reader rejection (T4), purpose budgets, segmented streaming persistence and verification plans. Do not re-ask these choices. `docs/technical-design.zh-CN.md` records the target design, not current runtime behavior. Do not treat completion of the planning/interview task as production implementation, new feature acceptance or authorization to execute every proposed change. Q9 explicitly selects finite automatic recovery as a new target, superseding the old zero-auto-retry product policy. Current code still has zero automatic Conversation network retry until the new runtime contract is implemented. Keep target requirements distinct from current capabilities.

## Required checks and change discipline

- Run `pnpm quality` before declaring any implementation Story or Gate technically complete. `pnpm e2e:sprint1` is the standalone offline editor round trip. Document-only work checks facts/references/diffs without claiming a new code quality pass.
- Cargo verification remains `--locked --offline`. Do not weaken lint, Clippy, schemas, capabilities, path, recovery, evidence or secret checks.
- Preserve user changes and unrelated worktree state. Tests use verified isolated synthetic fixtures and loopback synthetic keys, never a real manuscript or credential.
- Keep UI/default writing language Simplified Chinese; no Korean-specific branches.
- Keep Manifest, Handler, generated Permission, Capability JSON and all production TypeScript clients synchronized. Current production has 38 commands; see `apps/desktop/src-tauri/src/command_manifest.rs`. Interface changes require matching boundary validation.
- Historical Story/Gate machine claims, manifests and technical snapshots stay frozen. Later checks record current source/IPC/Pack separately; no historical success can stand in for current verification.
- Current product approval is BMAD `current_gate`; `successor_packet` and `story_progress` describe later delivery. Old `gate`/`nr4_packet` fields are explicitly scoped compatibility snapshots, not current approval or progress.
- `AGENTS.md` is the intentional pointer to this authoritative file; do not duplicate rules or replace it merely to satisfy a generic symlink convention.

## Runtime and data boundaries

- Embed the self-contained Novelist projection (currently 1.4.0, nine capabilities/37 resources); never load external plugin caches, user SKILL.md, Python sidecars or an external runtime host.
- AI chooses methods/tools semantically. T2/T2a permits keyword/entity/alias/pronoun detection to trigger context retrieval and consistency checks, not fixed-word write/method routing or editorial-review authorization. Do not add Anchor, Control JSON, user-visible Skill selection or a separate Task Center.
- Current NR-5 tool registry: `novelist.read_context`, `novelist.propose_change`, `novelist.publish`. Writes support five data types and one chapter per Turn; real local EPUB preflight/build exists. Image generation remains unavailable. Historical NR-4 and test-only stand-ins retain their own scope.
- Require trustworthy persisted message references, delivered method/full-read receipts, source hashes, strict schemas, scoped paths, writer coordination and durable transactions. Source hashes validate provenance, not model intent; offline scripts cannot prove semantic authorization.
- Local confirmation/Undo APIs accept opaque identifiers, revision and closed actions, never arbitrary replacement bytes or execution paths. Editor protection must be acknowledged by Host before editing; unsaved manual content cannot be overwritten by AI.
- Only a durable Committed receipt means saved. Task succeeded and model prose are not save evidence. Commit precedes final reply; a later Provider/Task failure cannot deny it. Chapter finalization is version-bound and invalidated by edits/Undo; publication requires current valid final versions and its own result evidence.
- Undo requires all targets to match recorded afterHash. Preserve immutable before/after snapshots and inverse intent; unresolved external edits keep recovery_required and block new writes, not chat. Do not clear recovery records to bypass uncertainty.
- No arbitrary filesystem, URL, Shell, Process or Opener capability in the application. Manual writes retain relative-path validation, source hashes, writer lease, atomic replacement and recovery. No project lock across HTTP/tool execution; short local transactions and Task snapshots are coordinated per physical project.
- History: `.novelgainker/conversation/`; derived Memory: `.novelgainker/memory/`; Task: `.novelgainker/tasks/`; Change: `.novelgainker/changes/`. Internal state is non-Canon and excluded from manuscript indexing/publication. Preserve default `session.json`/`state.json` paths; new sessions have category-specific `sessions/{UUID}.json` paths.
- Task stores bounded hashes, byte counts, controlled status/labels only. Change stores normalized snapshots/receipts and source references, not raw Provider requests/results. Hidden prompts, Provider Continuation, credentials and absolute paths must not enter project state, IPC, logs or evidence.
- Project Metadata is strict schemaVersion 2. The v1 disposable-state reset targets exact `.novelgainker/**` only under writer lease, canonical containment and recursive symlink/reparse rejection, generates Project ID once and preserves Canon/assets/settings/credentials/unrelated projects. Do not add a v1 migration reader, converter, alias, Workflow-version replacement field or feature flag. Future state evolution needs its own explicit data contract, not reuse of this reset to delete content.
- Sessions share current Canon and allow one active project run; a branch copies visible chat, not manuscript versions or executable authority. Archive consumes storage; no automatic eviction or loss of recovery/Undo evidence to free quota.

## Provider, budgets, conversation and close

- Credentials use fixed Windows Credential Manager targets through Rust CredentialVault. IPC may expose status, one-way save, confirmed delete, local selection and bounded run/connection commands, never read/export/mask/suffix/fingerprint keys.
- User-initiated production conversation may use the configured key only inside Rust. Automated evidence remains offline with loopback synthetic keys; no automatic real Live tests.
- Persist the user message, Running and attempt identity before Provider invocation. Current streaming deltas are not each persisted. Preserve nonempty AI text; abnormal terminal appends `本次内容未生成完成。`, zero text shows `本次回复未生成。`.
- Event is low-latency; Store is terminal truth. Listener readiness gates sending, active reconciliation is non-overlapping and at most 2000 ms; synchronization retry must never call Provider or duplicate messages. Match captured session and attempt identities.
- Only unsaved manual Markdown may block closing with 保存/放弃/取消. Do not cancel AI before that decision. Committed close cancels the active Turn and exits without waiting for Provider, flush or project cleanup. Pending AI proposals add no close blocker.
- Current NR-5 budget: four reads/two mutations/one publication, eight Agent plus one Compactor, 33792 cumulative requested output tokens; one chapter per Turn, seventh tool at most, eighth Agent tool-less. Arguments/results 64 KiB, cumulative native Continuation 512 KiB, actual native body 768 KiB. Reserve exact serialized results before effects. Calls/tools remain serial; partial failure cannot undo independent commits or erase visible text.
- Current Conversation uses zero-auto-retry single-call Provider paths. NR-603 allows one eligible explicit retry of the original zero-output/zero-tool failed attempt, under the original budget. Finite automatic recovery is the new approved target, requiring bounded attempts, aggregate cost accounting, effect reconciliation and tests before replacing this implementation contract.
- Other quotas and data limits are recorded in current architecture and responsible contracts; historical NR-3/4 profiles remain separately tested. Capacity exhaustion never authorizes deleting Canon or silently evicting records.

## Authoritative documents

- Current implementation: `docs/architecture.zh-CN.md`, `docs/product-current-state-report.zh-CN.md`.
- Target product/design and interview: `docs/product-spec.zh-CN.md`, `docs/technical-design.zh-CN.md`, `docs/reviews/project-reframe-2026-09-09.zh-CN.md`.
- Delivery/validation: `docs/sprint-plan.zh-CN.md`, `bmad/workflow-status.yaml`, `bmad/sprint-status.yaml`.
- Development/recovery/evidence: `docs/development.md`.
- Existing implementation approval: `docs/reviews/novelist-agent-runtime-nr5-nr6-implementation-approval.zh-CN.md`.
- Approved delivery packet: `docs/stories/novelist-agent-runtime-nr5-nr6-story-packet-draft-1.zh-CN.md`; its engineering contracts and implementation evidence live in `docs/reviews/`.
- Formal user testing: `docs/testing/novelist-agent-runtime-nr6-multi-user-test-plan.zh-CN.md`.
- Historical NR-G5 approval: `docs/reviews/novelist-agent-runtime-nr-g5-product-approval.zh-CN.md`. Historical plans and Gate submissions retain their original dates; they do not override later explicit user decisions.
