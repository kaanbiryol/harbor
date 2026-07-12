use async_trait::async_trait;
use harbor_domain::{CheckRun, DiffFile, PullRequest, RepoId, WorkflowRun};
use harbor_github::Result;
use harbor_storage::{PullRequestDetailSection, SqliteStore, detail_target_key};

#[async_trait]
pub trait PullRequestCiSource: Send + Sync {
    async fn list_check_runs(
        &self,
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> Result<Vec<CheckRun>>;

    async fn list_workflow_runs_for_head(
        &self,
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> Result<Vec<WorkflowRun>>;
}

#[async_trait]
pub trait PullRequestContentSource: Send + Sync {
    async fn get_pull_request(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest>;

    async fn list_pull_request_files(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<DiffFile>>;
}

pub struct PullRequestDetailRefresh<T> {
    pub result: Result<T>,
    pub cache_error: Option<String>,
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
        (Some(store), Ok(pull_request)) => store
            .save_pull_request_metadata(pull_request)
            .await
            .err()
            .map(|error| error.to_string()),
        (Some(store), Err(error)) => store
            .record_sync_failure(
                &detail_target_key(repository, number, PullRequestDetailSection::Metadata),
                &error.to_string(),
            )
            .await
            .err()
            .map(|error| error.to_string()),
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
            .err()
            .map(|error| error.to_string()),
        (Some(store), Err(error)) => store
            .record_sync_failure(
                &detail_target_key(repository, number, PullRequestDetailSection::Files),
                &error.to_string(),
            )
            .await
            .err()
            .map(|error| error.to_string()),
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
            .err()
            .map(|error| error.to_string()),
        (Some(store), Err(error)) => store
            .record_sync_failure(
                &detail_target_key(repository, number, PullRequestDetailSection::CheckRuns),
                &error.to_string(),
            )
            .await
            .err()
            .map(|error| error.to_string()),
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
            .err()
            .map(|error| error.to_string()),
        (Some(store), Err(error)) => store
            .record_sync_failure(
                &detail_target_key(repository, number, PullRequestDetailSection::WorkflowRuns),
                &error.to_string(),
            )
            .await
            .err()
            .map(|error| error.to_string()),
        (None, _) => None,
    };

    PullRequestDetailRefresh {
        result,
        cache_error,
    }
}
