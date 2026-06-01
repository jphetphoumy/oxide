mod app;
mod event;
mod handler;
mod input_buffer;
mod ui;

use std::io;
use std::time::Duration;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::App;
use crate::event::{AppEvent, EventReader};
use crate::handler::{apply_action, handle_key_event};
use crate::input_buffer::InputBuffer;
use crate::ui::{input_height, render_input, render_layout, render_messages};

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) {
    let _ = disable_raw_mode();
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));

    let mut terminal = setup_terminal()?;
    let mut app = App::new("echo-bot", "mock");
    let mut events = EventReader::new(Duration::from_millis(250));
    let mut input = InputBuffer::new();

    loop {
        terminal.draw(|frame| {
            let lines: Vec<String> = input.lines().iter().map(|s| (*s).to_string()).collect();
            let input_h = input_height(&lines, frame.area().width, frame.area().height);
            let layout = render_layout(frame, &app, input_h);
            render_messages(frame, &app, layout.messages);
            render_input(frame, &input, layout.input);
        })?;

        if let Some(event) = events.next().await {
            match event {
                AppEvent::Key(key) => {
                    let action = handle_key_event(key);
                    apply_action(&mut app, &mut input, action);
                }
                AppEvent::Tick => {}
            }
        }

        if app.should_quit() {
            break;
        }
    }

    restore_terminal(&mut terminal);
    Ok(())
}
