use crate::composer_state::ComposerState;
use crate::model_picker_state::ModelPickerState;
use crate::popup_state::AppPopupState;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BottomPaneState {
    composer: ComposerState,
    popup: AppPopupState,
    model_picker: ModelPickerState,
}

impl BottomPaneState {
    #[must_use]
    pub fn composer(&self) -> &ComposerState {
        &self.composer
    }

    pub fn composer_mut(&mut self) -> &mut ComposerState {
        &mut self.composer
    }

    #[must_use]
    pub fn popup(&self) -> &AppPopupState {
        &self.popup
    }

    pub fn popup_mut(&mut self) -> &mut AppPopupState {
        &mut self.popup
    }

    #[must_use]
    pub fn model_picker(&self) -> &ModelPickerState {
        &self.model_picker
    }

    pub fn model_picker_mut(&mut self) -> &mut ModelPickerState {
        &mut self.model_picker
    }
}

#[cfg(test)]
mod tests {
    use super::BottomPaneState;

    #[test]
    fn bottom_pane_state_groups_bottom_pane_modules() {
        let mut state = BottomPaneState::default();
        state.composer_mut().set_buffer("hello");
        state
            .model_picker_mut()
            .set_selected_model_id(Some("gpt-5.4".to_string()));
        state.popup_mut().open_busy_action();

        assert_eq!(state.composer().buffer(), "hello");
        assert_eq!(state.model_picker().selected_model_id(), Some("gpt-5.4"));
        assert!(state.popup().mode().is_some());
    }
}
