use crate::composer_state::ComposerState;
use crate::model_picker_state::ModelPickerState;
use crate::popup_state::{AppPopupState, PopupMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BottomPaneSurface {
    Composer,
    Mention,
    ModelPicker,
    Permission,
    SkillAction,
    SkillToggle,
    BusyAction,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BottomPaneState {
    composer: ComposerState,
    popup: AppPopupState,
    model_picker: ModelPickerState,
}

impl BottomPaneState {
    #[must_use]
    pub fn active_surface(&self) -> BottomPaneSurface {
        if self.popup.is_mode(PopupMode::Permission) {
            BottomPaneSurface::Permission
        } else if self.popup.is_mode(PopupMode::SkillAction) {
            BottomPaneSurface::SkillAction
        } else if self.popup.is_mode(PopupMode::SkillToggle) {
            BottomPaneSurface::SkillToggle
        } else if self.popup.is_mode(PopupMode::BusyAction) {
            BottomPaneSurface::BusyAction
        } else if self.model_picker.is_open() {
            BottomPaneSurface::ModelPicker
        } else if self.popup.is_mode(PopupMode::Mention) {
            BottomPaneSurface::Mention
        } else {
            BottomPaneSurface::Composer
        }
    }

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
    use super::{BottomPaneState, BottomPaneSurface};
    use crate::popup_state::PopupMode;

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

    #[test]
    fn bottom_pane_surface_respects_shell_precedence() {
        let mut state = BottomPaneState::default();
        assert_eq!(state.active_surface(), BottomPaneSurface::Composer);

        state.popup_mut().open_mention();
        assert_eq!(state.active_surface(), BottomPaneSurface::Mention);

        state.model_picker_mut().open();
        assert_eq!(state.active_surface(), BottomPaneSurface::ModelPicker);

        state.popup_mut().open_busy_action();
        assert_eq!(state.active_surface(), BottomPaneSurface::BusyAction);
        assert_eq!(state.popup().mode(), Some(&PopupMode::BusyAction));
    }
}
