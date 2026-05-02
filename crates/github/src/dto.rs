use harbor_domain::{
    CheckConclusion, CheckRun, CheckStatus, ChecksSummary, DiffFile, FileStatus, Label, MergeState,
    PullRequest, PullRequestReview, PullRequestReviewState, PullRequestState, RepoId,
    ReviewComment, ReviewCommentPosition, ReviewSide, ReviewThread, ReviewThreadState,
    WorkflowConclusion, WorkflowJob, WorkflowRun, WorkflowStatus, WorkflowStep,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{GitHubError, Result};

#[derive(Debug, Deserialize)]
struct ApiPullRequest {
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

#[derive(Debug, Deserialize)]
struct ApiCheckRunsResponse {
    #[serde(default)]
    check_runs: Vec<ApiCheckRun>,
}

#[derive(Debug, Deserialize)]
struct ApiCheckRun {
    id: Option<u64>,
    name: String,
    status: String,
    conclusion: Option<String>,
    #[serde(default)]
    details_url: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
struct ApiWorkflowRunsResponse {
    #[serde(default)]
    workflow_runs: Vec<ApiWorkflowRun>,
}

#[derive(Debug, Deserialize)]
struct ApiWorkflowRun {
    id: u64,
    #[serde(default)]
    workflow_id: Option<u64>,
    name: String,
    #[serde(default)]
    display_title: Option<String>,
    status: String,
    conclusion: Option<String>,
    head_branch: String,
    head_sha: String,
    event: String,
    url: String,
    html_url: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
struct ApiWorkflowJobsResponse {
    #[serde(default)]
    jobs: Vec<ApiWorkflowJob>,
}

#[derive(Debug, Deserialize)]
struct ApiWorkflowJob {
    id: u64,
    name: String,
    status: String,
    conclusion: Option<String>,
    #[serde(default)]
    steps: Vec<ApiWorkflowStep>,
}

#[derive(Debug, Deserialize)]
struct ApiWorkflowStep {
    name: String,
    number: u32,
    status: String,
    conclusion: Option<String>,
    #[serde(default)]
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
struct ApiPullRequestReview {
    id: u64,
    state: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    user: Option<ApiUser>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewThreadsResponse {
    data: Option<GraphQlReviewThreadsData>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewThreadsData {
    repository: Option<GraphQlReviewThreadsRepository>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewThreadsRepository {
    #[serde(rename = "pullRequest")]
    pull_request: Option<GraphQlReviewThreadsPullRequest>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewThreadsPullRequest {
    #[serde(rename = "reviewThreads")]
    review_threads: GraphQlNodes<GraphQlReviewThread>,
}

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct GraphQlNodes<T> {
    #[serde(default)]
    nodes: Vec<Option<T>>,
}

impl<T> Default for GraphQlNodes<T> {
    fn default() -> Self {
        Self { nodes: Vec::new() }
    }
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewThread {
    id: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default, rename = "line")]
    line: Option<u32>,
    #[serde(default, rename = "originalLine")]
    original_line: Option<u32>,
    #[serde(default, rename = "isResolved")]
    is_resolved: bool,
    #[serde(default, rename = "isOutdated")]
    is_outdated: bool,
    comments: GraphQlNodes<GraphQlReviewComment>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewComment {
    id: String,
    body: String,
    author: Option<ApiUser>,
    #[serde(rename = "createdAt")]
    created_at: chrono::DateTime<chrono::Utc>,
    #[serde(default, rename = "updatedAt")]
    updated_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    line: Option<u32>,
    #[serde(default, rename = "originalLine")]
    original_line: Option<u32>,
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

pub fn diff_files_from_value(value: Value) -> Result<Vec<DiffFile>> {
    let files: Vec<ApiDiffFile> =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;

    Ok(files.into_iter().map(ApiDiffFile::into_domain).collect())
}

pub fn check_runs_from_value(value: Value) -> Result<Vec<CheckRun>> {
    let response: ApiCheckRunsResponse =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;

    Ok(response
        .check_runs
        .into_iter()
        .map(ApiCheckRun::into_domain)
        .collect())
}

pub fn workflow_runs_from_value(value: Value) -> Result<Vec<WorkflowRun>> {
    let response: ApiWorkflowRunsResponse =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;

    Ok(response
        .workflow_runs
        .into_iter()
        .map(ApiWorkflowRun::into_domain)
        .collect())
}

pub fn workflow_jobs_from_value(value: Value) -> Result<Vec<WorkflowJob>> {
    let response: ApiWorkflowJobsResponse =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;

    Ok(response
        .jobs
        .into_iter()
        .map(ApiWorkflowJob::into_domain)
        .collect())
}

pub fn pull_request_reviews_from_value(value: Value) -> Result<Vec<PullRequestReview>> {
    let reviews: Vec<ApiPullRequestReview> =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;

    Ok(reviews
        .into_iter()
        .map(ApiPullRequestReview::into_domain)
        .collect())
}

pub fn review_threads_from_graphql_value(value: Value) -> Result<Vec<ReviewThread>> {
    let response: GraphQlReviewThreadsResponse =
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

    Ok(pull_request
        .review_threads
        .nodes
        .into_iter()
        .flatten()
        .map(GraphQlReviewThread::into_domain)
        .collect())
}

impl ApiPullRequest {
    fn into_domain(self, repo: RepoId) -> PullRequest {
        PullRequest {
            repo,
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

impl ApiCheckRun {
    fn into_domain(self) -> CheckRun {
        CheckRun {
            id: self.id,
            name: self.name,
            status: map_check_status(&self.status),
            conclusion: self.conclusion.as_deref().and_then(map_check_conclusion),
            details_url: self.details_url,
            html_url: self.html_url,
            started_at: self.started_at,
            completed_at: self.completed_at,
        }
    }
}

impl ApiWorkflowRun {
    fn into_domain(self) -> WorkflowRun {
        WorkflowRun {
            id: self.id,
            workflow_id: self.workflow_id,
            name: self.display_title.unwrap_or_else(|| self.name.clone()),
            workflow_name: Some(self.name),
            status: map_workflow_status(&self.status),
            conclusion: self.conclusion.as_deref().and_then(map_workflow_conclusion),
            head_branch: self.head_branch,
            head_sha: self.head_sha,
            event: self.event,
            url: self.url,
            html_url: self.html_url,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl ApiWorkflowJob {
    fn into_domain(self) -> WorkflowJob {
        WorkflowJob {
            id: self.id,
            name: self.name,
            status: map_workflow_status(&self.status),
            conclusion: self.conclusion.as_deref().and_then(map_workflow_conclusion),
            steps: self
                .steps
                .into_iter()
                .map(ApiWorkflowStep::into_domain)
                .collect(),
        }
    }
}

impl ApiWorkflowStep {
    fn into_domain(self) -> WorkflowStep {
        WorkflowStep {
            name: self.name,
            number: self.number,
            status: map_workflow_status(&self.status),
            conclusion: self.conclusion.as_deref().and_then(map_workflow_conclusion),
            started_at: self.started_at,
            completed_at: self.completed_at,
        }
    }
}

impl ApiPullRequestReview {
    fn into_domain(self) -> PullRequestReview {
        PullRequestReview {
            id: self.id.to_string(),
            author: self
                .user
                .map(|user| user.login)
                .unwrap_or_else(|| "ghost".to_string()),
            state: map_pull_request_review_state(&self.state),
            body: self.body.filter(|body| !body.is_empty()),
            submitted_at: self.submitted_at,
        }
    }
}

impl GraphQlReviewThread {
    fn into_domain(self) -> ReviewThread {
        let fallback_path = self.path.unwrap_or_default();
        let fallback_line = self.line;
        let fallback_original_line = self.original_line;
        let comments: Vec<ReviewComment> = self
            .comments
            .nodes
            .into_iter()
            .flatten()
            .map(|comment| {
                comment.into_domain(fallback_path.clone(), fallback_line, fallback_original_line)
            })
            .collect();
        let path = if fallback_path.is_empty() {
            comments
                .iter()
                .find_map(|comment| comment.position.as_ref())
                .map(|position| position.path.clone())
                .unwrap_or_else(|| "unknown".to_string())
        } else {
            fallback_path
        };

        ReviewThread {
            id: self.id,
            path,
            state: map_review_thread_state(self.is_resolved, self.is_outdated),
            comments,
        }
    }
}

impl GraphQlReviewComment {
    fn into_domain(
        self,
        fallback_path: String,
        fallback_line: Option<u32>,
        fallback_original_line: Option<u32>,
    ) -> ReviewComment {
        let path = self.path.unwrap_or(fallback_path);
        let line = self.line.or(fallback_line);
        let original_line = self.original_line.or(fallback_original_line);
        let position = if path.is_empty() && line.is_none() && original_line.is_none() {
            None
        } else {
            Some(ReviewCommentPosition {
                path,
                line,
                original_line,
                side: ReviewSide::Right,
            })
        };

        ReviewComment {
            id: self.id,
            author: self
                .author
                .map(|user| user.login)
                .unwrap_or_else(|| "ghost".to_string()),
            body: self.body,
            created_at: self.created_at,
            updated_at: self.updated_at,
            position,
        }
    }
}

fn map_pull_request_state(state: &str, merged: Option<bool>) -> PullRequestState {
    if merged.unwrap_or(false) {
        PullRequestState::Merged
    } else if state.eq_ignore_ascii_case("closed") {
        PullRequestState::Closed
    } else {
        PullRequestState::Open
    }
}

fn map_pull_request_review_state(state: &str) -> PullRequestReviewState {
    match state.to_ascii_lowercase().as_str() {
        "pending" => PullRequestReviewState::Pending,
        "approved" => PullRequestReviewState::Approved,
        "changes_requested" => PullRequestReviewState::ChangesRequested,
        "dismissed" => PullRequestReviewState::Dismissed,
        _ => PullRequestReviewState::Commented,
    }
}

fn map_review_thread_state(is_resolved: bool, is_outdated: bool) -> ReviewThreadState {
    if is_resolved {
        ReviewThreadState::Resolved
    } else if is_outdated {
        ReviewThreadState::Outdated
    } else {
        ReviewThreadState::Unresolved
    }
}

fn map_merge_state(state: &str) -> MergeState {
    match state {
        "clean" | "unstable" | "has_hooks" => MergeState::Clean,
        "dirty" => MergeState::Dirty,
        "blocked" => MergeState::Blocked,
        "behind" => MergeState::Behind,
        _ => MergeState::Unknown,
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

fn map_check_status(status: &str) -> CheckStatus {
    match status {
        "completed" => CheckStatus::Completed,
        "in_progress" => CheckStatus::InProgress,
        _ => CheckStatus::Queued,
    }
}

fn map_check_conclusion(conclusion: &str) -> Option<CheckConclusion> {
    match conclusion {
        "success" => Some(CheckConclusion::Success),
        "failure" | "startup_failure" => Some(CheckConclusion::Failure),
        "neutral" => Some(CheckConclusion::Neutral),
        "cancelled" => Some(CheckConclusion::Cancelled),
        "skipped" => Some(CheckConclusion::Skipped),
        "timed_out" => Some(CheckConclusion::TimedOut),
        "action_required" => Some(CheckConclusion::ActionRequired),
        _ => None,
    }
}

fn map_workflow_status(status: &str) -> WorkflowStatus {
    match status {
        "completed" => WorkflowStatus::Completed,
        "in_progress" => WorkflowStatus::InProgress,
        _ => WorkflowStatus::Queued,
    }
}

fn map_workflow_conclusion(conclusion: &str) -> Option<WorkflowConclusion> {
    match conclusion {
        "success" => Some(WorkflowConclusion::Success),
        "failure" | "startup_failure" => Some(WorkflowConclusion::Failure),
        "cancelled" => Some(WorkflowConclusion::Cancelled),
        "skipped" => Some(WorkflowConclusion::Skipped),
        "timed_out" => Some(WorkflowConclusion::TimedOut),
        "action_required" => Some(WorkflowConclusion::ActionRequired),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use harbor_domain::{
        CheckConclusion, CheckStatus, FileStatus, MergeState, PullRequestReviewState,
        PullRequestState, ReviewThreadState, WorkflowConclusion, WorkflowStatus,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn maps_pull_request_list() {
        let value = json!([
            {
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
                "mergeable_state": "clean"
            }
        ]);

        let pulls = pull_requests_from_value(RepoId::new("acme", "app"), value).unwrap();

        assert_eq!(pulls.len(), 1);
        assert_eq!(pulls[0].repo.full_name(), "acme/app");
        assert_eq!(pulls[0].number, 42);
        assert_eq!(pulls[0].author, "octocat");
        assert_eq!(pulls[0].head_ref, "feature/list");
        assert_eq!(pulls[0].base_ref, "main");
        assert_eq!(pulls[0].state, PullRequestState::Open);
        assert_eq!(pulls[0].merge_state, Some(MergeState::Clean));
        assert_eq!(pulls[0].labels[0].name, "performance");
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
    }

    #[test]
    fn maps_check_runs() {
        let value = json!({
            "total_count": 2,
            "check_runs": [
                {
                    "id": 1001,
                    "name": "build",
                    "status": "completed",
                    "conclusion": "success",
                    "details_url": "https://ci.example/build",
                    "html_url": "https://github.com/acme/app/runs/1001",
                    "started_at": "2026-05-01T10:00:00Z",
                    "completed_at": "2026-05-01T10:05:00Z"
                },
                {
                    "id": 1002,
                    "name": "test",
                    "status": "in_progress",
                    "conclusion": null,
                    "details_url": null,
                    "html_url": "https://github.com/acme/app/runs/1002",
                    "started_at": null,
                    "completed_at": null
                }
            ]
        });

        let check_runs = check_runs_from_value(value).unwrap();

        assert_eq!(check_runs.len(), 2);
        assert_eq!(check_runs[0].status, CheckStatus::Completed);
        assert_eq!(check_runs[0].conclusion, Some(CheckConclusion::Success));
        assert_eq!(check_runs[1].status, CheckStatus::InProgress);
        assert_eq!(check_runs[1].conclusion, None);
    }

    #[test]
    fn maps_workflow_runs() {
        let value = json!({
            "total_count": 1,
            "workflow_runs": [
                {
                    "id": 2001,
                    "workflow_id": 901,
                    "name": "CI",
                    "display_title": "run tests",
                    "status": "completed",
                    "conclusion": "failure",
                    "head_branch": "feature/test",
                    "head_sha": "abc123",
                    "event": "pull_request",
                    "url": "https://api.github.com/repos/acme/app/actions/runs/2001",
                    "html_url": "https://github.com/acme/app/actions/runs/2001",
                    "created_at": "2026-05-01T10:00:00Z",
                    "updated_at": "2026-05-01T10:05:00Z"
                }
            ]
        });

        let workflow_runs = workflow_runs_from_value(value).unwrap();

        assert_eq!(workflow_runs.len(), 1);
        assert_eq!(workflow_runs[0].workflow_id, Some(901));
        assert_eq!(workflow_runs[0].name, "run tests");
        assert_eq!(workflow_runs[0].workflow_name.as_deref(), Some("CI"));
        assert_eq!(workflow_runs[0].status, WorkflowStatus::Completed);
        assert_eq!(
            workflow_runs[0].conclusion,
            Some(WorkflowConclusion::Failure)
        );
    }

    #[test]
    fn maps_workflow_jobs() {
        let value = json!({
            "total_count": 1,
            "jobs": [
                {
                    "id": 3001,
                    "name": "test",
                    "status": "completed",
                    "conclusion": "failure",
                    "steps": [
                        {
                            "name": "install",
                            "number": 1,
                            "status": "completed",
                            "conclusion": "success",
                            "started_at": "2026-05-01T10:00:00Z",
                            "completed_at": "2026-05-01T10:01:00Z"
                        },
                        {
                            "name": "unit tests",
                            "number": 2,
                            "status": "completed",
                            "conclusion": "failure",
                            "started_at": null,
                            "completed_at": null
                        }
                    ]
                }
            ]
        });

        let jobs = workflow_jobs_from_value(value).unwrap();

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, 3001);
        assert_eq!(jobs[0].status, WorkflowStatus::Completed);
        assert_eq!(jobs[0].conclusion, Some(WorkflowConclusion::Failure));
        assert_eq!(jobs[0].steps.len(), 2);
        assert_eq!(jobs[0].steps[1].name, "unit tests");
        assert_eq!(
            jobs[0].steps[1].conclusion,
            Some(WorkflowConclusion::Failure)
        );
    }

    #[test]
    fn maps_pull_request_reviews() {
        let value = json!([
            {
                "id": 401,
                "state": "APPROVED",
                "body": "ship it",
                "submitted_at": "2026-05-01T11:00:00Z",
                "user": { "login": "octocat" }
            },
            {
                "id": 402,
                "state": "CHANGES_REQUESTED",
                "body": "",
                "submitted_at": null,
                "user": null
            }
        ]);

        let reviews = pull_request_reviews_from_value(value).unwrap();

        assert_eq!(reviews.len(), 2);
        assert_eq!(reviews[0].id, "401");
        assert_eq!(reviews[0].author, "octocat");
        assert_eq!(reviews[0].state, PullRequestReviewState::Approved);
        assert_eq!(reviews[0].body.as_deref(), Some("ship it"));
        assert_eq!(reviews[1].author, "ghost");
        assert_eq!(reviews[1].state, PullRequestReviewState::ChangesRequested);
        assert_eq!(reviews[1].body, None);
    }

    #[test]
    fn maps_review_threads_from_graphql() {
        let value = json!({
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "nodes": [
                                {
                                    "id": "thread-1",
                                    "path": "src/app.rs",
                                    "line": 42,
                                    "originalLine": 40,
                                    "isResolved": false,
                                    "isOutdated": false,
                                    "comments": {
                                        "nodes": [
                                            {
                                                "id": "comment-1",
                                                "body": "This can be cheaper.",
                                                "author": { "login": "reviewer" },
                                                "createdAt": "2026-05-01T10:00:00Z",
                                                "updatedAt": "2026-05-01T10:05:00Z",
                                                "path": "src/app.rs",
                                                "line": 42,
                                                "originalLine": 40
                                            },
                                            {
                                                "id": "comment-2",
                                                "body": "Updated.",
                                                "author": null,
                                                "createdAt": "2026-05-01T10:10:00Z",
                                                "updatedAt": null,
                                                "path": null,
                                                "line": null,
                                                "originalLine": null
                                            }
                                        ]
                                    }
                                },
                                {
                                    "id": "thread-2",
                                    "path": "src/old.rs",
                                    "line": null,
                                    "originalLine": 9,
                                    "isResolved": false,
                                    "isOutdated": true,
                                    "comments": { "nodes": [] }
                                }
                            ]
                        }
                    }
                }
            }
        });

        let threads = review_threads_from_graphql_value(value).unwrap();

        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].id, "thread-1");
        assert_eq!(threads[0].path, "src/app.rs");
        assert_eq!(threads[0].state, ReviewThreadState::Unresolved);
        assert_eq!(threads[0].comments.len(), 2);
        assert_eq!(threads[0].comments[0].author, "reviewer");
        assert_eq!(
            threads[0].comments[0]
                .position
                .as_ref()
                .map(|position| position.line),
            Some(Some(42))
        );
        assert_eq!(threads[0].comments[1].author, "ghost");
        assert_eq!(threads[1].state, ReviewThreadState::Outdated);
    }
}
