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

## License

MIT
