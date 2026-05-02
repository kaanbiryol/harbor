use harbor_domain::{
    CheckConclusion, CheckRun, CheckStatus, ChecksSummary, DiffFile, FileStatus, Label, MergeState,
    PullRequest, PullRequestState, RepoId, WorkflowConclusion, WorkflowRun, WorkflowStatus,
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

fn map_pull_request_state(state: &str, merged: Option<bool>) -> PullRequestState {
    if merged.unwrap_or(false) {
        PullRequestState::Merged
    } else if state.eq_ignore_ascii_case("closed") {
        PullRequestState::Closed
    } else {
        PullRequestState::Open
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
        CheckConclusion, CheckStatus, FileStatus, MergeState, PullRequestState, WorkflowConclusion,
        WorkflowStatus,
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
        assert_eq!(workflow_runs[0].name, "run tests");
        assert_eq!(workflow_runs[0].workflow_name.as_deref(), Some("CI"));
        assert_eq!(workflow_runs[0].status, WorkflowStatus::Completed);
        assert_eq!(
            workflow_runs[0].conclusion,
            Some(WorkflowConclusion::Failure)
        );
    }
}
