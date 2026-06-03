use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::input_buffer::InputBuffer;

pub fn render_input(frame: &mut Frame, buf: &InputBuffer, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" > ");

    if buf.is_empty() {
        let placeholder = Paragraph::new(Line::from(Span::styled(
            "Type a message...",
            Style::default().fg(Color::DarkGray),
        )))
        .block(block)
        .wrap(Wrap { trim: false });
        frame.render_widget(placeholder, area);
    } else {
        let paragraph = Paragraph::new(buf.content())
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    // Place the cursor
    let inner = area.inner(ratatui::layout::Margin::new(1, 1));
    let inner_width = inner.width as usize;
    if inner_width > 0 {
        let (row, col) = visual_cursor_position(buf, inner_width);
        let Ok(col_u16) = u16::try_from(col) else {
            return;
        };
        let Ok(row_u16) = u16::try_from(row) else {
            return;
        };
        let cursor_x = inner.x + col_u16;
        let cursor_y = inner.y + row_u16;
        if cursor_y < inner.y + inner.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
}

/// Calculate the visual (row, col) of the cursor accounting for soft wrapping.
/// Counts each char as width 1. Does not account for double-width characters
/// (CJK, emoji) — would require `unicode-width` crate for full accuracy.
fn visual_cursor_position(buf: &InputBuffer, wrap_width: usize) -> (usize, usize) {
    let content = buf.content();
    let cursor = buf.cursor();
    let before_cursor = &content[..cursor];

    let mut visual_row = 0;
    let mut col = 0;

    for ch in before_cursor.chars() {
        if ch == '\n' {
            visual_row += 1;
            col = 0;
        } else {
            col += 1;
            if col >= wrap_width {
                visual_row += 1;
                col = 0;
            }
        }
    }

    (visual_row, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf_with(text: &str) -> InputBuffer {
        let mut buf = InputBuffer::new();
        for c in text.chars() {
            buf.insert_char(c);
        }
        buf
    }

    #[test]
    fn cursor_at_start() {
        let buf = InputBuffer::new();
        assert_eq!(visual_cursor_position(&buf, 20), (0, 0));
    }

    #[test]
    fn cursor_short_text() {
        let buf = buf_with("hello");
        assert_eq!(visual_cursor_position(&buf, 20), (0, 5));
    }

    #[test]
    fn cursor_wraps_at_boundary() {
        // width=5, typing 5 chars should wrap cursor to next line
        let buf = buf_with("abcde");
        assert_eq!(visual_cursor_position(&buf, 5), (1, 0));
    }

    #[test]
    fn cursor_wraps_mid_long_line() {
        // width=10, 15 chars -> row 1, col 5
        let buf = buf_with(&"x".repeat(15));
        assert_eq!(visual_cursor_position(&buf, 10), (1, 5));
    }

    #[test]
    fn cursor_with_newline() {
        let buf = buf_with("ab\ncd");
        assert_eq!(visual_cursor_position(&buf, 20), (1, 2));
    }

    #[test]
    fn cursor_newline_then_wrap() {
        // "ab\n" + 12 chars at width 10 -> row 1 (newline), then wraps at 10 -> row 2, col 2
        let mut buf = buf_with("ab\n");
        for c in "x".repeat(12).chars() {
            buf.insert_char(c);
        }
        assert_eq!(visual_cursor_position(&buf, 10), (2, 2));
    }
}
