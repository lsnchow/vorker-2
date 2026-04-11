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
}
