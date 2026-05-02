use harbor_domain::{CheckRun, DiffFile, PullRequest, RepoId, WorkflowJob, WorkflowRun};
use serde_json::json;

use crate::{GitHubTransport, Result, dto};

#[derive(Clone, Debug)]
pub struct GitHubClient<T> {
    transport: T,
}

impl<T> GitHubClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }
}

impl<T> GitHubClient<T>
where
    T: GitHubTransport,
{
    pub async fn list_open_pull_requests(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<PullRequest>> {
        let path = format!("/repos/{owner}/{repo}/pulls");
        let response = self
            .transport
            .rest_get(
                &path,
                &[("state", "open"), ("per_page", "50"), ("sort", "updated")],
            )
            .await?;

        dto::pull_requests_from_value(RepoId::new(owner, repo), response)
    }

    pub async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<PullRequest> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}");
        let response = self.transport.rest_get(&path, &[]).await?;

        dto::pull_request_from_value(RepoId::new(owner, repo), response)
    }

    pub async fn list_pull_request_files(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<DiffFile>> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}/files");
        let response = self
            .transport
            .rest_get(&path, &[("per_page", "100")])
            .await?;

        dto::diff_files_from_value(response)
    }

    pub async fn list_check_runs(
        &self,
        owner: &str,
        repo: &str,
        git_ref: &str,
    ) -> Result<Vec<CheckRun>> {
        let path = format!("/repos/{owner}/{repo}/commits/{git_ref}/check-runs");
        let response = self
            .transport
            .rest_get(&path, &[("per_page", "100")])
            .await?;

        dto::check_runs_from_value(response)
    }

    pub async fn list_workflow_runs_for_head(
        &self,
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> Result<Vec<WorkflowRun>> {
        let path = format!("/repos/{owner}/{repo}/actions/runs");
        let response = self
            .transport
            .rest_get(&path, &[("head_sha", head_sha), ("per_page", "50")])
            .await?;

        dto::workflow_runs_from_value(response)
    }

    pub async fn list_workflow_jobs_for_run(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
    ) -> Result<Vec<WorkflowJob>> {
        let path = format!("/repos/{owner}/{repo}/actions/runs/{run_id}/jobs");
        let response = self
            .transport
            .rest_get(&path, &[("per_page", "100")])
            .await?;

        dto::workflow_jobs_from_value(response)
    }

    pub async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String> {
        self.transport.workflow_run_log(owner, repo, run_id).await
    }

    pub async fn rerun_workflow_run(&self, owner: &str, repo: &str, run_id: u64) -> Result<()> {
        let path = format!("/repos/{owner}/{repo}/actions/runs/{run_id}/rerun");
        self.transport.rest_post(&path, json!({})).await?;

        Ok(())
    }

    pub async fn rerun_failed_jobs(&self, owner: &str, repo: &str, run_id: u64) -> Result<()> {
        let path = format!("/repos/{owner}/{repo}/actions/runs/{run_id}/rerun-failed-jobs");
        self.transport.rest_post(&path, json!({})).await?;

        Ok(())
    }

    pub async fn dispatch_workflow(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: u64,
        git_ref: &str,
    ) -> Result<()> {
        let path = format!("/repos/{owner}/{repo}/actions/workflows/{workflow_id}/dispatches");
        self.transport
            .rest_post(&path, json!({ "ref": git_ref }))
            .await?;

        Ok(())
    }

    pub async fn approve_pull_request(&self, owner: &str, repo: &str, number: u64) -> Result<()> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}/reviews");
        self.transport
            .rest_post(&path, json!({ "event": "APPROVE" }))
            .await?;

        Ok(())
    }

    pub async fn request_pull_request_changes(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &str,
    ) -> Result<()> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}/reviews");
        self.transport
            .rest_post(&path, json!({ "event": "REQUEST_CHANGES", "body": body }))
            .await?;

        Ok(())
    }

    pub async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        head_sha: &str,
    ) -> Result<()> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}/merge");
        self.transport
            .rest_put(
                &path,
                json!({
                    "sha": head_sha,
                    "merge_method": "squash",
                }),
            )
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use serde_json::{Value, json};

    use super::*;
    use crate::{GitHubError, GitHubTransport};

    #[derive(Clone, Default)]
    struct RecordingTransport {
        posts: Arc<Mutex<Vec<(String, Value)>>>,
        puts: Arc<Mutex<Vec<(String, Value)>>>,
        log: Arc<Mutex<Option<String>>>,
    }

    #[async_trait]
    impl GitHubTransport for RecordingTransport {
        async fn rest_get(&self, _: &str, _: &[(&str, &str)]) -> Result<Value> {
            Err(GitHubError::Transport(
                "rest_get should not be called".to_string(),
            ))
        }

        async fn rest_post(&self, path: &str, body: Value) -> Result<Value> {
            self.posts
                .lock()
                .expect("posts mutex should not be poisoned")
                .push((path.to_string(), body));
            Ok(Value::Null)
        }

        async fn rest_put(&self, path: &str, body: Value) -> Result<Value> {
            self.puts
                .lock()
                .expect("puts mutex should not be poisoned")
                .push((path.to_string(), body));
            Ok(Value::Null)
        }

        async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String> {
            let log = format!("{owner}/{repo}#{run_id}");
            *self.log.lock().expect("log mutex should not be poisoned") = Some(log.clone());
            Ok(log)
        }

        async fn graphql(&self, _: &str, _: Value) -> Result<Value> {
            Err(GitHubError::Transport(
                "graphql should not be called".to_string(),
            ))
        }
    }

    #[test]
    fn posts_rerun_failed_jobs_endpoint() {
        let transport = RecordingTransport::default();
        let client = GitHubClient::new(transport.clone());

        smol::block_on(client.rerun_failed_jobs("acme", "app", 42)).unwrap();

        let posts = transport
            .posts
            .lock()
            .expect("posts mutex should not be poisoned");
        assert_eq!(posts.len(), 1);
        assert_eq!(
            posts[0].0,
            "/repos/acme/app/actions/runs/42/rerun-failed-jobs"
        );
        assert_eq!(posts[0].1, json!({}));
    }

    #[test]
    fn posts_workflow_dispatch_ref() {
        let transport = RecordingTransport::default();
        let client = GitHubClient::new(transport.clone());

        smol::block_on(client.dispatch_workflow("acme", "app", 9, "feature/build")).unwrap();

        let posts = transport
            .posts
            .lock()
            .expect("posts mutex should not be poisoned");
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].0, "/repos/acme/app/actions/workflows/9/dispatches");
        assert_eq!(posts[0].1, json!({ "ref": "feature/build" }));
    }

    #[test]
    fn posts_pull_request_approval() {
        let transport = RecordingTransport::default();
        let client = GitHubClient::new(transport.clone());

        smol::block_on(client.approve_pull_request("acme", "app", 7)).unwrap();

        let posts = transport
            .posts
            .lock()
            .expect("posts mutex should not be poisoned");
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].0, "/repos/acme/app/pulls/7/reviews");
        assert_eq!(posts[0].1, json!({ "event": "APPROVE" }));
    }

    #[test]
    fn posts_pull_request_change_request() {
        let transport = RecordingTransport::default();
        let client = GitHubClient::new(transport.clone());

        smol::block_on(client.request_pull_request_changes(
            "acme",
            "app",
            7,
            "Please address the failing path.",
        ))
        .unwrap();

        let posts = transport
            .posts
            .lock()
            .expect("posts mutex should not be poisoned");
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].0, "/repos/acme/app/pulls/7/reviews");
        assert_eq!(
            posts[0].1,
            json!({
                "event": "REQUEST_CHANGES",
                "body": "Please address the failing path.",
            })
        );
    }

    #[test]
    fn puts_pull_request_squash_merge() {
        let transport = RecordingTransport::default();
        let client = GitHubClient::new(transport.clone());

        smol::block_on(client.merge_pull_request("acme", "app", 7, "abc123")).unwrap();

        let puts = transport
            .puts
            .lock()
            .expect("puts mutex should not be poisoned");
        assert_eq!(puts.len(), 1);
        assert_eq!(puts[0].0, "/repos/acme/app/pulls/7/merge");
        assert_eq!(
            puts[0].1,
            json!({
                "sha": "abc123",
                "merge_method": "squash",
            })
        );
    }

    #[test]
    fn delegates_workflow_run_log() {
        let transport = RecordingTransport::default();
        let client = GitHubClient::new(transport.clone());

        let log = smol::block_on(client.workflow_run_log("acme", "app", 42)).unwrap();

        assert_eq!(log, "acme/app#42");
        assert_eq!(
            transport
                .log
                .lock()
                .expect("log mutex should not be poisoned")
                .as_deref(),
            Some("acme/app#42")
        );
    }
}
