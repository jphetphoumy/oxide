# ADR-0003: Use Conventional Commits

**Date**: 2026-06-01
**Status**: Accepted

## Context

As the project grows, commit history becomes documentation. We need a consistent format that is both human-readable and machine-parseable (for changelogs, release notes, semantic versioning).

## Decision

Adopt [Conventional Commits](https://www.conventionalcommits.org/) for all commits:

```
<type>(<scope>): <description>
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, `ci`, `perf`, `build`.

## Consequences

- Commit messages are predictable and greppable
- Enables automated changelog generation in the future
- Pre-commit hooks enforce code quality; commit message linting can be added later
