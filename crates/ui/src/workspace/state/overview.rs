use std::collections::HashMap;

use gpui::{Entity, ListState};
use gpui_component::text::TextViewState;

pub(crate) struct OverviewMarkdownState {
    pub(crate) source: String,
    pub(crate) state: Entity<TextViewState>,
}

pub(crate) struct OverviewUiState {
    pub(crate) list_state: ListState,
    pub(crate) list_item_keys: Vec<String>,
    pub(crate) markdown_states: HashMap<String, OverviewMarkdownState>,
    pub(crate) thread_expansion_overrides: HashMap<String, bool>,
}

impl OverviewUiState {
    pub(crate) fn new(list_state: ListState) -> Self {
        Self {
            list_state,
            list_item_keys: Vec::new(),
            markdown_states: HashMap::new(),
            thread_expansion_overrides: HashMap::new(),
        }
    }

    pub(crate) fn clear_content(&mut self) {
        self.list_item_keys.clear();
        self.markdown_states.clear();
        self.thread_expansion_overrides.clear();
    }
}
