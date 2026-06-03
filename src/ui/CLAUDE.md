# ui — TUI Rendering

Ratatui-based immediate-mode rendering. Each function takes a `Frame` and `App` reference, renders one section of the UI.

## Files

| File | Purpose |
|------|---------|
| `mod.rs` | Public exports: `render_layout`, `render_input`, `render_messages`, `render_command_menu`, `render_picker` |
| `layout.rs` | Main 4-row layout (title, messages, input, status) + dynamic input height calculation |
| `messages.rs` | Message list with role-based colors (User=Green, Agent=Yellow, System=Red) and word wrapping |
| `input.rs` | Input box with cursor positioning, placeholder text, and soft wrapping |
| `command_menu.rs` | Inline slash command menu — appears above input when typing `/`, render-time only (not a mode) |
| `picker.rs` | Modal agent selection overlay — centered popup with search filter, used in `Picker` mode |

## Layout Structure

```
┌─────────────────────────────┐
│ " Oxide"           (Cyan)   │  ← title bar (1 row)
├─────────────────────────────┤
│                             │  ← messages area (fills remaining space)
│  you                        │
│    Hello!                   │
│  @dust                      │
│    Hi there...              │
│                             │
├─────────────────────────────┤
│ ┌ > ─────────────────────┐  │  ← input box (dynamic height: 3 to terminal_height/2)
│ │ Type a message...      │  │
│ └────────────────────────┘  │
├─────────────────────────────┤
│ agent: dust  ~/project ...  │  ← status line (1 row): agent, cwd, streaming indicator, keybindings
└─────────────────────────────┘
```

## Key Patterns

- **Input height**: Calculated by `input_height()` based on visual line wrapping. Counts chars (not bytes), adds 2 for borders, clamps to [3, terminal_height/2].
- **Cursor positioning**: `visual_cursor_position()` in `input.rs` walks chars tracking newlines + soft wraps. Does not handle double-width chars (CJK/emoji).
- **Word wrapping**: `wrap_line()` in `messages.rs` breaks on word boundaries, falls back to hard break. Uses char count for UTF-8 safety.
- **Command menu**: Pure render-time widget. Filters `slash::COMMANDS` by input prefix, renders as a `List` above the input area. Not a mode — no state to manage.
- **Picker overlay**: Renders a centered popup (60% x 60%) with filter input, agent list, and keybinding hints. Uses `Clear` widget to erase background.
- **Path display**: Status line shows cwd with `~` home replacement and ellipsis truncation for narrow terminals.

## Testing

All rendering logic has unit tests using `ratatui::backend::TestBackend`. The `command_menu` tests render to a test buffer and inspect specific cells/rows. Layout tests verify height calculations with various terminal sizes.

Manual TUI testing uses tmux (see root CLAUDE.md).
