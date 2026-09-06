# NovelGainker architecture

The authoritative current architecture is [当前架构（NR-4）](architecture.zh-CN.md).

NovelGainker is a Windows-first, local-first Tauri application with a persistent three-column workspace. The NR-4 implementation embeds Novelist projection 1.1.0 (9 capabilities, 37 resources), a bounded read/write Agent Loop, durable Canon change receipts, local Diff/confirmation, and recoverable Undo.

A write-enabled project registers two concrete tools: `novelist.read_context` and `novelist.propose_change`. Only characters/world/materials/plot/outlines can be changed. Conversation remains available without tool selection. Source intent is model-driven; Host validates provenance, hashes, permissions and transactions, not semantics.

NR-401–406 have passed their individual complete quality gates and NR-G5 is product-approved. The remaining plan is NR-5–6 (12 Stories), ending at NR-G7; proposed contracts await product decisions. Cloud sync is not a user requirement and has been removed entirely, not deferred. No real Provider Live Smoke or cloud execution is authorized. Current state lives in [sprint-plan](sprint-plan.zh-CN.md), not the frozen historical Gate manifests.
