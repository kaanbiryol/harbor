use harbor_domain::{Label, PullRequestPerson, RepoId};
use serde::Deserialize;

use crate::{
    GitHubError, GitHubTransport, PullRequestMetadataOptions, RepositoryList, Result, dto,
};

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
            reviewers: reviewers_from_value(reviewers)?,
            assignees: people_from_value(assignees)?,
            labels: labels_from_value(labels)?,
        })
    }
}

#[derive(Deserialize)]
struct ApiPerson {
    login: String,
    #[serde(default)]
    avatar_url: Option<String>,
    #[serde(default)]
    permissions: Option<ApiPermissions>,
}

#[derive(Deserialize)]
struct ApiPermissions {
    #[serde(default)]
    push: bool,
}

#[derive(Deserialize)]
struct ApiLabel {
    name: String,
    #[serde(default)]
    color: Option<String>,
}

fn people_from_value(value: serde_json::Value) -> Result<Vec<PullRequestPerson>> {
    let people: Vec<ApiPerson> =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;
    Ok(people
        .into_iter()
        .map(|person| PullRequestPerson {
            login: person.login,
            avatar_url: person.avatar_url,
        })
        .collect())
}

fn reviewers_from_value(value: serde_json::Value) -> Result<Vec<PullRequestPerson>> {
    let people: Vec<ApiPerson> =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;
    Ok(people
        .into_iter()
        .filter(|person| {
            person
                .permissions
                .as_ref()
                .is_some_and(|permissions| permissions.push)
        })
        .map(|person| PullRequestPerson {
            login: person.login,
            avatar_url: person.avatar_url,
        })
        .collect())
}

fn labels_from_value(value: serde_json::Value) -> Result<Vec<Label>> {
    let labels: Vec<ApiLabel> =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;
    Ok(labels
        .into_iter()
        .map(|label| Label {
            name: label.name,
            color: label.color,
        })
        .collect())
}
