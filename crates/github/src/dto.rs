use harbor_domain::{
    ChecksSummary, DiffFile, FileStatus, Label, MergeState, PullRequest, PullRequestState, RepoId,
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

#[cfg(test)]
mod tests {
    use harbor_domain::{FileStatus, MergeState, PullRequestState};
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
}
