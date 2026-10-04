use std::sync::{Arc, Mutex};

use crate::{
    ConditionalFetch, GhCliTransport, GitHubAuthApi, GitHubClient, GitHubError,
    GitHubPullRequestApi, GitHubPullRequestMutationApi, GitHubRateLimitStatus, GitHubRepositoryApi,
    GitHubReviewApi, GitHubReviewMutationApi, GitHubTransportSource, GitHubWorkflowApi,
    GitHubWorkflowMutationApi, HttpCacheValidator, OctocrabTransport, PullRequestCiSource,
    PullRequestContentSource, PullRequestEnrichment, PullRequestInboxSource, PullRequestListFilter,
    PullRequestPage, PullRequestPageCursor, RepositoryList, Result, SubmitPullRequestReviewEvent,
};
use async_trait::async_trait;
use harbor_domain::{
    CheckRun, DiffFile, MergeMethod, PullRequest, PullRequestComment, PullRequestCommit,
    PullRequestMetadataOptions, PullRequestReview, ReactionContent, RepoId, ReviewCommentRange,
    ReviewThread, Workflow, WorkflowJob, WorkflowRun,
};
#[derive(Clone, Debug, Default)]
pub struct GitHubSession {
    state: Arc<Mutex<SessionState>>,
}

#[derive(Clone, Debug, Default)]
struct SessionState {
    client: Option<GitHubClient<GitHubTransportSource>>,
    current_user_login: Option<String>,
    generation: u64,
}

impl GitHubSession {
    pub(super) fn client(&self) -> Result<GitHubClient<GitHubTransportSource>> {
        self.state
            .lock()
            .map_err(|error| GitHubError::Transport(error.to_string()))?
            .client
            .clone()
            .ok_or(GitHubError::Unauthenticated)
    }

    fn replace_client(&self, client: Option<GitHubClient<GitHubTransportSource>>) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|error| GitHubError::Transport(error.to_string()))?;
        state.generation = state.generation.wrapping_add(1);
        state.client = client;
        state.current_user_login = None;
        Ok(())
    }

    fn cache_current_user_login(&self, generation: u64, login: String) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|error| GitHubError::Transport(error.to_string()))?;
        if state.generation != generation {
            return Err(GitHubError::SessionChanged);
        }
        state.current_user_login = Some(login);
        Ok(())
    }
}

#[async_trait]
impl PullRequestInboxSource for GitHubSession {
    fn latest_rate_limits(&self) -> Vec<GitHubRateLimitStatus> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.client.as_ref().map(GitHubClient::latest_rate_limits))
            .unwrap_or_default()
    }

    async fn list_repository_pull_request_page(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
        cursor: Option<PullRequestPageCursor>,
        page_size: usize,
    ) -> Result<PullRequestPage> {
        self.client()?
            .list_repository_pull_request_page(repository, filter, cursor, page_size)
            .await
    }

    async fn search_repository_pull_request_page(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
        query: &str,
        cursor: Option<PullRequestPageCursor>,
        page_size: usize,
    ) -> Result<PullRequestPage> {
        self.client()?
            .search_repository_pull_request_page(repository, filter, query, cursor, page_size)
            .await
    }

    async fn count_repository_pull_requests(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
    ) -> Result<usize> {
        self.client()?
            .count_repository_pull_requests(repository, filter)
            .await
    }

    async fn list_repository_pull_requests_light_page(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
        cursor: Option<PullRequestPageCursor>,
        page_size: usize,
        validator: Option<HttpCacheValidator>,
    ) -> Result<ConditionalFetch<PullRequestPage>> {
        self.client()?
            .list_repository_pull_requests_light_page(
                repository,
                filter,
                cursor,
                page_size,
                validator.as_ref(),
            )
            .await
    }

    async fn enrich_pull_requests_by_node_ids(
        &self,
        node_ids: &[String],
    ) -> Result<Vec<PullRequestEnrichment>> {
        self.client()?
            .enrich_pull_requests_by_node_ids(node_ids)
            .await
    }
}

#[async_trait]
impl PullRequestCiSource for GitHubSession {
    async fn list_check_runs(
        &self,
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> Result<Vec<CheckRun>> {
        self.client()?.list_check_runs(owner, repo, head_sha).await
    }

    async fn list_workflow_runs_for_head(
        &self,
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> Result<Vec<WorkflowRun>> {
        self.client()?
            .list_workflow_runs_for_head(owner, repo, head_sha)
            .await
    }
}

#[async_trait]
impl PullRequestContentSource for GitHubSession {
    async fn get_pull_request(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest> {
        self.client()?.get_pull_request(owner, repo, number).await
    }

    async fn list_pull_request_files(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<DiffFile>> {
        self.client()?
            .list_pull_request_files(owner, repo, number)
            .await
    }

    async fn list_commit_files(&self, owner: &str, repo: &str, sha: &str) -> Result<Vec<DiffFile>> {
        self.client()?.list_commit_files(owner, repo, sha).await
    }
}

#[async_trait]
impl GitHubAuthApi for GitHubSession {
    fn snapshot(&self) -> Option<Arc<dyn crate::GitHubApi>> {
        let state = match self.state.lock() {
            Ok(state) => state.clone(),
            Err(error) => {
                tracing::error!(%error, "failed to snapshot github session");
                SessionState::default()
            }
        };
        Some(Arc::new(Self {
            state: Arc::new(Mutex::new(state)),
        }))
    }

    fn latest_rate_limit(&self) -> Option<GitHubRateLimitStatus> {
        self.state.lock().ok().and_then(|state| {
            state
                .client
                .as_ref()
                .and_then(GitHubClient::latest_rate_limit)
        })
    }

    fn configure_token(&self, token: String) -> Result<()> {
        let transport = OctocrabTransport::with_token(token)?;
        self.replace_client(Some(GitHubClient::new(GitHubTransportSource::Token(
            transport,
        ))))
    }

    fn configure_gh_cli(&self) -> Result<()> {
        self.replace_client(Some(GitHubClient::new(GitHubTransportSource::GhCli(
            GhCliTransport::default(),
        ))))
    }

    fn clear_auth(&self) -> Result<()> {
        self.replace_client(None)
    }

    fn has_auth(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.client.is_some())
            .unwrap_or(false)
    }
}

#[async_trait]
impl GitHubRepositoryApi for GitHubSession {
    async fn list_repositories(&self) -> Result<RepositoryList> {
        self.client()?.list_repositories().await
    }

    async fn get_repository(&self, repository: &RepoId) -> Result<RepoId> {
        self.client()?.get_repository(repository).await
    }

    async fn list_pull_request_metadata_options(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<PullRequestMetadataOptions> {
        self.client()?
            .list_pull_request_metadata_options(owner, repo)
            .await
    }

    async fn current_user(&self) -> Result<String> {
        let (client, generation) = {
            let state = self
                .state
                .lock()
                .map_err(|error| GitHubError::Transport(error.to_string()))?;
            if let Some(login) = &state.current_user_login {
                return Ok(login.clone());
            }
            (
                state.client.clone().ok_or(GitHubError::Unauthenticated)?,
                state.generation,
            )
        };
        let login = client.current_user().await?;
        self.cache_current_user_login(generation, login.clone())?;
        Ok(login)
    }
}

#[async_trait]
impl GitHubPullRequestApi for GitHubSession {
    async fn list_pull_request_commits(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PullRequestCommit>> {
        self.client()?
            .list_pull_request_commits(owner, repo, number)
            .await
    }

    async fn mark_pull_request_file_viewed(
        &self,
        pull_request_node_id: &str,
        path: &str,
    ) -> Result<()> {
        self.client()?
            .mark_pull_request_file_viewed(pull_request_node_id, path)
            .await
    }

    async fn unmark_pull_request_file_viewed(
        &self,
        pull_request_node_id: &str,
        path: &str,
    ) -> Result<()> {
        self.client()?
            .unmark_pull_request_file_viewed(pull_request_node_id, path)
            .await
    }
}

#[async_trait]
impl GitHubWorkflowApi for GitHubSession {
    async fn list_workflows(&self, owner: &str, repo: &str) -> Result<Vec<Workflow>> {
        self.client()?.list_workflows(owner, repo).await
    }

    async fn list_repository_workflow_run_page(
        &self,
        owner: &str,
        repo: &str,
        page: usize,
    ) -> Result<crate::WorkflowRunPage> {
        self.client()?
            .list_repository_workflow_run_page(owner, repo, page)
            .await
    }

    async fn list_workflow_run_page_for_workflow(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: u64,
        page: usize,
    ) -> Result<crate::WorkflowRunPage> {
        self.client()?
            .list_workflow_run_page_for_workflow(owner, repo, workflow_id, page)
            .await
    }

    async fn list_workflow_jobs_for_run(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
        attempt: Option<u64>,
    ) -> Result<Vec<WorkflowJob>> {
        self.client()?
            .list_workflow_jobs_for_run(owner, repo, run_id, attempt)
            .await
    }

    async fn workflow_run_log(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
        attempt: Option<u64>,
    ) -> Result<String> {
        self.client()?
            .workflow_run_log(owner, repo, run_id, attempt)
            .await
    }
}

#[async_trait]
impl GitHubWorkflowMutationApi for GitHubSession {
    async fn dispatch_workflow(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: u64,
        git_ref: &str,
    ) -> Result<()> {
        self.client()?
            .dispatch_workflow(owner, repo, workflow_id, git_ref)
            .await
    }

    async fn rerun_failed_jobs(&self, owner: &str, repo: &str, run_id: u64) -> Result<()> {
        self.client()?.rerun_failed_jobs(owner, repo, run_id).await
    }
}

#[async_trait]
impl GitHubReviewApi for GitHubSession {
    async fn list_pull_request_reviews(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PullRequestReview>> {
        self.client()?
            .list_pull_request_reviews(owner, repo, number)
            .await
    }

    async fn list_pull_request_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PullRequestComment>> {
        self.client()?
            .list_pull_request_comments(owner, repo, number)
            .await
    }

    async fn pull_request_review_comment_count(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        review_id: &str,
    ) -> Result<usize> {
        self.client()?
            .pull_request_review_comment_count(owner, repo, number, review_id)
            .await
    }

    async fn list_review_threads(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<ReviewThread>> {
        self.client()?
            .list_review_threads(owner, repo, number)
            .await
    }
}

#[async_trait]
impl GitHubReviewMutationApi for GitHubSession {
    async fn submit_pull_request_review(
        &self,
        pull_request_review_node_id: &str,
        event: SubmitPullRequestReviewEvent,
        body: Option<&str>,
    ) -> Result<()> {
        self.client()?
            .submit_pull_request_review(pull_request_review_node_id, event, body)
            .await
    }

    async fn create_pull_request_review_comment(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        commit_id: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<()> {
        self.client()?
            .create_pull_request_review_comment(owner, repo, number, commit_id, range, body)
            .await
    }

    async fn start_pull_request_review(
        &self,
        pull_request_node_id: &str,
        commit_id: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<String> {
        self.client()?
            .start_pull_request_review(pull_request_node_id, commit_id, range, body)
            .await
    }

    async fn add_pending_review_thread(
        &self,
        pull_request_review_node_id: &str,
        range: &ReviewCommentRange,
        body: &str,
    ) -> Result<()> {
        self.client()?
            .add_pending_review_thread(pull_request_review_node_id, range, body)
            .await
    }

    async fn add_review_thread_reply(
        &self,
        thread_id: &str,
        pull_request_review_node_id: Option<&str>,
        body: &str,
    ) -> Result<()> {
        self.client()?
            .add_review_thread_reply(thread_id, pull_request_review_node_id, body)
            .await
    }

    async fn resolve_review_thread(&self, thread_id: &str) -> Result<()> {
        self.client()?.resolve_review_thread(thread_id).await
    }

    async fn unresolve_review_thread(&self, thread_id: &str) -> Result<()> {
        self.client()?.unresolve_review_thread(thread_id).await
    }

    async fn update_review_comment(&self, comment_id: &str, body: &str) -> Result<()> {
        self.client()?.update_review_comment(comment_id, body).await
    }

    async fn delete_review_comment(&self, comment_id: &str) -> Result<()> {
        self.client()?.delete_review_comment(comment_id).await
    }

    async fn add_review_comment_reaction(
        &self,
        comment_id: &str,
        content: ReactionContent,
    ) -> Result<()> {
        self.client()?
            .add_review_comment_reaction(comment_id, content)
            .await
    }

    async fn remove_review_comment_reaction(
        &self,
        comment_id: &str,
        content: ReactionContent,
    ) -> Result<()> {
        self.client()?
            .remove_review_comment_reaction(comment_id, content)
            .await
    }
}

#[async_trait]
impl GitHubPullRequestMutationApi for GitHubSession {
    async fn update_pull_request_body(&self, pull_request_node_id: &str, body: &str) -> Result<()> {
        self.client()?
            .update_pull_request_body(pull_request_node_id, body)
            .await
    }

    async fn request_pull_request_reviewer(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        reviewer: &str,
    ) -> Result<()> {
        self.client()?
            .request_pull_request_reviewer(owner, repo, number, reviewer)
            .await
    }

    async fn add_pull_request_assignee(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        assignee: &str,
    ) -> Result<()> {
        self.client()?
            .add_pull_request_assignee(owner, repo, number, assignee)
            .await
    }

    async fn add_pull_request_label(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        label: &str,
    ) -> Result<()> {
        self.client()?
            .add_pull_request_label(owner, repo, number, label)
            .await
    }

    async fn create_pull_request_comment(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &str,
    ) -> Result<()> {
        self.client()?
            .create_pull_request_comment(owner, repo, number, body)
            .await
    }

    async fn approve_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: Option<&str>,
    ) -> Result<()> {
        self.client()?
            .approve_pull_request(owner, repo, number, body)
            .await
    }

    async fn request_pull_request_changes(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &str,
    ) -> Result<()> {
        self.client()?
            .request_pull_request_changes(owner, repo, number, body)
            .await
    }

    async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        head_sha: &str,
        method: MergeMethod,
    ) -> Result<()> {
        self.client()?
            .merge_pull_request(owner, repo, number, head_sha, method)
            .await
    }

    async fn merge_pull_request_when_ready(
        &self,
        pull_request_node_id: &str,
        head_sha: &str,
    ) -> Result<()> {
        self.client()?
            .merge_pull_request_when_ready(pull_request_node_id, head_sha)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_configuration_clears_cached_user_state() {
        let session = GitHubSession::default();
        session
            .cache_current_user_login(0, "octocat".to_string())
            .expect("cache current user");

        session.configure_gh_cli().expect("configure github cli");

        assert!(session.has_auth());
        assert_eq!(
            session
                .state
                .lock()
                .expect("session state")
                .current_user_login,
            None
        );

        session.clear_auth().expect("clear github auth");
        assert!(!session.has_auth());
    }
    #[test]
    fn delayed_current_user_cannot_repopulate_a_replaced_session() {
        let session = GitHubSession::default();
        session.configure_gh_cli().expect("configure old session");
        let old_generation = session.state.lock().expect("session state").generation;
        session.configure_gh_cli().expect("replace session");
        assert!(matches!(
            session.cache_current_user_login(old_generation, "old-account".into()),
            Err(GitHubError::SessionChanged)
        ));
        assert!(
            session
                .state
                .lock()
                .expect("session state")
                .current_user_login
                .is_none()
        );
    }
    #[test]
    fn request_snapshot_keeps_its_credentials_after_sign_out() {
        let session = GitHubSession::default();
        session.configure_gh_cli().unwrap();
        let request = session.snapshot().unwrap();
        session.clear_auth().unwrap();
        assert!(!session.has_auth());
        assert!(request.has_auth());
        assert!(!session.snapshot().unwrap().has_auth());
    }
}
