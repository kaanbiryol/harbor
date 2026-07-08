use harbor_domain::{PullRequest, RepoId};
use harbor_sync::PullRequestInboxPageInfo;

use crate::workspace::PullRequestInboxMode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PullRequestInboxRefreshIntent {
    PreferCache,
    SwitchMode,
    LightRefresh,
    ManualRefresh,
}

impl PullRequestInboxRefreshIntent {
    pub(super) fn uses_cache(self) -> bool {
        matches!(self, Self::PreferCache | Self::SwitchMode)
    }

    pub(super) fn resets_detail_state(self) -> bool {
        self != Self::LightRefresh
    }

    pub(super) fn force_enrichment(self) -> bool {
        self == Self::ManualRefresh
    }

    pub(super) fn prefetches_counts(self) -> bool {
        matches!(self, Self::PreferCache | Self::ManualRefresh)
    }

    pub(super) fn refreshes_counts(self) -> bool {
        matches!(self, Self::PreferCache | Self::ManualRefresh)
    }
}

pub(super) fn pull_request_inbox_loading_status(
    repository: &RepoId,
    mode: PullRequestInboxMode,
) -> String {
    format!(
        "Loading {} from {}",
        mode.status_label(),
        repository.full_name()
    )
}

pub(super) fn pull_request_inbox_loaded_status(
    repository: &RepoId,
    mode: PullRequestInboxMode,
    count: usize,
    page_info: &PullRequestInboxPageInfo,
) -> String {
    match page_info.total_count {
        Some(total_count) if count < total_count => format!(
            "Loaded {count} of {total_count} {} from {}",
            mode.status_label(),
            repository.full_name()
        ),
        _ if page_info.has_next_page() => format!(
            "Loaded first {count} {} from {}",
            mode.status_label(),
            repository.full_name()
        ),
        _ => format!(
            "Loaded {count} {} from {}",
            mode.status_label(),
            repository.full_name()
        ),
    }
}

pub(super) fn pull_request_inbox_loaded_more_status(
    repository: &RepoId,
    mode: PullRequestInboxMode,
    appended_count: usize,
    loaded_count: usize,
    page_info: &PullRequestInboxPageInfo,
) -> String {
    match page_info.total_count {
        Some(total_count) => format!(
            "Loaded {appended_count} more {}; showing {loaded_count} of {total_count} from {}",
            mode.status_label(),
            repository.full_name()
        ),
        None => format!(
            "Loaded {appended_count} more {}; showing {loaded_count} from {}",
            mode.status_label(),
            repository.full_name()
        ),
    }
}

pub(super) fn pull_request_inbox_failed_status(
    repository: &RepoId,
    mode: PullRequestInboxMode,
) -> String {
    format!(
        "Failed to load {} from {}",
        mode.status_label(),
        repository.full_name()
    )
}

pub(super) fn append_pull_request_page(
    pull_requests: &mut Vec<PullRequest>,
    page_pull_requests: Vec<PullRequest>,
) -> usize {
    let mut appended_count = 0;

    for pull_request in page_pull_requests {
        if let Some(existing) = pull_requests.iter_mut().find(|existing| {
            existing.repo == pull_request.repo && existing.number == pull_request.number
        }) {
            *existing = pull_request;
        } else {
            pull_requests.push(pull_request);
            appended_count += 1;
        }
    }

    appended_count
}

#[cfg(test)]
mod tests {
    use harbor_github::PullRequestPageCursor;

    use super::*;
    use crate::test_fixtures::pull_request;

    #[test]
    fn append_pull_request_page_replaces_existing_rows_and_counts_new_rows() {
        let mut existing_pull_request = pull_request();
        existing_pull_request.title = "Old title".to_string();
        let mut updated_pull_request = existing_pull_request.clone();
        updated_pull_request.title = "Updated title".to_string();
        let mut new_pull_request = pull_request();
        new_pull_request.number = 8;
        new_pull_request.node_id = "pr-node-8".to_string();

        let mut pull_requests = vec![existing_pull_request];
        let appended_count = append_pull_request_page(
            &mut pull_requests,
            vec![updated_pull_request, new_pull_request],
        );

        assert_eq!(appended_count, 1);
        assert_eq!(
            pull_requests
                .iter()
                .map(|pull_request| (pull_request.number, pull_request.title.as_str()))
                .collect::<Vec<_>>(),
            vec![(7, "Updated title"), (8, "Add feature")]
        );
    }

    #[test]
    fn formats_pull_request_inbox_refresh_statuses() {
        let repository = RepoId::new("acme", "app");
        let mode = PullRequestInboxMode::Open;
        let partial_page = PullRequestInboxPageInfo {
            total_count: Some(12),
            next_cursor: Some(PullRequestPageCursor::RestPage(2)),
        };
        let paged_without_total = PullRequestInboxPageInfo {
            total_count: None,
            next_cursor: Some(PullRequestPageCursor::RestPage(2)),
        };
        let complete_page = PullRequestInboxPageInfo::default();

        assert_eq!(
            pull_request_inbox_loading_status(&repository, mode),
            "Loading open pull requests from acme/app"
        );
        assert_eq!(
            pull_request_inbox_loaded_status(&repository, mode, 10, &partial_page),
            "Loaded 10 of 12 open pull requests from acme/app"
        );
        assert_eq!(
            pull_request_inbox_loaded_status(&repository, mode, 10, &paged_without_total),
            "Loaded first 10 open pull requests from acme/app"
        );
        assert_eq!(
            pull_request_inbox_loaded_status(&repository, mode, 10, &complete_page),
            "Loaded 10 open pull requests from acme/app"
        );
        assert_eq!(
            pull_request_inbox_loaded_more_status(&repository, mode, 2, 12, &partial_page),
            "Loaded 2 more open pull requests; showing 12 of 12 from acme/app"
        );
        assert_eq!(
            pull_request_inbox_failed_status(&repository, mode),
            "Failed to load open pull requests from acme/app"
        );
    }
}
