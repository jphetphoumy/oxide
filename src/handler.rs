use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::app::App;
use crate::input_buffer::InputBuffer;
use crate::slash;

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Quit,
    Submit,
    TabComplete,
    InsertChar(char),
    InsertNewline,
    DeleteBack,
    DeleteForward,
    MoveLeft,
    MoveRight,
    MoveHome,
    MoveEnd,
    ScrollUp(usize),
    ScrollDown(usize),
    ApproveTool,
    DenyTool,
    None,
}

const SCROLL_LINES: usize = 5;
const MOUSE_SCROLL_LINES: usize = 3;

#[allow(clippy::missing_const_for_fn)] // match guard prevents const
pub fn handle_key_event(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c' | 'd'), KeyModifiers::CONTROL) => Action::Quit,
        (KeyCode::Enter, m) if m.contains(KeyModifiers::ALT) => Action::InsertNewline,
        (KeyCode::Enter, _) => Action::Submit,
        (KeyCode::Tab, _) => Action::TabComplete,
        (KeyCode::Backspace, _) => Action::DeleteBack,
        (KeyCode::Delete, _) => Action::DeleteForward,
        (KeyCode::Left, _) => Action::MoveLeft,
        (KeyCode::Right, _) => Action::MoveRight,
        (KeyCode::Home, _) => Action::MoveHome,
        (KeyCode::End, _) => Action::MoveEnd,
        (KeyCode::PageUp, _) => Action::ScrollUp(SCROLL_LINES),
        (KeyCode::PageDown, _) => Action::ScrollDown(SCROLL_LINES),
        (KeyCode::Char(c), _) => Action::InsertChar(c),
        _ => Action::None,
    }
}

#[allow(clippy::missing_const_for_fn)] // enum matching prevents const
pub fn handle_mouse_event(mouse: MouseEvent) -> Action {
    match mouse.kind {
        MouseEventKind::ScrollUp => Action::ScrollUp(MOUSE_SCROLL_LINES),
        MouseEventKind::ScrollDown => Action::ScrollDown(MOUSE_SCROLL_LINES),
        _ => Action::None,
    }
}

pub const fn handle_tool_approval_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => Action::ApproveTool,
        KeyCode::Char('n') | KeyCode::Esc => Action::DenyTool,
        _ => Action::None,
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PickerAction {
    MoveUp,
    MoveDown,
    Select,
    Cancel,
    Type(char),
    Backspace,
    None,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ActionOutcome {
    pub submit: Option<String>,
    pub slash_command: Option<SlashCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommand {
    New,
    Switch,
    Resume,
    ActivateSkill(String),
}

fn parse_slash_command(content: &str) -> Option<SlashCommand> {
    let trimmed = content.trim();
    match trimmed {
        "/new" => Some(SlashCommand::New),
        "/switch" => Some(SlashCommand::Switch),
        "/resume" => Some(SlashCommand::Resume),
        s if s.starts_with("/skills:") => {
            let id = s.trim_start_matches("/skills:").to_string();
            if id.is_empty() {
                None
            } else {
                Some(SlashCommand::ActivateSkill(id))
            }
        }
        _ => None,
    }
}

pub fn apply_action(app: &mut App, input: &mut InputBuffer, action: Action) -> ActionOutcome {
    let mut outcome = ActionOutcome::default();

    match action {
        Action::Quit => app.quit(),
        Action::Submit => {
            let content = input.take();
            if let Some(command) = parse_slash_command(&content) {
                outcome.slash_command = Some(command);
            } else if app.send_message(&content) {
                outcome.submit = Some(content);
            }
        }
        Action::TabComplete => {
            let content = input.content();
            if let Some(prefix) = content.strip_prefix('/')
                && let Some(completed) = slash::complete(prefix)
            {
                input.set_content(&completed);
            }
        }
        Action::InsertChar(c) => input.insert_char(c),
        Action::InsertNewline => input.insert_newline(),
        Action::DeleteBack => input.delete_char_before_cursor(),
        Action::DeleteForward => input.delete_char_after_cursor(),
        Action::MoveLeft => input.move_left(),
        Action::MoveRight => input.move_right(),
        Action::MoveHome => input.move_home(),
        Action::MoveEnd => input.move_end(),
        Action::ScrollUp(n) => app.scroll_up(n),
        Action::ScrollDown(n) => app.scroll_down(n),
        Action::ApproveTool | Action::DenyTool | Action::None => {}
    }

    outcome
}

#[allow(clippy::missing_const_for_fn)]
pub fn handle_picker_key(key: KeyEvent) -> PickerAction {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => PickerAction::Cancel,
        (KeyCode::Enter, _) => PickerAction::Select,
        (KeyCode::Up, _) => PickerAction::MoveUp,
        (KeyCode::Down, _) => PickerAction::MoveDown,
        (KeyCode::Backspace, _) => PickerAction::Backspace,
        (KeyCode::Char(c), _) => PickerAction::Type(c),
        _ => PickerAction::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{
        KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent,
        MouseEventKind,
    };

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn key_with_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn mouse(kind: MouseEventKind) -> MouseEvent {
        MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn ctrl_c_produces_quit() {
        let action = handle_key_event(key_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(matches!(action, Action::Quit));
    }

    #[test]
    fn ctrl_d_produces_quit() {
        let action = handle_key_event(key_with_mod(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert!(matches!(action, Action::Quit));
    }

    #[test]
    fn enter_produces_submit() {
        let action = handle_key_event(key(KeyCode::Enter));
        assert!(matches!(action, Action::Submit));
    }

    #[test]
    fn alt_enter_produces_insert_newline() {
        let action = handle_key_event(key_with_mod(KeyCode::Enter, KeyModifiers::ALT));
        assert!(matches!(action, Action::InsertNewline));
    }

    #[test]
    fn backspace_produces_delete_back() {
        let action = handle_key_event(key(KeyCode::Backspace));
        assert!(matches!(action, Action::DeleteBack));
    }

    #[test]
    fn delete_produces_delete_forward() {
        let action = handle_key_event(key(KeyCode::Delete));
        assert!(matches!(action, Action::DeleteForward));
    }

    #[test]
    fn arrow_keys_produce_move() {
        assert!(matches!(
            handle_key_event(key(KeyCode::Left)),
            Action::MoveLeft
        ));
        assert!(matches!(
            handle_key_event(key(KeyCode::Right)),
            Action::MoveRight
        ));
    }

    #[test]
    fn home_end_produce_move() {
        assert!(matches!(
            handle_key_event(key(KeyCode::Home)),
            Action::MoveHome
        ));
        assert!(matches!(
            handle_key_event(key(KeyCode::End)),
            Action::MoveEnd
        ));
    }

    #[test]
    fn page_up_produces_scroll_up() {
        assert!(matches!(
            handle_key_event(key(KeyCode::PageUp)),
            Action::ScrollUp(_)
        ));
    }

    #[test]
    fn page_down_produces_scroll_down() {
        assert!(matches!(
            handle_key_event(key(KeyCode::PageDown)),
            Action::ScrollDown(_)
        ));
    }

    #[test]
    fn mouse_scroll_up_produces_scroll_up() {
        assert!(matches!(
            handle_mouse_event(mouse(MouseEventKind::ScrollUp)),
            Action::ScrollUp(3)
        ));
    }

    #[test]
    fn mouse_scroll_down_produces_scroll_down() {
        assert!(matches!(
            handle_mouse_event(mouse(MouseEventKind::ScrollDown)),
            Action::ScrollDown(3)
        ));
    }

    #[test]
    fn non_scroll_mouse_event_produces_none() {
        assert!(matches!(
            handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Left))),
            Action::None
        ));
    }

    #[test]
    fn char_produces_insert_char() {
        let action = handle_key_event(key(KeyCode::Char('a')));
        assert!(matches!(action, Action::InsertChar('a')));
    }

    #[test]
    fn esc_produces_none() {
        assert!(matches!(handle_key_event(key(KeyCode::Esc)), Action::None));
    }

    #[test]
    fn apply_quit_sets_should_quit() {
        let mut app = App::new("a", "/workspace", None);
        let mut input = InputBuffer::new();
        let outcome = apply_action(&mut app, &mut input, Action::Quit);
        assert_eq!(outcome, ActionOutcome::default());
        assert!(app.should_quit());
    }

    #[test]
    fn apply_insert_char() {
        let mut app = App::new("a", "/workspace", None);
        let mut input = InputBuffer::new();
        let outcome = apply_action(&mut app, &mut input, Action::InsertChar('x'));
        assert_eq!(outcome, ActionOutcome::default());
        assert_eq!(input.content(), "x");
    }

    #[test]
    fn apply_submit_returns_content_in_outcome() {
        let mut app = App::new("a", "/workspace", None);
        let mut input = InputBuffer::new();
        input.insert_char('h');
        input.insert_char('i');

        let outcome = apply_action(&mut app, &mut input, Action::Submit);

        assert_eq!(outcome.submit, Some("hi".to_string()));
        assert!(input.is_empty());
    }

    #[test]
    fn apply_scroll_up() {
        let mut app = App::new("a", "/workspace", None);
        let mut input = InputBuffer::new();
        let outcome = apply_action(&mut app, &mut input, Action::ScrollUp(5));
        assert_eq!(outcome, ActionOutcome::default());
        assert_eq!(app.scroll_offset(), 5);
    }

    #[test]
    fn submit_switch_command_produces_slash_command() {
        let mut app = App::new("a", "/workspace", None);
        let mut input = InputBuffer::new();
        for c in "/switch".chars() {
            input.insert_char(c);
        }

        let outcome = apply_action(&mut app, &mut input, Action::Submit);

        assert_eq!(outcome.slash_command, Some(SlashCommand::Switch));
        assert!(outcome.submit.is_none());
        assert!(app.messages().is_empty()); // not sent as a message
    }

    #[test]
    fn submit_switch_with_whitespace_still_detected() {
        let mut app = App::new("a", "/workspace", None);
        let mut input = InputBuffer::new();
        for c in "  /switch  ".chars() {
            input.insert_char(c);
        }

        let outcome = apply_action(&mut app, &mut input, Action::Submit);
        assert_eq!(outcome.slash_command, Some(SlashCommand::Switch));
    }

    #[test]
    fn submit_normal_message_has_no_slash_command() {
        let mut app = App::new("a", "/workspace", None);
        let mut input = InputBuffer::new();
        for c in "hello".chars() {
            input.insert_char(c);
        }

        let outcome = apply_action(&mut app, &mut input, Action::Submit);
        assert!(outcome.slash_command.is_none());
        assert!(outcome.submit.is_some());
    }

    #[test]
    fn picker_esc_cancels() {
        let action = handle_picker_key(key(KeyCode::Esc));
        assert!(matches!(action, PickerAction::Cancel));
    }

    #[test]
    fn picker_ctrl_c_cancels() {
        let action = handle_picker_key(key_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(matches!(action, PickerAction::Cancel));
    }

    #[test]
    fn picker_enter_selects() {
        let action = handle_picker_key(key(KeyCode::Enter));
        assert!(matches!(action, PickerAction::Select));
    }

    #[test]
    fn picker_arrows_move() {
        assert!(matches!(
            handle_picker_key(key(KeyCode::Up)),
            PickerAction::MoveUp
        ));
        assert!(matches!(
            handle_picker_key(key(KeyCode::Down)),
            PickerAction::MoveDown
        ));
    }

    #[test]
    fn picker_char_types() {
        assert!(matches!(
            handle_picker_key(key(KeyCode::Char('a'))),
            PickerAction::Type('a')
        ));
    }

    #[test]
    fn picker_backspace_deletes() {
        assert!(matches!(
            handle_picker_key(key(KeyCode::Backspace)),
            PickerAction::Backspace
        ));
    }

    #[test]
    fn tab_produces_tab_complete() {
        let action = handle_key_event(key(KeyCode::Tab));
        assert!(matches!(action, Action::TabComplete));
    }

    #[test]
    fn tab_complete_on_slash_prefix_completes_command() {
        let mut app = App::new("a", "/workspace", None);
        let mut input = InputBuffer::new();
        for c in "/sw".chars() {
            input.insert_char(c);
        }

        apply_action(&mut app, &mut input, Action::TabComplete);
        assert_eq!(input.content(), "/switch");
    }

    #[test]
    fn tab_complete_no_match_does_nothing() {
        let mut app = App::new("a", "/workspace", None);
        let mut input = InputBuffer::new();
        for c in "/xyz".chars() {
            input.insert_char(c);
        }

        apply_action(&mut app, &mut input, Action::TabComplete);
        assert_eq!(input.content(), "/xyz");
    }

    #[test]
    fn tab_complete_without_slash_does_nothing() {
        let mut app = App::new("a", "/workspace", None);
        let mut input = InputBuffer::new();
        for c in "hello".chars() {
            input.insert_char(c);
        }

        apply_action(&mut app, &mut input, Action::TabComplete);
        assert_eq!(input.content(), "hello");
    }

    #[test]
    fn apply_scroll_down() {
        let mut app = App::new("a", "/workspace", None);
        let mut input = InputBuffer::new();
        app.scroll_up(10);
        let outcome = apply_action(&mut app, &mut input, Action::ScrollDown(3));
        assert_eq!(outcome, ActionOutcome::default());
        assert_eq!(app.scroll_offset(), 7);
    }

    #[test]
    fn parse_new_slash_command() {
        assert_eq!(parse_slash_command("/new"), Some(SlashCommand::New));
    }

    #[test]
    fn parse_new_with_whitespace() {
        assert_eq!(parse_slash_command("  /new  "), Some(SlashCommand::New));
    }

    #[test]
    fn submit_new_command_produces_slash_command() {
        let mut app = App::new("a", "/workspace", None);
        let mut input = InputBuffer::new();
        for c in "/new".chars() {
            input.insert_char(c);
        }

        let outcome = apply_action(&mut app, &mut input, Action::Submit);

        assert_eq!(outcome.slash_command, Some(SlashCommand::New));
        assert!(outcome.submit.is_none());
        assert!(app.messages().is_empty());
    }

    #[test]
    fn submit_new_with_whitespace_still_detected() {
        let mut app = App::new("a", "/workspace", None);
        let mut input = InputBuffer::new();
        for c in "  /new  ".chars() {
            input.insert_char(c);
        }

        let outcome = apply_action(&mut app, &mut input, Action::Submit);
        assert_eq!(outcome.slash_command, Some(SlashCommand::New));
    }

    #[test]
    fn parse_resume_slash_command() {
        assert_eq!(parse_slash_command("/resume"), Some(SlashCommand::Resume));
    }

    #[test]
    fn submit_resume_command_produces_slash_command() {
        let mut app = App::new("a", "/workspace", None);
        let mut input = InputBuffer::new();
        for c in "/resume".chars() {
            input.insert_char(c);
        }

        let outcome = apply_action(&mut app, &mut input, Action::Submit);

        assert_eq!(outcome.slash_command, Some(SlashCommand::Resume));
        assert!(outcome.submit.is_none());
        assert!(app.messages().is_empty());
    }
}
