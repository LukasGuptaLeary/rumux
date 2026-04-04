#[derive(Debug, Clone, PartialEq)]
pub enum TextInputAction {
    Changed,
    Confirm,
    Cancel,
    None,
}

#[derive(Debug, Clone)]
pub struct TextInputState {
    pub text: String,
    cursor: usize,
}

impl TextInputState {
    pub fn new(initial: &str) -> Self {
        let len = initial.len();
        Self {
            text: initial.to_string(),
            cursor: len,
        }
    }

    pub fn handle_key(&mut self, key: &str, ctrl: bool) -> TextInputAction {
        if ctrl {
            return TextInputAction::None;
        }
        match key {
            "enter" => TextInputAction::Confirm,
            "escape" => TextInputAction::Cancel,
            "backspace" => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.text.remove(self.cursor);
                }
                TextInputAction::Changed
            }
            "delete" => {
                if self.cursor < self.text.len() {
                    self.text.remove(self.cursor);
                }
                TextInputAction::Changed
            }
            "left" => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                TextInputAction::None
            }
            "right" => {
                if self.cursor < self.text.len() {
                    self.cursor += 1;
                }
                TextInputAction::None
            }
            "home" => {
                self.cursor = 0;
                TextInputAction::None
            }
            "end" => {
                self.cursor = self.text.len();
                TextInputAction::None
            }
            key if key.len() == 1 => {
                self.text.insert_str(self.cursor, key);
                self.cursor += key.len();
                TextInputAction::Changed
            }
            "space" => {
                self.text.insert(self.cursor, ' ');
                self.cursor += 1;
                TextInputAction::Changed
            }
            _ => TextInputAction::None,
        }
    }

    /// Returns (text_before_cursor, text_after_cursor) for rendering with a cursor indicator
    pub fn display_parts(&self) -> (&str, &str) {
        (&self.text[..self.cursor], &self.text[self.cursor..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_typing() {
        let mut input = TextInputState::new("");
        assert_eq!(input.handle_key("h", false), TextInputAction::Changed);
        assert_eq!(input.handle_key("i", false), TextInputAction::Changed);
        assert_eq!(input.text, "hi");
    }

    #[test]
    fn test_backspace() {
        let mut input = TextInputState::new("hello");
        input.handle_key("backspace", false);
        assert_eq!(input.text, "hell");
    }

    #[test]
    fn test_confirm_cancel() {
        let mut input = TextInputState::new("test");
        assert_eq!(input.handle_key("enter", false), TextInputAction::Confirm);
        assert_eq!(input.handle_key("escape", false), TextInputAction::Cancel);
    }

    #[test]
    fn test_cursor_movement() {
        let mut input = TextInputState::new("abc");
        input.handle_key("left", false);
        input.handle_key("x", false);
        assert_eq!(input.text, "abxc");
    }

    #[test]
    fn test_home_end() {
        let mut input = TextInputState::new("abc");
        input.handle_key("home", false);
        input.handle_key("x", false);
        assert_eq!(input.text, "xabc");
        input.handle_key("end", false);
        input.handle_key("y", false);
        assert_eq!(input.text, "xabcy");
    }
}
