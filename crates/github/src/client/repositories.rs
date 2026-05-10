use harbor_domain::RepoId;

use crate::{GitHubTransport, Result, dto};

use super::{
    GitHubClient,
    requests::{REPOSITORY_PAGE_SIZE, REPOSITORY_PAGE_SIZE_QUERY},
};

const REPOSITORY_LIST_PAGE_LIMIT: usize = 20;

impl<T> GitHubClient<T>
where
    T: GitHubTransport,
{
    pub async fn current_user(&self) -> Result<String> {
        let response = self.transport.rest_get("/user", &[]).await?;

        dto::current_user_login_from_value(response)
    }

    pub async fn list_repositories(&self) -> Result<Vec<RepoId>> {
        let mut repositories = Vec::new();
        let mut page = 1;

        loop {
            if page > REPOSITORY_LIST_PAGE_LIMIT {
                return Err(crate::GitHubError::RequestBudget(format!(
                    "stopped loading repositories after {REPOSITORY_LIST_PAGE_LIMIT} pages"
                )));
            }

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
}
