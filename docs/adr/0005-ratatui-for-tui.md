# ADR-0005: Use Ratatui for the terminal UI

**Date**: 2026-06-01
**Status**: Accepted

## Context

Oxide needs a rich terminal interface for interactive agent conversations — streaming responses, input handling, panels, and status indicators. A raw `println!` approach won't scale.

## Decision

Use [Ratatui](https://ratatui.rs/) as the TUI framework.

### Why Ratatui

- Most active Rust TUI library (successor to `tui-rs`)
- Immediate-mode rendering: simple mental model, easy to test
- Rich widget ecosystem (text areas, lists, tabs, scrolling)
- Crossterm backend: works on Linux, macOS, and Windows
- Well-documented with strong community support

## Consequences

- UI code follows the Ratatui render loop pattern (event → update state → draw)
- Testable: state logic is separated from rendering
- Adds `ratatui` and `crossterm` as dependencies
