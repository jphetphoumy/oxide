use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};

use crate::app::AppMode;
use crate::dust::types::{AgentInfo, ConversationSummary};

pub fn render_picker(frame: &mut Frame, mode: &AppMode, filtered: &[&AgentInfo], selected: usize) {
    let AppMode::Picker(state) = mode else {
        return;
    };

    let area = frame.area();
    let popup = centered_rect(60, 60, area);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Switch Agent ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let chunks = Layout::vertical([
        Constraint::Length(1), // search input
        Constraint::Length(1), // separator
        Constraint::Min(1),    // agent list
        Constraint::Length(1), // footer hints
    ])
    .split(inner);

    // Search input
    let filter_display = if state.filter.is_empty() {
        Span::styled("Type to filter...", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(
            format!(" {}", state.filter),
            Style::default().fg(Color::White),
        )
    };
    let search = Paragraph::new(Line::from(vec![
        Span::styled(" / ", Style::default().fg(Color::Cyan)),
        filter_display,
    ]));
    frame.render_widget(search, chunks[0]);

    // Separator
    let sep = Paragraph::new(Line::from("─".repeat(inner.width as usize)))
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(sep, chunks[1]);

    // Agent list or loading/empty state
    if state.loading {
        let loading = Paragraph::new(Line::from(Span::styled(
            "  Fetching agents...",
            Style::default().fg(Color::Yellow),
        )));
        frame.render_widget(loading, chunks[2]);
    } else if filtered.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "  No agents match",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, chunks[2]);
    } else {
        let items: Vec<ListItem> = filtered
            .iter()
            .map(|agent| {
                let desc = if agent.description.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", agent.description)
                };
                let max_desc =
                    (inner.width as usize).saturating_sub(agent.name.chars().count() + 5);
                let truncated_desc: String = desc.chars().take(max_desc).collect();

                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("  {} ", agent.name),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(truncated_desc, Style::default().fg(Color::DarkGray)),
                ]))
            })
            .collect();

        let list = List::new(items).highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );

        let mut list_state = ListState::default();
        list_state.select(Some(selected));
        frame.render_stateful_widget(list, chunks[2], &mut list_state);
    }

    // Footer hints
    let hints = Paragraph::new(Line::from(vec![Span::styled(
        " ↑↓ navigate  Enter select  Esc cancel",
        Style::default().fg(Color::DarkGray),
    )]));
    frame.render_widget(hints, chunks[3]);
}

pub fn render_resume_picker(
    frame: &mut Frame,
    mode: &AppMode,
    filtered: &[&ConversationSummary],
    selected: usize,
) {
    let AppMode::ResumePicker(state) = mode else {
        return;
    };

    let area = frame.area();
    let popup = centered_rect(60, 60, area);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Resume Conversation ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let chunks = Layout::vertical([
        Constraint::Length(1), // search input
        Constraint::Length(1), // separator
        Constraint::Min(1),    // conversation list
        Constraint::Length(1), // footer hints
    ])
    .split(inner);

    // Search input
    let filter_display = if state.filter.is_empty() {
        Span::styled("Type to filter...", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(
            format!(" {}", state.filter),
            Style::default().fg(Color::White),
        )
    };
    let search = Paragraph::new(Line::from(vec![
        Span::styled(" / ", Style::default().fg(Color::Cyan)),
        filter_display,
    ]));
    frame.render_widget(search, chunks[0]);

    // Separator
    let sep = Paragraph::new(Line::from("─".repeat(inner.width as usize)))
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(sep, chunks[1]);

    // Conversation list or loading/empty state
    if state.loading {
        let loading = Paragraph::new(Line::from(Span::styled(
            "  Fetching conversations...",
            Style::default().fg(Color::Yellow),
        )));
        frame.render_widget(loading, chunks[2]);
    } else if filtered.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "  No conversations match",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, chunks[2]);
    } else {
        let items: Vec<ListItem> = filtered
            .iter()
            .map(|conv| {
                let title = conv.title.as_deref().unwrap_or("(untitled)");
                let relative_date = format_relative_time(conv.created);
                let max_date_len = (inner.width as usize).saturating_sub(title.len() + 5);
                let truncated_date: String = relative_date.chars().take(max_date_len).collect();

                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("  {title} "),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(truncated_date, Style::default().fg(Color::DarkGray)),
                ]))
            })
            .collect();

        let list = List::new(items).highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );

        let mut list_state = ListState::default();
        list_state.select(Some(selected));
        frame.render_stateful_widget(list, chunks[2], &mut list_state);
    }

    // Footer hints
    let hints = Paragraph::new(Line::from(vec![Span::styled(
        " ↑↓ navigate  Enter select  Esc cancel",
        Style::default().fg(Color::DarkGray),
    )]));
    frame.render_widget(hints, chunks[3]);
}

fn format_relative_time(timestamp_ms: i64) -> String {
    #[allow(clippy::cast_possible_truncation)]
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);

    let elapsed_ms = now - timestamp_ms;
    let elapsed_secs = elapsed_ms / 1000;

    if elapsed_secs < 60 {
        "just now".to_string()
    } else if elapsed_secs < 3600 {
        let mins = elapsed_secs / 60;
        format!("{} minute{} ago", mins, if mins == 1 { "" } else { "s" })
    } else if elapsed_secs < 86400 {
        let hours = elapsed_secs / 3600;
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else {
        let days = elapsed_secs / 86400;
        format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_rect_is_smaller_than_area() {
        let area = Rect::new(0, 0, 100, 50);
        let popup = centered_rect(60, 60, area);
        assert!(popup.width <= 60);
        assert!(popup.height <= 30);
        assert!(popup.x > 0);
        assert!(popup.y > 0);
    }

    #[test]
    fn centered_rect_is_centered() {
        let area = Rect::new(0, 0, 100, 50);
        let popup = centered_rect(60, 60, area);
        // Center should be roughly at area center
        let center_x = popup.x + popup.width / 2;
        let center_y = popup.y + popup.height / 2;
        assert!((center_x as i32 - 50).unsigned_abs() <= 1);
        assert!((center_y as i32 - 25).unsigned_abs() <= 1);
    }
}
