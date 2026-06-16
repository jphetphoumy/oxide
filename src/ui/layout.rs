use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use std::path::Path;

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
    let height = u16::try_from(visual_lines)
        .unwrap_or(u16::MAX)
        .saturating_add(2); // +2 for borders
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

    let title = Paragraph::new(Line::from(vec![Span::styled(
        " Oxide",
        Style::default().fg(Color::Cyan),
    )]));
    frame.render_widget(title, chunks[0]);

    let hints = "Ctrl+C quit  Enter send  Alt+Enter newline";

    // Format active skills indicator
    let skills_text = if app.active_skills().is_empty() {
        String::new()
    } else {
        let skill_names = app
            .active_skills()
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!(" [skills: {skill_names}]")
    };

    // Format subagent indicator
    let subagent_text = if app.subagent_count() > 0 {
        format!(" [{} subagent(s) running]", app.subagent_count())
    } else {
        String::new()
    };

    // Format context usage indicator
    let ctx_pct = app.context_usage_percent();
    let ctx_text = ctx_pct.map_or_else(String::new, |pct| format!(" ctx:{pct}%"));

    let agent_text = format!(" agent: {}", app.agent_name());
    let cwd_room = usize::from(area.width).saturating_sub(
        agent_text.chars().count()
            + skills_text.chars().count()
            + subagent_text.chars().count()
            + ctx_text.chars().count()
            + hints.chars().count()
            + 2, // ", " separator
    );
    let cwd_text = format_cwd_display(app.cwd(), app.home_dir(), cwd_room);
    let cwd_separator = if cwd_text.is_empty() { "" } else { ", " };
    let hint_width = agent_text.chars().count()
        + cwd_separator.chars().count()
        + cwd_text.chars().count()
        + skills_text.chars().count()
        + subagent_text.chars().count()
        + ctx_text.chars().count()
        + hints.chars().count();
    let padding = area
        .width
        .saturating_sub(u16::try_from(hint_width).unwrap_or(area.width));
    let mut spans = vec![Span::styled(
        agent_text,
        Style::default().fg(Color::DarkGray),
    )];
    if !cwd_text.is_empty() {
        spans.push(Span::styled(
            cwd_separator,
            Style::default().fg(Color::DarkGray),
        ));
        spans.push(Span::styled(cwd_text, Style::default().fg(Color::Cyan)));
    }
    if !skills_text.is_empty() {
        spans.push(Span::styled(
            skills_text,
            Style::default().fg(Color::Rgb(188, 140, 255)),
        ));
    }
    if !subagent_text.is_empty() {
        spans.push(Span::styled(
            subagent_text,
            Style::default().fg(Color::Rgb(188, 140, 255)),
        ));
    }
    if let Some(pct) = ctx_pct {
        let ctx_color = match pct {
            80.. => Color::Red,
            70.. => Color::Yellow,
            _ => Color::DarkGray,
        };
        spans.push(Span::styled(ctx_text, Style::default().fg(ctx_color)));
    }
    spans.push(Span::raw(" ".repeat(padding.into())));
    spans.push(Span::styled(hints, Style::default().fg(Color::DarkGray)));
    let status_line = Line::from(spans);
    frame.render_widget(Paragraph::new(status_line), chunks[3]);

    AppLayout {
        title: chunks[0],
        messages: chunks[1],
        input: chunks[2],
        status: chunks[3],
    }
}

fn format_cwd_display(cwd: &Path, home_dir: Option<&Path>, max_width: usize) -> String {
    let raw = home_dir.map_or_else(
        || cwd.display().to_string(),
        |home_dir| {
            cwd.strip_prefix(home_dir).map_or_else(
                |_| cwd.display().to_string(),
                |relative| {
                    if relative.as_os_str().is_empty() {
                        "~".to_string()
                    } else {
                        format!("~/{}", relative.display())
                    }
                },
            )
        },
    );

    shorten_path_display(&raw, max_width)
}

fn shorten_path_display(display: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let chars: Vec<char> = display.chars().collect();
    if chars.len() <= max_width {
        return display.to_string();
    }

    if max_width <= 3 {
        return chars.into_iter().take(max_width).collect();
    }

    if let Some(last_segment) = display.rsplit(std::path::MAIN_SEPARATOR).next() {
        let last_segment_len = last_segment.chars().count();
        if last_segment_len + 4 <= max_width {
            return format!("...{}{last_segment}", std::path::MAIN_SEPARATOR);
        }
    }

    let tail_len = max_width - 3;
    let tail: String = chars[chars.len().saturating_sub(tail_len)..]
        .iter()
        .collect();
    format!("...{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

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

    #[test]
    fn format_cwd_display_replaces_home_prefix_with_tilde() {
        let cwd = Path::new("/home/alice/projects/oxide");
        let home = Path::new("/home/alice");
        assert_eq!(format_cwd_display(cwd, Some(home), 80), "~/projects/oxide");
    }

    #[test]
    fn format_cwd_display_truncates_to_last_segment_when_needed() {
        let cwd = Path::new("/home/alice/projects/oxide");
        let home = Path::new("/home/alice");
        assert_eq!(format_cwd_display(cwd, Some(home), 12), ".../oxide");
    }

    #[test]
    fn shorten_path_display_handles_zero_width() {
        assert_eq!(shorten_path_display("abcdef", 0), "");
    }

    #[test]
    fn shorten_path_display_keeps_prefix_for_tiny_widths() {
        assert_eq!(shorten_path_display("abcdef", 3), "abc");
    }

    #[test]
    fn shorten_path_display_uses_raw_tail_when_needed() {
        assert_eq!(
            shorten_path_display("/home/alice/projects/oxide", 8),
            "...oxide"
        );
    }
}
