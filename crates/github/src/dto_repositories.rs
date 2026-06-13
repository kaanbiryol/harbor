use harbor_domain::RepoId;
use serde::Deserialize;
use serde_json::Value;

use crate::{GitHubError, Result};

#[derive(Debug, Deserialize)]
struct ApiRepository {
    name: String,
    owner: ApiRepositoryOwner,
}

#[derive(Debug, Deserialize)]
struct ApiRepositoryOwner {
    login: String,
}

pub fn repositories_from_value(value: Value) -> Result<Vec<RepoId>> {
    let repositories: Vec<ApiRepository> =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;

    Ok(repositories
        .into_iter()
        .map(repo_id_from_api_repository)
        .collect())
}

pub fn repository_from_value(value: Value) -> Result<RepoId> {
    let repository: ApiRepository =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;

    Ok(repo_id_from_api_repository(repository))
}

fn repo_id_from_api_repository(repository: ApiRepository) -> RepoId {
    RepoId::new(repository.owner.login, repository.name)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::repositories_from_value;

    #[test]
    fn maps_repository_list() {
        let value = json!([
            {
                "name": "app",
                "owner": { "login": "acme" }
            },
            {
                "name": "tools",
                "owner": { "login": "octo" }
            }
        ]);

        let repositories = repositories_from_value(value).unwrap();

        assert_eq!(repositories.len(), 2);
        assert_eq!(repositories[0].full_name(), "acme/app");
        assert_eq!(repositories[1].full_name(), "octo/tools");
    }
}
