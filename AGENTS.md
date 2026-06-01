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
