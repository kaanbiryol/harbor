use std::collections::{HashMap, VecDeque};

use gpui::{Entity, ListState, Subscription};
use gpui_component::text::TextViewState;

use crate::workspace::PullRequestDetailCacheKey;

pub(crate) struct OverviewMarkdownState {
    pub(crate) source: String,
    pub(crate) state: Entity<TextViewState>,
}

#[derive(Default)]
struct OverviewContentState {
    ready: bool,
    description_ready: bool,
    description_markdown_subscription: Option<Subscription>,
    markdown_states: HashMap<String, OverviewMarkdownState>,
    thread_expansion_overrides: HashMap<String, bool>,
}

pub(crate) struct OverviewUiState {
    pull_request_key: Option<PullRequestDetailCacheKey>,
    content: OverviewContentState,
    cached_content: HashMap<PullRequestDetailCacheKey, OverviewContentState>,
    cached_content_order: VecDeque<PullRequestDetailCacheKey>,
    pub(crate) list_state: ListState,
    pub(crate) list_item_keys: Vec<String>,
}

const MAX_CACHED_OVERVIEWS: usize = 8;

impl OverviewUiState {
    pub(crate) fn new(list_state: ListState) -> Self {
        Self {
            pull_request_key: None,
            content: OverviewContentState::default(),
            cached_content: HashMap::new(),
            cached_content_order: VecDeque::new(),
            list_state,
            list_item_keys: Vec::new(),
        }
    }

    pub(crate) fn clear_content(&mut self) {
        self.pull_request_key = None;
        self.clear_current_content();
    }

    pub(crate) fn clear_cached_content(&mut self) {
        self.clear_content();
        self.cached_content.clear();
        self.cached_content_order.clear();
    }

    fn clear_current_content(&mut self) {
        self.content = OverviewContentState::default();
        self.list_item_keys.clear();
    }

    pub(crate) fn cache_current_content(&mut self) {
        let Some(key) = self.pull_request_key.take() else {
            return;
        };
        let content = std::mem::take(&mut self.content);
        self.clear_current_content();
        self.insert_cached_content(key, content);
    }

    pub(crate) fn prepare_pull_request(
        &mut self,
        key: PullRequestDetailCacheKey,
        restore_cached: bool,
    ) {
        if self.pull_request_key.as_ref() == Some(&key) {
            return;
        }

        self.cache_current_content();
        self.clear_current_content();
        self.pull_request_key = Some(key.clone());

        let cached = self.cached_content.remove(&key);
        self.cached_content_order
            .retain(|cached_key| cached_key != &key);
        if restore_cached && let Some(cached) = cached {
            self.content = cached;
        }
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.content.ready
    }

    pub(crate) fn description_ready(&self) -> bool {
        self.content.description_ready
    }

    pub(crate) fn description_preparing(&self) -> bool {
        self.content.description_markdown_subscription.is_some()
    }

    pub(crate) fn set_description_markdown_subscription(&mut self, subscription: Subscription) {
        self.content.description_markdown_subscription = Some(subscription);
    }

    pub(crate) fn mark_description_ready(&mut self, key: &PullRequestDetailCacheKey) -> bool {
        if self.pull_request_key.as_ref() == Some(key) {
            if self.content.description_ready {
                return false;
            }
            self.content.description_ready = true;
            return true;
        }

        let Some(cached) = self.cached_content.get_mut(key) else {
            return false;
        };
        if cached.description_ready {
            return false;
        }
        cached.description_ready = true;
        true
    }

    pub(crate) fn mark_ready(&mut self) {
        self.content.ready = true;
    }

    pub(crate) fn markdown_state_mut(&mut self, key: &str) -> Option<&mut OverviewMarkdownState> {
        self.content.markdown_states.get_mut(key)
    }

    pub(crate) fn insert_markdown_state(&mut self, key: String, state: OverviewMarkdownState) {
        self.content.markdown_states.insert(key, state);
    }

    #[cfg(test)]
    pub(crate) fn markdown_state_source(&self, key: &str) -> Option<&str> {
        self.content
            .markdown_states
            .get(key)
            .map(|state| state.source.as_str())
    }

    pub(crate) fn thread_expansion_override(&self, thread_id: &str) -> Option<bool> {
        self.content
            .thread_expansion_overrides
            .get(thread_id)
            .copied()
    }

    pub(crate) fn set_thread_expansion_override(&mut self, thread_id: String, expanded: bool) {
        self.content
            .thread_expansion_overrides
            .insert(thread_id, expanded);
    }

    fn insert_cached_content(
        &mut self,
        key: PullRequestDetailCacheKey,
        content: OverviewContentState,
    ) {
        self.cached_content_order
            .retain(|cached_key| cached_key != &key);
        self.cached_content_order.push_back(key.clone());
        self.cached_content.insert(key, content);

        while self.cached_content_order.len() > MAX_CACHED_OVERVIEWS {
            if let Some(expired) = self.cached_content_order.pop_front() {
                self.cached_content.remove(&expired);
            }
        }
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
    fn readiness_is_restored_for_a_cached_pull_request() {
        let mut state = OverviewUiState::new(ListState::new(0, ListAlignment::Top, px(100.0)));
        let first = detail_key(1);

        state.prepare_pull_request(first.clone(), false);
        assert!(!state.is_ready());
        assert!(!state.description_ready());
        assert!(state.mark_description_ready(&first));
        assert!(state.description_ready());
        state.mark_ready();
        state.prepare_pull_request(first.clone(), true);
        assert!(state.is_ready());
        assert!(state.description_ready());

        state.prepare_pull_request(detail_key(2), false);
        assert!(!state.is_ready());
        assert!(!state.description_ready());

        state.prepare_pull_request(first, true);
        assert!(state.is_ready());
        assert!(state.description_ready());
    }

    #[test]
    fn description_completion_updates_cached_content() {
        let mut state = OverviewUiState::new(ListState::new(0, ListAlignment::Top, px(100.0)));
        let first = detail_key(1);

        state.prepare_pull_request(first.clone(), false);
        state.prepare_pull_request(detail_key(2), false);

        assert!(state.mark_description_ready(&first));
        state.prepare_pull_request(first, true);
        assert!(state.description_ready());
    }
}
