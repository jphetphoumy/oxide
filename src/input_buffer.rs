/// A simple text buffer that tracks content and cursor position.
/// Cursor is a byte offset into the flat string.
pub struct InputBuffer {
    content: String,
    cursor: usize,
}

impl InputBuffer {
    pub const fn new() -> Self {
        Self {
            content: String::new(),
            cursor: 0,
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    pub const fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    pub fn insert_char(&mut self, c: char) {
        self.content.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    pub fn delete_char_before_cursor(&mut self) {
        if self.cursor == 0 {
            return;
        }
        // Find the previous char boundary
        let prev = self.content[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
        self.content.drain(prev..self.cursor);
        self.cursor = prev;
    }

    pub fn delete_char_after_cursor(&mut self) {
        if self.cursor >= self.content.len() {
            return;
        }
        let next = self.content[self.cursor..]
            .char_indices()
            .nth(1)
            .map_or(self.content.len(), |(i, _)| self.cursor + i);
        self.content.drain(self.cursor..next);
    }

    pub fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor = self.content[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
    }

    pub fn move_right(&mut self) {
        if self.cursor >= self.content.len() {
            return;
        }
        self.cursor = self.content[self.cursor..]
            .char_indices()
            .nth(1)
            .map_or(self.content.len(), |(i, _)| self.cursor + i);
    }

    pub const fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub const fn move_end(&mut self) {
        self.cursor = self.content.len();
    }

    /// Replace the buffer content and move cursor to end.
    pub fn set_content(&mut self, content: &str) {
        self.content = content.to_string();
        self.cursor = self.content.len();
    }

    /// Take the content, clearing the buffer and resetting the cursor.
    pub fn take(&mut self) -> String {
        self.cursor = 0;
        std::mem::take(&mut self.content)
    }

    /// Returns the content split into lines (for rendering).
    pub fn lines(&self) -> Vec<&str> {
        if self.content.is_empty() {
            return vec![""];
        }
        self.content.split('\n').collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_buffer_is_empty() {
        let buf = InputBuffer::new();
        assert!(buf.is_empty());
        assert_eq!(buf.content(), "");
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn insert_char_appends_at_cursor() {
        let mut buf = InputBuffer::new();
        buf.insert_char('h');
        buf.insert_char('i');
        assert_eq!(buf.content(), "hi");
        assert_eq!(buf.cursor(), 2);
    }

    #[test]
    fn insert_char_in_middle() {
        let mut buf = InputBuffer::new();
        buf.insert_char('a');
        buf.insert_char('c');
        buf.move_left();
        buf.insert_char('b');
        assert_eq!(buf.content(), "abc");
    }

    #[test]
    fn insert_newline() {
        let mut buf = InputBuffer::new();
        buf.insert_char('a');
        buf.insert_newline();
        buf.insert_char('b');
        assert_eq!(buf.content(), "a\nb");
    }

    #[test]
    fn delete_char_before_cursor() {
        let mut buf = InputBuffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.delete_char_before_cursor();
        assert_eq!(buf.content(), "a");
        assert_eq!(buf.cursor(), 1);
    }

    #[test]
    fn delete_char_before_cursor_at_start_does_nothing() {
        let mut buf = InputBuffer::new();
        buf.delete_char_before_cursor();
        assert_eq!(buf.content(), "");
    }

    #[test]
    fn delete_char_after_cursor() {
        let mut buf = InputBuffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.move_home();
        buf.delete_char_after_cursor();
        assert_eq!(buf.content(), "b");
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn delete_char_after_cursor_at_end_does_nothing() {
        let mut buf = InputBuffer::new();
        buf.insert_char('x');
        buf.delete_char_after_cursor();
        assert_eq!(buf.content(), "x");
    }

    #[test]
    fn move_left_and_right() {
        let mut buf = InputBuffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.insert_char('c');
        buf.move_left();
        assert_eq!(buf.cursor(), 2);
        buf.move_left();
        assert_eq!(buf.cursor(), 1);
        buf.move_right();
        assert_eq!(buf.cursor(), 2);
    }

    #[test]
    fn move_left_at_start_stays() {
        let mut buf = InputBuffer::new();
        buf.move_left();
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn move_right_at_end_stays() {
        let mut buf = InputBuffer::new();
        buf.insert_char('x');
        buf.move_right();
        assert_eq!(buf.cursor(), 1);
    }

    #[test]
    fn move_home_and_end() {
        let mut buf = InputBuffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.insert_char('c');
        buf.move_home();
        assert_eq!(buf.cursor(), 0);
        buf.move_end();
        assert_eq!(buf.cursor(), 3);
    }

    #[test]
    fn take_clears_buffer() {
        let mut buf = InputBuffer::new();
        buf.insert_char('h');
        buf.insert_char('i');
        let content = buf.take();
        assert_eq!(content, "hi");
        assert!(buf.is_empty());
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn lines_splits_on_newline() {
        let mut buf = InputBuffer::new();
        buf.insert_char('a');
        buf.insert_newline();
        buf.insert_char('b');
        assert_eq!(buf.lines(), vec!["a", "b"]);
    }

    #[test]
    fn lines_empty_buffer() {
        let buf = InputBuffer::new();
        assert_eq!(buf.lines(), vec![""]);
    }

    #[test]
    fn set_content_replaces_buffer() {
        let mut buf = InputBuffer::new();
        buf.insert_char('x');
        buf.set_content("/switch");
        assert_eq!(buf.content(), "/switch");
        assert_eq!(buf.cursor(), 7);
    }

    #[test]
    fn unicode_support() {
        let mut buf = InputBuffer::new();
        buf.insert_char('é');
        buf.insert_char('ñ');
        assert_eq!(buf.content(), "éñ");
        buf.delete_char_before_cursor();
        assert_eq!(buf.content(), "é");
    }
}
