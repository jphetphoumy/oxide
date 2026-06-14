use std::path::{Path, PathBuf};

use crate::dust::types::{AgentInfo, ConversationSummary};
use crate::mcp::ToolCall;

#[derive(Debug, Clone)]
pub struct PickerState {
    pub agents: Vec<AgentInfo>,
    pub filter: String,
    pub selected: usize,
    pub loading: bool,
}

#[derive(Debug, Clone)]
pub struct ResumePickerState {
    pub conversations: Vec<ConversationSummary>,
    pub filter: String,
    pub selected: usize,
    pub loading: bool,
}

#[derive(Debug, Clone)]
#[allow(clippy::struct_field_names)]
pub struct McpApproveInfo {
    pub action_id: String,
    pub conversation_id: String,
    pub message_id: String,
}

#[derive(Debug, Clone)]
pub struct ToolApprovalState {
    pub tool_call: ToolCall,
    /// Present when this approval is for an MCP tool — on approve, call `validate_action`.
    pub mcp_approve: Option<McpApproveInfo>,
}

#[derive(Debug, Clone)]
pub enum AppMode {
    Chat,
    Picker(PickerState),
    ResumePicker(ResumePickerState),
    ToolApproval(ToolApprovalState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    User,
    Agent(String),
    System,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

pub struct App {
    messages: Vec<Message>,
    scroll_offset: usize,
    agent_name: String,
    agent_id: String,
    cwd: PathBuf,
    home_dir: Option<PathBuf>,
    should_quit: bool,
    conversation_id: Option<String>,
    user_message_id: Option<String>,
    is_streaming: bool,
    mode: AppMode,
    auto_approve_tools: bool,
}

impl App {
    pub fn new(agent_name: &str, cwd: impl Into<PathBuf>, home_dir: Option<PathBuf>) -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            agent_name: agent_name.to_string(),
            agent_id: agent_name.to_string(),
            cwd: cwd.into(),
            home_dir,
            should_quit: false,
            conversation_id: None,
            user_message_id: None,
            is_streaming: false,
            mode: AppMode::Chat,
            auto_approve_tools: false,
        }
    }

    pub const fn set_auto_approve(&mut self, auto_approve: bool) {
        self.auto_approve_tools = auto_approve;
    }

    pub const fn auto_approve_tools(&self) -> bool {
        self.auto_approve_tools
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn agent_name(&self) -> &str {
        &self.agent_name
    }

    pub fn cwd(&self) -> &Path {
        self.cwd.as_path()
    }

    pub fn home_dir(&self) -> Option<&Path> {
        self.home_dir.as_deref()
    }

    pub fn conversation_id(&self) -> Option<&str> {
        self.conversation_id.as_deref()
    }

    pub fn user_message_id(&self) -> Option<&str> {
        self.user_message_id.as_deref()
    }

    pub fn set_user_message_id(&mut self, id: impl Into<String>) {
        self.user_message_id = Some(id.into());
    }

    pub const fn is_streaming(&self) -> bool {
        self.is_streaming
    }

    pub const fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub const fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub const fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn send_message(&mut self, content: &str) -> bool {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return false;
        }

        self.messages.push(Message {
            role: Role::User,
            content: trimmed.to_string(),
        });
        self.messages.push(Message {
            role: Role::Agent(self.agent_name.clone()),
            content: String::new(),
        });
        self.is_streaming = true;
        self.scroll_offset = 0;
        true
    }

    pub fn set_conversation_id(&mut self, conversation_id: impl Into<String>) {
        self.conversation_id = Some(conversation_id.into());
    }

    pub fn append_agent_token(&mut self, token: &str) {
        let was_at_bottom = self.scroll_offset == 0;
        if let Some(message) = self
            .messages
            .iter_mut()
            .rev()
            .find(|message| matches!(message.role, Role::Agent(_)))
        {
            message.content.push_str(token);
            if was_at_bottom {
                self.scroll_offset = 0;
            }
        }
    }

    pub fn complete_stream(&mut self, content: Option<&str>) {
        let was_at_bottom = self.scroll_offset == 0;
        if let Some(content) = content
            && let Some(message) = self
                .messages
                .iter_mut()
                .rev()
                .find(|message| matches!(message.role, Role::Agent(_)))
        {
            message.content = content.to_string();
        }

        self.is_streaming = false;
        if was_at_bottom {
            self.scroll_offset = 0;
        }
    }

    pub fn push_system_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: Role::System,
            content: content.to_string(),
        });
        self.is_streaming = false;
        self.scroll_offset = 0;
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub const fn mode(&self) -> &AppMode {
        &self.mode
    }

    pub fn enter_picker(&mut self) {
        self.mode = AppMode::Picker(PickerState {
            agents: Vec::new(),
            filter: String::new(),
            selected: 0,
            loading: true,
        });
    }

    pub fn exit_picker(&mut self) {
        self.mode = AppMode::Chat;
    }

    pub fn set_picker_agents(&mut self, agents: Vec<AgentInfo>) {
        if let AppMode::Picker(state) = &mut self.mode {
            state.agents = agents;
            state.loading = false;
            state.selected = 0;
        }
    }

    pub fn set_picker_filter(&mut self, filter: &str) {
        if let AppMode::Picker(state) = &mut self.mode {
            state.filter = filter.to_string();
            state.selected = 0;
        }
    }

    pub fn picker_filtered_agents(&self) -> Vec<&AgentInfo> {
        if let AppMode::Picker(state) = &self.mode {
            if state.filter.is_empty() {
                state.agents.iter().collect()
            } else {
                let filter = state.filter.to_lowercase();
                state
                    .agents
                    .iter()
                    .filter(|a| {
                        a.name.to_lowercase().contains(&filter)
                            || a.description.to_lowercase().contains(&filter)
                    })
                    .collect()
            }
        } else {
            Vec::new()
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn picker_selected(&self) -> usize {
        if let AppMode::Picker(state) = &self.mode {
            state.selected
        } else {
            0
        }
    }

    pub fn picker_move_selection(&mut self, delta: i32) {
        if let AppMode::Picker(state) = &mut self.mode {
            let count = if state.filter.is_empty() {
                state.agents.len()
            } else {
                let filter = state.filter.to_lowercase();
                state
                    .agents
                    .iter()
                    .filter(|a| {
                        a.name.to_lowercase().contains(&filter)
                            || a.description.to_lowercase().contains(&filter)
                    })
                    .count()
            };
            if count == 0 {
                return;
            }
            if delta > 0 {
                state.selected = (state.selected + 1) % count;
            } else {
                state.selected = state.selected.checked_sub(1).unwrap_or(count - 1);
            }
        }
    }

    pub fn switch_agent(&mut self, agent_id: &str, agent_name: &str) {
        self.agent_id = agent_id.to_string();
        self.agent_name = agent_name.to_string();
        self.push_system_message(&format!("Switched to {agent_name}"));
        self.mode = AppMode::Chat;
    }

    pub fn enter_resume_picker(&mut self) {
        self.mode = AppMode::ResumePicker(ResumePickerState {
            conversations: Vec::new(),
            filter: String::new(),
            selected: 0,
            loading: true,
        });
    }

    pub fn exit_resume_picker(&mut self) {
        self.mode = AppMode::Chat;
    }

    pub fn set_resume_conversations(&mut self, conversations: Vec<ConversationSummary>) {
        if let AppMode::ResumePicker(state) = &mut self.mode {
            state.conversations = conversations;
            state.loading = false;
            state.selected = 0;
        }
    }

    pub fn set_resume_filter(&mut self, filter: &str) {
        if let AppMode::ResumePicker(state) = &mut self.mode {
            state.filter = filter.to_string();
            state.selected = 0;
        }
    }

    pub fn resume_filtered_conversations(&self) -> Vec<&ConversationSummary> {
        if let AppMode::ResumePicker(state) = &self.mode {
            if state.filter.is_empty() {
                state.conversations.iter().collect()
            } else {
                let filter = state.filter.to_lowercase();
                state
                    .conversations
                    .iter()
                    .filter(|c| {
                        c.title.is_none()
                            || c.title
                                .as_ref()
                                .is_some_and(|t| t.to_lowercase().contains(&filter))
                    })
                    .collect()
            }
        } else {
            Vec::new()
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn resume_picker_selected(&self) -> usize {
        if let AppMode::ResumePicker(state) = &self.mode {
            state.selected
        } else {
            0
        }
    }

    pub fn resume_picker_move_selection(&mut self, delta: i32) {
        let count = self.resume_filtered_conversations().len();
        if count == 0 {
            return;
        }
        if let AppMode::ResumePicker(state) = &mut self.mode {
            if delta > 0 {
                state.selected = (state.selected + 1) % count;
            } else {
                state.selected = state.selected.checked_sub(1).unwrap_or(count - 1);
            }
        }
    }

    pub fn restore_conversation(
        &mut self,
        conversation_id: String,
        messages: Vec<(Role, String)>,
        title: Option<&str>,
    ) {
        self.conversation_id = Some(conversation_id);
        self.messages.clear();
        for (role, content) in messages {
            self.messages.push(Message { role, content });
        }
        self.is_streaming = false;
        self.scroll_offset = 0;
        self.mode = AppMode::Chat;
        let title_str = title.unwrap_or("(untitled)");
        self.push_system_message(&format!("Resumed conversation: {title_str}"));
    }

    pub fn new_conversation(&mut self) {
        self.messages.clear();
        self.is_streaming = false;
        self.conversation_id = None;
        self.scroll_offset = 0;
        self.push_system_message(&format!(
            "Started a new conversation with {}",
            self.agent_name
        ));
    }

    pub fn enter_tool_approval(&mut self, tool_call: ToolCall) {
        self.scroll_offset = 0;
        self.mode = AppMode::ToolApproval(ToolApprovalState {
            tool_call,
            mcp_approve: None,
        });
    }

    pub fn enter_mcp_tool_approval(&mut self, tool_call: ToolCall, mcp_approve: McpApproveInfo) {
        self.scroll_offset = 0;
        self.mode = AppMode::ToolApproval(ToolApprovalState {
            tool_call,
            mcp_approve: Some(mcp_approve),
        });
    }

    pub fn exit_tool_approval(&mut self) {
        self.mode = AppMode::Chat;
    }

    pub const fn current_tool_approval_state(&self) -> Option<&ToolApprovalState> {
        if let AppMode::ToolApproval(state) = &self.mode {
            Some(state)
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub const fn current_tool_call(&self) -> Option<&ToolCall> {
        if let AppMode::ToolApproval(state) = &self.mode {
            Some(&state.tool_call)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dust::types::AgentInfo;
    use crate::mcp::ToolCall;
    use std::path::PathBuf;

    #[test]
    fn new_app_has_empty_messages() {
        let app = App::new(
            "test-agent",
            "/workspace",
            Some(PathBuf::from("/home/alice")),
        );
        assert!(app.messages().is_empty());
    }

    #[test]
    fn new_app_stores_agent_name() {
        let app = App::new("my-agent", "/workspace", Some(PathBuf::from("/home/alice")));
        assert_eq!(app.agent_name(), "my-agent");
    }

    #[test]
    fn new_app_stores_cwd() {
        let app = App::new("my-agent", "/workspace", Some(PathBuf::from("/home/alice")));
        assert_eq!(app.cwd(), &PathBuf::from("/workspace"));
    }

    #[test]
    fn new_app_stores_home_dir() {
        let home_dir = PathBuf::from("/home/alice");
        let app = App::new("my-agent", "/workspace", Some(home_dir.clone()));
        assert_eq!(app.home_dir(), Some(home_dir.as_path()));
    }

    #[test]
    fn new_app_should_not_quit() {
        let app = App::new("a", "/workspace", None);
        assert!(!app.should_quit());
    }

    #[test]
    fn quit_sets_should_quit() {
        let mut app = App::new("a", "/workspace", None);
        app.quit();
        assert!(app.should_quit());
    }

    #[test]
    fn send_message_pushes_user_and_placeholder_agent_messages() {
        let mut app = App::new("echo-bot", "/workspace", None);
        assert!(app.send_message("hello"));
        let msgs = app.messages();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, Role::User);
        assert_eq!(msgs[0].content, "hello");
        assert_eq!(msgs[1].role, Role::Agent("echo-bot".to_string()));
        assert_eq!(msgs[1].content, "");
        assert!(app.is_streaming());
    }

    #[test]
    fn send_message_trims_whitespace() {
        let mut app = App::new("a", "/workspace", None);
        assert!(app.send_message("  hi  "));
        assert_eq!(app.messages()[0].content, "hi");
    }

    #[test]
    fn send_empty_message_does_nothing() {
        let mut app = App::new("a", "/workspace", None);
        assert!(!app.send_message(""));
        assert!(app.messages().is_empty());
        assert!(!app.send_message("   "));
        assert!(app.messages().is_empty());
    }

    #[test]
    fn scroll_up_increases_offset() {
        let mut app = App::new("a", "/workspace", None);
        app.scroll_up(3);
        assert_eq!(app.scroll_offset(), 3);
    }

    #[test]
    fn scroll_down_decreases_offset_to_zero() {
        let mut app = App::new("a", "/workspace", None);
        app.scroll_up(5);
        app.scroll_down(3);
        assert_eq!(app.scroll_offset(), 2);
        app.scroll_down(10);
        assert_eq!(app.scroll_offset(), 0);
    }

    #[test]
    fn send_message_resets_scroll() {
        let mut app = App::new("a", "/workspace", None);
        app.scroll_up(10);
        assert!(app.send_message("new msg"));
        assert_eq!(app.scroll_offset(), 0);
    }

    #[test]
    fn append_agent_token_updates_last_agent_message() {
        let mut app = App::new("a", "/workspace", None);
        assert!(app.send_message("hello"));
        app.append_agent_token("he");
        app.append_agent_token("llo");
        assert_eq!(app.messages()[1].content, "hello");
    }

    #[test]
    fn append_agent_token_preserves_scroll_when_not_at_bottom() {
        let mut app = App::new("a", "/workspace", None);
        assert!(app.send_message("hello"));
        app.scroll_up(3);
        app.append_agent_token("world");
        assert_eq!(app.scroll_offset(), 3);
    }

    #[test]
    fn complete_stream_can_replace_content() {
        let mut app = App::new("a", "/workspace", None);
        assert!(app.send_message("hello"));
        app.complete_stream(Some("final answer"));
        assert_eq!(app.messages()[1].content, "final answer");
        assert!(!app.is_streaming());
    }

    #[test]
    fn complete_stream_preserves_scroll_when_not_at_bottom() {
        let mut app = App::new("a", "/workspace", None);
        assert!(app.send_message("hello"));
        app.scroll_up(4);
        app.complete_stream(Some("final answer"));
        assert_eq!(app.scroll_offset(), 4);
    }

    #[test]
    fn new_app_starts_in_chat_mode() {
        let app = App::new("a", "/workspace", None);
        assert!(matches!(app.mode(), AppMode::Chat));
    }

    #[test]
    fn enter_picker_switches_mode() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_picker();
        assert!(matches!(app.mode(), AppMode::Picker(_)));
    }

    #[test]
    fn enter_picker_starts_loading() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_picker();
        if let AppMode::Picker(state) = app.mode() {
            assert!(state.loading);
            assert!(state.agents.is_empty());
            assert!(state.filter.is_empty());
            assert_eq!(state.selected, 0);
        } else {
            panic!("expected Picker mode");
        }
    }

    #[test]
    fn exit_picker_returns_to_chat() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_picker();
        app.exit_picker();
        assert!(matches!(app.mode(), AppMode::Chat));
    }

    #[test]
    fn set_picker_agents_updates_state() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_picker();
        let agents = vec![
            AgentInfo {
                s_id: "a1".into(),
                name: "dust".into(),
                description: "General".into(),
                scope: "workspace".into(),
            },
            AgentInfo {
                s_id: "a2".into(),
                name: "helper".into(),
                description: "Code".into(),
                scope: "published".into(),
            },
        ];
        app.set_picker_agents(agents);
        if let AppMode::Picker(state) = app.mode() {
            assert_eq!(state.agents.len(), 2);
            assert!(!state.loading);
        } else {
            panic!("expected Picker mode");
        }
    }

    #[test]
    fn picker_filter_narrows_results() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_picker();
        app.set_picker_agents(vec![
            AgentInfo {
                s_id: "a1".into(),
                name: "dust".into(),
                description: "".into(),
                scope: "".into(),
            },
            AgentInfo {
                s_id: "a2".into(),
                name: "helper".into(),
                description: "".into(),
                scope: "".into(),
            },
        ]);
        app.set_picker_filter("hel");
        let filtered = app.picker_filtered_agents();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "helper");
    }

    #[test]
    fn picker_filter_is_case_insensitive() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_picker();
        app.set_picker_agents(vec![AgentInfo {
            s_id: "a1".into(),
            name: "Dust".into(),
            description: "".into(),
            scope: "".into(),
        }]);
        app.set_picker_filter("dust");
        assert_eq!(app.picker_filtered_agents().len(), 1);
    }

    #[test]
    fn picker_selection_wraps() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_picker();
        app.set_picker_agents(vec![
            AgentInfo {
                s_id: "a1".into(),
                name: "one".into(),
                description: "".into(),
                scope: "".into(),
            },
            AgentInfo {
                s_id: "a2".into(),
                name: "two".into(),
                description: "".into(),
                scope: "".into(),
            },
        ]);
        app.picker_move_selection(1);
        assert_eq!(app.picker_selected(), 1);
        app.picker_move_selection(1); // wraps to 0
        assert_eq!(app.picker_selected(), 0);
    }

    #[test]
    fn picker_move_up_wraps_to_end() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_picker();
        app.set_picker_agents(vec![
            AgentInfo {
                s_id: "a1".into(),
                name: "one".into(),
                description: "".into(),
                scope: "".into(),
            },
            AgentInfo {
                s_id: "a2".into(),
                name: "two".into(),
                description: "".into(),
                scope: "".into(),
            },
        ]);
        app.picker_move_selection(-1); // wraps to last
        assert_eq!(app.picker_selected(), 1);
    }

    #[test]
    fn switch_agent_updates_name_and_pushes_system_message() {
        let mut app = App::new("old-agent", "/workspace", None);
        app.switch_agent("new-id", "new-agent");
        assert_eq!(app.agent_name(), "new-agent");
        assert_eq!(app.messages().len(), 1);
        assert_eq!(app.messages()[0].role, Role::System);
        assert!(app.messages()[0].content.contains("new-agent"));
    }

    #[test]
    fn push_system_message_adds_system_role() {
        let mut app = App::new("a", "/workspace", None);
        app.push_system_message("network down");
        assert_eq!(app.messages()[0].role, Role::System);
        assert_eq!(app.messages()[0].content, "network down");
    }

    #[test]
    fn new_conversation_empties_messages() {
        let mut app = App::new("a", "/workspace", None);
        assert!(app.send_message("hello"));
        assert!(app.send_message("world"));
        assert_eq!(app.messages().len(), 4); // 2 user + 2 agent placeholder
        app.new_conversation();
        assert_eq!(app.messages().len(), 1); // only system message
    }

    #[test]
    fn new_conversation_stops_streaming() {
        let mut app = App::new("a", "/workspace", None);
        assert!(app.send_message("hello"));
        assert!(app.is_streaming());
        app.new_conversation();
        assert!(!app.is_streaming());
    }

    #[test]
    fn new_conversation_clears_conversation_id() {
        let mut app = App::new("a", "/workspace", None);
        app.set_conversation_id("conv-123");
        assert_eq!(app.conversation_id(), Some("conv-123"));
        app.new_conversation();
        assert_eq!(app.conversation_id(), None);
    }

    #[test]
    fn new_conversation_pushes_system_message() {
        let mut app = App::new("my-agent", "/workspace", None);
        app.new_conversation();
        assert_eq!(app.messages().len(), 1);
        assert_eq!(app.messages()[0].role, Role::System);
        assert!(app.messages()[0].content.contains("my-agent"));
        assert!(app.messages()[0].content.contains("new conversation"));
    }

    #[test]
    fn new_conversation_resets_scroll_offset() {
        let mut app = App::new("a", "/workspace", None);
        app.scroll_up(10);
        assert_eq!(app.scroll_offset(), 10);
        app.new_conversation();
        assert_eq!(app.scroll_offset(), 0);
    }

    #[test]
    fn new_conversation_clears_all_state() {
        let mut app = App::new("my-agent", "/workspace", None);
        assert!(app.send_message("hello"));
        assert!(app.send_message("world"));
        app.set_conversation_id("conv-123");
        app.scroll_up(5);

        app.new_conversation();

        assert_eq!(app.messages().len(), 1); // only system message
        assert!(!app.is_streaming());
        assert_eq!(app.conversation_id(), None);
        assert_eq!(app.scroll_offset(), 0);
        assert!(app.messages()[0].content.contains("my-agent"));
    }

    #[test]
    fn enter_resume_picker_switches_mode() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_resume_picker();
        assert!(matches!(app.mode(), AppMode::ResumePicker(_)));
    }

    #[test]
    fn set_resume_conversations_updates_state() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_resume_picker();
        let conversations = vec![
            ConversationSummary {
                s_id: "c1".into(),
                title: Some("First chat".into()),
                created: 1707900000000,
                updated: Some(1707950000000),
            },
            ConversationSummary {
                s_id: "c2".into(),
                title: None,
                created: 1707800000000,
                updated: None,
            },
        ];
        app.set_resume_conversations(conversations);
        if let AppMode::ResumePicker(state) = app.mode() {
            assert_eq!(state.conversations.len(), 2);
            assert!(!state.loading);
        } else {
            panic!("expected ResumePicker mode");
        }
    }

    #[test]
    fn resume_filtered_conversations_narrows_results() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_resume_picker();
        app.set_resume_conversations(vec![
            ConversationSummary {
                s_id: "c1".into(),
                title: Some("Project brainstorm".into()),
                created: 1707900000000,
                updated: None,
            },
            ConversationSummary {
                s_id: "c2".into(),
                title: Some("Bug discussion".into()),
                created: 1707800000000,
                updated: None,
            },
        ]);
        app.set_resume_filter("project");
        let filtered = app.resume_filtered_conversations();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, Some("Project brainstorm".into()));
    }

    #[test]
    fn resume_filter_is_case_insensitive() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_resume_picker();
        app.set_resume_conversations(vec![ConversationSummary {
            s_id: "c1".into(),
            title: Some("Project Brainstorm".into()),
            created: 1707900000000,
            updated: None,
        }]);
        app.set_resume_filter("project");
        assert_eq!(app.resume_filtered_conversations().len(), 1);
    }

    #[test]
    fn resume_filter_includes_untitled_conversations() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_resume_picker();
        app.set_resume_conversations(vec![
            ConversationSummary {
                s_id: "c1".into(),
                title: None, // untitled
                created: 1707900000000,
                updated: None,
            },
            ConversationSummary {
                s_id: "c2".into(),
                title: Some("Project Brainstorm".into()),
                created: 1707800000000,
                updated: None,
            },
        ]);
        app.set_resume_filter("project");
        let filtered = app.resume_filtered_conversations();
        assert_eq!(filtered.len(), 2); // both untitled and matching titled conversations
        assert_eq!(filtered[0].s_id, "c1"); // untitled still shows
        assert_eq!(filtered[1].s_id, "c2"); // titled match still shows
    }

    #[test]
    fn resume_picker_selection_wraps() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_resume_picker();
        app.set_resume_conversations(vec![
            ConversationSummary {
                s_id: "c1".into(),
                title: Some("first".into()),
                created: 1707900000000,
                updated: None,
            },
            ConversationSummary {
                s_id: "c2".into(),
                title: Some("second".into()),
                created: 1707800000000,
                updated: None,
            },
        ]);
        app.resume_picker_move_selection(1);
        assert_eq!(app.resume_picker_selected(), 1);
        app.resume_picker_move_selection(1); // wraps to 0
        assert_eq!(app.resume_picker_selected(), 0);
    }

    #[test]
    fn restore_conversation_sets_conversation_id() {
        let mut app = App::new("a", "/workspace", None);
        app.restore_conversation("conv-123".into(), vec![], None);
        assert_eq!(app.conversation_id(), Some("conv-123"));
    }

    #[test]
    fn restore_conversation_populates_messages() {
        let mut app = App::new("a", "/workspace", None);
        app.restore_conversation(
            "conv-123".into(),
            vec![
                (Role::User, "Hello".into()),
                (Role::Agent("agent".into()), "Hi there".into()),
            ],
            Some("Test Conversation"),
        );
        assert_eq!(app.messages().len(), 3); // 2 restored + 1 system message
        assert_eq!(app.messages()[0].role, Role::User);
        assert_eq!(app.messages()[0].content, "Hello");
        assert_eq!(app.messages()[1].role, Role::Agent("agent".into()));
        assert_eq!(
            app.messages()[2].content,
            "Resumed conversation: Test Conversation"
        );
    }

    #[test]
    fn restore_conversation_exits_picker_mode() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_resume_picker();
        assert!(matches!(app.mode(), AppMode::ResumePicker(_)));
        app.restore_conversation("conv-123".into(), vec![], None);
        assert!(matches!(app.mode(), AppMode::Chat));
    }

    #[test]
    fn restore_conversation_resets_scroll() {
        let mut app = App::new("a", "/workspace", None);
        app.enter_resume_picker();
        app.scroll_up(5);
        app.restore_conversation("conv-123".into(), vec![], None);
        assert_eq!(app.scroll_offset(), 0);
    }

    #[test]
    fn restore_conversation_includes_untitled_fallback() {
        let mut app = App::new("a", "/workspace", None);
        app.restore_conversation("conv-123".into(), vec![], None);
        assert_eq!(
            app.messages()[0].content,
            "Resumed conversation: (untitled)"
        );
    }

    #[test]
    fn restore_conversation_includes_title_in_message() {
        let mut app = App::new("a", "/workspace", None);
        app.restore_conversation(
            "conv-123".into(),
            vec![(Role::User, "hello".into())],
            Some("My Project"),
        );
        assert_eq!(
            app.messages()[1].content,
            "Resumed conversation: My Project"
        );
    }

    #[test]
    fn enter_tool_approval_sets_mode() {
        let mut app = App::new("test-agent", "/workspace", None);
        let tool_call = ToolCall {
            id: "tool_123".into(),
            name: "bash".into(),
            input: serde_json::json!({"command": "ls"}),
        };

        app.scroll_up(5);
        app.enter_tool_approval(tool_call.clone());
        match app.mode() {
            AppMode::ToolApproval(_) => {}
            _ => panic!("Expected ToolApproval mode"),
        }
        assert_eq!(
            app.scroll_offset(),
            0,
            "entering tool approval should reset scroll to bottom"
        );
    }

    #[test]
    fn current_tool_call_returns_tool_in_approval_mode() {
        let mut app = App::new("test-agent", "/workspace", None);
        let tool_call = ToolCall {
            id: "tool_123".into(),
            name: "bash".into(),
            input: serde_json::json!({"command": "ls"}),
        };

        app.enter_tool_approval(tool_call.clone());
        let current = app.current_tool_call();
        assert!(current.is_some());
        assert_eq!(current.unwrap().id, "tool_123");
        assert_eq!(current.unwrap().name, "bash");
    }

    #[test]
    fn exit_tool_approval_returns_to_chat_mode() {
        let mut app = App::new("test-agent", "/workspace", None);
        let tool_call = ToolCall {
            id: "tool_123".into(),
            name: "bash".into(),
            input: serde_json::json!({"command": "ls"}),
        };

        app.enter_tool_approval(tool_call);
        app.exit_tool_approval();
        match app.mode() {
            AppMode::Chat => {}
            _ => panic!("Expected Chat mode"),
        }
    }
}
