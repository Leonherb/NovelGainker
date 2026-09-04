# NovelGainker Architecture (NR-2)

NovelGainker is a Windows-first, local-first Tauri 2 application for Chinese novel writing. Its persistent three-column workspace keeps project/files/provider settings on the left, natural conversation in the center, and the real Markdown editor on the right. Creating or opening a project is the only startup requirement.

NR-2 adds a bounded Host Agent Loop, Provider-native Tool Request/Result, an independent Task Store, and inline Task Activity projection to the NR-1 long-conversation baseline. The production Tool Registry remains empty, so the current release still does not execute tools, load Novelist Skills, read manuscript Canon into model context, or let AI mutate Markdown.

## Runtime

```text
React UI
  -> 3 typed conversation commands + 1 event
Rust Tauri Host
  -> project/session registry
  -> project-bound Conversation Store (complete visible History)
  -> derived Memory Store + Context assembler
  -> optional one-attempt Compactor
  -> Agent Attempt 1 -> zero-or-one Tool -> zero-or-one Agent Attempt 2
  -> empty production Tool Registry + independent Task Store
  -> DeepSeek/OpenAI Provider-native adapters + Windows CredentialVault
  -> atomic persistence and general safety kernels
```

The production surface contains 24 commands, including exactly three Conversation commands: `conversation_session_load`, `conversation_turn_start`, and `conversation_turn_cancel`. Streaming uses `conversation-turn-event`. Manifest, handler, generated permission, main-window capability, and all production TypeScript clients must expose identical command sets.

## History, Memory, and context

Visible conversation is stored at `.novelgainker/conversation/session.json` and remains the complete factual source. The Host persists the user message and `running` before any Provider attempt. Stream deltas remain in memory; the complete visible result is persisted at terminal state. A leftover `running` becomes `本次回复未生成。` on reopen and is never retried automatically.

Derived Memory is stored separately at `.novelgainker/memory/state.json`. It binds to project/session, a continuous UTF-8-safe Segment Cursor interval, and a hash of the covered History bytes. Missing, corrupt, mismatched, or stale Memory is discarded without changing History or manuscript files. Memory is not Canon and is never rendered as a Conversation message.

Independent Task state is stored at `.novelgainker/tasks/session.json`. It contains only bounded hashes, byte counts, controlled labels, stable error codes, status, and monotonic revisions—never raw arguments, results, Provider Continuation, credentials, or absolute paths. Tool protocol data does not become visible Conversation or Memory.

Before a main reply, the Host assembles fixed system instructions, valid Memory, the newest exact History after Memory, and the current user message exactly once. Recent exact History wins over a conflicting summary. Failed or cancelled turns contribute only actual nonempty output and its incomplete state; generated placeholder text does not become semantic Memory.

## Provider path and frozen profile

Below the 64 KiB context trigger, a no-tool turn makes zero Compactor attempts and at most one streaming main attempt. At or above the trigger, it may make one non-streaming Compactor attempt before the main attempt. With an eligible test Tool Registry, the main path allows one Tool execution and one continuation attempt. Each primitive performs exactly one outbound HTTP attempt and never retries; the per-turn ceiling is three Provider attempts including Compactor.

The frozen profile keeps a 24 KiB exact tail, 6 KiB segments, 72 KiB Compactor input, 8 KiB Backfill reserve, 12 KiB Memory summary, 1,024 Compactor output tokens, and 4,096 tokens for each of at most two Agent attempts. The maximum cumulative requested output is 9,216 tokens. Provider hard limits remain 64 messages, 96 KiB per request, 32 KiB per input message, and 8 MiB per response.

## Conversation reconciliation and close

Events are a low-latency path; the Store is authoritative. Sending is disabled until the listener is ready. While a Turn is active, one non-overlapping reconciliation reads the Store every 1000ms, and terminal events trigger an immediate read. Lost, delayed, duplicated, or reordered events cannot cause a second Provider turn or a permanently active UI.

A raw window-close request only checks unsaved manual Markdown and offers `保存 / 放弃 / 取消`. Committed close destroys the window immediately without waiting for project cleanup, Compactor, main reply, or Conversation flush. Window destruction cancels the shared token; persistence recovery handles a possible leftover `running` on the next open. A partial Compactor result is never committed.

## Project metadata clean slate

Current Project Metadata uses the generic `schemaVersion = 2`. A v1 project is disposable retired application state and is reset exactly once under writer-lease, canonical-root, and recursive reparse checks. Manuscript Markdown, materials, assets, Provider settings, UI preferences, unrelated projects, and Windows credentials remain unchanged.

## Gates

NR-G1, NR-G2, and NR-G3 are product-approved. NR-201 through NR-206 and NR-G3-R01 through NR-G3-R03 are implemented, and the runtime remains stopped at the passed NR-G3 Gate. NR3-D2-R01 through NR3-D2-R06 and FR01 through FR02 are approved in the NR-3 Story Packet Draft 2 Final, and the workflow is stopped at the Packet approval Gate. NR-301 through NR-306 are not authorized for implementation. The production Registry therefore remains empty and no Skill or Canon-read capability is active. No real Provider Live Smoke was run, and no NR-4 through NR-7 Story Packet has been formed.
