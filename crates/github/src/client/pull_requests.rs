use harbor_domain::{DiffFile, PullRequest, RepoId};
use serde_json::json;

use crate::{GitHubError, GitHubTransport, Result, dto};

use super::{
    GitHubClient, PullRequestListFilter,
    requests::{REPOSITORY_PULL_REQUESTS_QUERY, repository_pull_requests_query},
};

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

    pub async fn list_repository_pull_requests(
        &self,
        repository: &RepoId,
        filter: PullRequestListFilter,
    ) -> Result<Vec<PullRequest>> {
        let mut pull_requests = Vec::new();
        let mut after = None;
        let search_query = repository_pull_requests_query(repository, filter);

        loop {
            let response = self
                .transport
                .graphql(
                    REPOSITORY_PULL_REQUESTS_QUERY,
                    json!({
                        "searchQuery": search_query,
                        "after": after,
                    }),
                )
                .await?;
            let page = dto::pull_request_search_page_from_graphql_value(response)?;
            pull_requests.extend(page.pull_requests);

            if !page.has_next_page {
                break;
            }

            after = Some(page.end_cursor.ok_or_else(|| {
                GitHubError::Mapping(
                    "repository pull request page was missing an end cursor".to_string(),
                )
            })?);
        }

        Ok(pull_requests)
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
