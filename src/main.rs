mod app;
mod auth;
mod cli;
mod config;
mod dust;
mod event;
mod handler;
mod input_buffer;
mod ui;

use std::io;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;

use crate::app::App;
use crate::auth::device_flow::build_http_client;
use crate::auth::workspace_selection;
use crate::cli::{Cli, Command};
use crate::config::Config;
use crate::dust::client::{DustClient, DustEvent, resolve_agent_id};
use crate::event::{AppEvent, EventReader};
use crate::handler::{ActionOutcome, apply_action, handle_key_event};
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
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Login) => auth::device_flow::login().await?,
        Some(Command::Logout) => auth::logout()?,
        Some(Command::Status) => auth::status().await?,
        None => run_tui().await?,
    }

    Ok(())
}

fn install_terminal_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));
}

async fn run_tui() -> io::Result<()> {
    let http = build_http_client().map_err(|error| io::Error::other(error.to_string()))?;
    workspace_selection::ensure_workspace_selected_with_client(&http)
        .await
        .map_err(|error| io::Error::other(error.to_string()))?;
    install_terminal_panic_hook();

    let mut terminal = setup_terminal()?;
    let config = Config::load().map_err(|error| io::Error::other(error.to_string()))?;
    let agent_name = resolve_agent_id(config.agent_id(), std::env::var("OXIDE_AGENT_ID").ok());
    let mut app = App::new(&agent_name);
    let mut events = EventReader::new(Duration::from_millis(250));
    let mut input = InputBuffer::new();
    let (dust_tx, mut dust_rx) = mpsc::unbounded_channel::<DustEvent>();
    let client = DustClient::from_env().ok();
    let mut pending_submit: Option<String> = None;

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
                    let outcome: ActionOutcome = apply_action(&mut app, &mut input, action);
                    if let Some(content) = outcome.submit {
                        pending_submit = Some(content);
                    }
                }
                AppEvent::Tick => {}
            }
        }

        while let Ok(message) = dust_rx.try_recv() {
            match message {
                DustEvent::Token(token) => app.append_agent_token(&token),
                DustEvent::Complete(content) => app.complete_stream(content.as_deref()),
                DustEvent::Error(error) => app.push_system_message(&error),
                DustEvent::ConversationCreated(conversation_id) => {
                    app.set_conversation_id(conversation_id);
                }
            }
        }

        if let Some(content) = pending_submit.take() {
            if let Some(client) = client.clone() {
                let conversation_id = app.conversation_id().map(ToOwned::to_owned);
                let dust_tx = dust_tx.clone();
                tokio::spawn(async move {
                    if let Err(error) = client
                        .send_message_flow(conversation_id, content, dust_tx.clone())
                        .await
                    {
                        let _ = dust_tx.send(DustEvent::Error(error.to_string()));
                    }
                });
            } else {
                let _ = dust_tx.send(DustEvent::Error(
                    "Dust client could not be initialised. Try running `oxide login` again."
                        .to_string(),
                ));
            }
        }

        if app.should_quit() {
            break;
        }
    }

    restore_terminal(&mut terminal);
    Ok(())
}
