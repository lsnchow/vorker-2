use crate::composer_state::ComposerState;
use crate::model_picker_state::ModelPickerState;
use crate::popup_state::{AppPopupState, PopupMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposerKeyAction {
    Escape,
    AutocompleteSlash,
    NavigateSlash(isize),
    RecallHistory(isize),
    Submit,
    Backspace,
    Insert(char),
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BottomPaneDispatch {
    Permission(KeyEvent),
    SkillAction(KeyEvent),
    SkillToggle(KeyEvent),
    BusyAction(KeyEvent),
    ModelPicker(KeyEvent),
    Mention(KeyEvent),
    Composer(ComposerKeyAction),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListSurfaceAction {
    Move(isize),
    Submit,
    Close,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillToggleSurfaceAction {
    Move(isize),
    ToggleSelected,
    QueryBackspace,
    QueryInsert(char),
    Close,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusySurfaceAction {
    Move(isize),
    Submit,
    EditBackspace,
    EditInsert(char),
    Close,
    None,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BottomPaneState {
    composer: ComposerState,
    popup: AppPopupState,
    model_picker: ModelPickerState,
}

impl BottomPaneState {
    #[must_use]
    pub fn dispatch_key(&self, key: KeyEvent, prompt_history_empty: bool) -> BottomPaneDispatch {
        match self.active_surface() {
            BottomPaneSurface::Permission => BottomPaneDispatch::Permission(key),
            BottomPaneSurface::SkillAction => BottomPaneDispatch::SkillAction(key),
            BottomPaneSurface::SkillToggle => BottomPaneDispatch::SkillToggle(key),
            BottomPaneSurface::BusyAction => BottomPaneDispatch::BusyAction(key),
            BottomPaneSurface::ModelPicker => BottomPaneDispatch::ModelPicker(key),
            BottomPaneSurface::Mention => BottomPaneDispatch::Mention(key),
            BottomPaneSurface::Composer => {
                BottomPaneDispatch::Composer(self.dispatch_composer_key(key, prompt_history_empty))
            }
        }
    }

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

    fn dispatch_composer_key(
        &self,
        key: KeyEvent,
        prompt_history_empty: bool,
    ) -> ComposerKeyAction {
        match key.code {
            KeyCode::Esc => ComposerKeyAction::Escape,
            KeyCode::Tab => ComposerKeyAction::AutocompleteSlash,
            KeyCode::Up if prompt_history_empty => ComposerKeyAction::NavigateSlash(-1),
            KeyCode::Up => ComposerKeyAction::RecallHistory(-1),
            KeyCode::Down if prompt_history_empty => ComposerKeyAction::NavigateSlash(1),
            KeyCode::Down => ComposerKeyAction::RecallHistory(1),
            KeyCode::Enter => ComposerKeyAction::Submit,
            KeyCode::Backspace => ComposerKeyAction::Backspace,
            KeyCode::Char(ch)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                ComposerKeyAction::Insert(ch)
            }
            _ => ComposerKeyAction::None,
        }
    }

    #[must_use]
    pub fn dispatch_permission_key(&self, key: KeyEvent) -> ListSurfaceAction {
        dispatch_list_surface_key(key)
    }

    #[must_use]
    pub fn dispatch_skill_action_key(&self, key: KeyEvent) -> ListSurfaceAction {
        dispatch_list_surface_key(key)
    }

    #[must_use]
    pub fn dispatch_model_picker_key(&self, key: KeyEvent) -> ListSurfaceAction {
        dispatch_list_surface_key(key)
    }

    #[must_use]
    pub fn dispatch_mention_key(&self, key: KeyEvent) -> ListSurfaceAction {
        dispatch_list_surface_key(key)
    }

    #[must_use]
    pub fn dispatch_skill_toggle_key(&self, key: KeyEvent) -> SkillToggleSurfaceAction {
        match key.code {
            KeyCode::Up => SkillToggleSurfaceAction::Move(-1),
            KeyCode::Down => SkillToggleSurfaceAction::Move(1),
            KeyCode::Enter | KeyCode::Char(' ') => SkillToggleSurfaceAction::ToggleSelected,
            KeyCode::Backspace => SkillToggleSurfaceAction::QueryBackspace,
            KeyCode::Esc => SkillToggleSurfaceAction::Close,
            KeyCode::Char(ch)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                SkillToggleSurfaceAction::QueryInsert(ch)
            }
            _ => SkillToggleSurfaceAction::None,
        }
    }

    #[must_use]
    pub fn dispatch_busy_action_key(&self, key: KeyEvent) -> BusySurfaceAction {
        match key.code {
            KeyCode::Up => BusySurfaceAction::Move(-1),
            KeyCode::Down => BusySurfaceAction::Move(1),
            KeyCode::Enter => BusySurfaceAction::Submit,
            KeyCode::Esc => BusySurfaceAction::Close,
            KeyCode::Backspace => BusySurfaceAction::EditBackspace,
            KeyCode::Char(ch)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                BusySurfaceAction::EditInsert(ch)
            }
            _ => BusySurfaceAction::None,
        }
    }
}

fn dispatch_list_surface_key(key: KeyEvent) -> ListSurfaceAction {
    match key.code {
        KeyCode::Up => ListSurfaceAction::Move(-1),
        KeyCode::Down => ListSurfaceAction::Move(1),
        KeyCode::Enter => ListSurfaceAction::Submit,
        KeyCode::Esc => ListSurfaceAction::Close,
        _ => ListSurfaceAction::None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BottomPaneDispatch, BottomPaneState, BottomPaneSurface, BusySurfaceAction,
        ComposerKeyAction, ListSurfaceAction, SkillToggleSurfaceAction,
    };
    use crate::popup_state::PopupMode;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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

    #[test]
    fn bottom_pane_dispatches_composer_keys() {
        let state = BottomPaneState::default();
        assert_eq!(
            state.dispatch_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), true),
            BottomPaneDispatch::Composer(ComposerKeyAction::AutocompleteSlash)
        );
        assert_eq!(
            state.dispatch_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), false),
            BottomPaneDispatch::Composer(ComposerKeyAction::RecallHistory(1))
        );
    }

    #[test]
    fn bottom_pane_dispatches_surface_specific_keys() {
        let state = BottomPaneState::default();
        assert_eq!(
            state.dispatch_permission_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            ListSurfaceAction::Move(1)
        );
        assert_eq!(
            state.dispatch_skill_toggle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            SkillToggleSurfaceAction::QueryInsert('x')
        );
        assert_eq!(
            state.dispatch_busy_action_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
            BusySurfaceAction::EditBackspace
        );
    }
}
