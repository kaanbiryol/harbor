use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestComment, PullRequestReview, RepoId, ReviewThread,
    WorkflowRun,
};
use harbor_github::{PullRequestCiSource, PullRequestContentSource, Result};
use harbor_storage::{PullRequestDetailSection, SqliteStore, StorageError, detail_target_key};

#[derive(Clone, Debug, Default)]
pub struct CachedPullRequestDetail {
    pub metadata: Option<PullRequest>,
    pub files: Option<Vec<DiffFile>>,
    pub check_runs: Option<Vec<CheckRun>>,
    pub workflow_runs: Option<Vec<WorkflowRun>>,
    pub review_data: Option<(
        Vec<PullRequestReview>,
        Vec<PullRequestComment>,
        Vec<ReviewThread>,
    )>,
}

pub async fn load_cached_pull_request_detail(
    store: &SqliteStore,
    repository: &RepoId,
    number: u64,
    head_sha: &str,
) -> harbor_storage::Result<CachedPullRequestDetail> {
    Ok(CachedPullRequestDetail {
        metadata: store
            .load_pull_request_metadata(repository, number, head_sha)
            .await?,
        files: store
            .load_pull_request_files(repository, number, head_sha)
            .await?,
        check_runs: store
            .load_pull_request_check_runs(repository, number, head_sha)
            .await?,
        workflow_runs: store
            .load_pull_request_workflow_runs(repository, number, head_sha)
            .await?,
        review_data: store
            .load_pull_request_reviews(repository, number, head_sha)
            .await?,
    })
}

pub struct PullRequestDetailRefresh<T> {
    pub result: Result<T>,
    pub cache_error: Option<StorageError>,
}

pub async fn refresh_pull_request_metadata<S>(
    source: &S,
    store: Option<&SqliteStore>,
    repository: &RepoId,
    number: u64,
) -> PullRequestDetailRefresh<PullRequest>
where
    S: PullRequestContentSource + ?Sized,
{
    let result = source
        .get_pull_request(&repository.owner, &repository.name, number)
        .await;
    let cache_error = match (store, result.as_ref()) {
        (Some(store), Ok(pull_request)) => {
            store.save_pull_request_metadata(pull_request).await.err()
        }
        (Some(store), Err(error)) => store
            .record_sync_failure(
                &detail_target_key(repository, number, PullRequestDetailSection::Metadata),
                &error.to_string(),
            )
            .await
            .err(),
        (None, _) => None,
    };

    PullRequestDetailRefresh {
        result,
        cache_error,
    }
}

pub async fn refresh_pull_request_files<S>(
    source: &S,
    store: Option<&SqliteStore>,
    repository: &RepoId,
    number: u64,
    head_sha: &str,
) -> PullRequestDetailRefresh<Vec<DiffFile>>
where
    S: PullRequestContentSource + ?Sized,
{
    let result = source
        .list_pull_request_files(&repository.owner, &repository.name, number)
        .await;
    let cache_error = match (store, result.as_ref()) {
        (Some(store), Ok(files)) => store
            .save_pull_request_files(repository, number, head_sha, files)
            .await
            .err(),
        (Some(store), Err(error)) => store
            .record_sync_failure(
                &detail_target_key(repository, number, PullRequestDetailSection::Files),
                &error.to_string(),
            )
            .await
            .err(),
        (None, _) => None,
    };

    PullRequestDetailRefresh {
        result,
        cache_error,
    }
}

pub async fn refresh_pull_request_check_runs<S>(
    source: &S,
    store: Option<&SqliteStore>,
    repository: &RepoId,
    number: u64,
    head_sha: &str,
) -> PullRequestDetailRefresh<Vec<CheckRun>>
where
    S: PullRequestCiSource + ?Sized,
{
    let result = if head_sha.is_empty() {
        Ok(Vec::new())
    } else {
        source
            .list_check_runs(&repository.owner, &repository.name, head_sha)
            .await
    };
    let cache_error = match (store, result.as_ref()) {
        (Some(store), Ok(check_runs)) => store
            .save_pull_request_check_runs(repository, number, head_sha, check_runs)
            .await
            .err(),
        (Some(store), Err(error)) => store
            .record_sync_failure(
                &detail_target_key(repository, number, PullRequestDetailSection::CheckRuns),
                &error.to_string(),
            )
            .await
            .err(),
        (None, _) => None,
    };

    PullRequestDetailRefresh {
        result,
        cache_error,
    }
}

pub async fn refresh_pull_request_workflow_runs<S>(
    source: &S,
    store: Option<&SqliteStore>,
    repository: &RepoId,
    number: u64,
    head_sha: &str,
) -> PullRequestDetailRefresh<Vec<WorkflowRun>>
where
    S: PullRequestCiSource + ?Sized,
{
    let result = if head_sha.is_empty() {
        Ok(Vec::new())
    } else {
        source
            .list_workflow_runs_for_head(&repository.owner, &repository.name, head_sha)
            .await
    };
    let cache_error = match (store, result.as_ref()) {
        (Some(store), Ok(workflow_runs)) => store
            .save_pull_request_workflow_runs(repository, number, head_sha, workflow_runs)
            .await
            .err(),
        (Some(store), Err(error)) => store
            .record_sync_failure(
                &detail_target_key(repository, number, PullRequestDetailSection::WorkflowRuns),
                &error.to_string(),
            )
            .await
            .err(),
        (None, _) => None,
    };

    PullRequestDetailRefresh {
        result,
        cache_error,
    }
}
