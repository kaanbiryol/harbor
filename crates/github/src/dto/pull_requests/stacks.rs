use harbor_domain::{PullRequestStack, PullRequestStackMembership};
use serde::Deserialize;
use serde_json::Value;

use super::{ApiRef, GitHubError, GraphQlPullRequestSearchNode, Result};

#[derive(Debug, Deserialize)]
pub(super) struct ApiStackMembership {
    number: u64,
    position: usize,
    size: usize,
    base: ApiRef,
}

impl ApiStackMembership {
    pub(super) fn into_domain(self) -> PullRequestStackMembership {
        PullRequestStackMembership {
            number: self.number,
            position: self.position,
            size: self.size,
            base_ref: self.base.name,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApiPullRequestStack {
    pub(crate) number: u64,
    base: ApiRef,
    pub(crate) pull_requests: Vec<ApiStackPullRequest>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApiStackPullRequest {
    pub(crate) number: u64,
}

pub(crate) fn stack_from_graphql_value(
    stack: ApiPullRequestStack,
    value: Value,
) -> Result<PullRequestStack> {
    if let Some(errors) = value
        .get("errors")
        .filter(|errors| errors.as_array().is_none_or(|errors| !errors.is_empty()))
    {
        return Err(GitHubError::Mapping(format!(
            "Failed to load stack pull requests: {errors}"
        )));
    }
    let repository = value
        .pointer("/data/repository")
        .ok_or_else(|| GitHubError::Mapping("Missing stack repository".into()))?;
    let size = stack.pull_requests.len();
    let pull_requests = stack
        .pull_requests
        .into_iter()
        .enumerate()
        .map(|(index, member)| {
            let node = repository
                .get(format!("pr{}", member.number))
                .ok_or_else(|| {
                    GitHubError::Mapping(format!("Missing stack PR #{}", member.number))
                })?;
            let node: GraphQlPullRequestSearchNode = serde_json::from_value(node.clone())
                .map_err(|error| GitHubError::Mapping(error.to_string()))?;
            let mut pull_request = node.into_domain()?;
            if pull_request.number != member.number {
                return Err(GitHubError::Mapping(
                    "Stack pull request number mismatch".into(),
                ));
            }
            pull_request.stack = Some(PullRequestStackMembership {
                number: stack.number,
                position: index + 1,
                size,
                base_ref: stack.base.name.clone(),
            });
            Ok(pull_request)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(PullRequestStack {
        number: stack.number,
        base_ref: stack.base.name,
        pull_requests,
    })
}
