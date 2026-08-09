use std::collections::HashMap;

use gpui::{Entity, ListState};
use gpui_component::text::TextViewState;

use crate::workspace::PullRequestDetailCacheKey;

pub(crate) struct OverviewMarkdownState {
    pub(crate) source: String,
    pub(crate) state: Entity<TextViewState>,
}

pub(crate) struct OverviewUiState {
    pull_request_key: Option<PullRequestDetailCacheKey>,
    ready: bool,
    pub(crate) list_state: ListState,
    pub(crate) list_item_keys: Vec<String>,
    pub(crate) markdown_states: HashMap<String, OverviewMarkdownState>,
    pub(crate) thread_expansion_overrides: HashMap<String, bool>,
}

impl OverviewUiState {
    pub(crate) fn new(list_state: ListState) -> Self {
        Self {
            pull_request_key: None,
            ready: false,
            list_state,
            list_item_keys: Vec::new(),
            markdown_states: HashMap::new(),
            thread_expansion_overrides: HashMap::new(),
        }
    }

    pub(crate) fn clear_content(&mut self) {
        self.pull_request_key = None;
        self.ready = false;
        self.list_item_keys.clear();
        self.markdown_states.clear();
        self.thread_expansion_overrides.clear();
    }

    pub(crate) fn prepare_pull_request(&mut self, key: PullRequestDetailCacheKey) {
        if self.pull_request_key.as_ref() == Some(&key) {
            return;
        }

        self.clear_content();
        self.pull_request_key = Some(key);
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.ready
    }

    pub(crate) fn mark_ready(&mut self) {
        self.ready = true;
    }
}

#[cfg(test)]
mod tests {
    use gpui::{ListAlignment, ListState, px};
    use harbor_domain::RepoId;

    use super::*;

    fn detail_key(number: u64) -> PullRequestDetailCacheKey {
        PullRequestDetailCacheKey::new(RepoId::new("acme", "app"), number, "head".to_string())
    }

    #[test]
    fn readiness_survives_refreshes_and_resets_for_another_pull_request() {
        let mut state = OverviewUiState::new(ListState::new(0, ListAlignment::Top, px(100.0)));
        let first = detail_key(1);

        state.prepare_pull_request(first.clone());
        assert!(!state.is_ready());
        state.mark_ready();
        state.prepare_pull_request(first);
        assert!(state.is_ready());

        state.prepare_pull_request(detail_key(2));
        assert!(!state.is_ready());
    }
}
