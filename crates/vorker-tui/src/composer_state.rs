use crate::mentions::ComposerMentionBinding;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComposerState {
    buffer: String,
    slash_selected_index: usize,
    mention_bindings: Vec<ComposerMentionBinding>,
}

impl ComposerState {
    #[must_use]
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    pub fn clear_buffer(&mut self) {
        self.buffer.clear();
    }

    pub fn set_buffer(&mut self, value: impl Into<String>) {
        self.buffer = value.into();
    }

    pub fn push_char(&mut self, ch: char) {
        self.buffer.push(ch);
    }

    pub fn pop_char(&mut self) -> Option<char> {
        self.buffer.pop()
    }

    #[must_use]
    pub fn slash_selected_index(&self) -> usize {
        self.slash_selected_index
    }

    pub fn set_slash_selected_index(&mut self, index: usize) {
        self.slash_selected_index = index;
    }

    pub fn clear_mentions(&mut self) {
        self.mention_bindings.clear();
    }

    pub fn set_mention_bindings(&mut self, bindings: Vec<ComposerMentionBinding>) {
        self.mention_bindings = bindings;
    }

    #[must_use]
    pub fn mention_bindings(&self) -> &[ComposerMentionBinding] {
        &self.mention_bindings
    }
}

#[cfg(test)]
mod tests {
    use super::ComposerState;

    #[test]
    fn composer_state_tracks_buffer_and_selection() {
        let mut state = ComposerState::default();
        state.push_char('h');
        state.push_char('i');
        state.set_slash_selected_index(2);
        assert_eq!(state.buffer(), "hi");
        assert_eq!(state.slash_selected_index(), 2);
        let popped = state.pop_char();
        assert_eq!(popped, Some('i'));
        assert_eq!(state.buffer(), "h");
        state.clear_buffer();
        assert!(state.buffer().is_empty());
    }
}
