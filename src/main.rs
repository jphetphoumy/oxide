mod app;
mod auth;
mod cli;
mod config;
mod dust;
mod event;
mod handler;
mod input_buffer;
mod observability;
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

use crate::app::{App, AppMode};
use crate::auth::device_flow::build_http_client;
use crate::auth::workspace_selection;
use crate::cli::{Cli, Command};
use crate::config::Config;
use crate::dust::client::{DustClient, DustEvent, resolve_agent_id};
use crate::dust::types::AgentInfo;
use crate::event::{AppEvent, EventReader};
use crate::handler::{
    ActionOutcome, PickerAction, SlashCommand, apply_action, handle_key_event, handle_picker_key,
};
use crate::input_buffer::InputBuffer;
use crate::ui::{input_height, render_input, render_layout, render_messages, render_picker};

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
    let (_log_guard, log_path) = observability::init()?;
    tracing::debug!(log_path = %log_path.display(), "starting oxide");

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

#[allow(clippy::too_many_lines)]
async fn run_tui() -> io::Result<()> {
    let http = build_http_client().map_err(|error| io::Error::other(error.to_string()))?;
    workspace_selection::ensure_workspace_selected_with_client(&http)
        .await
        .map_err(|error| io::Error::other(error.to_string()))?;
    install_terminal_panic_hook();

    let mut terminal = setup_terminal()?;
    let config = Config::load().map_err(|error| io::Error::other(error.to_string()))?;
    let agent_name = resolve_agent_id(config.agent_id(), std::env::var("OXIDE_AGENT_ID").ok());
    let cwd = std::env::current_dir()?;
    let home_dir = dirs::home_dir();
    let mut app = App::new(&agent_name, cwd, home_dir);
    let mut events = EventReader::new(Duration::from_millis(250));
    let mut input = InputBuffer::new();
    let (dust_tx, mut dust_rx) = mpsc::unbounded_channel::<DustEvent>();
    let (agent_tx, mut agent_rx) = mpsc::unbounded_channel::<Vec<AgentInfo>>();
    let mut client = DustClient::from_env().ok();
    let mut pending_submit: Option<String> = None;

    loop {
        terminal.draw(|frame| {
            let lines: Vec<String> = input.lines().iter().map(|s| (*s).to_string()).collect();
            let input_h = input_height(&lines, frame.area().width, frame.area().height);
            let layout = render_layout(frame, &app, input_h);
            render_messages(frame, &app, layout.messages);
            render_input(frame, &input, layout.input);

            let filtered = app.picker_filtered_agents();
            let selected = app.picker_selected();
            render_picker(frame, app.mode(), &filtered, selected);
        })?;

        tokio::select! {
            event = events.next() => {
                match event {
                    Some(AppEvent::Key(key)) => {
                        if matches!(app.mode(), AppMode::Picker(_)) {
                            let picker_action = handle_picker_key(key);
                            match picker_action {
                                PickerAction::Cancel => app.exit_picker(),
                                PickerAction::Select => {
                                    let filtered = app.picker_filtered_agents();
                                    let selected = app.picker_selected();
                                    if let Some(agent) = filtered.get(selected) {
                                        let agent_id = agent.s_id.clone();
                                        let agent_name = agent.name.clone();
                                        app.switch_agent(&agent_id, &agent_name);
                                        if let Some(ref mut c) = client {
                                            c.set_agent(&agent_id);
                                        }
                                    }
                                }
                                PickerAction::MoveUp => app.picker_move_selection(-1),
                                PickerAction::MoveDown => app.picker_move_selection(1),
                                PickerAction::Type(c) => {
                                    if let AppMode::Picker(state) = app.mode() {
                                        let mut filter = state.filter.clone();
                                        filter.push(c);
                                        app.set_picker_filter(&filter);
                                    }
                                }
                                PickerAction::Backspace => {
                                    if let AppMode::Picker(state) = app.mode() {
                                        let mut filter = state.filter.clone();
                                        filter.pop();
                                        app.set_picker_filter(&filter);
                                    }
                                }
                                PickerAction::None => {}
                            }
                        } else {
                            let action = handle_key_event(key);
                            let outcome: ActionOutcome = apply_action(&mut app, &mut input, action);
                            if let Some(content) = outcome.submit {
                                pending_submit = Some(content);
                            }
                            if outcome.slash_command == Some(SlashCommand::Switch) {
                                app.enter_picker();
                                if let Some(c) = client.clone() {
                                    let tx = agent_tx.clone();
                                    tokio::spawn(async move {
                                        match c.list_agents().await {
                                            Ok(agents) => { let _ = tx.send(agents); }
                                            Err(e) => {
                                                tracing::error!(error = %e, "failed to list agents");
                                            }
                                        }
                                    });
                                }
                            }
                        }
                    }
                    Some(AppEvent::Tick) => {}
                    None => break,
                }
            }
            message = dust_rx.recv() => {
                if let Some(message) = message {
                    match message {
                        DustEvent::Token(token) => app.append_agent_token(&token),
                        DustEvent::Complete(content) => app.complete_stream(content.as_deref()),
                        DustEvent::Error(error) => app.push_system_message(&error),
                        DustEvent::ConversationCreated(conversation_id) => {
                            app.set_conversation_id(conversation_id);
                        }
                    }
                } else {
                    break;
                }
            }
            agents = agent_rx.recv() => {
                if let Some(agents) = agents {
                    app.set_picker_agents(agents);
                }
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
