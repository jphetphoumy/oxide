# Oxide

A Rust CLI that talks to [Dust](https://dust.tt) agents — the way Claude Code talks to Claude.

**Rust = iron oxide. Dust + Rust = Oxide.**

## Why

Dust agents are powerful, but interacting with them deserves a proper engineering tool: fast, extensible, and built for the terminal. Oxide aims to be that tool.

## Goals

- Talk to Dust agents from the command line
- Extensible hook and skill system
- Sub-agent spawning and orchestration
- MCP server registration
- Fast — written in Rust

## Usage

```sh
oxide chat
oxide run
oxide agent
```

## Development

Requires [Nix](https://nixos.org/):

```sh
nix develop
cargo build
cargo run
```

### Testing

```sh
cargo test          # unit tests
cargo clippy        # lint checks
```

To manually test the TUI without a visible terminal (useful in CI or from scripts):

```sh
tmux new-session -d -s oxide-test -x 80 -y 24 "cargo run"
sleep 2
tmux capture-pane -t oxide-test -p   # inspect the screen
tmux send-keys -t oxide-test "hello" # type text
tmux send-keys -t oxide-test Enter   # submit
tmux send-keys -t oxide-test C-c     # quit
tmux kill-session -t oxide-test      # clean up
```

## License

MIT
