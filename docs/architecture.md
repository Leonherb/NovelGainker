# NovelGainker Architecture (NR-0)

NovelGainker is a Windows-first, local-first Tauri 2 application for Chinese novel writing. Its persistent three-column workspace keeps project/files/provider settings on the left, natural conversation in the center, and the real Markdown editor on the right. Creating or opening a project is the only startup requirement.

NR-0 provides one reliable plain-text conversation path. It does not load Novelist Skills, read manuscript Canon into model context, call tools, or let AI mutate Markdown. Any later capability requires a separately approved Story Packet and gate.

## Runtime

```text
React UI
  -> 3 typed conversation commands + 1 event
Rust Tauri Host
  -> project/session registry
  -> project-bound Conversation Store
  -> DeepSeek/OpenAI text adapters
  -> Windows CredentialVault
  -> atomic persistence and general safety kernels
```

The production surface contains 24 commands, including exactly three Conversation commands: `conversation_session_load`, `conversation_turn_start`, and `conversation_turn_cancel`. Streaming uses `conversation-turn-event`. Manifest, handler, generated permission, main-window capability, and all production TypeScript clients must expose identical command sets.

## Conversation persistence and reconciliation

Visible conversation is stored at `.novelgainker/conversation/session.json`. The Store contains stable project/session/turn/message identifiers, ordering, visible text, timestamps, provider/model metadata, and terminal status. It never stores credentials, hidden prompts, raw Provider payloads, absolute paths, or future capability state.

The Host persists the user message and `running` before invoking a Provider. Stream deltas remain in memory; the complete visible result is persisted at terminal state. A leftover `running` becomes `本次回复未生成。` on reopen and is never retried automatically.

Events are a low-latency path; the Store is authoritative. Sending is disabled until the listener is ready. While a Turn is active, one non-overlapping reconciliation reads the Store every 1000ms, and terminal events trigger an immediate read. Lost, delayed, duplicated, or reordered events cannot cause a second Provider call or a permanently active UI. Listener or Store failures surface as a synchronization error and retry only listener setup or Store loading.

## Provider path

DeepSeek and OpenAI share one text request model and terminalization rule. Each Turn performs exactly one Provider call and zero Skill, Tool, or Canon operations. Automated evidence uses local loopbacks with no real credentials or external requests. Production borrows the configured Windows credential only inside Rust after an explicit user send.

## Project Metadata clean slate

Current Project Metadata uses the generic `schemaVersion = 2`. Before strict parsing, the Host reads a bounded envelope. v2 opens normally; missing Metadata follows the existing confirmed-import path; corrupt, missing, or unknown schema fails without deletion.

v1 identifies disposable retired application state. With a writer lease, a canonical project root, and no symlink/reparse point below `.novelgainker`, the Host replaces that exact local state with a prepared v2 Metadata state and a new Project ID. Failure cannot leave a valid mixed v1/v2 state. A successful second open keeps the new Project ID and Conversation.

One App State transaction removes old `recent_projects` and `project_files` rows associated with the same canonical root before recording the new ID. Provider settings, UI preferences, unrelated projects, Windows credentials, manuscript Markdown, materials, and assets remain unchanged.

## Close lifecycle

A raw window-close request only checks unsaved manual Markdown and offers `保存 / 放弃 / 取消`. Committed close destroys the window immediately without waiting for project cleanup, Provider completion, or Conversation flush. Window destruction cancels the active request; persistence recovery handles a possible leftover `running` on the next open.

## Release gate

The NR-G1 gate recursively discovers production TS/TSX, Tauri/Rust sources, current contracts, and explicitly configured production resources. It excludes tests and historical documents from retired-product reference counts, compares all five IPC surfaces, consumes upstream test evidence, checks the current report claims, and proves with a temporary production file that a newly introduced retired entry is rejected.

NR-G1 is product-approved. NR-1 Story Packet Draft 1 exists for review only: memory, compaction, and long-conversation context are not implemented, and all skill, tool, and file-operation work remains in later stages. Real Provider Live Smoke was not run.
