use std::time::Duration;

use harbor_domain::{PullRequestStack, RepoId, StackMergeAction, StackMergeOutcome};
use serde::Deserialize;
use serde_json::json;

use crate::{GitHubError, GitHubTransport, Result, dto};

use super::{GitHubClient, pull_requests::merge_method_name};

const MAX_ASYNC_MERGE_POLLS: usize = 120;

const STACK_PULL_REQUEST_FIELDS: &str = r#"
    id number title body url state isDraft headRefName baseRefName headRefOid
    author { login avatarUrl(size: 48) }
    repository { name viewerPermission owner { login } }
    reviewDecision mergeStateStatus isMergeQueueEnabled isInMergeQueue
    viewerCanMergeAsAdmin createdAt updatedAt
    statusCheckRollup {
        contexts(first: 100) {
            nodes {
                __typename
                ... on CheckRun { status conclusion }
                ... on StatusContext { state }
            }
        }
    }
"#;

#[derive(Deserialize)]
#[serde(tag = "status", content = "details", rename_all = "snake_case")]
enum AsyncMergeResponse {
    Pending { uuid: String },
    Merged {},
    Enqueued {},
    Failed { message: String },
}

impl<T: GitHubTransport> GitHubClient<T> {
    pub async fn get_pull_request_stack(
        &self,
        repository: &RepoId,
        number: u64,
    ) -> Result<PullRequestStack> {
        let path = format!(
            "/repos/{}/{}/stacks/{number}",
            repository.owner, repository.name
        );
        let response = self.transport.rest_get(&path, &[]).await?;
        let stack: dto::ApiPullRequestStack = serde_json::from_value(response)
            .map_err(|error| GitHubError::Mapping(error.to_string()))?;
        if stack.number != number
            || stack.pull_requests.is_empty()
            || stack.pull_requests.len() > 100
        {
            return Err(GitHubError::Mapping(
                "Invalid pull request stack returned by GitHub".into(),
            ));
        }
        let fields = stack
            .pull_requests
            .iter()
            .map(|member| {
                format!(
                    "pr{}: pullRequest(number: {}) {{ {STACK_PULL_REQUEST_FIELDS} }}",
                    member.number, member.number
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let query = format!(
            "query HarborPullRequestStack($owner: String!, $repo: String!) {{ repository(owner: $owner, name: $repo) {{ {fields} }} rateLimit {{ cost remaining limit used }} }}"
        );
        let response = self
            .transport
            .graphql(
                &query,
                json!({"owner": repository.owner, "repo": repository.name}),
            )
            .await?;
        dto::stack_from_graphql_value(stack, response)
    }

    pub async fn merge_pull_request_stack(
        &self,
        repository: &RepoId,
        number: u64,
        head_sha: &str,
        action: StackMergeAction,
    ) -> Result<StackMergeOutcome> {
        if head_sha.is_empty() {
            return Err(GitHubError::Mapping(
                "The stack is missing its head SHA".into(),
            ));
        }
        let path = format!(
            "/repos/{}/{}/pulls/{number}/merge-async",
            repository.owner, repository.name
        );
        let body = match action {
            StackMergeAction::Enqueue => json!({"sha": head_sha, "merge_action": "merge_queue"}),
            StackMergeAction::Merge(method) => {
                json!({"sha": head_sha, "merge_action": "direct_merge", "merge_method": merge_method_name(method)})
            }
        };
        let mut response = self.transport.rest_put(&path, body).await?;
        for attempt in 0..=MAX_ASYNC_MERGE_POLLS {
            let status: AsyncMergeResponse = serde_json::from_value(response)
                .map_err(|error| GitHubError::Mapping(error.to_string()))?;
            match status {
                AsyncMergeResponse::Merged {} => return Ok(StackMergeOutcome::Merged),
                AsyncMergeResponse::Enqueued {} => return Ok(StackMergeOutcome::Enqueued),
                AsyncMergeResponse::Failed { message } => {
                    return Err(GitHubError::Transport(message));
                }
                AsyncMergeResponse::Pending { uuid } => {
                    if attempt == MAX_ASYNC_MERGE_POLLS {
                        break;
                    }
                    if uuid.is_empty()
                        || !uuid
                            .bytes()
                            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
                    {
                        return Err(GitHubError::Mapping(
                            "Invalid asynchronous merge request identifier".into(),
                        ));
                    }
                    if attempt > 0 {
                        smol::Timer::after(Duration::from_secs(1)).await;
                    }
                    response = self
                        .transport
                        .rest_get(&format!("{path}/{uuid}"), &[])
                        .await?;
                }
            }
        }
        Err(GitHubError::RequestBudget("GitHub is still processing this stack. Check its status on GitHub before trying again.".into()))
    }
}
