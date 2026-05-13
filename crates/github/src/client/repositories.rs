use harbor_domain::RepoId;

use crate::{GitHubTransport, RepositoryList, Result, dto};

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
}
