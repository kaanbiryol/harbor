use harbor_domain::RepoId;

use crate::{GitHubTransport, PullRequestMetadataOptions, RepositoryList, Result, dto};

use super::{
    GitHubClient,
    requests::{REPOSITORY_PAGE_SIZE, REPOSITORY_PAGE_SIZE_QUERY},
};

impl<T> GitHubClient<T>
where
    T: GitHubTransport,
{
    pub async fn current_user(&self) -> Result<String> {
        let response = self.transport.rest_get("/user", &[]).await?;

        dto::current_user_login_from_value(response)
    }

    pub async fn list_repositories(&self) -> Result<RepositoryList> {
        let response = self
            .transport
            .rest_get(
                "/user/repos",
                &[
                    ("affiliation", "owner,collaborator,organization_member"),
                    ("per_page", REPOSITORY_PAGE_SIZE_QUERY),
                    ("sort", "updated"),
                ],
            )
            .await?;
        let repositories = dto::repositories_from_value(response)?;
        let possibly_limited = repositories.len() == REPOSITORY_PAGE_SIZE;

        Ok(RepositoryList {
            repositories,
            possibly_limited,
        })
    }

    pub async fn get_repository(&self, repository: &RepoId) -> Result<RepoId> {
        let path = format!("/repos/{}/{}", repository.owner, repository.name);
        let response = self.transport.rest_get(&path, &[]).await?;

        dto::repository_from_value(response)
    }

    pub async fn list_pull_request_metadata_options(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<PullRequestMetadataOptions> {
        let collaborators_path = format!("/repos/{owner}/{repo}/collaborators");
        let assignees_path = format!("/repos/{owner}/{repo}/assignees");
        let labels_path = format!("/repos/{owner}/{repo}/labels");
        let reviewers = self
            .transport
            .rest_get(&collaborators_path, &[("per_page", "100")])
            .await?;
        let assignees = self
            .transport
            .rest_get(&assignees_path, &[("per_page", "100")])
            .await?;
        let labels = self
            .transport
            .rest_get(&labels_path, &[("per_page", "100")])
            .await?;

        Ok(PullRequestMetadataOptions {
            reviewers: dto::reviewers_from_value(reviewers)?,
            assignees: dto::people_from_value(assignees)?,
            labels: dto::labels_from_value(labels)?,
        })
    }
}
