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
    pub call_id: String,
    pub tool_call: ToolCall,
    /// Present when this approval is for an MCP tool — on approve, call `validate_action`.
    pub mcp_approve: Option<McpApproveInfo>,
    /// True when this approval is for an MCP transport tool call (`McpToolUse` event).
    /// On approve/deny, use `post_mcp_result()` instead of `submit_tool_result()`.
    pub mcp_transport: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallStatus {
    Pending,
    Running,
    Done,
    Failed,
    Denied,
}

#[derive(Debug, Clone)]
pub struct ToolCallEntry {
    pub call_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub status: ToolCallStatus,
    pub result: Option<String>,
    pub expanded: bool,
    pub started_at: std::time::Instant,
    pub finished_at: Option<std::time::Instant>,
}

#[derive(Debug, Clone)]
pub enum AppMode {
    Chat,
    Picker(PickerState),
    ResumePicker(ResumePickerState),
    ToolApproval(ToolApprovalState),
}

#[derive(Debug, Clone)]
pub enum SubagentCallStatus {
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone)]
pub struct SubagentCallState {
    pub call_id: String,
    pub description: Option<String>,
    pub status: SubagentCallStatus,
    pub started_at: std::time::Instant,
    pub finished_at: Option<std::time::Instant>,
}

#[derive(Debug, Clone)]
pub enum Role {
    User,
    Agent(String),
    System,
    SubagentCall(SubagentCallState),
    ToolCall(ToolCallEntry),
}

impl PartialEq for Role {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::User, Self::User) | (Self::System, Self::System) => true,
            (Self::Agent(a), Self::Agent(b)) => a == b,
            (Self::SubagentCall(a), Self::SubagentCall(b)) => a.call_id == b.call_id,
            (Self::ToolCall(a), Self::ToolCall(b)) => a.call_id == b.call_id,
            _ => false,
        }
    }
}

impl Eq for Role {}

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
    streaming_started_at: Option<std::time::Instant>,
    mode: AppMode,
    auto_approve_tools: bool,
    /// Tool names approved via `ToolApproveExecution` (Dust-side gate) awaiting the subsequent
    /// MCP transport `tools/call`. Consumed by `consume_transport_pre_approval`.
    pending_mcp_transport_approvals: std::collections::VecDeque<String>,
    skills: Vec<crate::skills::Skill>,
    active_skills: Vec<crate::skills::Skill>,
    subagent_count: usize,
    tick: u64,
    context_usage: Option<(u32, u32)>,
    context_size: Option<u32>,
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
            streaming_started_at: None,
            mode: AppMode::Chat,
            auto_approve_tools: false,
            pending_mcp_transport_approvals: std::collections::VecDeque::new(),
            skills: Vec::new(),
            active_skills: Vec::new(),
            subagent_count: 0,
            tick: 0,
            context_usage: None,
            context_size: None,
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

    #[cfg(test)]
    pub fn agent_id(&self) -> &str {
        &self.agent_id
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

    pub const fn streaming_started_at(&self) -> Option<std::time::Instant> {
        self.streaming_started_at
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

    pub const fn subagent_count(&self) -> usize {
        self.subagent_count
    }

    pub fn push_subagent_started(&mut self, call_id: String, description: Option<String>) {
        self.messages.push(Message {
            role: Role::SubagentCall(SubagentCallState {
                call_id,
                description,
                status: SubagentCallStatus::Running,
                started_at: std::time::Instant::now(),
                finished_at: None,
            }),
            content: String::new(),
        });
        self.scroll_offset = 0;
        self.subagent_count += 1;
    }

    pub fn complete_subagent(&mut self, call_id: &str, success: bool) {
        for msg in self.messages.iter_mut().rev() {
            if let Role::SubagentCall(state) = &mut msg.role
                && state.call_id == call_id
            {
                state.status = if success {
                    SubagentCallStatus::Done
                } else {
                    SubagentCallStatus::Failed
                };
                state.finished_at = Some(std::time::Instant::now());
                self.subagent_count = self.subagent_count.saturating_sub(1);
                break;
            }
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    pub const fn tick_count(&self) -> u64 {
        self.tick
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
        self.streaming_started_at = Some(std::time::Instant::now());
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
        self.streaming_started_at = None;
        if was_at_bottom {
            self.scroll_offset = 0;
        }
    }

    pub const fn cancel_stream(&mut self) {
        self.is_streaming = false;
        self.streaming_started_at = None;
    }

    pub fn push_system_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: Role::System,
            content: content.to_string(),
        });
        self.is_streaming = false;
        self.streaming_started_at = None;
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
        if self.context_size.is_none()
            && let Some(agent) = agents.iter().find(|a| a.s_id == self.agent_id)
        {
            self.context_size = agent.context_size();
        }
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

    pub fn switch_agent(&mut self, agent_id: &str, agent_name: &str, context_size: Option<u32>) {
        self.agent_id = agent_id.to_string();
        self.agent_name = agent_name.to_string();
        self.context_size = context_size;
        self.context_usage = None;
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
        self.streaming_started_at = None;
        self.scroll_offset = 0;
        // context_size is preserved — the agent hasn't changed
        self.context_usage = None;
        self.mode = AppMode::Chat;
        let title_str = title.unwrap_or("(untitled)");
        self.push_system_message(&format!("Resumed conversation: {title_str}"));
    }

    pub fn new_conversation(&mut self) {
        self.messages.clear();
        self.is_streaming = false;
        self.conversation_id = None;
        self.scroll_offset = 0;
        self.clear_active_skills();
        self.pending_mcp_transport_approvals.clear();
        // context_size is preserved — the agent hasn't changed
        self.context_usage = None;
        self.push_system_message(&format!(
            "Started a new conversation with {}",
            self.agent_name
        ));
    }

    pub fn enter_tool_approval(&mut self, tool_call: ToolCall, call_id: String) {
        self.scroll_offset = 0;
        self.mode = AppMode::ToolApproval(ToolApprovalState {
            call_id,
            tool_call,
            mcp_approve: None,
            mcp_transport: false,
        });
    }

    pub fn enter_mcp_tool_approval(
        &mut self,
        tool_call: ToolCall,
        call_id: String,
        mcp_approve: McpApproveInfo,
    ) {
        self.scroll_offset = 0;
        self.mode = AppMode::ToolApproval(ToolApprovalState {
            call_id,
            tool_call,
            mcp_approve: Some(mcp_approve),
            mcp_transport: false,
        });
    }

    pub fn enter_mcp_transport_tool_approval(&mut self, tool_call: ToolCall, call_id: String) {
        self.scroll_offset = 0;
        self.mode = AppMode::ToolApproval(ToolApprovalState {
            call_id,
            tool_call,
            mcp_approve: None,
            mcp_transport: true,
        });
    }

    /// Records that the user approved a Dust-side `ToolApproveExecution` for `tool_name`.
    /// The next `McpToolUse` for the same tool will be auto-approved to avoid a double prompt.
    /// Called synchronously before the `validate_action` spawn, so the entry is always present
    /// before Dust can deliver the subsequent `tools/call` over the MCP transport.
    pub fn mark_tool_transport_pre_approved(&mut self, tool_name: String) {
        tracing::debug!(tool = %tool_name, "marking MCP transport call as pre-approved");
        self.pending_mcp_transport_approvals.push_back(tool_name);
    }

    /// Returns `true` and consumes the pre-approval if one exists for `tool_name`.
    pub fn consume_transport_pre_approval(&mut self, tool_name: &str) -> bool {
        if let Some(pos) = self
            .pending_mcp_transport_approvals
            .iter()
            .position(|n| n == tool_name)
        {
            self.pending_mcp_transport_approvals.remove(pos);
            tracing::debug!(tool = %tool_name, "consuming MCP transport pre-approval (skipping duplicate gate)");
            true
        } else {
            false
        }
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

    pub fn push_tool_call(&mut self, tool_call: ToolCall) -> String {
        let call_id = tool_call.id.clone();
        // Deduplicate: SSE resume can re-emit the same tool call event
        let already_exists = self
            .messages
            .iter()
            .any(|m| matches!(&m.role, Role::ToolCall(e) if e.call_id == call_id));
        if already_exists {
            return call_id;
        }
        self.messages.push(Message {
            role: Role::ToolCall(ToolCallEntry {
                call_id: call_id.clone(),
                tool_name: tool_call.name,
                input: tool_call.input,
                status: ToolCallStatus::Pending,
                result: None,
                expanded: false,
                started_at: std::time::Instant::now(),
                finished_at: None,
            }),
            content: String::new(),
        });
        self.scroll_offset = 0;
        call_id
    }

    /// Returns the call_id of an existing non-failed/denied tool call with the same
    /// name and input — used to detect ToolApproveExecution + McpToolUse duplicates.
    pub fn find_active_tool_call_by_name_and_input(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
    ) -> Option<String> {
        self.messages.iter().find_map(|m| {
            if let Role::ToolCall(e) = &m.role
                && e.tool_name == tool_name
                && &e.input == input
                && !matches!(e.status, ToolCallStatus::Failed | ToolCallStatus::Denied)
            {
                Some(e.call_id.clone())
            } else {
                None
            }
        })
    }

    pub fn set_tool_call_running(&mut self, call_id: &str) {
        for msg in self.messages.iter_mut().rev() {
            if let Role::ToolCall(entry) = &mut msg.role
                && entry.call_id == call_id
            {
                entry.status = ToolCallStatus::Running;
                break;
            }
        }
    }

    pub fn complete_tool_call(&mut self, call_id: &str, result: String) {
        for msg in self.messages.iter_mut().rev() {
            if let Role::ToolCall(entry) = &mut msg.role
                && entry.call_id == call_id
            {
                entry.status = ToolCallStatus::Done;
                entry.result = Some(result);
                entry.finished_at = Some(std::time::Instant::now());
                break;
            }
        }
    }

    pub fn fail_tool_call(&mut self, call_id: &str, error: String) {
        for msg in self.messages.iter_mut().rev() {
            if let Role::ToolCall(entry) = &mut msg.role
                && entry.call_id == call_id
            {
                entry.status = ToolCallStatus::Failed;
                entry.result = Some(error);
                entry.finished_at = Some(std::time::Instant::now());
                break;
            }
        }
    }

    pub fn deny_tool_call(&mut self, call_id: &str) {
        for msg in self.messages.iter_mut().rev() {
            if let Role::ToolCall(entry) = &mut msg.role
                && entry.call_id == call_id
            {
                entry.status = ToolCallStatus::Denied;
                entry.finished_at = Some(std::time::Instant::now());
                break;
            }
        }
    }

    pub fn toggle_tool_call_expanded(&mut self, call_id: &str) {
        for msg in self.messages.iter_mut().rev() {
            if let Role::ToolCall(entry) = &mut msg.role
                && entry.call_id == call_id
            {
                entry.expanded = !entry.expanded;
                break;
            }
        }
    }

    pub fn last_tool_call_id(&self) -> Option<String> {
        for msg in self.messages.iter().rev() {
            if let Role::ToolCall(entry) = &msg.role {
                return Some(entry.call_id.clone());
            }
        }
        None
    }

    pub fn set_skills(&mut self, skills: Vec<crate::skills::Skill>) {
        self.skills = skills;
    }

    pub fn active_skills(&self) -> &[crate::skills::Skill] {
        &self.active_skills
    }

    pub fn activate_skill(&mut self, id: &str) {
        // Find the skill with this id
        if let Some(skill) = self.skills.iter().find(|s| s.id == id).cloned() {
            // Check if it's already active
            if !self.active_skills.iter().any(|s| s.id == id) {
                self.active_skills.push(skill);
            }
        }
    }

    pub fn clear_active_skills(&mut self) {
        self.active_skills.clear();
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn set_context_usage(&mut self, used: u32, size: u32) {
        if size > 0 {
            self.context_usage = Some((used, size));
            self.context_size = Some(size);
        }
    }

    pub fn context_usage_percent(&self) -> Option<u8> {
        self.context_usage.map(|(used, size)| {
            let percent = (f64::from(used) / f64::from(size)) * 100.0;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                percent.round().min(100.0) as u8
            }
        })
    }

    pub fn context_usage_display(&self) -> String {
        let size = self.context_usage.map(|(_, s)| s).or(self.context_size);
        match (self.context_usage_percent(), size) {
            (Some(pct), Some(s)) => format!(" ctx:{pct}%/{}", format_context_size(s)),
            (None, Some(s)) => format!(" ctx:0%/{}", format_context_size(s)),
            _ => " ctx:--".to_string(),
        }
    }
}

fn format_context_size(tokens: u32) -> String {
    if tokens >= 1_000_000 {
        format!("{}M", tokens / 1_000_000)
    } else if tokens >= 1_000 {
        format!("{}K", tokens / 1_000)
    } else {
        tokens.to_string()
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
                model: None,
            },
            AgentInfo {
                s_id: "a2".into(),
                name: "helper".into(),
                description: "Code".into(),
                scope: "published".into(),
                model: None,
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
                model: None,
            },
            AgentInfo {
                s_id: "a2".into(),
                name: "helper".into(),
                description: "".into(),
                scope: "".into(),
                model: None,
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
            model: None,
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
                model: None,
            },
            AgentInfo {
                s_id: "a2".into(),
                name: "two".into(),
                description: "".into(),
                scope: "".into(),
                model: None,
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
                model: None,
            },
            AgentInfo {
                s_id: "a2".into(),
                name: "two".into(),
                description: "".into(),
                scope: "".into(),
                model: None,
            },
        ]);
        app.picker_move_selection(-1); // wraps to last
        assert_eq!(app.picker_selected(), 1);
    }

    #[test]
    fn switch_agent_updates_name_and_pushes_system_message() {
        let mut app = App::new("old-agent", "/workspace", None);
        app.switch_agent("new-id", "new-agent", None);
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
        let call_id = "t1".to_string();
        app.enter_tool_approval(tool_call.clone(), call_id);
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

        let call_id = "t1".to_string();
        app.enter_tool_approval(tool_call.clone(), call_id);
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

        let call_id = "t1".to_string();
        app.enter_tool_approval(tool_call, call_id);
        app.exit_tool_approval();
        match app.mode() {
            AppMode::Chat => {}
            _ => panic!("Expected Chat mode"),
        }
    }

    #[test]
    fn pre_approval_consumed_once_suppresses_duplicate_gate() {
        let mut app = App::new("test-agent", "/workspace", None);

        // No pre-approval recorded yet → not consumed.
        assert!(!app.consume_transport_pre_approval("bash"));

        // Record a Dust-side approval for "bash".
        app.mark_tool_transport_pre_approved("bash".to_string());

        // First consumption returns true (suppresses duplicate gate).
        assert!(app.consume_transport_pre_approval("bash"));

        // Second consumption returns false (entry was removed).
        assert!(!app.consume_transport_pre_approval("bash"));
    }

    #[test]
    fn pre_approval_is_tool_name_scoped() {
        let mut app = App::new("test-agent", "/workspace", None);

        app.mark_tool_transport_pre_approved("bash".to_string());

        // A different tool name does not consume the bash pre-approval.
        assert!(!app.consume_transport_pre_approval("list_files"));

        // The bash entry is still intact.
        assert!(app.consume_transport_pre_approval("bash"));
    }

    #[test]
    fn multiple_pre_approvals_for_same_tool_require_multiple_consumptions() {
        let mut app = App::new("test-agent", "/workspace", None);

        app.mark_tool_transport_pre_approved("bash".to_string());
        app.mark_tool_transport_pre_approved("bash".to_string());

        assert!(app.consume_transport_pre_approval("bash"));
        assert!(app.consume_transport_pre_approval("bash"));
        assert!(!app.consume_transport_pre_approval("bash"));
    }

    #[test]
    fn new_conversation_clears_pending_pre_approvals() {
        let mut app = App::new("test-agent", "/workspace", None);
        app.mark_tool_transport_pre_approved("bash".to_string());
        app.new_conversation();
        // Pre-approval should be gone — a gate in the new conversation must not be suppressed.
        assert!(!app.consume_transport_pre_approval("bash"));
    }

    #[test]
    fn activate_skill_with_valid_id() {
        let mut app = App::new("test-agent", "/workspace", None);
        let skill = crate::skills::Skill {
            id: "code-review".to_string(),
            name: "Code Reviewer".to_string(),
            description: "Review code".to_string(),
            path: PathBuf::from(".agents/skills/code-review.md"),
        };
        app.set_skills(vec![skill]);

        app.activate_skill("code-review");

        assert_eq!(app.active_skills().len(), 1);
        assert_eq!(app.active_skills()[0].id, "code-review");
    }

    #[test]
    fn activate_skill_deduplicates_duplicate_ids() {
        let mut app = App::new("test-agent", "/workspace", None);
        let skill = crate::skills::Skill {
            id: "code-review".to_string(),
            name: "Code Reviewer".to_string(),
            description: "Review code".to_string(),
            path: PathBuf::from(".agents/skills/code-review.md"),
        };
        app.set_skills(vec![skill]);

        app.activate_skill("code-review");
        app.activate_skill("code-review");

        assert_eq!(
            app.active_skills().len(),
            1,
            "duplicate skills should not be added"
        );
    }

    #[test]
    fn activate_skill_with_unknown_id() {
        let mut app = App::new("test-agent", "/workspace", None);
        let skill = crate::skills::Skill {
            id: "code-review".to_string(),
            name: "Code Reviewer".to_string(),
            description: "Review code".to_string(),
            path: PathBuf::from(".agents/skills/code-review.md"),
        };
        app.set_skills(vec![skill]);

        app.activate_skill("nonexistent");

        assert!(
            app.active_skills().is_empty(),
            "activating unknown skill should have no effect"
        );
    }

    #[test]
    fn new_conversation_clears_active_skills() {
        let mut app = App::new("test-agent", "/workspace", None);
        let skill = crate::skills::Skill {
            id: "code-review".to_string(),
            name: "Code Reviewer".to_string(),
            description: "Review code".to_string(),
            path: PathBuf::from(".agents/skills/code-review.md"),
        };
        app.set_skills(vec![skill]);
        app.activate_skill("code-review");

        assert!(!app.active_skills().is_empty());
        app.new_conversation();
        assert!(
            app.active_skills().is_empty(),
            "new conversation should clear active skills"
        );
    }

    #[test]
    fn push_subagent_started_adds_message() {
        let mut app = App::new("a", "/ws", None);
        app.push_subagent_started("id-1".to_string(), Some("summarise PR".to_string()));
        assert_eq!(app.messages().len(), 1);
        assert!(matches!(app.messages()[0].role, Role::SubagentCall(_)));
        assert_eq!(app.subagent_count(), 1);
    }

    #[test]
    fn complete_subagent_updates_status() {
        let mut app = App::new("a", "/ws", None);
        app.push_subagent_started("id-1".to_string(), None);
        app.complete_subagent("id-1", true);
        if let Role::SubagentCall(state) = &app.messages()[0].role {
            assert!(matches!(state.status, SubagentCallStatus::Done));
        } else {
            panic!("expected SubagentCall");
        }
        assert_eq!(app.subagent_count(), 0);
    }

    #[test]
    fn complete_subagent_noop_for_unknown_id() {
        let mut app = App::new("a", "/ws", None);
        app.push_subagent_started("id-1".to_string(), None);
        app.complete_subagent("id-unknown", true);
        if let Role::SubagentCall(state) = &app.messages()[0].role {
            assert!(matches!(state.status, SubagentCallStatus::Running));
        }
        // Counter must not change when the call_id is not found
        assert_eq!(app.subagent_count(), 1);
    }

    #[test]
    fn tick_increments_counter() {
        let mut app = App::new("a", "/ws", None);
        assert_eq!(app.tick_count(), 0);
        app.tick();
        assert_eq!(app.tick_count(), 1);
        app.tick();
        assert_eq!(app.tick_count(), 2);
    }

    #[test]
    fn send_message_sets_streaming_started_at() {
        let mut app = App::new("a", "/workspace", None);
        assert!(app.streaming_started_at().is_none());
        assert!(app.send_message("hello"));
        assert!(app.streaming_started_at().is_some());
    }

    #[test]
    fn complete_stream_clears_streaming_started_at() {
        let mut app = App::new("a", "/workspace", None);
        assert!(app.send_message("hello"));
        assert!(app.streaming_started_at().is_some());
        app.complete_stream(Some("final"));
        assert!(app.streaming_started_at().is_none());
    }

    #[test]
    fn push_system_message_clears_streaming_started_at() {
        let mut app = App::new("a", "/workspace", None);
        assert!(app.send_message("hello"));
        app.push_system_message("error occurred");
        assert!(app.streaming_started_at().is_none());
    }

    #[test]
    fn restore_conversation_clears_streaming_started_at() {
        let mut app = App::new("a", "/workspace", None);
        assert!(app.send_message("hello"));
        assert!(app.streaming_started_at().is_some());
        app.restore_conversation("conv-123".into(), vec![], None);
        assert!(app.streaming_started_at().is_none());
    }

    #[test]
    fn agent_id_accessor_returns_initial_agent_id() {
        let app = App::new("initial-agent", "/workspace", None);
        assert_eq!(app.agent_id(), "initial-agent");
    }

    #[test]
    fn switch_agent_updates_agent_id() {
        let mut app = App::new("old-agent-id", "/workspace", None);
        app.switch_agent("new-agent-id", "new-agent-name", None);
        assert_eq!(app.agent_id(), "new-agent-id");
    }

    #[test]
    fn switch_agent_updates_both_id_and_name() {
        let mut app = App::new("initial-id", "/workspace", None);
        app.switch_agent("updated-id", "updated-name", None);
        assert_eq!(app.agent_id(), "updated-id");
        assert_eq!(app.agent_name(), "updated-name");
    }

    #[test]
    fn set_picker_filter_resets_selected_index() {
        let agents = vec![
            AgentInfo {
                s_id: "agent-1".to_string(),
                name: "hello".to_string(),
                description: "desc1".to_string(),
                scope: String::new(),
                model: None,
            },
            AgentInfo {
                s_id: "agent-2".to_string(),
                name: "world".to_string(),
                description: "desc2".to_string(),
                scope: String::new(),
                model: None,
            },
            AgentInfo {
                s_id: "agent-3".to_string(),
                name: "helper".to_string(),
                description: "desc3".to_string(),
                scope: String::new(),
                model: None,
            },
        ];
        let mut app = App::new("agent-id", "/workspace", None);
        app.enter_picker();
        app.set_picker_agents(agents);
        app.picker_move_selection(1);
        app.picker_move_selection(1);
        assert_eq!(app.picker_selected(), 2);

        app.set_picker_filter("hel");
        assert_eq!(app.picker_selected(), 0);
    }

    #[test]
    fn switch_agent_persists_after_filter() {
        let agents = vec![
            AgentInfo {
                s_id: "a1".to_string(),
                name: "Main Agent".to_string(),
                description: "".to_string(),
                scope: "".to_string(),
                model: None,
            },
            AgentInfo {
                s_id: "a2".to_string(),
                name: "Helper".to_string(),
                description: "".to_string(),
                scope: "".to_string(),
                model: None,
            },
        ];
        let mut app = App::new("main-agent-id", "/workspace", None);
        app.enter_picker();
        app.set_picker_agents(agents);
        app.set_picker_filter("Helper");
        let filtered = app.picker_filtered_agents();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].s_id, "a2");
        app.switch_agent("a2", "Helper", None);
        assert_eq!(app.agent_id(), "a2");
        assert_eq!(app.agent_name(), "Helper");
    }

    #[test]
    fn set_context_usage_stores_value() {
        let mut app = App::new("a", "/workspace", None);
        app.set_context_usage(42000, 100000);
        assert_eq!(app.context_usage_percent(), Some(42));
    }

    #[test]
    fn set_context_usage_zero_size_is_ignored() {
        let mut app = App::new("a", "/workspace", None);
        app.set_context_usage(100, 0);
        assert_eq!(app.context_usage_percent(), None);
    }

    #[test]
    fn new_conversation_clears_context_usage() {
        let mut app = App::new("a", "/workspace", None);
        app.set_context_usage(42000, 100000);
        assert_eq!(app.context_usage_percent(), Some(42));
        app.new_conversation();
        assert_eq!(app.context_usage_percent(), None);
    }

    #[test]
    fn restore_conversation_clears_context_usage() {
        let mut app = App::new("a", "/workspace", None);
        app.set_context_usage(42000, 100000);
        assert_eq!(app.context_usage_percent(), Some(42));
        app.restore_conversation("conv_123".to_string(), vec![], None);
        assert_eq!(app.context_usage_percent(), None);
    }

    #[test]
    fn context_usage_percent_rounds_correctly() {
        let mut app = App::new("a", "/workspace", None);
        app.set_context_usage(0, 100);
        assert_eq!(app.context_usage_percent(), Some(0));
        app.set_context_usage(69, 100);
        assert_eq!(app.context_usage_percent(), Some(69));
        app.set_context_usage(70, 100);
        assert_eq!(app.context_usage_percent(), Some(70));
        app.set_context_usage(79, 100);
        assert_eq!(app.context_usage_percent(), Some(79));
        app.set_context_usage(80, 100);
        assert_eq!(app.context_usage_percent(), Some(80));
        app.set_context_usage(100, 100);
        assert_eq!(app.context_usage_percent(), Some(100));
    }

    #[test]
    fn context_usage_display_no_data() {
        let app = App::new("a", "/workspace", None);
        assert_eq!(app.context_usage_display(), " ctx:--");
    }

    #[test]
    fn context_usage_display_size_known_no_usage() {
        let mut app = App::new("a", "/workspace", None);
        app.context_size = Some(200_000);
        assert_eq!(app.context_usage_display(), " ctx:0%/200K");
    }

    #[test]
    fn context_usage_display_both_known() {
        let mut app = App::new("a", "/workspace", None);
        app.set_context_usage(100_000, 200_000);
        assert_eq!(app.context_usage_display(), " ctx:50%/200K");
    }

    #[test]
    fn format_context_size_plain_number() {
        assert_eq!(format_context_size(999), "999");
    }

    #[test]
    fn format_context_size_with_k_suffix() {
        assert_eq!(format_context_size(1_000), "1K");
        assert_eq!(format_context_size(200_000), "200K");
        assert_eq!(format_context_size(999_999), "999K");
    }

    #[test]
    fn format_context_size_with_m_suffix() {
        assert_eq!(format_context_size(1_000_000), "1M");
        assert_eq!(format_context_size(5_000_000), "5M");
    }

    #[test]
    fn switch_agent_with_context_size() {
        let mut app = App::new("old-agent", "/workspace", None);
        app.set_context_usage(50_000, 100_000);
        assert_eq!(app.context_usage_percent(), Some(50));
        app.switch_agent("new-agent-id", "new-agent", Some(200_000));
        assert_eq!(app.context_usage_percent(), None);
        assert_eq!(app.context_usage_display(), " ctx:0%/200K");
    }

    #[test]
    fn push_tool_call_adds_pending_entry() {
        let mut app = App::new("a", "/workspace", None);
        let tool_call = ToolCall {
            id: "t1".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "ls"}),
        };
        let call_id = app.push_tool_call(tool_call);
        assert_eq!(call_id, "t1");
        assert_eq!(app.messages().len(), 1);
        if let Role::ToolCall(entry) = &app.messages()[0].role {
            assert_eq!(entry.call_id, "t1");
            assert_eq!(entry.tool_name, "Bash");
            assert_eq!(entry.status, ToolCallStatus::Pending);
            assert!(entry.result.is_none());
        } else {
            panic!("expected ToolCall role");
        }
    }

    #[test]
    fn set_tool_call_running_transitions_status() {
        let mut app = App::new("a", "/workspace", None);
        let tool_call = ToolCall {
            id: "t1".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "ls"}),
        };
        app.push_tool_call(tool_call);
        app.set_tool_call_running("t1");
        if let Role::ToolCall(entry) = &app.messages()[0].role {
            assert_eq!(entry.status, ToolCallStatus::Running);
        } else {
            panic!("expected ToolCall role");
        }
    }

    #[test]
    fn complete_tool_call_updates_status_and_result() {
        let mut app = App::new("a", "/workspace", None);
        let tool_call = ToolCall {
            id: "t1".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "ls"}),
        };
        app.push_tool_call(tool_call);
        app.complete_tool_call("t1", "file1.txt\nfile2.txt".into());
        if let Role::ToolCall(entry) = &app.messages()[0].role {
            assert_eq!(entry.status, ToolCallStatus::Done);
            assert_eq!(entry.result, Some("file1.txt\nfile2.txt".into()));
            assert!(entry.finished_at.is_some());
        } else {
            panic!("expected ToolCall role");
        }
    }

    #[test]
    fn fail_tool_call_sets_failed_status() {
        let mut app = App::new("a", "/workspace", None);
        let tool_call = ToolCall {
            id: "t1".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "bad"}),
        };
        app.push_tool_call(tool_call);
        app.fail_tool_call("t1", "command not found".into());
        if let Role::ToolCall(entry) = &app.messages()[0].role {
            assert_eq!(entry.status, ToolCallStatus::Failed);
            assert_eq!(entry.result, Some("command not found".into()));
            assert!(entry.finished_at.is_some());
        } else {
            panic!("expected ToolCall role");
        }
    }

    #[test]
    fn deny_tool_call_sets_denied_status() {
        let mut app = App::new("a", "/workspace", None);
        let tool_call = ToolCall {
            id: "t1".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "rm"}),
        };
        app.push_tool_call(tool_call);
        app.deny_tool_call("t1");
        if let Role::ToolCall(entry) = &app.messages()[0].role {
            assert_eq!(entry.status, ToolCallStatus::Denied);
            assert!(entry.finished_at.is_some());
        } else {
            panic!("expected ToolCall role");
        }
    }

    #[test]
    fn toggle_tool_call_expanded_flips_bool() {
        let mut app = App::new("a", "/workspace", None);
        let tool_call = ToolCall {
            id: "t1".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "ls"}),
        };
        app.push_tool_call(tool_call);

        if let Role::ToolCall(entry) = &app.messages()[0].role {
            assert!(!entry.expanded);
        } else {
            panic!("expected ToolCall role");
        }

        app.toggle_tool_call_expanded("t1");
        if let Role::ToolCall(entry) = &app.messages()[0].role {
            assert!(entry.expanded);
        } else {
            panic!("expected ToolCall role");
        }

        app.toggle_tool_call_expanded("t1");
        if let Role::ToolCall(entry) = &app.messages()[0].role {
            assert!(!entry.expanded);
        } else {
            panic!("expected ToolCall role");
        }
    }

    #[test]
    fn last_tool_call_id_returns_most_recent() {
        let mut app = App::new("a", "/workspace", None);
        let tool_call1 = ToolCall {
            id: "t1".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "ls"}),
        };
        let tool_call2 = ToolCall {
            id: "t2".into(),
            name: "Python".into(),
            input: serde_json::json!({"command": "print"}),
        };
        app.push_tool_call(tool_call1);
        app.push_tool_call(tool_call2);
        assert_eq!(app.last_tool_call_id(), Some("t2".into()));
    }

    #[test]
    fn last_tool_call_id_none_when_no_tool_calls() {
        let app = App::new("a", "/workspace", None);
        assert_eq!(app.last_tool_call_id(), None);
    }

    #[test]
    fn last_tool_call_id_ignores_other_roles() {
        let mut app = App::new("a", "/workspace", None);
        app.push_system_message("hello");
        assert!(app.send_message("user message"));
        assert_eq!(app.last_tool_call_id(), None);
    }

    #[test]
    fn push_tool_call_deduplicates_same_id() {
        let mut app = App::new("a", "/workspace", None);
        let tool_call = ToolCall {
            id: "call-123".to_string(),
            name: "oxide_bash".to_string(),
            input: serde_json::json!({"command": "ls -al"}),
        };
        app.push_tool_call(tool_call.clone());
        app.push_tool_call(tool_call);
        let tool_calls: Vec<_> = app
            .messages()
            .iter()
            .filter(|m| matches!(&m.role, Role::ToolCall(e) if e.call_id == "call-123"))
            .collect();
        assert_eq!(
            tool_calls.len(),
            1,
            "duplicate tool call must not appear twice"
        );
    }

    #[test]
    fn find_active_tool_call_returns_none_when_empty() {
        let app = App::new("a", "/workspace", None);
        assert_eq!(
            app.find_active_tool_call_by_name_and_input(
                "oxide_bash",
                &serde_json::json!({"command": "ls -al"})
            ),
            None
        );
    }

    #[test]
    fn find_active_tool_call_matches_pending_entry() {
        let mut app = App::new("a", "/workspace", None);
        let tool_call = ToolCall {
            id: "act-abc".to_string(),
            name: "oxide_bash".to_string(),
            input: serde_json::json!({"command": "ls -al"}),
        };
        app.push_tool_call(tool_call);
        assert_eq!(
            app.find_active_tool_call_by_name_and_input(
                "oxide_bash",
                &serde_json::json!({"command": "ls -al"})
            ),
            Some("act-abc".to_string())
        );
    }

    #[test]
    fn find_active_tool_call_returns_none_after_failed() {
        let mut app = App::new("a", "/workspace", None);
        let tool_call = ToolCall {
            id: "act-abc".to_string(),
            name: "oxide_bash".to_string(),
            input: serde_json::json!({"command": "ls -al"}),
        };
        app.push_tool_call(tool_call);
        app.fail_tool_call("act-abc", "some error".to_string());
        assert_eq!(
            app.find_active_tool_call_by_name_and_input(
                "oxide_bash",
                &serde_json::json!({"command": "ls -al"})
            ),
            None
        );
    }
}
