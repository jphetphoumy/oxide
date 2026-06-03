# ADR-0008: Slash command framework with inline menu

**Date**: 2026-06-03
**Status**: Accepted

## Context

Oxide needs a way for users to invoke built-in actions (switching agents, clearing history, etc.) without those inputs being sent to the Dust agent. The first such action is `/switch`, which opens an agent picker overlay.

We observed how Claude Code handles this: typing `/` immediately shows an inline autocomplete menu below the input, filtering as the user types, with Tab completing the top match. This pattern is familiar, discoverable, and extensible.

Two design questions arose:

1. Should the command menu be a separate app mode (like the agent picker), or a render-time UI hint?
2. Should the command menu be an overlay (centered popup) or inline (anchored to the input)?

## Decision

We adopt a two-layer design:

### Slash command menu: render-time, inline

The command menu is **not** an `AppMode` variant. It is rendered whenever the input buffer starts with `/` while in `Chat` mode. The filter text is derived from the input buffer content (everything after `/`). This means:

- No state to manage for menu open/close — it appears and disappears naturally as the user types.
- Tab completion replaces the input buffer content with the top match.
- Enter submits the command through the existing `apply_action` → `parse_slash_command` path.
- Esc or Backspace past `/` hides the menu implicitly.

The menu renders **inline above the input box**, showing matching commands with name and description.

### Command actions: separate modes where needed

When a slash command requires its own UI (like `/switch` needing an agent picker), it transitions to a dedicated `AppMode`. Simple commands (like a future `/clear`) execute immediately without a mode change.

### Command registry: static array

Commands are defined as a `const` array of `SlashCommandDef { name, description }`. Filtering and completion are pure functions over this array. Adding a new command requires only adding an entry to the array and a match arm in the handler.

## Consequences

- The `AppMode` enum stays small — only modes that need their own keybindings and rendering (like `Picker`) are added.
- The command menu is zero-cost when the user isn't typing `/` — no state tracking, no mode transitions.
- Tab completion is tied to the `/` prefix — Tab does nothing when the input doesn't start with `/`, leaving it available for future uses (e.g. file path completion).
- Adding new slash commands is a two-step process: add to registry, add handler match arm. No UI changes needed.
- The inline menu position (above input) may overlap with short message areas on small terminals. This is acceptable since the menu is transient and shows at most a handful of items.
