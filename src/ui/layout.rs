use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;

#[allow(dead_code)]
pub struct AppLayout {
    pub title: Rect,
    pub messages: Rect,
    pub input: Rect,
    pub status: Rect,
}

/// Calculate the visual height needed for the input box.
/// Accounts for line wrapping: each logical line may span multiple visual rows.
/// Adds 2 for the border. Clamps between 3 and half the terminal height.
pub fn input_height(lines: &[String], terminal_width: u16, terminal_height: u16) -> u16 {
    let inner_width = terminal_width.saturating_sub(2).max(1) as usize; // border eats 2 cols
    let visual_lines: usize = lines
        .iter()
        .map(|line| {
            if line.is_empty() {
                1
            } else {
                line.chars().count().div_ceil(inner_width)
            }
        })
        .sum();
    let height = (visual_lines as u16).saturating_add(2); // +2 for borders
    let max_height = terminal_height / 2;
    height.clamp(3, max_height.max(3))
}

pub fn render_layout(frame: &mut Frame, app: &App, input_h: u16) -> AppLayout {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(1),       // title bar
        Constraint::Min(5),          // messages
        Constraint::Length(input_h), // input box (dynamic)
        Constraint::Length(1),       // status line
    ])
    .split(area);

    // Title bar
    let title = Paragraph::new(Line::from(vec![Span::styled(
        " Oxide",
        Style::default().fg(Color::Cyan),
    )]));
    frame.render_widget(title, chunks[0]);

    // Status line: model on left, hints on right
    let model_text = format!(" model: {}", app.model());
    let hints = "Ctrl+C quit  Enter send  Alt+Enter newline";
    let padding = area
        .width
        .saturating_sub(u16::try_from(model_text.len() + hints.len()).unwrap_or(area.width));
    let status_line = Line::from(vec![
        Span::styled(model_text, Style::default().fg(Color::DarkGray)),
        Span::raw(" ".repeat(padding.into())),
        Span::styled(hints, Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(status_line), chunks[3]);

    AppLayout {
        title: chunks[0],
        messages: chunks[1],
        input: chunks[2],
        status: chunks[3],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> String {
        text.to_string()
    }

    #[test]
    fn single_short_line_returns_min_height() {
        // 1 visual line + 2 border = 3
        assert_eq!(input_height(&[s("hello")], 80, 24), 3);
    }

    #[test]
    fn empty_input_returns_min_height() {
        assert_eq!(input_height(&[s("")], 80, 24), 3);
    }

    #[test]
    fn long_line_wraps_and_grows() {
        // inner_width = 80 - 2 = 78
        // 200 chars → ceil(200/78) = 3 visual lines → 3 + 2 = 5
        let long = "a".repeat(200);
        assert_eq!(input_height(&[s(&long)], 80, 24), 5);
    }

    #[test]
    fn multiple_lines_stack() {
        // 3 short lines → 3 visual lines + 2 border = 5
        let lines = vec![s("one"), s("two"), s("three")];
        assert_eq!(input_height(&lines, 80, 24), 5);
    }

    #[test]
    fn mixed_short_and_long_lines() {
        // inner_width = 40 - 2 = 38
        // "hi" → 1 visual line
        // "a" * 80 → ceil(80/38) = 3 visual lines
        // total = 4 + 2 = 6
        let lines = vec![s("hi"), s(&"a".repeat(80))];
        assert_eq!(input_height(&lines, 40, 24), 6);
    }

    #[test]
    fn height_is_clamped_to_half_terminal_height() {
        // terminal height 24 → max = 12
        // 100 lines → 100 + 2 = 102, clamped to 12
        let lines: Vec<String> = (0..100).map(|_| s("x")).collect();
        assert_eq!(input_height(&lines, 80, 24), 12);
    }

    #[test]
    fn short_terminal_still_allows_min_height() {
        // terminal height 4 → max = max(4/2, 3) = 3
        // even on a tiny terminal, input gets at least 3 rows
        assert_eq!(input_height(&[s("hello")], 80, 4), 3);
    }
}
