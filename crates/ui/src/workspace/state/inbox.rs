use std::collections::HashMap;

use harbor_github::PullRequestPageCursor;
use harbor_sync::PullRequestInboxPageInfo;

use crate::workspace::{
    PullRequestInboxCacheKey, PullRequestInboxMode, PullRequestInboxSnapshot, status::LoadStatus,
};

#[derive(Default)]
pub(crate) struct PullRequestInboxState {
    visible: bool,
    mode: PullRequestInboxMode,
    cache: HashMap<PullRequestInboxCacheKey, PullRequestInboxSnapshot>,
    counts: HashMap<PullRequestInboxCacheKey, usize>,
    page_info: PullRequestInboxPageInfo,
    load: LoadStatus,
    more_load: LoadStatus,
}

impl PullRequestInboxState {
    pub(crate) fn visible_by_default() -> Self {
        Self {
            visible: true,
            ..Self::default()
        }
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.visible
    }

    pub(crate) fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub(crate) fn toggle_visible(&mut self) {
        self.visible = !self.visible;
    }

    pub(crate) fn mode(&self) -> PullRequestInboxMode {
        self.mode
    }

    pub(crate) fn set_mode(&mut self, mode: PullRequestInboxMode) {
        self.mode = mode;
    }

    pub(crate) fn start_loading(&mut self) {
        self.load.start();
        self.more_load.reset();
    }

    pub(crate) fn apply_success(&mut self) {
        self.load.succeed();
    }

    pub(crate) fn apply_failure(&mut self, error: impl Into<String>) {
        self.load.fail(error);
    }

    pub(crate) fn reset_load(&mut self) {
        self.load.reset();
        self.more_load.reset();
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.load.is_loading()
    }

    pub(crate) fn load_error(&self) -> Option<&str> {
        self.load.error()
    }

    pub(crate) fn can_cache_snapshot(&self) -> bool {
        !self.is_loading()
            && self.load_error().is_none()
            && !self.is_loading_more()
            && self.load_more_error().is_none()
    }

    pub(crate) fn page_info(&self) -> &PullRequestInboxPageInfo {
        &self.page_info
    }

    pub(crate) fn set_page_info(&mut self, page_info: PullRequestInboxPageInfo) {
        self.page_info = page_info;
    }

    pub(crate) fn clear_page_info(&mut self) {
        self.page_info = PullRequestInboxPageInfo::default();
    }

    pub(crate) fn total_count(&self) -> Option<usize> {
        self.page_info.total_count
    }

    pub(crate) fn has_next_page(&self) -> bool {
        self.page_info.has_next_page()
    }

    pub(crate) fn next_page_cursor(&self) -> Option<PullRequestPageCursor> {
        self.page_info.next_cursor.clone()
    }

    pub(crate) fn start_loading_more(&mut self) {
        self.more_load.start();
    }

    pub(crate) fn apply_load_more_success(&mut self) {
        self.more_load.succeed();
    }

    pub(crate) fn apply_load_more_failure(&mut self, error: impl Into<String>) {
        self.more_load.fail(error);
    }

    pub(crate) fn is_loading_more(&self) -> bool {
        self.more_load.is_loading()
    }

    pub(crate) fn load_more_error(&self) -> Option<&str> {
        self.more_load.error()
    }

    pub(crate) fn insert_snapshot(
        &mut self,
        key: PullRequestInboxCacheKey,
        snapshot: PullRequestInboxSnapshot,
    ) {
        if let Some(count) = snapshot.count() {
            self.counts.insert(key.clone(), count);
        }
        self.cache.insert(key, snapshot);
    }

    pub(crate) fn insert_count(&mut self, key: PullRequestInboxCacheKey, count: usize) {
        self.counts.insert(key, count);
    }

    pub(crate) fn stored_count(&self, key: &PullRequestInboxCacheKey) -> Option<usize> {
        self.counts.get(key).copied()
    }

    pub(crate) fn snapshot(
        &self,
        key: &PullRequestInboxCacheKey,
    ) -> Option<&PullRequestInboxSnapshot> {
        self.cache.get(key)
    }

    pub(crate) fn snapshot_count(&self, key: &PullRequestInboxCacheKey) -> Option<usize> {
        self.counts.get(key).copied().or_else(|| {
            self.cache
                .get(key)
                .and_then(PullRequestInboxSnapshot::count)
        })
    }
}
