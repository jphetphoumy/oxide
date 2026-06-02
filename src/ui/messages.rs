use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, Role};

const INDENT: &str = "   ";

pub fn render_messages(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    let body_width = (area.width as usize).saturating_sub(INDENT.len());

    for msg in app.messages() {
        let (label, color) = match &msg.role {
            Role::User => ("you".to_string(), Color::Green),
            Role::Agent(name) => (format!("@{name}"), Color::Yellow),
            Role::System => ("system".to_string(), Color::Red),
        };

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  {label}"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )));

        for text_line in msg.content.lines() {
            for visual_line in wrap_line(text_line, body_width) {
                lines.push(Line::from(format!("{INDENT}{visual_line}")));
            }
        }
    }

    let total_lines = lines.len();
    let visible_height = area.height as usize;
    let max_scroll = total_lines.saturating_sub(visible_height);
    let scroll = app.scroll_offset().min(max_scroll);

    let messages_widget = Paragraph::new(lines)
        .block(Block::default().borders(Borders::NONE))
        .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0));

    frame.render_widget(messages_widget, area);
}

/// Wrap a single line of text into chunks that fit within `max_width` characters.
/// Breaks on word boundaries when possible, hard-breaks otherwise.
/// Uses char count (not byte length) to handle multi-byte UTF-8 safely.
/// Note: does not account for double-width characters (CJK, emoji). That would
/// require a display-width crate like `unicode-width`.
fn wrap_line(text: &str, max_width: usize) -> Vec<&str> {
    if max_width == 0 || text.chars().count() <= max_width {
        return vec![text];
    }

    let mut result = Vec::new();
    let mut remaining = text;

    while remaining.chars().count() > max_width {
        // Find the byte offset of the char at position max_width
        let byte_boundary = remaining
            .char_indices()
            .nth(max_width)
            .map_or(remaining.len(), |(i, _)| i);

        // Try to find a space to break on within those bytes
        let break_at = remaining[..byte_boundary]
            .rfind(' ')
            .map_or(byte_boundary, |pos| pos + 1);

        result.push(&remaining[..break_at]);
        remaining = &remaining[break_at..];
    }

    if !remaining.is_empty() {
        result.push(remaining);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_line_no_wrap() {
        assert_eq!(wrap_line("hello", 20), vec!["hello"]);
    }

    #[test]
    fn exact_fit_no_wrap() {
        assert_eq!(wrap_line("12345", 5), vec!["12345"]);
    }

    #[test]
    fn wraps_on_word_boundary() {
        let result = wrap_line("hello world foo", 12);
        assert_eq!(result, vec!["hello world ", "foo"]);
    }

    #[test]
    fn hard_break_no_spaces() {
        let result = wrap_line("abcdefghij", 5);
        assert_eq!(result, vec!["abcde", "fghij"]);
    }

    #[test]
    fn multiple_wraps() {
        let result = wrap_line("aa bb cc dd ee", 6);
        assert_eq!(result, vec!["aa bb ", "cc dd ", "ee"]);
    }

    #[test]
    fn empty_line() {
        assert_eq!(wrap_line("", 10), vec![""]);
    }

    #[test]
    fn zero_width_returns_whole_line() {
        assert_eq!(wrap_line("hello", 0), vec!["hello"]);
    }

    #[test]
    fn unicode_does_not_panic() {
        // "éàü" is 6 bytes but 3 chars — wrapping at width 2 must not panic
        let result = wrap_line("éàü", 2);
        assert_eq!(result, vec!["éà", "ü"]);
    }

    #[test]
    fn unicode_wraps_on_char_boundary() {
        // 5 multi-byte chars, wrap at 3
        let result = wrap_line("ñéàüö", 3);
        assert_eq!(result, vec!["ñéà", "üö"]);
    }
}
