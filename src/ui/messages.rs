use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, AppMode, Role};

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

    if let AppMode::ToolApproval(state) = app.mode() {
        lines.extend(tool_approval_lines(
            &state.tool_call.name,
            &state.tool_call.input,
            body_width,
        ));
    }

    let total_lines = lines.len();
    let visible_height = area.height as usize;
    let max_scroll = total_lines.saturating_sub(visible_height);
    // scroll_offset 0 = "at the bottom" (latest messages visible).
    // Higher offset = further up in history.  Convert to ratatui's
    // top-origin scroll by subtracting from max_scroll.
    let clamped = app.scroll_offset().min(max_scroll);
    let scroll = max_scroll.saturating_sub(clamped);

    let messages_widget = Paragraph::new(lines)
        .block(Block::default().borders(Borders::NONE))
        .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0));

    frame.render_widget(messages_widget, area);
}

fn tool_approval_lines(
    tool_name: &str,
    input: &serde_json::Value,
    width: usize,
) -> Vec<Line<'static>> {
    let sep = "─".repeat(width.min(60));
    let mut lines: Vec<Line<'static>> = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ⚙ ", Style::default().fg(Color::Yellow)),
            Span::styled(
                tool_name.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            format!("  {sep}"),
            Style::default().fg(Color::DarkGray),
        )),
    ];

    match input {
        serde_json::Value::Object(obj) => {
            for (key, val) in obj {
                let val_str = match val {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Null => "null".to_string(),
                    _ => serde_json::to_string(val).unwrap_or_else(|_| "?".to_string()),
                };
                lines.push(Line::from(Span::styled(
                    format!("{INDENT}{key} = {val_str}"),
                    Style::default().fg(Color::Cyan),
                )));
            }
        }
        _ => {
            lines.push(Line::from(Span::styled(
                format!("{INDENT}{input}"),
                Style::default().fg(Color::Cyan),
            )));
        }
    }

    lines.push(Line::from(Span::styled(
        format!("  {sep}"),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("[y] approve", Style::default().fg(Color::Green)),
        Span::raw("  "),
        Span::styled("[n] deny", Style::default().fg(Color::Red)),
        Span::raw("  "),
        Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
    ]));
    lines
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
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render_messages_text(app: &App, width: u16, height: u16) -> Vec<String> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render_messages(frame, app, frame.area());
            })
            .unwrap();

        let buf = terminal.backend().buffer().clone();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| {
                        buf.cell((x, y))
                            .map_or(' ', |c| c.symbol().chars().next().unwrap_or(' '))
                    })
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

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

    #[test]
    fn zero_scroll_offset_renders_latest_messages() {
        let mut app = App::new("agent", "/workspace", None);
        app.push_system_message("one\ntwo");
        app.push_system_message("three\nfour");

        let rows = render_messages_text(&app, 20, 4);
        assert_eq!(rows, vec!["", "  system", "   three", "   four"]);
    }

    #[test]
    fn scrolling_up_reveals_older_messages() {
        let mut app = App::new("agent", "/workspace", None);
        app.push_system_message("one\ntwo");
        app.push_system_message("three\nfour");
        app.scroll_up(1);

        let rows = render_messages_text(&app, 20, 4);
        assert_eq!(rows, vec!["   two", "", "  system", "   three"]);
    }

    #[test]
    fn tool_approval_block_visible_at_bottom() {
        use crate::mcp::ToolCall;

        let mut app = App::new("agent", "/workspace", None);
        // Push enough messages that the approval block would be off-screen if scroll
        // were not reset to 0.
        for i in 0..10 {
            app.push_system_message(&format!("message {i}"));
        }
        let tool_call = ToolCall {
            id: "t1".into(),
            name: "oxide_bash".into(),
            input: serde_json::json!({"command": "ls -al"}),
        };
        app.enter_tool_approval(tool_call);
        assert_eq!(app.scroll_offset(), 0);

        let rows = render_messages_text(&app, 40, 6);
        // Last visible rows should contain the approval prompt
        let all = rows.join("\n");
        assert!(
            all.contains("oxide_bash"),
            "tool name not found in rendered output:\n{all}"
        );
        assert!(
            all.contains("[y] approve"),
            "approve hint not found in rendered output:\n{all}"
        );
    }

    #[test]
    fn tool_approval_block_not_shown_in_chat_mode() {
        let mut app = App::new("agent", "/workspace", None);
        app.push_system_message("hello");

        let rows = render_messages_text(&app, 40, 6);
        let all = rows.join("\n");
        assert!(
            !all.contains("[y] approve"),
            "approval block should not appear in Chat mode"
        );
    }
}
