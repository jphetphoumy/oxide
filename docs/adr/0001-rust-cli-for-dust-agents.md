# ADR-0001: Build a Rust CLI to interact with Dust agents

**Date**: 2026-06-01
**Status**: Accepted

## Context

Dust provides powerful AI agents but lacks a dedicated terminal-first client for engineers. Existing options are the web UI or raw API calls. We want a tool that fits into developer workflows the way Claude Code fits into Claude — fast, scriptable, extensible.

## Decision

Build **Oxide**, a Rust CLI that communicates with the Dust API to run and orchestrate Dust agents from the terminal.

### Why Rust

- Performance: near-zero startup time matters for CLI tools
- Single binary distribution: no runtime dependencies
- Strong type system: safe foundation for agent orchestration
- Nix integration: reproducible builds and cross-platform packaging
- The name writes itself: Rust (iron oxide) + Dust = Oxide

### Why a dedicated CLI over scripting the API

- Consistent UX across sessions (conversation state, config, hooks)
- Extensible architecture (skills, sub-agents, MCP servers)
- First-class terminal experience (streaming, colors, interactive mode)

## Consequences

- We commit to Rust and its ecosystem (async runtime, HTTP client, TUI libraries)
- We need to track Dust API changes and maintain compatibility
- Cross-platform builds are achievable via Nix flake (`eachDefaultSystem`)
