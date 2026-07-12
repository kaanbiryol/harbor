use harbor_domain::{PullRequest, RepoId};
use harbor_github::{ConditionalFetch, GitHubError, PullRequestPageCursor};
use harbor_storage::SqliteStore;

use super::{
    PULL_REQUEST_INBOX_PAGE_SIZE, PullRequestInboxMode, PullRequestInboxPageInfo,
    PullRequestInboxRefresh, PullRequestInboxSource,
    enrichment::{
        apply_pull_request_enrichments, merge_light_pull_request_rows,
        pull_request_enrichment_node_ids,
    },
    http_cache::{
        github_validator_from_storage, http_validator_key, storage_validator_from_github,
    },
};

pub(super) async fn refresh_light_pull_request_inbox<S>(
    source: &S,
    store: Option<&SqliteStore>,
    repository: &RepoId,
    mode: PullRequestInboxMode,
    page_cursor: Option<PullRequestPageCursor>,
    previous_pull_requests: &[PullRequest],
    force_enrichment: bool,
) -> std::result::Result<PullRequestInboxRefresh, GitHubError>
where
    S: PullRequestInboxSource + ?Sized,
{
    let is_first_page = page_cursor.is_none();
    let validator_key = http_validator_key(repository, mode);
    let validator = if is_first_page && !previous_pull_requests.is_empty() {
        match store {
            Some(store) => store
                .load_http_cache_validator(&validator_key)
                .await
                .map_err(|error| GitHubError::Transport(error.to_string()))?
                .map(github_validator_from_storage),
            None => None,
        }
    } else {
        None
    };

    let fetch = source
        .list_repository_pull_requests_light_page(
            repository,
            mode.list_filter(),
            page_cursor,
            PULL_REQUEST_INBOX_PAGE_SIZE,
            validator,
        )
        .await?;

    let (page, validator) = match fetch {
        ConditionalFetch::NotModified { validator } => {
            if let (Some(store), Some(validator)) = (store, validator) {
                store
                    .save_http_cache_validator(
                        &validator_key,
                        &storage_validator_from_github(validator),
                    )
                    .await
                    .map_err(|error| GitHubError::Transport(error.to_string()))?;
            }
            return Ok(PullRequestInboxRefresh::NotModified);
        }
        ConditionalFetch::Modified { value, validator } => (value, validator),
    };

    if is_first_page && let (Some(store), Some(validator)) = (store, validator) {
        store
            .save_http_cache_validator(&validator_key, &storage_validator_from_github(validator))
            .await
            .map_err(|error| GitHubError::Transport(error.to_string()))?;
    }

    let page_info = PullRequestInboxPageInfo::from_page(&page);
    let mut pull_requests = page.pull_requests;

    merge_light_pull_request_rows(previous_pull_requests, &mut pull_requests);

    let node_ids = pull_request_enrichment_node_ids(&pull_requests, force_enrichment);
    let enrichment_error = if node_ids.is_empty() {
        None
    } else {
        tracing::info!(
            repository = %repository.full_name(),
            mode = mode.key(),
            pull_request_count = node_ids.len(),
            forced = force_enrichment,
            "github graphql source: pull request row enrichment"
        );
        match source.enrich_pull_requests_by_node_ids(&node_ids).await {
            Ok(enrichments) => {
                apply_pull_request_enrichments(&mut pull_requests, enrichments);
                None
            }
            Err(error) => Some(error.to_string()),
        }
    };

    Ok(PullRequestInboxRefresh::Modified {
        pull_requests,
        page_info,
        enrichment_error,
    })
}
