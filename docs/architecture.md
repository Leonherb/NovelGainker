# NovelGainker Architecture (NR-1)

NovelGainker is a Windows-first, local-first Tauri 2 application for Chinese novel writing. Its persistent three-column workspace keeps project/files/provider settings on the left, natural conversation in the center, and the real Markdown editor on the right. Creating or opening a project is the only startup requirement.

NR-1 adds long-conversation continuity to the reliable NR-0 conversation path. It still does not load Novelist Skills, read manuscript Canon into model context, call tools, or let AI mutate Markdown. Any later capability requires a separately approved Story Packet and gate.

## Runtime

```text
React UI
  -> 3 typed conversation commands + 1 event
Rust Tauri Host
  -> project/session registry
  -> project-bound Conversation Store (complete visible History)
  -> derived Memory Store + Context assembler
  -> optional one-attempt Compactor -> one-attempt streaming main reply
  -> DeepSeek/OpenAI adapters + Windows CredentialVault
  -> atomic persistence and general safety kernels
```

The production surface contains 24 commands, including exactly three Conversation commands: `conversation_session_load`, `conversation_turn_start`, and `conversation_turn_cancel`. Streaming uses `conversation-turn-event`. Manifest, handler, generated permission, main-window capability, and all production TypeScript clients must expose identical command sets.

## History, Memory, and context

Visible conversation is stored at `.novelgainker/conversation/session.json` and remains the complete factual source. The Host persists the user message and `running` before any Provider attempt. Stream deltas remain in memory; the complete visible result is persisted at terminal state. A leftover `running` becomes `本次回复未生成。` on reopen and is never retried automatically.

Derived Memory is stored separately at `.novelgainker/memory/state.json`. It binds to project/session, a continuous UTF-8-safe Segment Cursor interval, and a hash of the covered History bytes. Missing, corrupt, mismatched, or stale Memory is discarded without changing History or manuscript files. Memory is not Canon and is never rendered as a Conversation message.

Before a main reply, the Host assembles fixed system instructions, valid Memory, the newest exact History after Memory, and the current user message exactly once. Recent exact History wins over a conflicting summary. Failed or cancelled turns contribute only actual nonempty output and its incomplete state; generated placeholder text does not become semantic Memory.

## Provider path and frozen profile

Below the 64 KiB context trigger, a turn makes zero Compactor attempts and at most one streaming main attempt. At or above the trigger, it may make one non-streaming Compactor attempt before one streaming main attempt. Both primitives are cancellable in flight, perform exactly one outbound HTTP attempt, and never retry. An ordinary Compactor failure is invisible and falls back to the last valid Memory plus bounded recent History; cancellation stops the entire turn before main starts.

The NR-G2 technical submission freezes a 24 KiB exact tail, 6 KiB segments, 72 KiB Compactor input, 8 KiB Backfill reserve, 12 KiB Memory summary, 1,024 Compactor output tokens, 4,096 main output tokens, and a 5,120 cumulative requested-output ceiling. Provider hard limits remain 64 messages, 96 KiB per request, 32 KiB per input message, and 8 MiB per response.

## Conversation reconciliation and close

Events are a low-latency path; the Store is authoritative. Sending is disabled until the listener is ready. While a Turn is active, one non-overlapping reconciliation reads the Store every 1000ms, and terminal events trigger an immediate read. Lost, delayed, duplicated, or reordered events cannot cause a second Provider turn or a permanently active UI.

A raw window-close request only checks unsaved manual Markdown and offers `保存 / 放弃 / 取消`. Committed close destroys the window immediately without waiting for project cleanup, Compactor, main reply, or Conversation flush. Window destruction cancels the shared token; persistence recovery handles a possible leftover `running` on the next open. A partial Compactor result is never committed.

## Project metadata clean slate

Current Project Metadata uses the generic `schemaVersion = 2`. A v1 project is disposable retired application state and is reset exactly once under writer-lease, canonical-root, and recursive reparse checks. Manuscript Markdown, materials, assets, Provider settings, UI preferences, unrelated projects, and Windows credentials remain unchanged.

## Gates

NR-G1 is product-approved. NR-101 through NR-106 are implemented and currently stopped at `NR-G2-technical-submission`; `nrG2Approved = false`. The NR-G2 gate combines local Loopback state-machine tests, long-history/Segment tests, frozen budget checks, the existing release/IPC closure, and full `pnpm quality`. It does not claim real Provider summary semantics: no real Provider Live Smoke was run, and NR-2 is not authorized.
