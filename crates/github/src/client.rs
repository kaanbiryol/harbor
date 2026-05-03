use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestReview, RepoId, ReviewThread, WorkflowJob,
    WorkflowRun,
};
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
    pub async fn list_repositories(&self) -> Result<Vec<RepoId>> {
        let mut repositories = Vec::new();
        let mut page = 1;

        loop {
            let response = if page == 1 {
                self.transport
                    .rest_get(
                        "/user/repos",
                        &[
                            ("affiliation", "owner,collaborator,organization_member"),
                            ("per_page", REPOSITORY_PAGE_SIZE_QUERY),
                            ("sort", "updated"),
                        ],
                    )
                    .await?
            } else {
                let page_string = page.to_string();
                self.transport
                    .rest_get(
                        "/user/repos",
                        &[
                            ("affiliation", "owner,collaborator,organization_member"),
                            ("per_page", REPOSITORY_PAGE_SIZE_QUERY),
                            ("sort", "updated"),
                            ("page", page_string.as_str()),
                        ],
                    )
                    .await?
            };
            let mut page_repositories = dto::repositories_from_value(response)?;
            let page_repository_count = page_repositories.len();
            repositories.append(&mut page_repositories);

            if page_repository_count < REPOSITORY_PAGE_SIZE {
                break;
            }

            page += 1;
        }

        Ok(repositories)
    }

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

    pub async fn list_pull_request_reviews(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<PullRequestReview>> {
        let path = format!("/repos/{owner}/{repo}/pulls/{number}/reviews");
        let response = self
            .transport
            .rest_get(&path, &[("per_page", "100")])
            .await?;

        dto::pull_request_reviews_from_value(response)
    }

    pub async fn list_review_threads(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<ReviewThread>> {
        let mut threads = Vec::new();
        let mut after = None;

        loop {
            let response = self
                .transport
                .graphql(
                    REVIEW_THREADS_QUERY,
                    json!({
                        "owner": owner,
                        "repo": repo,
                        "number": number,
                        "after": after,
                    }),
                )
                .await?;
            let page = dto::review_threads_page_from_graphql_value(response)?;
            threads.extend(page.threads);

            if !page.has_next_page {
                break;
            }

            after = Some(page.end_cursor.ok_or_else(|| {
                crate::GitHubError::Mapping(
                    "review threads page was missing an end cursor".to_string(),
                )
            })?);
        }

        Ok(threads)
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

const REVIEW_THREADS_QUERY: &str = r#"
query HarborPullRequestReviewThreads($owner: String!, $repo: String!, $number: Int!, $after: String) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      reviewThreads(first: 100, after: $after) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          id
          path
          line
          originalLine
          isResolved
          isOutdated
          comments(first: 100) {
            nodes {
              id
              body
              author {
                login
              }
              createdAt
              updatedAt
              path
              line
              originalLine
            }
          }
        }
      }
    }
  }
}
"#;

const REPOSITORY_PAGE_SIZE: usize = 100;
const REPOSITORY_PAGE_SIZE_QUERY: &str = "100";

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use serde_json::{Value, json};

    use super::*;
    use crate::{GitHubError, GitHubTransport};

    type RecordedGet = (String, Vec<(String, String)>);

    #[derive(Clone, Default)]
    struct RecordingTransport {
        gets: Arc<Mutex<Vec<RecordedGet>>>,
        get_response: Arc<Mutex<Option<Value>>>,
        get_responses: Arc<Mutex<Vec<Value>>>,
        posts: Arc<Mutex<Vec<(String, Value)>>>,
        puts: Arc<Mutex<Vec<(String, Value)>>>,
        graphql_calls: Arc<Mutex<Vec<(String, Value)>>>,
        graphql_responses: Arc<Mutex<Vec<Value>>>,
        graphql_response: Arc<Mutex<Option<Value>>>,
        log: Arc<Mutex<Option<String>>>,
    }

    #[async_trait]
    impl GitHubTransport for RecordingTransport {
        async fn rest_get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
            self.gets
                .lock()
                .expect("gets mutex should not be poisoned")
                .push((
                    path.to_string(),
                    query
                        .iter()
                        .map(|(key, value)| (key.to_string(), value.to_string()))
                        .collect(),
                ));

            {
                let mut responses = self
                    .get_responses
                    .lock()
                    .expect("get responses mutex should not be poisoned");
                if !responses.is_empty() {
                    return Ok(responses.remove(0));
                }
            }

            self.get_response
                .lock()
                .expect("get response mutex should not be poisoned")
                .clone()
                .ok_or_else(|| GitHubError::Transport("missing GET response".to_string()))
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

        async fn graphql(&self, query: &str, variables: Value) -> Result<Value> {
            self.graphql_calls
                .lock()
                .expect("graphql calls mutex should not be poisoned")
                .push((query.to_string(), variables));
            let mut responses = self
                .graphql_responses
                .lock()
                .expect("graphql responses mutex should not be poisoned");
            if !responses.is_empty() {
                return Ok(responses.remove(0));
            }

            self.graphql_response
                .lock()
                .expect("graphql response mutex should not be poisoned")
                .clone()
                .ok_or_else(|| GitHubError::Transport("missing GraphQL response".to_string()))
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
    fn gets_pull_request_reviews_endpoint() {
        let transport = RecordingTransport::default();
        *transport
            .get_response
            .lock()
            .expect("get response mutex should not be poisoned") = Some(json!([]));
        let client = GitHubClient::new(transport.clone());

        smol::block_on(client.list_pull_request_reviews("acme", "app", 7)).unwrap();

        let gets = transport
            .gets
            .lock()
            .expect("gets mutex should not be poisoned");
        assert_eq!(gets.len(), 1);
        assert_eq!(gets[0].0, "/repos/acme/app/pulls/7/reviews");
        assert_eq!(gets[0].1, vec![("per_page".to_string(), "100".to_string())]);
    }

    #[test]
    fn gets_user_repositories_endpoint() {
        let transport = RecordingTransport::default();
        *transport
            .get_response
            .lock()
            .expect("get response mutex should not be poisoned") = Some(json!([]));
        let client = GitHubClient::new(transport.clone());

        smol::block_on(client.list_repositories()).unwrap();

        let gets = transport
            .gets
            .lock()
            .expect("gets mutex should not be poisoned");
        assert_eq!(gets.len(), 1);
        assert_eq!(gets[0].0, "/user/repos");
        assert_eq!(
            gets[0].1,
            vec![
                (
                    "affiliation".to_string(),
                    "owner,collaborator,organization_member".to_string()
                ),
                ("per_page".to_string(), "100".to_string()),
                ("sort".to_string(), "updated".to_string()),
            ]
        );
    }

    #[test]
    fn paginates_user_repositories_endpoint() {
        let transport = RecordingTransport::default();
        *transport
            .get_responses
            .lock()
            .expect("get responses mutex should not be poisoned") = vec![
            Value::Array(
                (0..REPOSITORY_PAGE_SIZE)
                    .map(|index| {
                        json!({
                            "name": format!("app-{index}"),
                            "owner": { "login": "acme" },
                        })
                    })
                    .collect(),
            ),
            json!([
                {
                    "name": "last",
                    "owner": { "login": "acme" },
                }
            ]),
        ];
        let client = GitHubClient::new(transport.clone());

        let repositories = smol::block_on(client.list_repositories()).unwrap();

        assert_eq!(repositories.len(), REPOSITORY_PAGE_SIZE + 1);
        assert_eq!(repositories[REPOSITORY_PAGE_SIZE].full_name(), "acme/last");

        let gets = transport
            .gets
            .lock()
            .expect("gets mutex should not be poisoned");
        assert_eq!(gets.len(), 2);
        assert_eq!(gets[0].0, "/user/repos");
        assert_eq!(
            gets[1].1,
            vec![
                (
                    "affiliation".to_string(),
                    "owner,collaborator,organization_member".to_string()
                ),
                ("per_page".to_string(), "100".to_string()),
                ("sort".to_string(), "updated".to_string()),
                ("page".to_string(), "2".to_string()),
            ]
        );
    }

    #[test]
    fn queries_pull_request_review_threads() {
        let transport = RecordingTransport::default();
        *transport
            .graphql_response
            .lock()
            .expect("graphql response mutex should not be poisoned") = Some(json!({
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "nodes": []
                        }
                    }
                }
            }
        }));
        let client = GitHubClient::new(transport.clone());

        smol::block_on(client.list_review_threads("acme", "app", 7)).unwrap();

        let calls = transport
            .graphql_calls
            .lock()
            .expect("graphql calls mutex should not be poisoned");
        assert_eq!(calls.len(), 1);
        assert!(calls[0].0.contains("reviewThreads"));
        assert_eq!(
            calls[0].1,
            json!({
                "owner": "acme",
                "repo": "app",
                "number": 7,
                "after": null,
            })
        );
    }

    #[test]
    fn paginates_pull_request_review_threads() {
        let transport = RecordingTransport::default();
        *transport
            .graphql_responses
            .lock()
            .expect("graphql responses mutex should not be poisoned") = vec![
            json!({
                "data": {
                    "repository": {
                        "pullRequest": {
                            "reviewThreads": {
                                "pageInfo": {
                                    "hasNextPage": true,
                                    "endCursor": "cursor-1"
                                },
                                "nodes": []
                            },
                        }
                    }
                }
            }),
            json!({
                "data": {
                    "repository": {
                        "pullRequest": {
                            "reviewThreads": {
                                "pageInfo": {
                                    "hasNextPage": false,
                                    "endCursor": null
                                },
                                "nodes": []
                            }
                        }
                    }
                }
            }),
        ];
        let client = GitHubClient::new(transport.clone());

        smol::block_on(client.list_review_threads("acme", "app", 7)).unwrap();

        let calls = transport
            .graphql_calls
            .lock()
            .expect("graphql calls mutex should not be poisoned");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].1["after"], Value::Null);
        assert_eq!(calls[1].1["after"], "cursor-1");
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
