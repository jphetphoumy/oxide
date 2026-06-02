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
    should_quit: bool,
    conversation_id: Option<String>,
    is_streaming: bool,
}

impl App {
    pub fn new(agent_name: &str) -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            agent_name: agent_name.to_string(),
            should_quit: false,
            conversation_id: None,
            is_streaming: false,
        }
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn agent_name(&self) -> &str {
        &self.agent_name
    }

    pub fn conversation_id(&self) -> Option<&str> {
        self.conversation_id.as_deref()
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_app_has_empty_messages() {
        let app = App::new("test-agent");
        assert!(app.messages().is_empty());
    }

    #[test]
    fn new_app_stores_agent_name() {
        let app = App::new("my-agent");
        assert_eq!(app.agent_name(), "my-agent");
    }

    #[test]
    fn new_app_should_not_quit() {
        let app = App::new("a");
        assert!(!app.should_quit());
    }

    #[test]
    fn quit_sets_should_quit() {
        let mut app = App::new("a");
        app.quit();
        assert!(app.should_quit());
    }

    #[test]
    fn send_message_pushes_user_and_placeholder_agent_messages() {
        let mut app = App::new("echo-bot");
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
        let mut app = App::new("a");
        assert!(app.send_message("  hi  "));
        assert_eq!(app.messages()[0].content, "hi");
    }

    #[test]
    fn send_empty_message_does_nothing() {
        let mut app = App::new("a");
        assert!(!app.send_message(""));
        assert!(app.messages().is_empty());
        assert!(!app.send_message("   "));
        assert!(app.messages().is_empty());
    }

    #[test]
    fn scroll_up_increases_offset() {
        let mut app = App::new("a");
        app.scroll_up(3);
        assert_eq!(app.scroll_offset(), 3);
    }

    #[test]
    fn scroll_down_decreases_offset_to_zero() {
        let mut app = App::new("a");
        app.scroll_up(5);
        app.scroll_down(3);
        assert_eq!(app.scroll_offset(), 2);
        app.scroll_down(10);
        assert_eq!(app.scroll_offset(), 0);
    }

    #[test]
    fn send_message_resets_scroll() {
        let mut app = App::new("a");
        app.scroll_up(10);
        assert!(app.send_message("new msg"));
        assert_eq!(app.scroll_offset(), 0);
    }

    #[test]
    fn append_agent_token_updates_last_agent_message() {
        let mut app = App::new("a");
        assert!(app.send_message("hello"));
        app.append_agent_token("he");
        app.append_agent_token("llo");
        assert_eq!(app.messages()[1].content, "hello");
    }

    #[test]
    fn append_agent_token_preserves_scroll_when_not_at_bottom() {
        let mut app = App::new("a");
        assert!(app.send_message("hello"));
        app.scroll_up(3);
        app.append_agent_token("world");
        assert_eq!(app.scroll_offset(), 3);
    }

    #[test]
    fn complete_stream_can_replace_content() {
        let mut app = App::new("a");
        assert!(app.send_message("hello"));
        app.complete_stream(Some("final answer"));
        assert_eq!(app.messages()[1].content, "final answer");
        assert!(!app.is_streaming());
    }

    #[test]
    fn complete_stream_preserves_scroll_when_not_at_bottom() {
        let mut app = App::new("a");
        assert!(app.send_message("hello"));
        app.scroll_up(4);
        app.complete_stream(Some("final answer"));
        assert_eq!(app.scroll_offset(), 4);
    }

    #[test]
    fn push_system_message_adds_system_role() {
        let mut app = App::new("a");
        app.push_system_message("network down");
        assert_eq!(app.messages()[0].role, Role::System);
        assert_eq!(app.messages()[0].content, "network down");
    }
}
