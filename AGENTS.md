# Agent Guidelines

## Environment

Always run commands inside the Nix dev shell:

```sh
nix develop
```

## Commit Convention

All commits **must** follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, `ci`, `perf`, `build`.

## Checks

Before committing, ensure:

- `cargo fmt` — code is formatted
- `cargo check` — code compiles
- `cargo clippy` — no lint warnings
- `cargo test` — all tests pass

## TUI Testing with tmux

The TUI needs a real terminal. Use tmux to test it headlessly:

```sh
# Launch in a detached session (set explicit size for reproducible output)
tmux new-session -d -s oxide-test -x 80 -y 24 "cargo run"

# Wait for startup, then capture the screen
sleep 2
tmux capture-pane -t oxide-test -p

# Type text and verify wrapping / rendering
tmux send-keys -t oxide-test "your text here"
sleep 1
tmux capture-pane -t oxide-test -p

# Submit with Enter, newline with Alt+Enter, quit with Ctrl+C
tmux send-keys -t oxide-test Enter
tmux send-keys -t oxide-test C-c

# Always clean up
tmux kill-session -t oxide-test
```

Use this workflow to validate any UI change — capture before and after to confirm rendering is correct.
