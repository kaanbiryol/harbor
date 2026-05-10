use harbor_domain::{
    CheckConclusion, CheckStatus, ChecksSummary, DiffFile, FileStatus, Label, MergeState,
    PullRequest, PullRequestState, RepoId, ReviewDecision,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{GitHubError, Result};

#[derive(Debug, Deserialize)]
struct ApiPullRequest {
    #[serde(default)]
    node_id: String,
    number: u64,
    title: String,
    body: Option<String>,
    #[serde(default)]
    html_url: String,
    state: String,
    #[serde(default)]
    draft: bool,
    user: Option<ApiUser>,
    head: ApiRef,
    base: ApiRef,
    #[serde(default)]
    labels: Vec<ApiLabel>,
    #[serde(default)]
    merged: Option<bool>,
    #[serde(default)]
    mergeable_state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiUser {
    login: String,
}

#[derive(Debug, Deserialize)]
struct ApiRef {
    #[serde(rename = "ref")]
    name: String,
    #[serde(default)]
    sha: String,
}

#[derive(Debug, Deserialize)]
struct ApiLabel {
    name: String,
    color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestSearchResponse {
    data: Option<GraphQlPullRequestSearchData>,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestSearchData {
    search: GraphQlPullRequestSearchConnection,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestSearchConnection {
    #[serde(default)]
    nodes: Vec<Option<GraphQlPullRequestSearchNode>>,
    #[serde(default, rename = "pageInfo")]
    page_info: GraphQlPageInfo,
}

#[derive(Debug, Default, Deserialize)]
struct GraphQlPageInfo {
    #[serde(default, rename = "hasNextPage")]
    has_next_page: bool,
    #[serde(default, rename = "endCursor")]
    end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestSearchNode {
    #[serde(default, rename = "__typename")]
    typename: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    number: Option<u64>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default, rename = "isDraft")]
    is_draft: bool,
    #[serde(default)]
    author: Option<ApiUser>,
    #[serde(default)]
    repository: Option<GraphQlRepository>,
    #[serde(default, rename = "headRefName")]
    head_ref_name: Option<String>,
    #[serde(default, rename = "baseRefName")]
    base_ref_name: Option<String>,
    #[serde(default, rename = "headRefOid")]
    head_ref_oid: Option<String>,
    #[serde(default, rename = "reviewDecision")]
    review_decision: Option<String>,
    #[serde(default, rename = "mergeStateStatus")]
    merge_state_status: Option<String>,
    #[serde(default, rename = "statusCheckRollup")]
    status_check_rollup: Option<GraphQlStatusCheckRollup>,
    #[serde(default)]
    labels: GraphQlNodes<GraphQlLabel>,
}

#[derive(Debug, Deserialize)]
struct GraphQlRepository {
    name: String,
    owner: GraphQlRepositoryOwner,
}

#[derive(Debug, Deserialize)]
struct GraphQlRepositoryOwner {
    login: String,
}

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct GraphQlNodes<T> {
    #[serde(default)]
    nodes: Vec<Option<T>>,
}

#[derive(Debug, Deserialize)]
struct GraphQlStatusCheckRollup {
    #[serde(default)]
    contexts: GraphQlNodes<GraphQlStatusCheckContext>,
}

#[derive(Debug, Deserialize)]
struct GraphQlStatusCheckContext {
    #[serde(default, rename = "__typename")]
    typename: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    state: Option<String>,
}

impl<T> Default for GraphQlNodes<T> {
    fn default() -> Self {
        Self { nodes: Vec::new() }
    }
}

#[derive(Debug, Deserialize)]
struct GraphQlLabel {
    name: String,
    color: Option<String>,
}

pub(crate) struct PullRequestSearchPage {
    pub(crate) pull_requests: Vec<PullRequest>,
    pub(crate) has_next_page: bool,
    pub(crate) end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiDiffFile {
    filename: String,
    #[serde(default)]
    previous_filename: Option<String>,
    status: String,
    additions: u32,
    deletions: u32,
    changes: u32,
    #[serde(default)]
    patch: Option<String>,
}

pub fn pull_requests_from_value(repo: RepoId, value: Value) -> Result<Vec<PullRequest>> {
    let pulls: Vec<ApiPullRequest> =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;

    Ok(pulls
        .into_iter()
        .map(|pull| pull.into_domain(repo.clone()))
        .collect())
}

pub fn pull_request_from_value(repo: RepoId, value: Value) -> Result<PullRequest> {
    let pull: ApiPullRequest =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;

    Ok(pull.into_domain(repo))
}

pub(crate) fn pull_request_search_page_from_graphql_value(
    value: Value,
) -> Result<PullRequestSearchPage> {
    let response: GraphQlPullRequestSearchResponse =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;
    let data = response
        .data
        .ok_or_else(|| GitHubError::Mapping("missing GraphQL response data".to_string()))?;

    let mut pull_requests = Vec::new();
    for node in data.search.nodes.into_iter().flatten() {
        if node.is_pull_request() {
            pull_requests.push(node.into_domain()?);
        }
    }

    Ok(PullRequestSearchPage {
        pull_requests,
        has_next_page: data.search.page_info.has_next_page,
        end_cursor: data.search.page_info.end_cursor,
    })
}

pub fn diff_files_from_value(value: Value) -> Result<Vec<DiffFile>> {
    let files: Vec<ApiDiffFile> =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;

    Ok(files.into_iter().map(ApiDiffFile::into_domain).collect())
}

impl ApiPullRequest {
    fn into_domain(self, repo: RepoId) -> PullRequest {
        PullRequest {
            repo,
            node_id: self.node_id,
            number: self.number,
            title: self.title,
            body: self.body,
            author: self
                .user
                .map(|user| user.login)
                .unwrap_or_else(|| "ghost".to_string()),
            url: self.html_url,
            state: map_pull_request_state(&self.state, self.merged),
            is_draft: self.draft,
            head_ref: self.head.name,
            base_ref: self.base.name,
            head_sha: self.head.sha,
            review_decision: None,
            merge_state: self
                .mergeable_state
                .as_deref()
                .map(map_merge_state)
                .or(Some(MergeState::Unknown)),
            labels: self
                .labels
                .into_iter()
                .map(|label| Label {
                    name: label.name,
                    color: label.color,
                })
                .collect(),
            checks_summary: ChecksSummary::default(),
            unresolved_threads: 0,
        }
    }
}

impl GraphQlPullRequestSearchNode {
    fn is_pull_request(&self) -> bool {
        self.typename.as_deref() == Some("PullRequest") || self.number.is_some()
    }

    fn into_domain(self) -> Result<PullRequest> {
        let repository = required_graphql_field(self.repository, "repository")?;
        let repo = RepoId::new(repository.owner.login, repository.name);

        Ok(PullRequest {
            repo,
            node_id: required_graphql_field(self.id, "id")?,
            number: required_graphql_field(self.number, "number")?,
            title: required_graphql_field(self.title, "title")?,
            body: self.body.filter(|body| !body.is_empty()),
            author: self
                .author
                .map(|author| author.login)
                .unwrap_or_else(|| "ghost".to_string()),
            url: required_graphql_field(self.url, "url")?,
            state: self
                .state
                .as_deref()
                .map(|state| map_pull_request_state(state, None))
                .unwrap_or(PullRequestState::Open),
            is_draft: self.is_draft,
            head_ref: required_graphql_field(self.head_ref_name, "headRefName")?,
            base_ref: required_graphql_field(self.base_ref_name, "baseRefName")?,
            head_sha: required_graphql_field(self.head_ref_oid, "headRefOid")?,
            review_decision: self
                .review_decision
                .as_deref()
                .and_then(map_review_decision),
            merge_state: self.merge_state_status.as_deref().map(map_merge_state),
            labels: self
                .labels
                .nodes
                .into_iter()
                .flatten()
                .map(|label| Label {
                    name: label.name,
                    color: label.color,
                })
                .collect(),
            checks_summary: self
                .status_check_rollup
                .map(checks_summary_from_graphql_rollup)
                .unwrap_or_default(),
            unresolved_threads: 0,
        })
    }
}

impl ApiDiffFile {
    fn into_domain(self) -> DiffFile {
        DiffFile {
            path: self.filename,
            previous_path: self.previous_filename,
            status: map_file_status(&self.status),
            additions: self.additions,
            deletions: self.deletions,
            changes: self.changes,
            patch: self.patch,
        }
    }
}

fn required_graphql_field<T>(value: Option<T>, label: &str) -> Result<T> {
    value.ok_or_else(|| GitHubError::Mapping(format!("missing GraphQL pull request {label}")))
}

fn map_pull_request_state(state: &str, merged: Option<bool>) -> PullRequestState {
    if merged.unwrap_or(false) || state.eq_ignore_ascii_case("merged") {
        PullRequestState::Merged
    } else if state.eq_ignore_ascii_case("closed") {
        PullRequestState::Closed
    } else {
        PullRequestState::Open
    }
}

fn map_merge_state(state: &str) -> MergeState {
    match state.to_ascii_lowercase().as_str() {
        "clean" | "unstable" | "has_hooks" => MergeState::Clean,
        "dirty" => MergeState::Dirty,
        "blocked" => MergeState::Blocked,
        "behind" => MergeState::Behind,
        _ => MergeState::Unknown,
    }
}

fn map_review_decision(decision: &str) -> Option<ReviewDecision> {
    match decision.to_ascii_lowercase().as_str() {
        "approved" => Some(ReviewDecision::Approved),
        "changes_requested" => Some(ReviewDecision::ChangesRequested),
        "review_required" => Some(ReviewDecision::ReviewRequired),
        _ => None,
    }
}

fn checks_summary_from_graphql_rollup(rollup: GraphQlStatusCheckRollup) -> ChecksSummary {
    let mut summary = ChecksSummary::default();

    for context in rollup.contexts.nodes.into_iter().flatten() {
        summary.total += 1;
        match context.typename.as_deref() {
            Some("CheckRun") => match (
                context.status.as_deref().map(map_graphql_check_status),
                context
                    .conclusion
                    .as_deref()
                    .and_then(map_graphql_check_conclusion),
            ) {
                (Some(CheckStatus::Completed), Some(CheckConclusion::Success)) => {
                    summary.passed += 1;
                }
                (
                    Some(CheckStatus::Completed),
                    Some(CheckConclusion::Skipped | CheckConclusion::Neutral),
                ) => {
                    summary.skipped += 1;
                }
                (Some(CheckStatus::Completed), _) => {
                    summary.failed += 1;
                }
                (Some(CheckStatus::InProgress | CheckStatus::Queued), _) | (None, _) => {
                    summary.pending += 1;
                }
            },
            Some("StatusContext") => match context.state.as_deref() {
                Some("SUCCESS") => summary.passed += 1,
                Some("ERROR" | "FAILURE") => summary.failed += 1,
                _ => summary.pending += 1,
            },
            _ => summary.pending += 1,
        }
    }

    summary
}

fn map_graphql_check_status(status: &str) -> CheckStatus {
    match status {
        "COMPLETED" => CheckStatus::Completed,
        "IN_PROGRESS" => CheckStatus::InProgress,
        _ => CheckStatus::Queued,
    }
}

fn map_graphql_check_conclusion(conclusion: &str) -> Option<CheckConclusion> {
    match conclusion {
        "SUCCESS" => Some(CheckConclusion::Success),
        "FAILURE" | "STARTUP_FAILURE" | "STALE" => Some(CheckConclusion::Failure),
        "NEUTRAL" => Some(CheckConclusion::Neutral),
        "CANCELLED" => Some(CheckConclusion::Cancelled),
        "SKIPPED" => Some(CheckConclusion::Skipped),
        "TIMED_OUT" => Some(CheckConclusion::TimedOut),
        "ACTION_REQUIRED" => Some(CheckConclusion::ActionRequired),
        _ => None,
    }
}

fn map_file_status(status: &str) -> FileStatus {
    match status {
        "added" => FileStatus::Added,
        "modified" => FileStatus::Modified,
        "removed" => FileStatus::Removed,
        "renamed" => FileStatus::Renamed,
        "copied" => FileStatus::Copied,
        "changed" => FileStatus::Changed,
        "unchanged" => FileStatus::Unchanged,
        _ => FileStatus::Modified,
    }
}
