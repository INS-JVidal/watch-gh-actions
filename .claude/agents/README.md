# Code Quality Analysis Agents

Six specialized read-only analysis agents for comprehensive code review.

## Agents

| Agent | Focus | Key checks |
|-------|-------|------------|
| `arch-analyzer` | Architecture & coupling | Module deps, data flow, abstraction quality, state management |
| `bug-reviewer` | Bugs & code quality | Logic errors, security, performance, unsafe patterns |
| `error-hunter` | Silent failures | Swallowed errors, bad fallbacks, missing propagation |
| `type-analyzer` | Type design | Encapsulation, invariants, illegal states, enforcement |
| `test-analyzer` | Test coverage gaps | Missing tests, weak assertions, edge cases |
| `comment-reviewer` | Comment accuracy | Stale docs, misleading comments, TODO markers |

## Usage

### Run all agents (recommended)

```
/code-quality                              # sonnet, full codebase
/code-quality crates/ghw                   # sonnet, scoped to path
/code-quality --model opus crates/ghw      # opus, scoped to path
/code-quality --model haiku                # haiku, full codebase
```

### Run a single agent

Reference by name in the Task tool:
```
Task: arch-analyzer — Analyze crates/ciw-core/src/app.rs
```

## Design

- All agents are **read-only** (tools: Read, Grep, Glob)
- Default model is **sonnet**; override with `--model opus|haiku` via `/code-quality`
- Zero inter-agent communication — each dimension is independent
- Output is severity-rated findings with `file:line` references
