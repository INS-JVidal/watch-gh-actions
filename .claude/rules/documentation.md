# Documentation Style

## Core principle
Document what code cannot express. If it can be inferred from reading the code, don't write it.

## Always document
- **Constant value rationale**: why this number, not just what it is
  - Good: `/// GitHub updates ~every 2s; 3s catches most transitions in one cycle.`
  - Bad: `/// Active polling interval in seconds.`
- **Non-obvious Option/enum semantics**: when None vs Some has domain meaning
  - Example: `jobs: Option<Vec<Job>>` — None=not fetched, Some(vec)=fetched
- **Trade-off decisions**: why this approach over the rejected alternative
- **Module-level `//!` docs**: only where the "why" isn't obvious from code

## Never document
- Self-evident names: `Action::Quit`, `RunStatus::Completed`
- Struct fields with obvious types: `pub name: String`
- Functions whose signature tells the whole story
- `# Examples` sections — test suite demonstrates usage

## See also
- `plans/ai-documentation-rationale.md` for full philosophy
- CLAUDE.md for architecture-level decisions
