use harbor_domain::{Label, PullRequestPerson, RepoId};
use serde::Deserialize;
use serde_json::Value;

use super::pull_requests::{ApiLabel, ApiUser};
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

#[derive(Debug, Deserialize)]
struct ApiCollaborator {
    #[serde(flatten)]
    user: ApiUser,
    #[serde(default)]
    permissions: Option<ApiPermissions>,
}

#[derive(Debug, Deserialize)]
struct ApiPermissions {
    #[serde(default)]
    push: bool,
}

pub fn current_user_login_from_value(value: Value) -> Result<String> {
    let user: ApiUser =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;

    Ok(user.into_domain().login)
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

pub fn people_from_value(value: Value) -> Result<Vec<PullRequestPerson>> {
    let people: Vec<ApiUser> =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;
    Ok(people.into_iter().map(ApiUser::into_domain).collect())
}

pub fn reviewers_from_value(value: Value) -> Result<Vec<PullRequestPerson>> {
    let collaborators: Vec<ApiCollaborator> =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;
    Ok(collaborators
        .into_iter()
        .filter(|collaborator| {
            collaborator
                .permissions
                .as_ref()
                .is_some_and(|permissions| permissions.push)
        })
        .map(|collaborator| collaborator.user.into_domain())
        .collect())
}

pub fn labels_from_value(value: Value) -> Result<Vec<Label>> {
    let labels: Vec<ApiLabel> =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;
    Ok(labels.into_iter().map(ApiLabel::into_domain).collect())
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
