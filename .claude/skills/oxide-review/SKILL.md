---
name: oxide-review
description: Review current oxide changes against a plan, ADR, or requested scope and write the review to feedback.md using conventional comments. Use when the user asks for a senior review of in-progress code, especially against docs/plan/*.html.
---

## When to use this

Use this skill when working on the oxide project and you need to:

- Review the current branch or worktree against a plan document
- Review a focused set of changed files before merge
- Write structured review feedback to `feedback.md`
- Produce findings in conventional comment style instead of patching code directly

## Review workflow

### 1. Read the review target first

Start with the source of truth the user named:

- `docs/plan/*.html` for implementation plans
- `docs/adr/*.md` for architectural intent
- Specific files or diffs called out by the user

Extract the acceptance criteria before looking at the code. The review should judge the implementation against that scope, not against a different feature you would have designed.

### 2. Review the current changes, not the whole repo

Prefer the active diff and touched files:

```bash
git status --short
git diff -- <paths>
```

Then read the changed files with enough surrounding context to understand control flow and state semantics.

### 3. Validate behavior with the smallest relevant check

Pick the narrowest command that materially supports the review:

- `nix develop --command cargo test` for unit and widget behavior
- Targeted `cargo test <name>` if the change is very localized
- A `tmux` smoke test for terminal interaction changes when the plan depends on real terminal behavior

Do not claim terminal behavior is verified unless you actually exercised it.

### 4. Prioritize findings

Default to a code review mindset:

- Find bugs, regressions, incorrect assumptions, missing tests, and mismatches with the plan
- Present findings first, ordered by severity
- If there are no blocking findings, say so explicitly
- Keep summaries short and secondary

### 5. Write the review to `feedback.md`

Replace or create `feedback.md` at the repo root.

Use concise conventional comments. Prefer these labels:

- `issue:` for bugs, regressions, or plan mismatches
- `suggestion:` for a concrete improvement that is not strictly blocking
- `question:` for ambiguity that affects correctness
- `praise:` for implementation choices that are notably correct or robust
- `nit:` for minor polish only

## Output format for `feedback.md`

Use this structure:

```md
# Review Feedback

<one short sentence with the overall result>

## Findings

`issue: short title`

Comment: <direct feedback with file reference and why it matters>

`issue: short title`

Comment: <next finding>

## Strengths

`praise: short title`

Comment: <what is correct and why>

## Residual Risk

`suggestion: short title`

Comment: <remaining verification gap or follow-up>
```

Rules:

- If there are blocking findings, keep `## Findings` first and do not bury them under praise.
- If there are no findings, say `No blocking findings` in the opening sentence and use `## Strengths` plus `## Residual Risk`.
- Reference exact files when possible.
- Keep each comment self-contained and actionable.
- Do not turn the document into a changelog.

## Oxide-specific expectations

- Run commands through `nix develop --command ...`
- Respect the repo lint policy in `Cargo.toml`
- For TUI features, prefer widget tests plus `tmux` for terminal-level behavior when relevant
- Keep the review aligned with existing plan language when the user asks for plan-based review
