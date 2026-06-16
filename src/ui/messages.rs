use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, AppMode, Role, SubagentCallState, ToolCallEntry, ToolCallStatus};

const INDENT: &str = "   ";
const SPINNER_FRAMES: &[&str] = &["⟳", "↻"];
const TOOL_RESULT_PREVIEW_LINES: usize = 5;

pub fn render_messages(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    let body_width = (area.width as usize).saturating_sub(INDENT.len());

    for msg in app.messages() {
        match &msg.role {
            Role::User => {
                let label = "you";
                let color = Color::Green;

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
            Role::Agent(name) => {
                let label = format!("@{name}");
                let color = Color::Yellow;

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
            Role::System => {
                let label = "system";
                let color = Color::Red;

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
            Role::SubagentCall(state) => {
                lines.push(Line::from(""));
                lines.extend(subagent_call_lines(state, app.tick_count()));
            }
            Role::ToolCall(entry) => {
                lines.push(Line::from(""));
                lines.extend(tool_call_lines(entry, body_width));
            }
        }
    }

    if app.is_streaming() {
        lines.extend(streaming_indicator_lines(
            app.streaming_started_at(),
            app.tick_count(),
        ));
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

fn tool_call_lines(entry: &ToolCallEntry, width: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Extract argument preview from input JSON
    let arg_preview = extract_arg_preview(&entry.input, 60);

    // Header line with bullet and tool name + args
    let (bullet, bullet_color) = match entry.status {
        ToolCallStatus::Pending => ("●", Color::Yellow),
        ToolCallStatus::Running => ("⟳", Color::Yellow),
        ToolCallStatus::Done => ("●", Color::White),
        ToolCallStatus::Failed => ("●", Color::Red),
        ToolCallStatus::Denied => ("●", Color::DarkGray),
    };

    lines.push(Line::from(vec![
        Span::styled(format!("  {bullet} "), Style::default().fg(bullet_color)),
        Span::styled(
            entry.tool_name.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("({arg_preview})")),
    ]));

    // Status-specific lines
    match entry.status {
        ToolCallStatus::Pending => {
            lines.push(Line::from(vec![
                Span::styled("    ⚠  ", Style::default().fg(Color::Yellow)),
                Span::styled("[y] approve", Style::default().fg(Color::Green)),
                Span::raw("  "),
                Span::styled("[n] deny", Style::default().fg(Color::Red)),
                Span::raw("  "),
                Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
            ]));
        }
        ToolCallStatus::Running => {
            lines.push(Line::from(Span::styled(
                "    executing...",
                Style::default().fg(Color::DarkGray),
            )));
        }
        ToolCallStatus::Done => {
            if let Some(result) = &entry.result {
                render_tool_result(result, &mut lines, width, entry.expanded);
            }
        }
        ToolCallStatus::Failed => {
            if let Some(error) = &entry.result {
                lines.push(Line::from(vec![
                    Span::styled("    ⎿  ", Style::default().fg(Color::Red)),
                    Span::styled(error.clone(), Style::default().fg(Color::Red)),
                ]));
            }
        }
        ToolCallStatus::Denied => {
            lines.push(Line::from(vec![
                Span::styled("    ⎿  ", Style::default().fg(Color::DarkGray)),
                Span::styled("denied by user", Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    lines
}

#[allow(clippy::option_if_let_else, clippy::map_unwrap_or)]
fn extract_arg_preview(input: &serde_json::Value, max_len: usize) -> String {
    let preview = match input {
        serde_json::Value::Object(obj) => {
            if let Some(cmd_val) = obj.get("command") {
                match cmd_val {
                    serde_json::Value::String(s) => s.clone(),
                    _ => cmd_val.to_string(),
                }
            } else if obj.len() == 1 {
                // Safe because we checked len() == 1 above
                obj.values()
                    .next()
                    .map(|val| match val {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => serde_json::to_string(val).unwrap_or_else(|_| "?".to_string()),
                    })
                    .unwrap_or_else(|| {
                        serde_json::to_string(input).unwrap_or_else(|_| "?".to_string())
                    })
            } else {
                serde_json::to_string(input).unwrap_or_else(|_| "?".to_string())
            }
        }
        serde_json::Value::String(s) => s.clone(),
        _ => serde_json::to_string(input).unwrap_or_else(|_| "?".to_string()),
    };

    if preview.len() > max_len {
        format!("{}…", &preview[..max_len])
    } else {
        preview
    }
}

fn render_tool_result(result: &str, lines: &mut Vec<Line<'static>>, width: usize, expanded: bool) {
    let result_lines: Vec<&str> = result.lines().collect();

    if result_lines.is_empty() {
        return;
    }

    let preview_lines = if expanded {
        result_lines.clone()
    } else {
        result_lines
            .iter()
            .take(TOOL_RESULT_PREVIEW_LINES)
            .copied()
            .collect()
    };

    // Render each line with wrapping
    for (i, line) in preview_lines.iter().enumerate() {
        let wrapped = wrap_line(line, width.saturating_sub(6)); // Account for "    ⎿  "
        for (j, visual_line) in wrapped.into_iter().enumerate() {
            if i == 0 && j == 0 {
                // First line gets the arrow
                lines.push(Line::from(format!("    ⎿  {visual_line}")));
            } else {
                // Subsequent lines get indent
                lines.push(Line::from(format!("       {visual_line}")));
            }
        }
    }

    // Add truncation hint if not expanded and there are more lines
    if !expanded && result_lines.len() > TOOL_RESULT_PREVIEW_LINES {
        let remaining = result_lines.len() - TOOL_RESULT_PREVIEW_LINES;
        lines.push(Line::from(Span::styled(
            format!("       … +{remaining} lines (ctrl+o to expand)"),
            Style::default().fg(Color::DarkGray),
        )));
    } else if expanded && result_lines.len() > TOOL_RESULT_PREVIEW_LINES {
        lines.push(Line::from(Span::styled(
            "       (ctrl+o to collapse)",
            Style::default().fg(Color::DarkGray),
        )));
    }
}

fn tool_approval_lines(
    tool_name: &str,
    input: &serde_json::Value,
    width: usize,
) -> Vec<Line<'static>> {
    let sep = "─".repeat(width.min(60));
    let mut lines: Vec<Line<'static>> = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  ⚠ Approve tool call? ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(Span::styled(
            format!("  {sep}"),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("  Tool: ", Style::default().fg(Color::White)),
            Span::styled(
                tool_name.to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Input:", Style::default().fg(Color::White))),
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

fn subagent_call_lines(state: &SubagentCallState, tick: u64) -> Vec<Line<'static>> {
    use crate::app::SubagentCallStatus;

    let label = state.description.as_deref().unwrap_or("subagent");

    match &state.status {
        SubagentCallStatus::Running => {
            #[allow(clippy::cast_possible_truncation)]
            let frame_idx = (tick as usize) % SPINNER_FRAMES.len();
            let spinner = SPINNER_FRAMES[frame_idx];
            vec![Line::from(vec![
                Span::styled(format!("  {spinner} "), Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("Agent({label})"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ])]
        }
        SubagentCallStatus::Done => {
            let elapsed_secs = state
                .finished_at
                .and_then(|finish| finish.checked_duration_since(state.started_at))
                .map_or(0, |d| d.as_secs());
            vec![Line::from(vec![
                Span::styled("  ✓ ", Style::default().fg(Color::Green)),
                Span::styled(
                    format!("Agent({label})"),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  ({elapsed_secs}s)"),
                    Style::default().fg(Color::DarkGray),
                ),
            ])]
        }
        SubagentCallStatus::Failed => {
            let elapsed_secs = state
                .finished_at
                .and_then(|finish| finish.checked_duration_since(state.started_at))
                .map_or(0, |d| d.as_secs());
            vec![Line::from(vec![
                Span::styled("  ✗ ", Style::default().fg(Color::Red)),
                Span::styled(
                    format!("Agent({label})"),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  failed ({elapsed_secs}s)"),
                    Style::default().fg(Color::DarkGray),
                ),
            ])]
        }
    }
}

fn streaming_indicator_lines(
    started_at: Option<std::time::Instant>,
    tick: u64,
) -> Vec<Line<'static>> {
    #[allow(clippy::cast_possible_truncation)]
    let frame_idx = (tick as usize) % SPINNER_FRAMES.len();
    let spinner = SPINNER_FRAMES[frame_idx];

    let elapsed_secs = started_at.map_or(0, |t| t.elapsed().as_secs());

    vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("  {spinner} "), Style::default().fg(Color::Yellow)),
            Span::styled(
                "Thinking...",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {elapsed_secs}s"),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ]
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
        let call_id = "t1".to_string();
        app.enter_tool_approval(tool_call, call_id);
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

    #[test]
    fn subagent_running_shows_spinner() {
        let mut app = App::new("agent", "/workspace", None);
        app.push_subagent_started("id-1".to_string(), Some("fetch data".to_string()));
        let rows = render_messages_text(&app, 40, 6);
        let all = rows.join("\n");
        assert!(all.contains("Agent(fetch data)"), "label missing: {all}");
        assert!(
            all.contains('⟳') || all.contains('↻'),
            "spinner missing: {all}"
        );
    }

    #[test]
    fn subagent_done_shows_checkmark_and_elapsed() {
        let mut app = App::new("agent", "/workspace", None);
        app.push_subagent_started("id-1".to_string(), Some("fetch data".to_string()));
        app.complete_subagent("id-1", true);
        let rows = render_messages_text(&app, 40, 6);
        let all = rows.join("\n");
        assert!(all.contains('✓'), "checkmark missing: {all}");
        assert!(all.contains("Agent(fetch data)"), "label missing: {all}");
        assert!(all.contains('s'), "elapsed seconds missing: {all}");
    }

    #[test]
    fn subagent_failed_shows_cross() {
        let mut app = App::new("agent", "/workspace", None);
        app.push_subagent_started("id-1".to_string(), Some("fetch data".to_string()));
        app.complete_subagent("id-1", false);
        let rows = render_messages_text(&app, 40, 6);
        let all = rows.join("\n");
        assert!(all.contains('✗'), "cross missing: {all}");
        assert!(all.contains("failed"), "failed text missing: {all}");
    }

    #[test]
    fn streaming_indicator_shows_spinner_while_streaming() {
        let mut app = App::new("agent", "/workspace", None);
        assert!(app.send_message("hello"));
        let rows = render_messages_text(&app, 40, 6);
        let all = rows.join("\n");
        assert!(
            all.contains("Thinking..."),
            "streaming indicator missing: {all}"
        );
        assert!(
            all.contains('⟳') || all.contains('↻'),
            "spinner missing: {all}"
        );
    }

    #[test]
    fn streaming_indicator_disappears_after_complete() {
        let mut app = App::new("agent", "/workspace", None);
        assert!(app.send_message("hello"));
        app.complete_stream(Some("done"));
        let rows = render_messages_text(&app, 40, 6);
        let all = rows.join("\n");
        assert!(
            !all.contains("Thinking..."),
            "streaming indicator should be gone after complete: {all}"
        );
    }

    #[test]
    fn streaming_indicator_not_shown_when_not_streaming() {
        let mut app = App::new("agent", "/workspace", None);
        app.push_system_message("welcome");
        let rows = render_messages_text(&app, 40, 6);
        let all = rows.join("\n");
        assert!(
            !all.contains("Thinking..."),
            "indicator should not appear when not streaming: {all}"
        );
    }

    #[test]
    fn tool_call_pending_shows_bullet_and_approval_hints() {
        use crate::mcp::ToolCall;

        let mut app = App::new("agent", "/workspace", None);
        let tool_call = ToolCall {
            id: "t1".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "ls -al"}),
        };
        app.push_tool_call(tool_call);

        let rows = render_messages_text(&app, 60, 8);
        let all = rows.join("\n");
        assert!(all.contains("Bash"), "tool name not found: {all}");
        assert!(
            all.contains("[y] approve"),
            "approval hint not found: {all}"
        );
        assert!(all.contains("[n] deny"), "deny hint not found: {all}");
    }

    #[test]
    fn tool_call_done_shows_result_with_arrow() {
        use crate::mcp::ToolCall;

        let mut app = App::new("agent", "/workspace", None);
        let tool_call = ToolCall {
            id: "t1".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "ls"}),
        };
        app.push_tool_call(tool_call);
        app.complete_tool_call("t1", "file1.txt\nfile2.txt".into());

        let rows = render_messages_text(&app, 60, 8);
        let all = rows.join("\n");
        assert!(all.contains("Bash"), "tool name not found: {all}");
        assert!(all.contains("file1.txt"), "result not found: {all}");
        assert!(all.contains("⎿"), "arrow not found: {all}");
    }

    #[test]
    fn tool_call_denied_shows_dim_denied() {
        use crate::mcp::ToolCall;

        let mut app = App::new("agent", "/workspace", None);
        let tool_call = ToolCall {
            id: "t1".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "rm"}),
        };
        app.push_tool_call(tool_call);
        app.deny_tool_call("t1");

        let rows = render_messages_text(&app, 60, 8);
        let all = rows.join("\n");
        assert!(
            all.contains("denied by user"),
            "denied message not found: {all}"
        );
    }

    #[test]
    fn tool_call_arg_preview_extracts_command_key() {
        let input = serde_json::json!({"command": "ls -la /tmp"});
        let preview = extract_arg_preview(&input, 60);
        assert_eq!(preview, "ls -la /tmp");
    }

    #[test]
    fn tool_call_arg_preview_truncates_long_value() {
        let input =
            serde_json::json!({"command": "this is a very long command that should be truncated"});
        let preview = extract_arg_preview(&input, 20);
        assert!(preview.ends_with('…'));
        // The actual length includes the 3-byte UTF-8 ellipsis character, which is more than 20+1
        assert!(preview.len() > 20);
    }

    #[test]
    fn tool_call_short_result_not_truncated() {
        use crate::mcp::ToolCall;

        let mut app = App::new("agent", "/workspace", None);
        let tool_call = ToolCall {
            id: "t1".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "ls"}),
        };
        app.push_tool_call(tool_call);
        app.complete_tool_call("t1", "line1\nline2\nline3".into());

        let rows = render_messages_text(&app, 60, 10);
        let all = rows.join("\n");
        assert!(
            !all.contains("… +"),
            "short result should not show ellipsis"
        );
    }

    #[test]
    fn tool_call_long_result_truncated_with_hint() {
        use crate::mcp::ToolCall;

        let mut app = App::new("agent", "/workspace", None);
        let tool_call = ToolCall {
            id: "t1".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "cargo build"}),
        };
        app.push_tool_call(tool_call);
        let long_output = (0..10)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        app.complete_tool_call("t1", long_output);

        let rows = render_messages_text(&app, 60, 20);
        let all = rows.join("\n");
        assert!(all.contains("… +"), "long result should show ellipsis hint");
    }

    #[test]
    fn tool_call_expanded_shows_all_lines_with_collapse_hint() {
        use crate::mcp::ToolCall;

        let mut app = App::new("agent", "/workspace", None);
        let tool_call = ToolCall {
            id: "t1".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "cargo build"}),
        };
        app.push_tool_call(tool_call);
        let long_output = (0..10)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        app.complete_tool_call("t1", long_output);
        app.toggle_tool_call_expanded("t1");

        let rows = render_messages_text(&app, 60, 20);
        let all = rows.join("\n");
        assert!(
            all.contains("line9"),
            "all lines should be shown when expanded"
        );
        assert!(
            all.contains("collapse"),
            "collapse hint should appear when expanded"
        );
    }
}
