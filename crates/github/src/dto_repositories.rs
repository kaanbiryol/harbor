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
        .map(|repository| RepoId::new(repository.owner.login, repository.name))
        .collect())
}
