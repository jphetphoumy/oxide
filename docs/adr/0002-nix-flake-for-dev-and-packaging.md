# ADR-0002: Use Nix flake for development and packaging

**Date**: 2026-06-01
**Status**: Accepted

## Context

Rust projects need a toolchain (rustc, cargo, clippy, rust-analyzer) and system dependencies (openssl, pkg-config). Team members and CI need identical environments.

## Decision

Use a Nix flake with `oxalica/rust-overlay` as the single source of truth for the development environment and package builds.

- `nix develop` provides the full toolchain and dependencies
- `nix build` produces the release binary
- `.envrc` with `use flake` enables automatic shell activation via direnv
- `pre-commit` is included in the dev shell for consistent code quality hooks

## Consequences

- All contributors need Nix installed (or use the CI-produced binaries)
- Toolchain upgrades are a single flake input update
- Reproducible builds across Linux and macOS (x86_64 and aarch64)
