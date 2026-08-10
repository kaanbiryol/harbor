use std::collections::{HashMap, HashSet, VecDeque};

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
    description_preparing: bool,
    description_generation: u64,
    description_blocks: Vec<OverviewMarkdownState>,
    description_pending_blocks: HashSet<usize>,
    description_markdown_subscriptions: Vec<Subscription>,
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

    pub(crate) fn start_description_preparation(&mut self) -> Option<u64> {
        if self.content.description_ready || self.content.description_preparing {
            return None;
        }

        self.content.description_preparing = true;
        self.content.description_generation = self.content.description_generation.wrapping_add(1);
        Some(self.content.description_generation)
    }

    pub(crate) fn is_current_description_preparation(
        &self,
        key: &PullRequestDetailCacheKey,
        generation: u64,
    ) -> bool {
        self.pull_request_key.as_ref() == Some(key)
            && self.content.description_preparing
            && self.content.description_generation == generation
    }

    pub(crate) fn cancel_description_preparation(
        &mut self,
        key: &PullRequestDetailCacheKey,
        generation: u64,
    ) {
        let content = if self.pull_request_key.as_ref() == Some(key) {
            Some(&mut self.content)
        } else {
            self.cached_content.get_mut(key)
        };
        if let Some(content) = content
            && content.description_generation == generation
        {
            content.description_preparing = false;
        }
    }

    pub(crate) fn take_description_blocks(
        &mut self,
        key: &PullRequestDetailCacheKey,
        generation: u64,
    ) -> Option<Vec<OverviewMarkdownState>> {
        if !self.is_current_description_preparation(key, generation) {
            return None;
        }

        self.content.description_pending_blocks.clear();
        self.content.description_markdown_subscriptions.clear();
        Some(std::mem::take(&mut self.content.description_blocks))
    }

    pub(crate) fn set_description_blocks(
        &mut self,
        key: &PullRequestDetailCacheKey,
        generation: u64,
        blocks: Vec<OverviewMarkdownState>,
        pending_blocks: HashSet<usize>,
        subscriptions: Vec<Subscription>,
    ) -> bool {
        if !self.is_current_description_preparation(key, generation) {
            return false;
        }

        self.content.description_blocks = blocks;
        self.content.description_pending_blocks = pending_blocks;
        self.content.description_markdown_subscriptions = subscriptions;
        if self.content.description_pending_blocks.is_empty() {
            self.content.description_preparing = false;
            self.content.description_ready = true;
            return true;
        }
        false
    }

    pub(crate) fn description_block_count(&self) -> usize {
        self.content.description_blocks.len()
    }

    pub(crate) fn description_block_state(&self, index: usize) -> Option<Entity<TextViewState>> {
        self.content
            .description_blocks
            .get(index)
            .map(|block| block.state.clone())
    }

    pub(crate) fn mark_description_block_ready(
        &mut self,
        key: &PullRequestDetailCacheKey,
        generation: u64,
        index: usize,
    ) -> Option<bool> {
        let content = if self.pull_request_key.as_ref() == Some(key) {
            Some(&mut self.content)
        } else {
            self.cached_content.get_mut(key)
        };
        let content = content?;
        if content.description_generation != generation
            || !content.description_pending_blocks.remove(&index)
        {
            return None;
        }
        let complete = content.description_pending_blocks.is_empty();

        if complete {
            content.description_preparing = false;
            content.description_ready = true;
        }
        Some(complete)
    }

    pub(crate) fn invalidate_description(&mut self, key: &PullRequestDetailCacheKey) {
        let content = if self.pull_request_key.as_ref() == Some(key) {
            Some(&mut self.content)
        } else {
            self.cached_content.get_mut(key)
        };
        let Some(content) = content else {
            return;
        };

        content.description_ready = false;
        content.description_preparing = false;
        content.description_generation = content.description_generation.wrapping_add(1);
        content.description_pending_blocks.clear();
        content.description_markdown_subscriptions.clear();
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
        let generation = state
            .start_description_preparation()
            .expect("description preparation should start");
        assert!(state.set_description_blocks(
            &first,
            generation,
            Vec::new(),
            HashSet::new(),
            Vec::new(),
        ));
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
        let generation = state
            .start_description_preparation()
            .expect("description preparation should start");
        assert!(!state.set_description_blocks(
            &first,
            generation,
            Vec::new(),
            HashSet::from([0]),
            Vec::new(),
        ));
        state.prepare_pull_request(detail_key(2), false);

        assert_eq!(
            state.mark_description_block_ready(&first, generation, 0),
            Some(true)
        );
        state.prepare_pull_request(first, true);
        assert!(state.description_ready());
    }

    #[test]
    fn stale_description_preparation_cannot_replace_invalidated_content() {
        let mut state = OverviewUiState::new(ListState::new(0, ListAlignment::Top, px(100.0)));
        let key = detail_key(1);
        state.prepare_pull_request(key.clone(), false);

        let first_generation = state
            .start_description_preparation()
            .expect("first preparation should start");
        state.invalidate_description(&key);
        let second_generation = state
            .start_description_preparation()
            .expect("replacement preparation should start");

        assert!(!state.set_description_blocks(
            &key,
            first_generation,
            Vec::new(),
            HashSet::new(),
            Vec::new(),
        ));
        assert!(!state.description_ready());
        assert!(state.set_description_blocks(
            &key,
            second_generation,
            Vec::new(),
            HashSet::new(),
            Vec::new(),
        ));
        assert!(state.description_ready());
    }
}
