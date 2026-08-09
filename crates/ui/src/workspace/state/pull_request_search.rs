use harbor_domain::PullRequest;
use harbor_github::{PullRequestPage, PullRequestPageCursor};

use crate::workspace::status::LoadStatus;

#[derive(Default)]
pub(crate) struct PullRequestSearchState {
    query: String,
    request_id: u64,
    results: Vec<PullRequest>,
    total_count: Option<usize>,
    next_cursor: Option<PullRequestPageCursor>,
    load: LoadStatus,
    more_load: LoadStatus,
}

impl PullRequestSearchState {
    pub(crate) fn start(&mut self, query: String) -> u64 {
        self.request_id = self.request_id.wrapping_add(1);
        self.query = query;
        self.results.clear();
        self.total_count = None;
        self.next_cursor = None;
        self.load.start();
        self.more_load.reset();
        self.request_id
    }

    pub(crate) fn clear(&mut self) {
        self.request_id = self.request_id.wrapping_add(1);
        self.query.clear();
        self.results.clear();
        self.total_count = None;
        self.next_cursor = None;
        self.load.reset();
        self.more_load.reset();
    }

    pub(crate) fn matches(&self, request_id: u64, query: &str) -> bool {
        self.request_id == request_id && self.query == query
    }

    pub(crate) fn apply_success(&mut self, page: PullRequestPage) {
        self.results = page.pull_requests;
        self.total_count = page.total_count;
        self.next_cursor = page.next_cursor;
        self.load.succeed();
    }

    pub(crate) fn apply_failure(&mut self, error: impl Into<String>) {
        self.load.fail(error);
    }

    pub(crate) fn start_loading_more(&mut self) {
        self.more_load.start();
    }

    pub(crate) fn apply_load_more_success(&mut self, page: PullRequestPage) {
        for pull_request in page.pull_requests {
            if let Some(existing) = self.results.iter_mut().find(|existing| {
                existing.repo == pull_request.repo && existing.number == pull_request.number
            }) {
                *existing = pull_request;
            } else {
                self.results.push(pull_request);
            }
        }
        self.total_count = page.total_count.or(self.total_count);
        self.next_cursor = page.next_cursor;
        self.more_load.succeed();
    }

    pub(crate) fn apply_load_more_failure(&mut self, error: impl Into<String>) {
        self.more_load.fail(error);
    }

    pub(crate) fn query(&self) -> &str {
        &self.query
    }

    pub(crate) fn request_id(&self) -> u64 {
        self.request_id
    }

    pub(crate) fn results(&self) -> &[PullRequest] {
        &self.results
    }

    pub(crate) fn total_count(&self) -> Option<usize> {
        self.total_count
    }

    pub(crate) fn next_cursor(&self) -> Option<PullRequestPageCursor> {
        self.next_cursor.clone()
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.load.is_loading()
    }

    pub(crate) fn is_loaded(&self) -> bool {
        self.load.is_loaded()
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.load.error()
    }

    pub(crate) fn is_loading_more(&self) -> bool {
        self.more_load.is_loading()
    }

    pub(crate) fn load_more_error(&self) -> Option<&str> {
        self.more_load.error()
    }
}

#[cfg(test)]
mod tests {
    use harbor_domain::RepoId;

    use super::*;
    use crate::test_fixtures::pull_request;

    #[test]
    fn replaces_initial_results_and_deduplicates_following_pages() {
        let mut state = PullRequestSearchState::default();
        let request_id = state.start("feature".to_string());
        assert!(state.matches(request_id, "feature"));

        let first = pull_request();
        state.apply_success(PullRequestPage {
            pull_requests: vec![first.clone()],
            total_count: Some(2),
            next_cursor: Some(PullRequestPageCursor::GraphQl("next".to_string())),
        });

        let mut updated = first;
        updated.title = "Updated feature".to_string();
        let mut second = pull_request();
        second.repo = RepoId::new("acme", "app");
        second.number = 8;
        state.start_loading_more();
        state.apply_load_more_success(PullRequestPage {
            pull_requests: vec![updated, second],
            total_count: Some(2),
            next_cursor: None,
        });

        assert_eq!(state.results().len(), 2);
        assert_eq!(state.results()[0].title, "Updated feature");
        assert_eq!(state.total_count(), Some(2));
        assert!(state.next_cursor().is_none());
    }
}
