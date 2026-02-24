# Execution Strategy: `glw` — GitLab CI Watcher TUI

## Problem

The implementation plan is ~700 lines spanning 6 phases, ~5,700 lines of Rust code, ~30 file moves/modifications. Executing in a single context window degrades quality as earlier diffs fill context and later phases get worse attention.

## Solution: Phased Worker Agents with Verification Gates

The coordinator (main conversation) stays lean — it only dispatches workers and runs build/test verification between phases. Each worker agent gets a **focused prompt** with only its relevant plan section + file paths, starting with a fresh context window.

## Agent Breakdown

| Agent | Phases | Focus | Key Challenge | Verification |
|-------|--------|-------|---------------|-------------|
| **Worker 1** | 1 + 2 + 3 | Workspace refactor: create crates, move files, wire traits, adapt ghw | Must be atomic — build breaks mid-way if split | `cargo build --workspace && cargo test --workspace` |
| **Worker 2** | 4 | Implement glw: cli, executor, parser, main | New code only — needs traits.rs + ghw/main.rs as reference | `cargo build -p glw` |
| **Worker 3** | 5 + 6 | Tests + build system: move tests, add glw tests, update Makefile/CI | Lightweight cleanup | `cargo test --workspace && cargo clippy --workspace` |

## Why Phases 1+2+3 are One Agent

These phases form an **atomic refactoring unit**. You cannot:
- Move `src/app.rs` to `crates/ciw-core/` without also updating `ghw/main.rs` imports (build breaks)
- Create trait objects in `poller.rs` without `GhExecutor` impl existing (build breaks)
- The build must compile at the end of the agent's work, not midway

Splitting further would require incomplete/broken intermediate states, which means the next agent can't verify its starting point.

## Execution Flow

```
Coordinator                Worker 1                Worker 2              Worker 3
    │                         │                        │                     │
    ├──spawn(Phase 1+2+3)────►│                        │                     │
    │                         ├─ create workspace      │                     │
    │                         ├─ move files to ciw-core│                     │
    │                         ├─ create traits.rs      │                     │
    │                         ├─ wire poller/startup   │                     │
    │                         ├─ wrap GhExecutor       │                     │
    │                         ├─ adapt ghw/main.rs     │                     │
    │◄─────────done───────────┤                        │                     │
    │                         │                        │                     │
    ├─ cargo build --workspace│                        │                     │
    ├─ cargo test --workspace │                        │                     │
    │                         │                        │                     │
    ├──spawn(Phase 4)─────────┼───────────────────────►│                     │
    │                         │                        ├─ write glw/cli.rs   │
    │                         │                        ├─ write executor.rs  │
    │                         │                        ├─ write parser.rs    │
    │                         │                        ├─ write main.rs      │
    │◄────────────────────────┼──────────done──────────┤                     │
    │                         │                        │                     │
    ├─ cargo build --workspace│                        │                     │
    │                         │                        │                     │
    ├──spawn(Phase 5+6)───────┼────────────────────────┼────────────────────►│
    │                         │                        │                     ├─ move tests
    │                         │                        │                     ├─ add glw tests
    │                         │                        │                     ├─ update Makefile
    │                         │                        │                     ├─ update CI
    │◄────────────────────────┼────────────────────────┼───────────done──────┤
    │                         │                        │                     │
    ├─ cargo test --workspace │                        │                     │
    ├─ cargo clippy --workspace                        │                     │
    ├─ cargo fmt --all --check│                        │                     │
    │                         │                        │                     │
    └─ DONE                   │                        │                     │
```

## Context Budget Per Agent

| Agent | Reads | Writes/Edits | Estimated context usage |
|-------|-------|-------------|------------------------|
| Worker 1 | ~5,700 lines (all src files) | ~5,700 lines (moved/modified) | ~70% of window |
| Worker 2 | ~500 lines (traits.rs, platform.rs, ghw/main.rs as ref) | ~1,200 lines (4 new files) | ~30% of window |
| Worker 3 | ~400 lines (test files, Makefile, CI) | ~600 lines | ~20% of window |

Worker 1 is the tightest — it needs to read almost everything and rewrite it. But because it starts with a clean context (no prior conversation), it has the full window available. Workers 2 and 3 are comfortable.

## Error Recovery

If any agent fails (build breaks), the coordinator:
1. Reads the error output
2. Spawns a **fix-up agent** with only the error message + relevant files
3. The fix-up agent has a fresh context focused on just the broken parts

This is strictly better than having a bloated context try to debug itself.

## Worker 1 Prompt Summary

The prompt for Worker 1 should include:
- The full plan Sections 1-3 (workspace structure, ciw-core extraction, ghw adaptation)
- The architecture diagram showing target file layout
- The trait definitions (CiExecutor, CiParser) verbatim
- The PlatformConfig struct
- The coupling points table (which lines in main.rs reference gh::executor/parser)
- Instruction to run `cargo build --workspace && cargo test --workspace` at the end

## Worker 2 Prompt Summary

The prompt for Worker 2 should include:
- Plan Section 4 only (implement glw)
- Instruction to read `crates/ciw-core/src/traits.rs` and `crates/ciw-core/src/platform.rs` first
- Instruction to read `crates/ghw/src/main.rs` as reference for the event loop
- The glab CLI command mapping table
- The GitLab JSON response formats and field mappings
- The status mapping table
- Instruction to run `cargo build -p glw` at the end

## Worker 3 Prompt Summary

The prompt for Worker 3 should include:
- Plan Sections 5-6 (testing + build system)
- The Makefile template
- The CI workflow changes needed
- Instruction to run `cargo test --workspace && cargo clippy --workspace` at the end
