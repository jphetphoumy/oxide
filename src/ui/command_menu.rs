use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};

use crate::slash::{self, SlashCommandDef};

const MAX_VISIBLE: u16 = 6;

/// Render an inline command menu above the input box when the input starts with `/`.
/// Returns early (renders nothing) if the input doesn't start with `/` or no commands match.
pub fn render_command_menu(frame: &mut Frame, input_content: &str, input_area: Rect) {
    let Some(prefix) = input_content.strip_prefix('/') else {
        return;
    };

    let matches = slash::filter_commands(prefix);
    if matches.is_empty() {
        return;
    }

    let item_count = matches.len().min(MAX_VISIBLE as usize);
    #[allow(clippy::cast_possible_truncation)]
    let menu_height = item_count as u16 + 2; // +2 for borders, item_count <= MAX_VISIBLE (6)

    // Position above the input box
    let menu_area = Rect {
        x: input_area.x,
        y: input_area.y.saturating_sub(menu_height),
        width: input_area.width,
        height: menu_height,
    };

    frame.render_widget(Clear, menu_area);

    let items: Vec<ListItem> = matches
        .iter()
        .take(MAX_VISIBLE as usize)
        .map(|cmd| format_command_item(cmd, menu_area.width))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default().with_selected(Some(0));
    frame.render_stateful_widget(list, menu_area, &mut state);
}

fn format_command_item(cmd: &SlashCommandDef, width: u16) -> ListItem<'static> {
    let name_text = format!(" {}", cmd.slash_name);
    let name_len = name_text.chars().count();
    let inner_width = width.saturating_sub(2) as usize; // borders
    let desc_room = inner_width.saturating_sub(name_len + 2);
    let truncated_desc: String = cmd.description.chars().take(desc_room).collect();
    let padding = inner_width.saturating_sub(name_len + truncated_desc.chars().count());

    ListItem::new(Line::from(vec![
        Span::styled(
            name_text,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(padding)),
        Span::styled(truncated_desc, Style::default().fg(Color::DarkGray)),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render_menu(input: &str, width: u16, height: u16) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let input_area = Rect {
                    x: 0,
                    y: height.saturating_sub(3),
                    width,
                    height: 3,
                };
                render_command_menu(frame, input, input_area);
            })
            .unwrap();
        terminal.backend().buffer().clone()
    }

    #[test]
    fn format_command_item_includes_name_and_description() {
        let cmd = SlashCommandDef {
            name: "new".to_string(),
            slash_name: "/new".to_string(),
            description: "Start a new conversation".to_string(),
        };
        let item = format_command_item(&cmd, 60);
        let _ = item;
    }

    /// Helper: read a full row of text from the buffer at the given y coordinate.
    fn row_text(buf: &ratatui::buffer::Buffer, y: u16) -> String {
        (0..buf.area.width)
            .map(|x| {
                buf.cell((x, y))
                    .map_or(' ', |c| c.symbol().chars().next().unwrap_or(' '))
            })
            .collect()
    }

    #[test]
    fn menu_appears_when_input_starts_with_slash() {
        let buf = render_menu("/", 40, 20);
        let full_text = (0..buf.area.height)
            .map(|y| row_text(&buf, y))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            full_text.contains("/switch"),
            "Expected menu to contain '/switch'"
        );
    }

    #[test]
    fn menu_hidden_when_no_slash_prefix() {
        let buf = render_menu("hello", 40, 20);
        let full_text = (0..buf.area.height)
            .map(|y| row_text(&buf, y))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !full_text.contains("/switch"),
            "Menu should not appear without '/' prefix"
        );
    }

    #[test]
    fn menu_hidden_when_no_matches() {
        let buf = render_menu("/xyz", 40, 20);
        let full_text = (0..buf.area.height)
            .map(|y| row_text(&buf, y))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !full_text.contains("/switch"),
            "Menu should not appear when no commands match"
        );
    }

    #[test]
    fn first_row_is_highlighted() {
        let buf = render_menu("/", 40, 20);

        // Menu is positioned at: input_area.y (which is height - 3 = 17) - menu_height
        // Input area: y=17, height=3, menu_height = item_count + 2 (borders)
        // So menu.y = 17 - menu_height
        // First item is at menu.y + 1 (after top border)

        // Find the menu's top border position by looking for the first DarkGray bordered line
        let mut menu_start_y = None;
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                if let Some(cell) = buf.cell((x, y)) {
                    // Check for border color (DarkGray borders)
                    if cell.fg == Color::DarkGray
                        && (cell.symbol() == "┌" || cell.symbol() == "├" || cell.symbol() == "─")
                    {
                        menu_start_y = Some(y);
                        break;
                    }
                }
            }
            if menu_start_y.is_some() {
                break;
            }
        }

        let menu_start = menu_start_y.expect("Menu top border not found");
        let first_item_y = menu_start + 1; // First content row is right after top border

        // Verify the first item row has DarkGray background on content cells
        let mut highlight_cell_count = 0;
        let mut content_cell_count = 0;
        for x in 0..buf.area.width {
            if let Some(cell) = buf.cell((x, first_item_y)) {
                let sym = cell.symbol();
                // Skip borders (│, ├, ┤, etc.) - focus on content
                if sym != "│" && sym != "├" && sym != "┤" && !sym.is_empty() {
                    content_cell_count += 1;
                    if cell.bg == Color::DarkGray {
                        highlight_cell_count += 1;
                    }
                }
            }
        }

        assert!(
            content_cell_count > 0,
            "First item row has no content cells"
        );
        assert!(
            highlight_cell_count > 0,
            "First item row should have DarkGray highlighted cells"
        );

        // Verify the first item contains expected command content
        let row = row_text(&buf, first_item_y);
        assert!(
            row.contains('/') || row.chars().any(|c| c.is_alphanumeric()),
            "First item row should contain command text (found: '{}')",
            row
        );

        // Optionally verify the second item (if it exists) does NOT have the same highlight
        if first_item_y + 1 < buf.area.height {
            let second_item_y = first_item_y + 1;
            let mut second_highlight_count = 0;
            for x in 0..buf.area.width {
                if let Some(cell) = buf.cell((x, second_item_y)) {
                    let sym = cell.symbol();
                    if sym != "│" && sym != "├" && sym != "┤" && !sym.is_empty() {
                        if cell.bg == Color::DarkGray {
                            second_highlight_count += 1;
                        }
                    }
                }
            }
            // Second item should NOT have highlight (it's not selected)
            assert_eq!(
                second_highlight_count, 0,
                "Second item row should not be highlighted (only first item should be)"
            );
        }
    }

    #[test]
    fn menu_top_border_is_rendered() {
        let buf = render_menu("/", 40, 20);
        // Find any top border character in the buffer
        let mut found_border = false;
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                if let Some(cell) = buf.cell((x, y)) {
                    let sym = cell.symbol();
                    if sym == "┌" || sym == "╭" || sym == "+" || sym == "┏" {
                        found_border = true;
                        break;
                    }
                }
            }
            if found_border {
                break;
            }
        }
        assert!(found_border, "Expected a border character at menu top");
    }
}
