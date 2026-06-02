use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::input_buffer::InputBuffer;

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Quit,
    Submit,
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
    None,
}

const SCROLL_LINES: usize = 5;

#[allow(clippy::missing_const_for_fn)] // match guard prevents const
pub fn handle_key_event(key: KeyEvent) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c' | 'd'), KeyModifiers::CONTROL) => Action::Quit,
        (KeyCode::Enter, m) if m.contains(KeyModifiers::ALT) => Action::InsertNewline,
        (KeyCode::Enter, _) => Action::Submit,
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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ActionOutcome {
    pub submit: Option<String>,
}

pub fn apply_action(app: &mut App, input: &mut InputBuffer, action: Action) -> ActionOutcome {
    let mut outcome = ActionOutcome::default();

    match action {
        Action::Quit => app.quit(),
        Action::Submit => {
            let content = input.take();
            if app.send_message(&content) {
                outcome.submit = Some(content);
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
        Action::None => {}
    }

    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

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
        let mut app = App::new("a");
        let mut input = InputBuffer::new();
        let outcome = apply_action(&mut app, &mut input, Action::Quit);
        assert_eq!(outcome, ActionOutcome::default());
        assert!(app.should_quit());
    }

    #[test]
    fn apply_insert_char() {
        let mut app = App::new("a");
        let mut input = InputBuffer::new();
        let outcome = apply_action(&mut app, &mut input, Action::InsertChar('x'));
        assert_eq!(outcome, ActionOutcome::default());
        assert_eq!(input.content(), "x");
    }

    #[test]
    fn apply_submit_returns_content_in_outcome() {
        let mut app = App::new("a");
        let mut input = InputBuffer::new();
        input.insert_char('h');
        input.insert_char('i');

        let outcome = apply_action(&mut app, &mut input, Action::Submit);

        assert_eq!(outcome.submit, Some("hi".to_string()));
        assert!(input.is_empty());
    }

    #[test]
    fn apply_scroll_up() {
        let mut app = App::new("a");
        let mut input = InputBuffer::new();
        let outcome = apply_action(&mut app, &mut input, Action::ScrollUp(5));
        assert_eq!(outcome, ActionOutcome::default());
        assert_eq!(app.scroll_offset(), 5);
    }

    #[test]
    fn apply_scroll_down() {
        let mut app = App::new("a");
        let mut input = InputBuffer::new();
        app.scroll_up(10);
        let outcome = apply_action(&mut app, &mut input, Action::ScrollDown(3));
        assert_eq!(outcome, ActionOutcome::default());
        assert_eq!(app.scroll_offset(), 7);
    }
}
