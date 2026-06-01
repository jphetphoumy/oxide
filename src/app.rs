#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    User,
    Agent(String),
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
    model: String,
    should_quit: bool,
}

impl App {
    pub fn new(agent_name: &str, model: &str) -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            agent_name: agent_name.to_string(),
            model: model.to_string(),
            should_quit: false,
        }
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    #[allow(dead_code)]
    pub fn agent_name(&self) -> &str {
        &self.agent_name
    }

    pub fn model(&self) -> &str {
        &self.model
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

    pub fn send_message(&mut self, content: &str) {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return;
        }
        self.messages.push(Message {
            role: Role::User,
            content: trimmed.to_string(),
        });
        // Mock echo reply
        self.messages.push(Message {
            role: Role::Agent(self.agent_name.clone()),
            content: format!("Echo: {trimmed}"),
        });
        self.scroll_offset = 0;
    }

    pub const fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    pub const fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_app_has_empty_messages() {
        let app = App::new("test-agent", "gpt-4");
        assert!(app.messages().is_empty());
    }

    #[test]
    fn new_app_stores_agent_name() {
        let app = App::new("my-agent", "gpt-4");
        assert_eq!(app.agent_name(), "my-agent");
    }

    #[test]
    fn new_app_stores_model() {
        let app = App::new("my-agent", "claude-sonnet");
        assert_eq!(app.model(), "claude-sonnet");
    }

    #[test]
    fn new_app_should_not_quit() {
        let app = App::new("a", "m");
        assert!(!app.should_quit());
    }

    #[test]
    fn quit_sets_should_quit() {
        let mut app = App::new("a", "m");
        app.quit();
        assert!(app.should_quit());
    }

    #[test]
    fn send_message_pushes_user_and_agent_messages() {
        let mut app = App::new("echo-bot", "m");
        app.send_message("hello");
        let msgs = app.messages();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, Role::User);
        assert_eq!(msgs[0].content, "hello");
        assert_eq!(msgs[1].role, Role::Agent("echo-bot".to_string()));
        assert!(msgs[1].content.contains("hello"));
    }

    #[test]
    fn send_message_trims_whitespace() {
        let mut app = App::new("a", "m");
        app.send_message("  hi  ");
        assert_eq!(app.messages()[0].content, "hi");
    }

    #[test]
    fn send_empty_message_does_nothing() {
        let mut app = App::new("a", "m");
        app.send_message("");
        assert!(app.messages().is_empty());
        app.send_message("   ");
        assert!(app.messages().is_empty());
    }

    #[test]
    fn scroll_up_increases_offset() {
        let mut app = App::new("a", "m");
        app.scroll_up(3);
        assert_eq!(app.scroll_offset(), 3);
    }

    #[test]
    fn scroll_down_decreases_offset_to_zero() {
        let mut app = App::new("a", "m");
        app.scroll_up(5);
        app.scroll_down(3);
        assert_eq!(app.scroll_offset(), 2);
        app.scroll_down(10);
        assert_eq!(app.scroll_offset(), 0);
    }

    #[test]
    fn send_message_resets_scroll() {
        let mut app = App::new("a", "m");
        app.scroll_up(10);
        app.send_message("new msg");
        assert_eq!(app.scroll_offset(), 0);
    }
}
