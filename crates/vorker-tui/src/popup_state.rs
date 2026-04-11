use crate::render::PopupItem;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionOptionView {
    pub option_id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PopupMode {
    Mention,
    Permission,
    SkillAction,
    SkillToggle,
    BusyAction,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppPopupState {
    mode: Option<PopupMode>,
    permission_title: Option<String>,
    permission_items: Vec<PermissionOptionView>,
    mention_items: Vec<String>,
    selected_index: usize,
    skill_toggle_query: String,
}

impl AppPopupState {
    #[must_use]
    pub fn mode(&self) -> Option<&PopupMode> {
        self.mode.as_ref()
    }

    #[must_use]
    pub fn is_mode(&self, mode: PopupMode) -> bool {
        self.mode.as_ref() == Some(&mode)
    }

    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn set_selected_index(&mut self, index: usize) {
        self.selected_index = index;
    }

    pub fn cycle_selected_index(&mut self, len: usize, delta: isize) {
        self.selected_index = cycle_index(self.selected_index, len, delta);
    }

    pub fn clamp_selected_index(&mut self, len: usize) {
        self.selected_index = self.selected_index.min(len.saturating_sub(1));
    }

    #[must_use]
    pub fn permission_option_id(&self) -> Option<String> {
        self.permission_items
            .get(self.selected_index)
            .map(|item| item.option_id.clone())
    }

    #[must_use]
    pub fn permission_items_len(&self) -> usize {
        self.permission_items.len()
    }

    pub fn open_permission_prompt(
        &mut self,
        title: impl Into<String>,
        items: Vec<PermissionOptionView>,
    ) {
        self.permission_title = Some(title.into());
        self.permission_items = items;
        self.selected_index = 0;
        self.mode = Some(PopupMode::Permission);
    }

    pub fn open_skill_action(&mut self) {
        self.selected_index = 0;
        self.mode = Some(PopupMode::SkillAction);
    }

    pub fn open_skill_toggle(&mut self, clear_query: bool) {
        if clear_query {
            self.skill_toggle_query.clear();
        }
        self.selected_index = 0;
        self.mode = Some(PopupMode::SkillToggle);
    }

    pub fn open_busy_action(&mut self) {
        self.selected_index = 0;
        self.mode = Some(PopupMode::BusyAction);
    }

    pub fn open_mention(&mut self) {
        self.mention_items.clear();
        self.mode = Some(PopupMode::Mention);
    }

    pub fn set_mention_items(&mut self, items: Vec<String>) {
        self.mention_items = items;
        self.clamp_selected_index(self.mention_items.len());
    }

    #[must_use]
    pub fn mention_items(&self) -> &[String] {
        &self.mention_items
    }

    pub fn close(&mut self) {
        self.mode = None;
        self.permission_title = None;
        self.permission_items.clear();
        self.mention_items.clear();
        self.selected_index = 0;
        self.skill_toggle_query.clear();
    }

    #[must_use]
    pub fn skill_toggle_query(&self) -> &str {
        &self.skill_toggle_query
    }

    pub fn push_skill_toggle_char(&mut self, ch: char) {
        self.skill_toggle_query.push(ch);
    }

    pub fn pop_skill_toggle_char(&mut self) {
        self.skill_toggle_query.pop();
    }

    #[must_use]
    pub fn render_state(
        &self,
        filtered_skill_items: &[PopupItem],
    ) -> (Option<String>, Vec<PopupItem>, usize) {
        match self.mode {
            Some(PopupMode::Permission) => (
                self.permission_title.clone(),
                self.permission_items
                    .iter()
                    .map(|item| PopupItem {
                        label: item.name.clone(),
                        description: None,
                        selectable: true,
                    })
                    .collect(),
                self.selected_index,
            ),
            Some(PopupMode::SkillAction) => (
                Some("Skills - choose an action".to_string()),
                vec![
                    PopupItem {
                        label: "1. List skills".to_string(),
                        description: Some("show installed skills and state".to_string()),
                        selectable: true,
                    },
                    PopupItem {
                        label: "2. Enable/Disable Skills".to_string(),
                        description: Some("Enable or disable skills.".to_string()),
                        selectable: true,
                    },
                ],
                self.selected_index,
            ),
            Some(PopupMode::SkillToggle) => (
                Some(if self.skill_toggle_query.is_empty() {
                    "Enable/Disable Skills - Type to search skills".to_string()
                } else {
                    format!(
                        "Enable/Disable Skills - search: {}",
                        self.skill_toggle_query
                    )
                }),
                filtered_skill_items.to_vec(),
                self.selected_index,
            ),
            Some(PopupMode::BusyAction) => (
                Some("Current work is active".to_string()),
                vec![
                    PopupItem {
                        label: "1. Queue after current turn".to_string(),
                        description: Some(
                            "Run this prompt when the current work finishes.".to_string(),
                        ),
                        selectable: true,
                    },
                    PopupItem {
                        label: "2. Send as steering guidance".to_string(),
                        description: Some(
                            "Send this text to the active turn instead of queueing it.".to_string(),
                        ),
                        selectable: true,
                    },
                ],
                self.selected_index,
            ),
            _ => (None, Vec::new(), 0),
        }
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
    use super::{AppPopupState, PermissionOptionView, PopupMode};

    #[test]
    fn popup_state_cycles_and_clamps_selection() {
        let mut state = AppPopupState::default();
        state.cycle_selected_index(3, 1);
        assert_eq!(state.selected_index(), 1);
        state.cycle_selected_index(3, 5);
        assert_eq!(state.selected_index(), 0);
        state.set_selected_index(4);
        state.clamp_selected_index(2);
        assert_eq!(state.selected_index(), 1);
    }

    #[test]
    fn popup_state_renders_permission_prompt() {
        let mut state = AppPopupState::default();
        state.open_permission_prompt(
            "Need approval",
            vec![PermissionOptionView {
                option_id: "approve".to_string(),
                name: "Approve".to_string(),
            }],
        );
        let (title, items, selected) = state.render_state(&[]);
        assert_eq!(state.mode(), Some(&PopupMode::Permission));
        assert_eq!(title.as_deref(), Some("Need approval"));
        assert_eq!(items[0].label, "Approve");
        assert_eq!(selected, 0);
    }
}
