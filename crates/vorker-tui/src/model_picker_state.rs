#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelPickerState {
    selected_model_id: Option<String>,
    model_choices: Vec<String>,
    open: bool,
}

impl ModelPickerState {
    #[must_use]
    pub fn selected_model_id(&self) -> Option<&str> {
        self.selected_model_id.as_deref()
    }

    pub fn set_selected_model_id(&mut self, value: Option<String>) {
        self.selected_model_id = value;
    }

    #[must_use]
    pub fn model_choices(&self) -> &[String] {
        &self.model_choices
    }

    pub fn set_model_choices(&mut self, choices: Vec<String>) {
        self.model_choices = choices;
        if self
            .selected_model_id
            .as_deref()
            .is_none_or(|selected| !self.model_choices.iter().any(|item| item == selected))
            && let Some(first) = self.model_choices.first()
        {
            self.selected_model_id = Some(first.clone());
        }
    }

    pub fn ensure_choice(&mut self, model: impl Into<String>) {
        let model = model.into();
        if !self.model_choices.iter().any(|item| item == &model) {
            self.model_choices.push(model);
        }
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn move_selection(&mut self, delta: isize) {
        let current = self
            .selected_model_id()
            .and_then(|selected| self.model_choices.iter().position(|item| item == selected))
            .unwrap_or(0);
        let index = cycle_index(current, self.model_choices.len(), delta);
        self.selected_model_id = self.model_choices.get(index).cloned();
    }

    pub fn confirm_selection(&mut self) -> Option<String> {
        let selected = self.selected_model_id.clone();
        self.close();
        selected
    }
}

fn cycle_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    let len = len as isize;
    let current = current as isize;
    (current + delta).rem_euclid(len) as usize
}

#[cfg(test)]
mod tests {
    use super::ModelPickerState;

    #[test]
    fn model_picker_state_keeps_selected_model_valid() {
        let mut state = ModelPickerState::default();
        state.set_model_choices(vec!["gpt-5.4".to_string(), "gpt-5.3-codex".to_string()]);
        assert_eq!(state.selected_model_id(), Some("gpt-5.4"));
        state.set_selected_model_id(Some("gpt-5.3-codex".to_string()));
        state.set_model_choices(vec!["gpt-5.3-codex".to_string()]);
        assert_eq!(state.selected_model_id(), Some("gpt-5.3-codex"));
    }

    #[test]
    fn model_picker_state_cycles_and_confirms_selection() {
        let mut state = ModelPickerState::default();
        state.set_model_choices(vec!["gpt-5.4".to_string(), "gpt-5.3-codex".to_string()]);
        state.open();
        state.move_selection(1);
        assert_eq!(state.selected_model_id(), Some("gpt-5.3-codex"));
        assert_eq!(state.confirm_selection().as_deref(), Some("gpt-5.3-codex"));
        assert!(!state.is_open());
    }
}
