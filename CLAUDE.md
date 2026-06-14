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
├── handler.rs           # Key events → Action enum, slash command dispatch
├── event.rs             # Async terminal event reader (keys + tick)
├── input_buffer.rs      # UTF-8 aware text buffer with cursor tracking
├── observability.rs     # tracing-appender log setup
├── slash.rs             # Slash command registry and tab completion
├── auth/
│   ├── mod.rs           # Public API: logout(), status()
│   ├── device_flow.rs   # WorkOS OAuth device code flow
│   ├── jwt.rs           # JWT claim extraction (no verification)
│   ├── token_storage.rs # System keyring (secure token persistence)
│   ├── token_refresh.rs # Token expiry check + auto-refresh
│   └── workspace_selection.rs  # Workspace prompt after login
├── dust/
│   ├── mod.rs           # Module exports
│   ├── client.rs        # Dust HTTP API (conversations, messages, agents, SSE streaming)
│   ├── stream.rs        # Server-Sent Event parser → StreamEvent enum
│   └── types.rs         # Request/response DTOs (serde)
└── ui/
    ├── mod.rs           # Module exports
    ├── layout.rs        # Ratatui 4-row layout (title, messages, input, status)
    ├── messages.rs      # Message rendering with role-based colors + word wrap
    ├── input.rs         # Input box with cursor, placeholder, soft wrap
    ├── command_menu.rs  # Inline slash command menu (render-time only)
    └── picker.rs        # Modal agent selection overlay with filtering
```

### Data flow

1. `event.rs` reads terminal events in a background task
2. `handler.rs` maps key events to `Action` variants
3. `main.rs` applies actions to `App` state
4. `app.rs` manages messages, streaming state, and mode transitions
5. `dust/client.rs` sends HTTP requests, opens SSE streams
6. `dust/stream.rs` parses SSE tokens, sends them via MPSC channel
7. `ui/layout.rs` renders the current `App` state each frame

### Key design decisions

Architectural Decision Records live in [`docs/adr/`](docs/adr/). Key ones:

- [ADR-0001](docs/adr/0001-use-rust.md) — Rust for performance + single binary
- [ADR-0005](docs/adr/0005-use-ratatui-for-tui.md) — Ratatui for TUI rendering
- [ADR-0006](docs/adr/0006-match-dust-cli-http-headers.md) — Match official Dust CLI headers
- [ADR-0008](docs/adr/0008-slash-commands-inline-menu.md) — Slash commands as inline menu + mode transitions

## Subagents

Project-specific subagents are available to parallelize and automate development workflows:

| Subagent | Model | When to use |
|----------|-------|-------------|
| `oxide-planner` | Sonnet | Planning features from issues. Spawns both explorers in parallel to gather oxide + dust context, generates comprehensive implementation plan. Use proactively when starting feature work. |
| `oxide-codebase-explorer` | Haiku | Understanding oxide architecture, finding patterns, locating implementations. Spawned by planner during planning phase. |
| `oxide-dust-codebase-explorer` | Haiku | Exploring dust codebase for API contracts and patterns. Spawned by planner during planning phase. |
| `oxide-implementer` | Haiku | Implementing features from a plan. Writes code, runs tests, commits when done, automatically spawns reviewer. |
| `oxide-reviewer` | Sonnet | Reviewing implementation against a plan. Spawns explorer if needed for architectural verification, returns structured feedback. |

### Typical workflow

1. **Planning**: You mention an issue or feature → `oxide-planner` spawned
   - Planner spawns both explorers in parallel (oxide + dust context)
   - Planner generates comprehensive HTML plan
   - Plan returns to main thread for your review

2. **Implementation**: You approve plan → `oxide-implementer` spawned
   - Checks `feedback.md` if it exists (knows what needs fixing)
   - Writes code, runs oxide-check, commits
   - Automatically spawns `oxide-reviewer`

3. **Review Loop**: `oxide-reviewer` analyzes code
   - Reads previous `feedback.md` to track what's been fixed
   - Creates/updates `feedback.md` with findings
   - Returns verdict: ✅ Ready to merge OR 🔄 Needs revision
   - If needs revision → Implementer reads feedback.md, fixes issues, commits, reviewer re-reviews
   - Loop continues until ✅ Ready to merge

4. **Cleanup**: When code is approved
   - Main thread commits the final code
   - Deletes `feedback.md` (cleanup before merge)
   - Feature is complete

**Key optimization**: `feedback.md` persists between review iterations, so implementer and reviewer don't waste tokens re-explaining old issues. Context accumulates instead of getting lost.

No manual subagent triggering needed — use natural language and subagents spawn as appropriate.

## Development

### Prerequisites

Enter the Nix dev shell before running any command:

```sh
nix develop
```

This provides: Rust stable, rust-analyzer, pkg-config, dbus, openssl, pre-commit.

### Commits

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, `ci`, `perf`, `build`.

### Pre-commit hooks

Hooks run automatically on commit (`cargo fmt`, `cargo check`, `clippy`). To install manually:

```sh
nix develop --command pre-commit install
```

## Testing

### Unit tests

```sh
cargo test
```

### Clippy (strict)

All code must pass with zero warnings:

```sh
cargo clippy -- -W clippy::all -W clippy::pedantic -W clippy::nursery
```

### TUI manual testing with tmux

The TUI requires a real terminal. Use tmux to test headlessly:

```sh
# Start the app in a detached tmux session
tmux new-session -d -s oxide-test -x 80 -y 24 "cargo run"

# Wait for startup, then capture the screen
sleep 2
tmux capture-pane -t oxide-test -p

# Send keystrokes and verify
tmux send-keys -t oxide-test "some text"
sleep 1
tmux capture-pane -t oxide-test -p

# Special keys
tmux send-keys -t oxide-test Enter      # Submit
tmux send-keys -t oxide-test M-Enter    # Newline
tmux send-keys -t oxide-test C-c        # Quit

# Always clean up
tmux kill-session -t oxide-test
```

Always capture the pane after sending keys (with a short sleep) to verify rendering.

## Config and Auth

- **Config file**: `~/.config/oxide/config.toml` — sets default agent, overridable via `OXIDE_AGENT_ID` env var
- **Tokens**: Stored in system keyring (Linux/macOS/Windows) via the `keyring` crate
- **Environment variables**: `OXIDE_AGENT_ID`, `OXIDE_USERNAME`, `OXIDE_EMAIL`, `OXIDE_FULL_NAME`, `TZ`

## Cargo.toml Lint Policy

Strict clippy is enforced in `Cargo.toml` under `[lints.clippy]`:

- `all` and `pedantic` = deny
- `nursery` = warn
- `unwrap_used`, `expect_used`, `panic` = deny (use `anyhow` for error handling)
- Allowed exceptions: `cast_possible_truncation`, `module_name_repetitions`
