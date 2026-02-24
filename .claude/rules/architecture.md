# Architecture Rules

## Trait boundary (CiExecutor / CiParser)
- `CiExecutor`: async, `&self` only (immutable), returns raw JSON strings
- `CiParser`: sync (CPU-bound, no I/O), returns typed models (`Vec<WorkflowRun>`, `Vec<Job>`)
- Library sees only `Arc<dyn CiExecutor>` / `Arc<dyn CiParser>` — never concrete types
- Parsing stays in the parser — executor never deserializes JSON
- This separation enables independent testing and size validation before deserialization

## State mutation
- All state lives in `AppState`, mutated ONLY in `run_app()` event loop on main thread
- Background tasks communicate via `mpsc::UnboundedSender<AppEvent>` — never mutate state directly
- No `Arc<Mutex<AppState>>` — ever. The event loop is the single writer.

## Tree model
- `tree_items: Vec<TreeItem>` rebuilt from scratch via `rebuild_tree()` after any state change
- TreeItem stores indices into `runs` vec, not cloned data — invalidated by mutation
- Always call `rebuild_tree()` after modifying: `runs`, `expanded_runs`, `expanded_jobs`, `filter`
- Never incrementally patch tree items

## Platform code placement
- Trait definitions and shared logic → `ciw-core` (library crate)
- Platform implementations → binary crates (`ghw`, `glw`)
- Never put GitHub-specific or GitLab-specific logic in ciw-core
- `spawn_monitored` lives in each binary's main.rs (coupled to the event loop wiring)
