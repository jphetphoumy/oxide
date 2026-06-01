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

## Pre-commit

Hooks run automatically on commit (`cargo fmt`, `cargo check`, `clippy`). To install manually:

```sh
nix develop --command pre-commit install
```
