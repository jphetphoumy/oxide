use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::app::AppMode;

pub fn render_tool_approval(frame: &mut Frame, mode: &AppMode) {
    let AppMode::ToolApproval(state) = mode else {
        return;
    };

    let area = frame.area();
    let popup = centered_rect(70, 70, area);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Tool Request ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let chunks = Layout::vertical([
        Constraint::Length(2), // tool name and desc
        Constraint::Length(1), // separator
        Constraint::Min(3),    // input/details
        Constraint::Length(1), // separator
        Constraint::Length(2), // prompts
    ])
    .split(inner);

    // Tool name header
    let tool_name = Paragraph::new(Line::from(vec![
        Span::styled("⚙ ", Style::default().fg(Color::Yellow)),
        Span::styled(
            &state.tool_call.name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    frame.render_widget(tool_name, chunks[0]);

    // Separator
    let sep = Paragraph::new(Line::from("─".repeat(inner.width as usize)))
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(sep, chunks[1]);

    // Input details (formatted JSON)
    let input_str = format_input(&state.tool_call.input);
    let input_display = Paragraph::new(input_str)
        .style(Style::default().fg(Color::Cyan))
        .wrap(Wrap { trim: true });
    frame.render_widget(input_display, chunks[2]);

    // Another separator
    let sep2 = Paragraph::new(Line::from("─".repeat(inner.width as usize)))
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(sep2, chunks[3]);

    // Prompts
    let prompts = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("[y] approve", Style::default().fg(Color::Green)),
            Span::raw("  "),
            Span::styled("[n] deny", Style::default().fg(Color::Red)),
            Span::raw("  "),
            Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
    ]);
    frame.render_widget(prompts, chunks[4]);
}

fn format_input(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(obj) => {
            let mut lines = vec!["Arguments:".to_string()];
            for (key, val) in obj {
                let val_str = match val {
                    serde_json::Value::String(s) => format!("\"{s}\""),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Null => "null".to_string(),
                    _ => serde_json::to_string(val).unwrap_or_else(|_| "?".to_string()),
                };
                lines.push(format!("  {key} = {val_str}"));
            }
            lines.join("\n")
        }
        _ => format!("  {value}"),
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
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
