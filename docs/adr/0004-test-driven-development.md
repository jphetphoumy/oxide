# ADR-0004: Test-Driven Development

**Date**: 2026-06-01
**Status**: Accepted

## Context

Oxide orchestrates external AI agents over a network API. This kind of system is prone to subtle integration bugs, state management issues, and regressions. We need confidence that changes don't break existing behavior.

## Decision

Adopt TDD as the default development workflow:

1. Write a failing test
2. Write the minimal code to make it pass
3. Refactor

Use Rust's built-in test framework (`#[cfg(test)]`, `#[test]`) for unit tests. Integration tests go in `tests/`. Use `cargo test` as the primary validation step.

## Consequences

- Every feature and bug fix starts with a test
- `cargo test` must pass before any commit (enforced by pre-commit hooks)
- Higher upfront cost per feature, lower cost of change over time
- Test coverage grows naturally with the codebase
