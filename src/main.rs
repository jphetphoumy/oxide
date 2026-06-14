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
mod skills;
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

use std::sync::Arc;

use crate::app::{App, AppMode, McpApproveInfo};
use crate::auth::device_flow::build_http_client;
use crate::auth::workspace_selection;
use crate::cli::{Cli, Command};
use crate::config::Config;
use crate::dust::client::{DustClient, DustEvent, resolve_agent_id};
use crate::dust::types::AgentInfo;
use crate::event::{AppEvent, EventReader};
use crate::handler::{
    Action, ActionOutcome, PickerAction, SlashCommand, apply_action, handle_key_event,
    handle_mouse_event, handle_picker_key, handle_tool_approval_key,
};
use crate::input_buffer::InputBuffer;
use crate::mcp::McpManager;
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
        Some(Command::McpServer) => run_mcp_server().await?,
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

    // Discover and register skills at startup (before MCP manager init)
    let skills = skills::discover_skills(std::path::Path::new(skills::SKILLS_DIR));

    // Create Dust client early so we can pass it to MCP manager
    let mut client = DustClient::from_env().ok();

    let mcp_manager = Arc::new(tokio::sync::Mutex::new(
        McpManager::init(config.mcp(), skills.clone(), client.clone())
            .await
            .map_err(|error| io::Error::other(error.to_string()))?,
    ));
    let agent_name = resolve_agent_id(config.agent_id(), std::env::var("OXIDE_AGENT_ID").ok());
    let cwd = std::env::current_dir()?;
    let home_dir = dirs::home_dir();
    let mut app = App::new(&agent_name, cwd, home_dir);
    app.set_auto_approve(config.mcp().auto_approve);

    // Register skills with slash commands
    app.set_skills(skills.clone());
    slash::register_skill_commands(&skills);

    let mut events = EventReader::new(Duration::from_millis(250));
    let mut input = InputBuffer::new();
    let (dust_tx, mut dust_rx) = mpsc::unbounded_channel::<DustEvent>();
    let (agent_tx, mut agent_rx) = mpsc::unbounded_channel::<Vec<AgentInfo>>();
    let mut pending_submit: Option<String> = None;

    // Set event_tx on MCP manager for SubagentStarted/Finished events
    mcp_manager.lock().await.set_event_tx(dust_tx.clone());

    // Spawn MCP transport: registers oxide-fs with Dust and receives tool calls via SSE
    let (mcp_server_id_tx, mut mcp_server_id_rx) = mpsc::unbounded_channel::<String>();
    if !mcp_manager.lock().await.list_tools().is_empty()
        && let Some(ref dust_client) = client
    {
        let (tool_call_tx, mut tool_call_rx) = mpsc::unbounded_channel::<crate::mcp::ToolCall>();
        let transport = crate::mcp::McpTransport::new(
            http.clone(),
            dust_client.base_url().to_string(),
            dust_client.workspace_id().to_string(),
            mcp_manager.clone(),
            tool_call_tx,
            mcp_server_id_tx,
        );
        let dust_tx_mcp = dust_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = transport.run().await {
                tracing::error!(error = %e, "MCP transport error");
            }
        });
        // Forward MCP tool calls to the main event loop for direct execution
        tokio::spawn(async move {
            while let Some(tool_call) = tool_call_rx.recv().await {
                let _ = dust_tx_mcp.send(DustEvent::McpToolUse(tool_call));
            }
        });
    }

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
            Some(server_id) = mcp_server_id_rx.recv() => {
                tracing::info!(server_id = %server_id, "MCP server registered, adding to client context");
                if let Some(ref mut c) = client {
                    c.set_mcp_server_id(server_id);
                }
            }
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
                                let action = handle_tool_approval_key(key);
                                match action {
                                    Action::ApproveTool => {
                                        if let Some(state) = app.current_tool_approval_state().cloned() {
                                            let tool_call = state.tool_call.clone();
                                            app.exit_tool_approval();
                                            let dust_client = client.clone();
                                            let mcp = mcp_manager.clone();
                                            let dust_tx = dust_tx.clone();
                                            if let Some(mcp_info) = state.mcp_approve {
                                                // MCP flow: just approve via validate_action
                                                tokio::spawn(async move {
                                                    if let Some(c) = dust_client
                                                        && let Err(e) = c.validate_action(
                                                            &mcp_info.conversation_id,
                                                            &mcp_info.message_id,
                                                            &mcp_info.action_id,
                                                            true,
                                                        ).await {
                                                            tracing::error!(error = %e, "failed to validate MCP action");
                                                        }
                                                });
                                            } else {
                                                // Old non-MCP flow: execute + submit result + resume stream
                                                let tool_name = tool_call.name.clone();
                                                let input_json = tool_call.input.clone();
                                                let tool_use_id = tool_call.id.clone();
                                                let conversation_id = app.conversation_id().map(ToString::to_string);
                                                let user_message_id = app.user_message_id().map(ToString::to_string);
                                                tokio::spawn(async move {
                                                    match mcp.lock().await.call_tool(&tool_name, input_json).await {
                                                        Ok(mut result) => {
                                                            result.tool_use_id = tool_use_id;
                                                            if let (Some(conv_id), Some(c)) =
                                                                (conversation_id.as_ref(), dust_client.as_ref())
                                                            {
                                                                if let Err(e) = c.submit_tool_result(conv_id, &result).await {
                                                                    tracing::error!(error = %e, "failed to submit tool result");
                                                                } else if let (Some(user_msg_id), Some(conv_id_str)) = (&user_message_id, &conversation_id)
                                                                    && let Err(e) = c.resume_message_stream(conv_id_str, user_msg_id, dust_tx.clone()).await {
                                                                        tracing::error!(error = %e, "failed to resume message stream");
                                                                    }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            tracing::error!(tool = %tool_name, error = %e, "tool execution failed");
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                    }
                                    Action::DenyTool => {
                                        if let Some(state) = app.current_tool_approval_state().cloned() {
                                            let tool_call = state.tool_call.clone();
                                            app.exit_tool_approval();
                                            let dust_client = client.clone();
                                            if let Some(mcp_info) = state.mcp_approve {
                                                // MCP flow: reject via validate_action
                                                tokio::spawn(async move {
                                                    if let Some(c) = dust_client
                                                        && let Err(e) = c.validate_action(
                                                            &mcp_info.conversation_id,
                                                            &mcp_info.message_id,
                                                            &mcp_info.action_id,
                                                            false,
                                                        ).await {
                                                            tracing::error!(error = %e, "failed to reject MCP action");
                                                        }
                                                });
                                            } else {
                                                // Old non-MCP flow: submit denial result
                                                let tool_use_id = tool_call.id.clone();
                                                let conversation_id = app.conversation_id().map(ToString::to_string);
                                                let denial_result = crate::mcp::ToolResult {
                                                    tool_use_id,
                                                    content: "denied by user".to_string(),
                                                    is_error: true,
                                                };
                                                tokio::spawn(async move {
                                                    if let (Some(conv_id), Some(c)) =
                                                        (conversation_id.as_ref(), dust_client.as_ref())
                                                        && let Err(e) = c.submit_tool_result(conv_id, &denial_result).await {
                                                            tracing::error!(error = %e, "failed to submit denial result");
                                                        }
                                                });
                                            }
                                        }
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
                                Some(SlashCommand::ActivateSkill(id)) => {
                                    app.activate_skill(&id);
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
                        DustEvent::UserMessageCreated(user_message_id) => {
                            app.set_user_message_id(user_message_id);
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
                        // Old non-MCP inline tool use from conversation SSE
                        DustEvent::ToolUse(tool_call) => {
                            let should_auto_approve = app.auto_approve_tools() || McpManager::is_builtin_tool(&tool_call.name);
                            if should_auto_approve {
                                let tool_name = tool_call.name.clone();
                                let input_json = tool_call.input.clone();
                                let tool_use_id = tool_call.id.clone();
                                let conversation_id = app.conversation_id().map(ToString::to_string);
                                let user_message_id = app.user_message_id().map(ToString::to_string);
                                let dust_client = client.clone();
                                let mcp = mcp_manager.clone();
                                let dust_tx = dust_tx.clone();
                                tokio::spawn(async move {
                                    match mcp.lock().await.call_tool(&tool_name, input_json).await {
                                        Ok(mut result) => {
                                            result.tool_use_id = tool_use_id;
                                            if let (Some(conv_id), Some(c)) =
                                                (conversation_id.as_ref(), dust_client.as_ref())
                                            {
                                                if let Err(e) = c.submit_tool_result(conv_id, &result).await {
                                                    tracing::error!(error = %e, "failed to submit tool result");
                                                } else if let (Some(user_msg_id), Some(conv_id_str)) = (&user_message_id, &conversation_id)
                                                    && let Err(e) = c.resume_message_stream(conv_id_str, user_msg_id, dust_tx.clone()).await {
                                                        tracing::error!(error = %e, "failed to resume message stream");
                                                    }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!(tool = %tool_name, error = %e, "tool execution failed");
                                        }
                                    }
                                });
                            } else {
                                app.enter_tool_approval(tool_call);
                            }
                        }
                        // MCP tool approval from conversation SSE: show UI, then validate_action
                        DustEvent::ToolApproveExecution { action_id, conversation_id, message_id, tool_name, inputs } => {
                            let fake_call = crate::mcp::ToolCall {
                                id: action_id.clone(),
                                name: tool_name.clone(),
                                input: inputs,
                            };
                            let mcp_info = McpApproveInfo { action_id, conversation_id, message_id };
                            let should_auto_approve = app.auto_approve_tools() || McpManager::is_builtin_tool(&tool_name);
                            if should_auto_approve {
                                tracing::debug!(action_id = %mcp_info.action_id, "auto-approving MCP tool");
                                let dust_client = client.clone();
                                tokio::spawn(async move {
                                    if let Some(c) = dust_client
                                        && let Err(e) = c.validate_action(
                                            &mcp_info.conversation_id,
                                            &mcp_info.message_id,
                                            &mcp_info.action_id,
                                            true,
                                        ).await {
                                            tracing::error!(error = %e, "failed to auto-validate MCP action");
                                        }
                                });
                            } else {
                                app.enter_mcp_tool_approval(fake_call, mcp_info);
                            }
                        }
                        // MCP tool call from MCP transport: execute, post result, then resume stream
                        DustEvent::McpToolUse(tool_call) => {
                            tracing::debug!(tool = %tool_call.name, "executing MCP tool call");
                            let tool_name = tool_call.name.clone();
                            let input_json = tool_call.input.clone();
                            let tool_use_id = tool_call.id.clone();
                            let dust_client = client.clone();
                            let mcp = mcp_manager.clone();
                            let conversation_id = app.conversation_id().map(ToString::to_string);
                            let user_message_id = app.user_message_id().map(ToString::to_string);
                            let resume_tx = dust_tx.clone();
                            tokio::spawn(async move {
                                let (content, is_error) =
                                    match mcp.lock().await.call_tool(&tool_name, input_json).await {
                                        Ok(result) => (result.content, result.is_error),
                                        Err(e) => {
                                            tracing::error!(tool = %tool_name, error = %e, "MCP tool execution failed");
                                            (format!("error: {e}"), true)
                                        }
                                    };
                                if let Some(c) = dust_client {
                                    if let Err(e) = c.post_mcp_result(&tool_use_id, &content, is_error).await {
                                        tracing::error!(error = %e, "failed to post MCP tool result");
                                        return;
                                    }
                                    // Resume the conversation SSE to get Dust's continued response.
                                    // The server closes the SSE while waiting for tool execution,
                                    // so we must reopen it after posting the result.
                                    if let (Some(conv_id), Some(user_msg_id)) =
                                        (conversation_id.as_deref(), user_message_id.as_deref())
                                        && let Err(e) = c.resume_message_stream(conv_id, user_msg_id, resume_tx).await {
                                            tracing::error!(error = %e, "failed to resume stream after MCP tool");
                                        }
                                }
                            });
                        }
                        DustEvent::SubagentStarted { .. } => {
                            app.increment_subagent();
                        }
                        DustEvent::SubagentFinished { .. } => {
                            app.decrement_subagent();
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
                DustEvent::UserMessageCreated(user_message_id) => {
                    app.set_user_message_id(user_message_id);
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
                let active_skills = app.active_skills().to_vec();
                tokio::spawn(async move {
                    if let Err(error) = client
                        .send_message_flow_with_skills(
                            conversation_id,
                            content,
                            dust_tx.clone(),
                            &active_skills,
                        )
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

#[allow(clippy::future_not_send)]
async fn run_mcp_server() -> Result<()> {
    use crate::mcp::McpJsonRpcServer;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let config = Config::load().map_err(|error| io::Error::other(error.to_string()))?;
    let skills = skills::discover_skills(std::path::Path::new(skills::SKILLS_DIR));
    // Attempt to build a Dust client for subagent support (may fail if not logged in)
    let dust_client = crate::dust::client::DustClient::from_env().ok();
    let mcp_manager = Arc::new(Mutex::new(
        McpManager::init(config.mcp(), skills, dust_client)
            .await
            .map_err(|error| io::Error::other(error.to_string()))?,
    ));

    let server = McpJsonRpcServer::new(mcp_manager);
    server.run().await?;

    Ok(())
}
