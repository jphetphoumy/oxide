# Oxide

Rust TUI client for [Dust](https://dust.tt) agents. Chat with AI agents from your terminal.

## Quick Reference

| Command | Purpose |
|---------|---------|
| `nix develop` | Enter dev shell (required before any command) |
| `cargo run` | Run the TUI |
| `cargo run -- login` | Authenticate via browser |
| `cargo test` | Run unit tests |
| `cargo clippy -- -W clippy::all -W clippy::pedantic -W clippy::nursery` | Lint (strict) |
| `cargo fmt` | Format code |

## Architecture

```
src/
├── main.rs              # Entry point: CLI routing + TUI event loop (tokio::select!)
├── app.rs               # App state machine (Chat / Picker modes), message buffer
├── cli.rs               # CLI arg parsing via clap (login, logout, status)
├── config.rs            # TOML config loader (~/.config/oxide/config.toml)
├── handler.rs           # Key events -> Action enum, slash command dispatch
├── event.rs             # Async terminal event reader (keys + tick)
├── input_buffer.rs      # UTF-8 aware text buffer with cursor tracking
├── observability.rs     # tracing-appender log setup
├── slash.rs             # Slash command registry and tab completion
├── auth/                # Device flow, JWT, token storage, token refresh
├── dust/                # Dust HTTP API, SSE stream parser, DTOs
└── ui/                  # Ratatui layout, messages, input, command menu, picker
```

Data flow:

1. `event.rs` reads terminal events in a background task.
2. `handler.rs` maps key events to `Action` variants.
3. `main.rs` applies actions to `App` state.
4. `app.rs` manages messages, streaming state, and mode transitions.
5. `dust/client.rs` sends HTTP requests and opens SSE streams.
6. `dust/stream.rs` parses SSE tokens and sends them via MPSC.
7. `ui/layout.rs` renders the current `App` state each frame.

## Codex Subagents

Project-scoped Codex custom agents live in `.codex/agents/`.

| Agent | Model | When to use |
|-------|-------|-------------|
| `oxide-planner` | `gpt-5.4` | Planning features from issues or descriptions. Spawns both explorers in parallel to gather oxide and Dust context, then generates a concrete implementation plan. |
| `oxide-codebase-explorer` | `gpt-5.4-mini` | Cheap read-only exploration of oxide architecture, patterns, and implementation locations. |
| `oxide-dust-codebase-explorer` | `gpt-5.4-mini` | Cheap read-only exploration of Dust API contracts, docs, and examples relevant to oxide. |
| `oxide-implementer` | `gpt-5.4-mini` | Implementing features from a plan, reading `feedback.md`, running checks, and reporting changed files. |
| `oxide-reviewer` | `gpt-5.4` | Reviewing implementation against a plan, maintaining `feedback.md`, and deciding ready vs revision. |
| `oxide-developer` | `gpt-5.4-mini` | Focused Rust/TUI development tasks that do not need the full plan-review loop. |

Use these agents proactively from natural language. The user should not have to name a subagent explicitly.

This section is the user's standing project instruction to use Codex subagents for the workflows below. When a request matches one of these triggers, treat it as explicit authorization to spawn the matching project-scoped custom agent from `.codex/agents/`. Do not merely emulate the workflow in the main thread unless the user asks you not to spawn agents.

Mandatory routing:

- Planning trigger: if the user mentions an issue, PR, feature, or asks for a plan, spawn `oxide-planner`.
- Exploration trigger: when planning needs oxide architecture context, spawn `oxide-codebase-explorer`.
- Dust API trigger: when planning or implementation may touch Dust API behavior, spawn `oxide-dust-codebase-explorer`.
- Implementation trigger: if the user approves a plan or asks to implement/fix, spawn `oxide-implementer` for the code-writing phase.
- Review trigger: after implementation or when the user asks for review, spawn `oxide-reviewer`.
- Small-task trigger: for a small direct Rust/TUI fix that does not need a full plan-review loop, spawn `oxide-developer`.

If a task matches a mandatory route, first state which project agent is being spawned, then spawn it. Only keep the work in the main thread when the task is trivial, the user explicitly asks not to use subagents, or spawning is unavailable.

Examples:

- "Plan issue 23" -> spawn `oxide-planner`.
- "Add /help slash command" -> spawn `oxide-planner` first, then wait for plan approval before implementation unless the user asked to implement directly.
- "Implement this plan" -> spawn `oxide-implementer`.
- "Review this branch" -> spawn `oxide-reviewer`.
- "Fix this small clippy warning" -> spawn `oxide-developer`.

### Typical Workflow

1. Planning: user mentions an issue or feature.
   - Spawn `oxide-planner`.
   - Planner gathers oxide and Dust context, spawning explorers in parallel when useful.
   - Planner writes or updates a plan under `docs/plan/` when the work is substantial.

2. Implementation: user approves a plan or asks for implementation.
   - Spawn `oxide-implementer`.
   - It reads `feedback.md` if present.
   - It edits focused files, runs checks, and summarizes changed files.

3. Review loop:
   - Spawn `oxide-reviewer`.
   - Reviewer creates or updates `feedback.md`.
   - Verdict is `READY TO MERGE` or `NEEDS REVISION`.
   - If revision is needed, spawn `oxide-implementer` again.
   - Continue until `READY TO MERGE`.

4. Cleanup:
   - Delete `feedback.md` only after code is approved and the feedback record is no longer needed.

## Development

Enter the Nix dev shell before running project commands:

```sh
nix develop
```

Use Conventional Commits:

```text
<type>(<scope>): <description>
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, `ci`, `perf`, `build`.

## Testing

Run focused tests when possible, then run the full suite before committing:

```sh
cargo fmt -- --check
cargo check
cargo clippy -- -W clippy::all -W clippy::pedantic -W clippy::nursery
cargo test
```

The TUI requires a real terminal. Use tmux for headless manual testing:

```sh
tmux new-session -d -s oxide-test -x 80 -y 24 "cargo run"
sleep 2
tmux capture-pane -t oxide-test -p
tmux send-keys -t oxide-test "some text"
sleep 1
tmux capture-pane -t oxide-test -p
tmux send-keys -t oxide-test C-c
tmux kill-session -t oxide-test
```

Always capture the pane after sending keys, with a short sleep, to verify rendering.

## Config and Auth

- Config file: `~/.config/oxide/config.toml`
- Tokens: stored in system keyring via the `keyring` crate
- Environment variables: `OXIDE_AGENT_ID`, `OXIDE_USERNAME`, `OXIDE_EMAIL`, `OXIDE_FULL_NAME`, `TZ`

## Cargo.toml Lint Policy

Strict clippy is enforced in `Cargo.toml` under `[lints.clippy]`:

- `all` and `pedantic` = deny
- `nursery` = warn
- `unwrap_used`, `expect_used`, `panic` = deny
- Allowed exceptions: `cast_possible_truncation`, `module_name_repetitions`
