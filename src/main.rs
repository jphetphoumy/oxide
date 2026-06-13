mod app;
mod auth;
mod cli;
mod config;
mod dust;
mod event;
mod handler;
mod input_buffer;
mod mcp;
mod observability;
mod slash;
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
    ActionOutcome, PickerAction, SlashCommand, apply_action, handle_key_event, handle_mouse_event,
    handle_picker_key,
};
use crate::input_buffer::InputBuffer;
use crate::ui::{
    input_height, render_command_menu, render_input, render_layout, render_messages, render_picker,
    render_resume_picker,
};

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
            render_command_menu(frame, input.content(), layout.input);

            let filtered = app.picker_filtered_agents();
            let selected = app.picker_selected();
            render_picker(frame, app.mode(), &filtered, selected);

            let filtered_convs = app.resume_filtered_conversations();
            let selected_conv = app.resume_picker_selected();
            render_resume_picker(frame, app.mode(), &filtered_convs, selected_conv);
        })?;

        tokio::select! {
            event = events.next() => {
                match event {
                    Some(AppEvent::Key(key)) => {
                        match app.mode() {
                            AppMode::Picker(_) => {
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
                            }
                            AppMode::ResumePicker(_) => {
                                let picker_action = handle_picker_key(key);
                                match picker_action {
                                    PickerAction::Cancel => app.exit_resume_picker(),
                                    PickerAction::Select => {
                                        let filtered = app.resume_filtered_conversations();
                                        let selected = app.resume_picker_selected();
                                        if let Some(conv) = filtered.get(selected) {
                                            let conversation_id = conv.s_id.clone();
                                            let title = conv.title.clone();
                                            if let Some(c) = client.clone() {
                                                let tx = dust_tx.clone();
                                                tokio::spawn(async move {
                                                    match c.get_conversation(&conversation_id).await {
                                                        Ok(conversation) => {
                                                            let messages: Vec<(String, String)> = conversation
                                                                .content
                                                                .iter()
                                                                .flat_map(|group| group.iter())
                                                                .filter_map(|msg| {
                                                                    match msg {
                                                                        crate::dust::types::ConversationMessage::UserMessage { content } => {
                                                                            Some(("user".to_string(), content.clone()))
                                                                        }
                                                                        crate::dust::types::ConversationMessage::AgentMessage { content, .. } => {
                                                                            content.as_ref().map(|c| ("agent".to_string(), c.clone()))
                                                                        }
                                                                        crate::dust::types::ConversationMessage::Other => None,
                                                                    }
                                                                })
                                                                .collect();
                                                            let _ = tx.send(DustEvent::ConversationLoaded {
                                                                conversation_id,
                                                                title,
                                                                messages,
                                                            });
                                                        }
                                                        Err(e) => {
                                                            tracing::error!(error = %e, "failed to get conversation");
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                    }
                                    PickerAction::MoveUp => app.resume_picker_move_selection(-1),
                                    PickerAction::MoveDown => app.resume_picker_move_selection(1),
                                    PickerAction::Type(c) => {
                                        if let AppMode::ResumePicker(state) = app.mode() {
                                            let mut filter = state.filter.clone();
                                            filter.push(c);
                                            app.set_resume_filter(&filter);
                                        }
                                    }
                                    PickerAction::Backspace => {
                                        if let AppMode::ResumePicker(state) = app.mode() {
                                            let mut filter = state.filter.clone();
                                            filter.pop();
                                            app.set_resume_filter(&filter);
                                        }
                                    }
                                    PickerAction::None => {}
                                }
                            }
                            AppMode::ToolApproval(_) => {
                                match key.code {
                                    crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Enter => {
                                        if let Some(tool_call) = app.current_tool_call() {
                                            let tool_name = tool_call.name.clone();
                                            let input_json = tool_call.input.clone();
                                            let tool_use_id = tool_call.id.clone();
                                            app.exit_tool_approval();

                                            // Tool execution will be handled after we can initialize McpManager
                                            tracing::debug!(tool_name = %tool_name, "tool approved by user");
                                        }
                                    }
                                    crossterm::event::KeyCode::Char('n') | crossterm::event::KeyCode::Esc => {
                                        app.exit_tool_approval();
                                        tracing::debug!("tool denied by user");
                                    }
                                    _ => {}
                                }
                            }
                            AppMode::Chat => {
                            let action = handle_key_event(key);
                            let outcome: ActionOutcome = apply_action(&mut app, &mut input, action);
                            if let Some(content) = outcome.submit {
                                pending_submit = Some(content);
                            }
                            match outcome.slash_command {
                                Some(SlashCommand::New) => app.new_conversation(),
                                Some(SlashCommand::Switch) => {
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
                                Some(SlashCommand::Resume) => {
                                    app.enter_resume_picker();
                                    if let Some(c) = client.clone() {
                                        let tx = dust_tx.clone();
                                        tokio::spawn(async move {
                                            match c.list_conversations().await {
                                                Ok(conversations) => {
                                                    let _ = tx.send(DustEvent::ConversationsListed(conversations));
                                                }
                                                Err(e) => {
                                                    tracing::error!(error = %e, "failed to list conversations");
                                                }
                                            }
                                        });
                                    }
                                }
                                None => {}
                            }
                            }
                        }
                    }
                    Some(AppEvent::Mouse(mouse)) => {
                        if matches!(app.mode(), AppMode::Chat) {
                            let action = handle_mouse_event(mouse);
                            let _ = apply_action(&mut app, &mut input, action);
                        }
                    }
                    Some(AppEvent::Tick) => {}
                    None => break,
                }
            }
            message = dust_rx.recv() => {
                if let Some(message) = message {
                    match message {
                        DustEvent::Token(token, conv_id) if conv_id == app.conversation_id().map(ToString::to_string) => {
                            app.append_agent_token(&token);
                        }
                        DustEvent::Complete(content, conv_id) if conv_id == app.conversation_id().map(ToString::to_string) => {
                            app.complete_stream(content.as_deref());
                        }
                        DustEvent::Error(error) => app.push_system_message(&error),
                        DustEvent::ConversationCreated(conversation_id) => {
                            app.set_conversation_id(conversation_id);
                        }
                        DustEvent::ConversationsListed(conversations) => {
                            app.set_resume_conversations(conversations);
                        }
                        DustEvent::ConversationLoaded {
                            conversation_id,
                            title,
                            messages,
                        } => {
                            let role_messages: Vec<_> = messages
                                .into_iter()
                                .map(|(role_str, content)| {
                                    let role = match role_str.as_str() {
                                        "user" => app::Role::User,
                                        "system" => app::Role::System,
                                        _ => app::Role::Agent("agent".to_string()),
                                    };
                                    (role, content)
                                })
                                .collect();
                            app.restore_conversation(conversation_id, role_messages, title.as_deref());
                        }
                        _ => {}
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
                DustEvent::Token(token, conv_id)
                    if conv_id == app.conversation_id().map(ToString::to_string) =>
                {
                    app.append_agent_token(&token);
                }
                DustEvent::Complete(content, conv_id)
                    if conv_id == app.conversation_id().map(ToString::to_string) =>
                {
                    app.complete_stream(content.as_deref());
                }
                DustEvent::Error(error) => app.push_system_message(&error),
                DustEvent::ConversationCreated(conversation_id) => {
                    app.set_conversation_id(conversation_id);
                }
                DustEvent::ConversationsListed(conversations) => {
                    app.set_resume_conversations(conversations);
                }
                DustEvent::ConversationLoaded {
                    conversation_id,
                    title,
                    messages,
                } => {
                    let role_messages: Vec<_> = messages
                        .into_iter()
                        .map(|(role_str, content)| {
                            let role = match role_str.as_str() {
                                "user" => app::Role::User,
                                "system" => app::Role::System,
                                _ => app::Role::Agent("agent".to_string()),
                            };
                            (role, content)
                        })
                        .collect();
                    app.restore_conversation(conversation_id, role_messages, title.as_deref());
                }
                _ => {}
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
