---
name: code-quality
description: Run comprehensive parallel code quality review using 6 specialized agents
argument-hint: "[--model sonnet|opus|haiku] [scope: path or 'full']"
---

# Parallel Code Quality Review

Launch 6 specialized analysis agents in parallel on the codebase. Each agent
works independently with zero inter-agent communication.

> **Note:** `--model` controls which Claude model runs the agents. Effort level
> (reasoning depth) is session-level only — set it via `/model` before invoking.

## Execution steps

1. **Parse arguments** from $ARGUMENTS:
   - Extract `--model <value>` if present (valid values: `sonnet`, `opus`, `haiku`). Default to `sonnet` if omitted.
   - Everything else is the **scope** (path or empty for full codebase).
   - Example: `--model opus crates/ghw` → model=opus, scope=crates/ghw
   - Example: `crates/ghw` → model=sonnet, scope=crates/ghw
   - Example: (empty) → model=sonnet, scope=full codebase

2. **Launch ALL 6 agents in parallel** using the Task tool with `run_in_background: true`. Use `subagent_type` matching each agent name and `model` set to the parsed value from step 1:

   For each agent, use the Task tool with:
   - `subagent_type`: the agent name (e.g., `arch-analyzer`)
   - `prompt`: "Analyze [scope]. Focus on your specialized dimension. Read-only, no modifications."
   - `run_in_background: true`

   The 6 agents to launch:
   - `arch-analyzer` — architecture, module coupling, data flow
   - `bug-reviewer` — bugs, security vulnerabilities, performance
   - `error-hunter` — silent failures, swallowed errors, missing propagation
   - `type-analyzer` — type design, encapsulation, invariants
   - `test-analyzer` — test coverage gaps, missing edge cases
   - `comment-reviewer` — comment accuracy, stale documentation

3. **Wait for all 6 agents to complete** by reading their results.

4. **Deduplicate findings** — if multiple agents flag the same file:line for the same issue, merge into one finding and note which agents flagged it.

5. **Present unified report** in this format:

   ## Summary
   | Dimension | Critical | High | Medium | Low | Total |
   |-----------|----------|------|--------|-----|-------|
   | Architecture | ... | ... | ... | ... | ... |
   | Bugs | ... | ... | ... | ... | ... |
   | Error Handling | ... | ... | ... | ... | ... |
   | Type Design | ... | ... | ... | ... | ... |
   | Test Coverage | ... | ... | ... | ... | ... |
   | Comments | ... | ... | ... | ... | ... |
   | **Total** | ... | ... | ... | ... | ... |

   ## Top Priority Findings
   Numbered list of CRITICAL and HIGH findings, deduplicated, highest severity first.
   Each with file:line, description, and which agent(s) flagged it.

   ## Per-Agent Detailed Reports
   One collapsible section per agent with their full findings.

   ## Recommended Next Steps
   Prioritized list of suggested actions. No code modifications — observation only.
