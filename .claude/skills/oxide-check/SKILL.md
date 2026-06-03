---
name: oxide-check
description: Run the full oxide pre-commit check suite (fmt, clippy, test). Use before committing or when you want to verify the codebase compiles and passes all checks.
---

## When to use this

Use this skill when working on the oxide project and you need to:

- Verify code compiles and passes all checks before committing
- Run the full lint + test suite after making changes
- Debug a failing pre-commit hook

## Steps

Run the following commands **sequentially** from the oxide project root. Stop at the first failure and fix the issue before continuing.

### 1. Format check

```bash
cargo fmt -- --check
```

If this fails, run `cargo fmt` to auto-fix, then re-run the check.

### 2. Compile check

```bash
cargo check
```

### 3. Clippy (strict)

Lint configuration lives in `Cargo.toml` under `[lints.clippy]` so no extra flags are needed:

```bash
cargo clippy -- -W clippy::all -W clippy::pedantic -W clippy::nursery
```

Key rules enforced:
- `all` and `pedantic` = deny
- `nursery` = warn
- `unwrap_used`, `expect_used`, `panic` = deny (use `anyhow` for error handling)
- Allowed exceptions: `cast_possible_truncation`, `module_name_repetitions`

### 4. Tests

```bash
cargo test
```

### 5. Pre-commit extras

These also run in the pre-commit hooks but are less likely to fail:

- Trailing whitespace
- End-of-file fixer
- YAML / TOML validity
- No merge conflict markers
- No large files added

## Definition of done

All four main commands exit 0:

- `cargo fmt -- --check` — no formatting issues
- `cargo check` — compiles without errors
- `cargo clippy -- -W clippy::all -W clippy::pedantic -W clippy::nursery` — zero warnings
- `cargo test` — all tests pass
