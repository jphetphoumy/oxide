# Oxide

## Development

Enter the dev shell before running any command:

```sh
nix develop
```

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, `ci`, `perf`, `build`.

## Testing

### Unit tests

```sh
cargo test
```

### TUI manual testing with tmux

The TUI requires a terminal, so use tmux to test interactively from scripts or agents:

```sh
# Start the app in a detached tmux session
tmux new-session -d -s oxide-test -x 80 -y 24 "cargo run"

# Capture the screen
tmux capture-pane -t oxide-test -p

# Send keystrokes
tmux send-keys -t oxide-test "some text"
tmux send-keys -t oxide-test Enter

# Send special keys
tmux send-keys -t oxide-test C-c    # Ctrl+C (quit)
tmux send-keys -t oxide-test M-Enter # Alt+Enter (newline)

# Clean up
tmux kill-session -t oxide-test
```

Always capture the pane after sending keys (with a short sleep) to verify the UI rendered correctly.

## Clippy

Run clippy in strict mode (pedantic + nursery). All new code must pass with zero warnings:

```sh
cargo clippy -- -W clippy::all -W clippy::pedantic -W clippy::nursery
```

## Pre-commit

Hooks run automatically on commit (`cargo fmt`, `cargo check`, `clippy`). To install manually:

```sh
nix develop --command pre-commit install
```
