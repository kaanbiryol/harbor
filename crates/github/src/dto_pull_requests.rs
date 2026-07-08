use chrono::{DateTime, Utc};
use harbor_domain::{
    CheckConclusion, CheckStatus, ChecksSummary, DiffFile, FileStatus, FileViewedState, Label,
    MergeState, PullRequest, PullRequestState, RepoId, ReviewDecision,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{GitHubError, PullRequestEnrichment, Result};

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
    assignees: Vec<ApiUser>,
    #[serde(default)]
    merged: Option<bool>,
    #[serde(default)]
    mergeable_state: Option<String>,
    #[serde(default)]
    created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    updated_at: Option<DateTime<Utc>>,
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
struct GraphQlPullRequestSearchCountResponse {
    data: Option<GraphQlPullRequestSearchCountData>,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestSearchCountData {
    search: GraphQlPullRequestSearchCount,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestEnrichmentResponse {
    data: Option<GraphQlPullRequestEnrichmentData>,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestEnrichmentData {
    #[serde(default)]
    nodes: Vec<Option<GraphQlPullRequestEnrichmentNode>>,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestFileViewedStatesResponse {
    data: Option<GraphQlPullRequestFileViewedStatesData>,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestFileViewedStatesData {
    repository: Option<GraphQlPullRequestFileViewedStatesRepository>,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestFileViewedStatesRepository {
    #[serde(rename = "pullRequest")]
    pull_request: Option<GraphQlPullRequestFileViewedStatesPullRequest>,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestFileViewedStatesPullRequest {
    files: GraphQlPullRequestFileViewedStatesConnection,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestFileViewedStatesConnection {
    #[serde(default)]
    nodes: Vec<Option<GraphQlPullRequestFileViewedStateNode>>,
    #[serde(default, rename = "pageInfo")]
    page_info: GraphQlPageInfo,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestFileViewedStateNode {
    path: String,
    #[serde(rename = "viewerViewedState")]
    viewer_viewed_state: String,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestSearchConnection {
    #[serde(default, rename = "issueCount")]
    issue_count: Option<usize>,
    #[serde(default)]
    nodes: Vec<Option<GraphQlPullRequestSearchNode>>,
    #[serde(default, rename = "pageInfo")]
    page_info: GraphQlPageInfo,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestSearchCount {
    #[serde(rename = "issueCount")]
    issue_count: usize,
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
    #[serde(default, rename = "createdAt")]
    created_at: Option<DateTime<Utc>>,
    #[serde(default, rename = "updatedAt")]
    updated_at: Option<DateTime<Utc>>,
    #[serde(default, rename = "reviewDecision")]
    review_decision: Option<String>,
    #[serde(default, rename = "mergeStateStatus")]
    merge_state_status: Option<String>,
    #[serde(default, rename = "statusCheckRollup")]
    status_check_rollup: Option<GraphQlStatusCheckRollup>,
    #[serde(default)]
    labels: GraphQlNodes<GraphQlLabel>,
    #[serde(default)]
    assignees: GraphQlNodes<ApiUser>,
}

#[derive(Debug, Deserialize)]
struct GraphQlPullRequestEnrichmentNode {
    #[serde(default, rename = "__typename")]
    typename: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default, rename = "reviewDecision")]
    review_decision: Option<String>,
    #[serde(default, rename = "mergeStateStatus")]
    merge_state_status: Option<String>,
    #[serde(default, rename = "statusCheckRollup")]
    status_check_rollup: Option<GraphQlStatusCheckRollup>,
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
    pub(crate) total_count: Option<usize>,
    pub(crate) has_next_page: bool,
    pub(crate) end_cursor: Option<String>,
}

pub(crate) struct PullRequestFileViewedStatesPage {
    pub(crate) file_states: Vec<PullRequestFileViewedState>,
    pub(crate) has_next_page: bool,
    pub(crate) end_cursor: Option<String>,
}

pub(crate) struct PullRequestFileViewedState {
    pub(crate) path: String,
    pub(crate) viewed_state: FileViewedState,
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
        total_count: data.search.issue_count,
        has_next_page: data.search.page_info.has_next_page,
        end_cursor: data.search.page_info.end_cursor,
    })
}

pub(crate) fn pull_request_search_count_from_graphql_value(value: Value) -> Result<usize> {
    let response: GraphQlPullRequestSearchCountResponse =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;
    let data = response
        .data
        .ok_or_else(|| GitHubError::Mapping("missing GraphQL response data".to_string()))?;

    Ok(data.search.issue_count)
}

pub(crate) fn pull_request_enrichments_from_graphql_value(
    value: Value,
) -> Result<Vec<PullRequestEnrichment>> {
    let response: GraphQlPullRequestEnrichmentResponse =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;
    let data = response
        .data
        .ok_or_else(|| GitHubError::Mapping("missing GraphQL response data".to_string()))?;

    data.nodes
        .into_iter()
        .flatten()
        .filter(|node| node.is_pull_request())
        .map(GraphQlPullRequestEnrichmentNode::into_domain)
        .collect()
}

pub(crate) fn pull_request_file_viewed_states_page_from_graphql_value(
    value: Value,
) -> Result<PullRequestFileViewedStatesPage> {
    let response: GraphQlPullRequestFileViewedStatesResponse =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;
    let data = response
        .data
        .ok_or_else(|| GitHubError::Mapping("missing GraphQL response data".to_string()))?;
    let repository = data
        .repository
        .ok_or_else(|| GitHubError::Mapping("missing GraphQL repository".to_string()))?;
    let pull_request = repository
        .pull_request
        .ok_or_else(|| GitHubError::Mapping("missing GraphQL pull request".to_string()))?;

    Ok(PullRequestFileViewedStatesPage {
        file_states: pull_request
            .files
            .nodes
            .into_iter()
            .flatten()
            .map(GraphQlPullRequestFileViewedStateNode::into_domain)
            .collect(),
        has_next_page: pull_request.files.page_info.has_next_page,
        end_cursor: pull_request.files.page_info.end_cursor,
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
            assignees: self
                .assignees
                .into_iter()
                .map(|assignee| assignee.login)
                .collect(),
            checks_summary: ChecksSummary::default(),
            unresolved_threads: 0,
            created_at: self.created_at,
            updated_at: self.updated_at,
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
            assignees: self
                .assignees
                .nodes
                .into_iter()
                .flatten()
                .map(|assignee| assignee.login)
                .collect(),
            checks_summary: self
                .status_check_rollup
                .map(checks_summary_from_graphql_rollup)
                .unwrap_or_default(),
            unresolved_threads: 0,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

impl GraphQlPullRequestEnrichmentNode {
    fn is_pull_request(&self) -> bool {
        self.typename.as_deref() == Some("PullRequest") || self.id.is_some()
    }

    fn into_domain(self) -> Result<PullRequestEnrichment> {
        Ok(PullRequestEnrichment {
            node_id: required_graphql_field(self.id, "id")?,
            review_decision: self
                .review_decision
                .as_deref()
                .and_then(map_review_decision),
            merge_state: self.merge_state_status.as_deref().map(map_merge_state),
            checks_summary: self
                .status_check_rollup
                .map(checks_summary_from_graphql_rollup)
                .unwrap_or_default(),
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
            viewed_state: FileViewedState::Unviewed,
        }
    }
}

impl GraphQlPullRequestFileViewedStateNode {
    fn into_domain(self) -> PullRequestFileViewedState {
        PullRequestFileViewedState {
            path: self.path,
            viewed_state: map_file_viewed_state(&self.viewer_viewed_state),
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

fn map_file_viewed_state(state: &str) -> FileViewedState {
    match state {
        "VIEWED" => FileViewedState::Viewed,
        "DISMISSED" => FileViewedState::ChangedSinceViewed,
        _ => FileViewedState::Unviewed,
    }
}

#[cfg(test)]
mod tests {
    use harbor_domain::{
        FileStatus, FileViewedState, MergeState, PullRequestState, RepoId, ReviewDecision,
    };
    use serde_json::json;

    use super::{
        diff_files_from_value, pull_request_file_viewed_states_page_from_graphql_value,
        pull_request_from_value, pull_request_search_count_from_graphql_value,
        pull_request_search_page_from_graphql_value, pull_requests_from_value,
    };

    #[test]
    fn maps_pull_request_list() {
        let value = json!([
            {
                "node_id": "pr-node-42",
                "number": 42,
                "title": "make list rendering fast",
                "body": "Use cached data first",
                "html_url": "https://github.com/acme/app/pull/42",
                "state": "open",
                "draft": false,
                "user": { "login": "octocat" },
                "head": { "ref": "feature/list", "sha": "abc123" },
                "base": { "ref": "main", "sha": "def456" },
                "labels": [{ "name": "performance", "color": "34d399" }],
                "assignees": [{ "login": "mona" }],
                "mergeable_state": "clean",
                "created_at": "2026-05-10T10:00:00Z"
            }
        ]);

        let pulls = pull_requests_from_value(RepoId::new("acme", "app"), value).unwrap();

        assert_eq!(pulls.len(), 1);
        assert_eq!(pulls[0].repo.full_name(), "acme/app");
        assert_eq!(pulls[0].node_id, "pr-node-42");
        assert_eq!(pulls[0].number, 42);
        assert_eq!(pulls[0].author, "octocat");
        assert_eq!(pulls[0].head_ref, "feature/list");
        assert_eq!(pulls[0].base_ref, "main");
        assert_eq!(pulls[0].state, PullRequestState::Open);
        assert_eq!(pulls[0].merge_state, Some(MergeState::Clean));
        assert_eq!(pulls[0].labels[0].name, "performance");
        assert_eq!(pulls[0].assignees, vec!["mona".to_string()]);
        assert_eq!(
            pulls[0].created_at.map(|time| time.to_rfc3339()),
            Some("2026-05-10T10:00:00+00:00".to_string())
        );
    }

    #[test]
    fn maps_pull_request_search_states() {
        let value = json!({
            "data": {
                "search": {
                    "pageInfo": {
                        "hasNextPage": false,
                        "endCursor": null
                    },
                    "nodes": [
                        {
                            "__typename": "PullRequest",
                            "id": "pr-node-42",
                            "number": 42,
                            "title": "make list rendering fast",
                            "body": "",
                            "url": "https://github.com/acme/app/pull/42",
                            "state": "OPEN",
                            "isDraft": false,
                            "author": { "login": "octocat" },
                            "repository": {
                                "name": "app",
                                "owner": { "login": "acme" }
                            },
                            "headRefName": "feature/list",
                            "baseRefName": "main",
                            "headRefOid": "abc123",
                            "createdAt": "2026-05-10T10:00:00Z",
                            "reviewDecision": "REVIEW_REQUIRED",
                            "mergeStateStatus": "CLEAN",
                            "statusCheckRollup": {
                                "contexts": {
                                    "nodes": [
                                        {
                                            "__typename": "CheckRun",
                                            "status": "COMPLETED",
                                            "conclusion": "SUCCESS"
                                        },
                                        {
                                            "__typename": "CheckRun",
                                            "status": "COMPLETED",
                                            "conclusion": "FAILURE"
                                        },
                                        {
                                            "__typename": "CheckRun",
                                            "status": "IN_PROGRESS",
                                            "conclusion": null
                                        },
                                        {
                                            "__typename": "StatusContext",
                                            "state": "SUCCESS"
                                        }
                                    ]
                                }
                            },
                            "labels": {
                                "nodes": [{ "name": "performance", "color": "34d399" }]
                            },
                            "assignees": {
                                "nodes": [{ "login": "mona" }]
                            }
                        },
                        {
                            "__typename": "PullRequest",
                            "id": "pr-node-43",
                            "number": 43,
                            "title": "close stale work",
                            "body": null,
                            "url": "https://github.com/acme/app/pull/43",
                            "state": "CLOSED",
                            "isDraft": false,
                            "author": { "login": "octocat" },
                            "repository": {
                                "name": "app",
                                "owner": { "login": "acme" }
                            },
                            "headRefName": "feature/stale",
                            "baseRefName": "main",
                            "headRefOid": "def456",
                            "reviewDecision": null,
                            "mergeStateStatus": "UNKNOWN",
                            "labels": {
                                "nodes": []
                            }
                        },
                        {
                            "__typename": "PullRequest",
                            "id": "pr-node-44",
                            "number": 44,
                            "title": "merge completed work",
                            "body": null,
                            "url": "https://github.com/acme/app/pull/44",
                            "state": "MERGED",
                            "isDraft": false,
                            "author": { "login": "octocat" },
                            "repository": {
                                "name": "app",
                                "owner": { "login": "acme" }
                            },
                            "headRefName": "feature/done",
                            "baseRefName": "main",
                            "headRefOid": "ghi789",
                            "reviewDecision": "APPROVED",
                            "mergeStateStatus": "CLEAN",
                            "labels": {
                                "nodes": []
                            }
                        }
                    ]
                }
            }
        });

        let page = pull_request_search_page_from_graphql_value(value).unwrap();

        assert_eq!(page.pull_requests.len(), 3);
        assert!(!page.has_next_page);
        assert_eq!(page.pull_requests[0].repo.full_name(), "acme/app");
        assert_eq!(page.pull_requests[0].node_id, "pr-node-42");
        assert_eq!(page.pull_requests[0].number, 42);
        assert_eq!(
            page.pull_requests[0].review_decision,
            Some(ReviewDecision::ReviewRequired)
        );
        assert_eq!(page.pull_requests[0].merge_state, Some(MergeState::Clean));
        assert_eq!(page.pull_requests[0].checks_summary.total, 4);
        assert_eq!(page.pull_requests[0].checks_summary.passed, 2);
        assert_eq!(page.pull_requests[0].checks_summary.failed, 1);
        assert_eq!(page.pull_requests[0].checks_summary.pending, 1);
        assert_eq!(page.pull_requests[0].labels[0].name, "performance");
        assert_eq!(page.pull_requests[0].assignees, vec!["mona".to_string()]);
        assert_eq!(
            page.pull_requests[0]
                .created_at
                .map(|time| time.to_rfc3339()),
            Some("2026-05-10T10:00:00+00:00".to_string())
        );
        assert_eq!(page.pull_requests[1].state, PullRequestState::Closed);
        assert_eq!(page.pull_requests[2].state, PullRequestState::Merged);
        assert_eq!(
            page.pull_requests[2].review_decision,
            Some(ReviewDecision::Approved)
        );
    }

    #[test]
    fn maps_pull_request_search_count() {
        let value = json!({
            "data": {
                "search": {
                    "issueCount": 17
                }
            }
        });

        let count = pull_request_search_count_from_graphql_value(value).unwrap();

        assert_eq!(count, 17);
    }

    #[test]
    fn maps_merged_pull_request() {
        let value = json!({
            "number": 9,
            "title": "merged pr",
            "body": null,
            "html_url": "https://github.com/acme/app/pull/9",
            "state": "closed",
            "draft": false,
            "user": null,
            "head": { "ref": "feature/done", "sha": "abc123" },
            "base": { "ref": "main", "sha": "def456" },
            "labels": [],
            "merged": true,
            "mergeable_state": "unknown"
        });

        let pull = pull_request_from_value(RepoId::new("acme", "app"), value).unwrap();

        assert_eq!(pull.state, PullRequestState::Merged);
        assert_eq!(pull.author, "ghost");
    }

    #[test]
    fn maps_pull_request_files_with_missing_patch() {
        let value = json!([
            {
                "filename": "src/app.rs",
                "status": "modified",
                "additions": 12,
                "deletions": 4,
                "changes": 16,
                "patch": "@@ -1 +1 @@"
            },
            {
                "filename": "assets/logo.png",
                "status": "renamed",
                "previous_filename": "assets/old-logo.png",
                "additions": 0,
                "deletions": 0,
                "changes": 0
            }
        ]);

        let files = diff_files_from_value(value).unwrap();

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].status, FileStatus::Modified);
        assert!(files[0].patch.is_some());
        assert_eq!(files[1].status, FileStatus::Renamed);
        assert_eq!(
            files[1].previous_path.as_deref(),
            Some("assets/old-logo.png")
        );
        assert!(files[1].patch.is_none());
        assert_eq!(files[1].viewed_state, FileViewedState::Unviewed);
    }

    #[test]
    fn maps_pull_request_file_viewed_states_from_graphql() {
        let value = json!({
            "data": {
                "repository": {
                    "pullRequest": {
                        "id": "pr-node",
                        "files": {
                            "pageInfo": {
                                "hasNextPage": true,
                                "endCursor": "cursor-1"
                            },
                            "nodes": [
                                {
                                    "path": "src/lib.rs",
                                    "viewerViewedState": "VIEWED"
                                },
                                {
                                    "path": "src/new.rs",
                                    "viewerViewedState": "DISMISSED"
                                }
                            ]
                        }
                    }
                }
            }
        });

        let page = pull_request_file_viewed_states_page_from_graphql_value(value).unwrap();

        assert!(page.has_next_page);
        assert_eq!(page.end_cursor.as_deref(), Some("cursor-1"));
        assert_eq!(page.file_states.len(), 2);
        assert_eq!(page.file_states[0].path, "src/lib.rs");
        assert_eq!(page.file_states[0].viewed_state, FileViewedState::Viewed);
        assert_eq!(page.file_states[1].path, "src/new.rs");
        assert_eq!(
            page.file_states[1].viewed_state,
            FileViewedState::ChangedSinceViewed
        );
    }
}
