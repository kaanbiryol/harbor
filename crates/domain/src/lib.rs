use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub mod diff;
pub mod diff_reviews;
pub mod reviews;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct RepoId {
    pub owner: String,
    pub name: String,
}

impl RepoId {
    pub fn new(owner: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            owner: owner.into(),
            name: name.into(),
        }
    }

    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Repository {
    pub id: RepoId,
    pub default_branch: String,
    pub private: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PullRequestPerson {
    pub login: String,
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PullRequestCommit {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub author_avatar_url: Option<String>,
    pub authored_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PullRequestTeam {
    pub name: String,
    pub slug: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PullRequestState {
    Open,
    Closed,
    Merged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ReviewDecision {
    Approved,
    ChangesRequested,
    ReviewRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MergeState {
    Clean,
    Dirty,
    Blocked,
    Behind,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

impl MergeMethod {
    pub fn label(self) -> &'static str {
        match self {
            Self::Merge => "Create a merge commit",
            Self::Squash => "Squash and merge",
            Self::Rebase => "Rebase and merge",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChecksSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub pending: usize,
    pub skipped: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PullRequest {
    pub repo: RepoId,
    pub node_id: String,
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub author: String,
    pub url: String,
    pub state: PullRequestState,
    pub is_draft: bool,
    pub head_ref: String,
    pub base_ref: String,
    pub head_sha: String,
    pub review_decision: Option<ReviewDecision>,
    pub merge_state: Option<MergeState>,
    pub labels: Vec<Label>,
    #[serde(default)]
    pub assignees: Vec<PullRequestPerson>,
    #[serde(default)]
    pub requested_reviewers: Vec<PullRequestPerson>,
    #[serde(default)]
    pub requested_teams: Vec<PullRequestTeam>,
    pub checks_summary: ChecksSummary,
    pub unresolved_threads: usize,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PullRequestComment {
    pub id: String,
    pub author: String,
    pub author_avatar_url: Option<String>,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FileStatus {
    Added,
    Modified,
    Removed,
    Renamed,
    Copied,
    Changed,
    Unchanged,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum FileViewedState {
    ChangedSinceViewed,
    #[default]
    Unviewed,
    Viewed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiffFile {
    pub path: String,
    pub previous_path: Option<String>,
    pub status: FileStatus,
    pub additions: u32,
    pub deletions: u32,
    pub changes: u32,
    pub patch: Option<String>,
    #[serde(default)]
    pub viewed_state: FileViewedState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ReviewThreadState {
    Resolved,
    Unresolved,
    Outdated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ReviewSide {
    Left,
    Right,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReviewCommentPosition {
    pub path: String,
    pub line: Option<u32>,
    pub original_line: Option<u32>,
    pub side: ReviewSide,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReviewCommentRange {
    pub path: String,
    pub line: u32,
    pub side: ReviewSide,
    pub start_line: Option<u32>,
    pub start_side: Option<ReviewSide>,
}

impl ReviewCommentRange {
    pub fn is_single_line(&self) -> bool {
        self.start_line.is_none() && self.start_side.is_none()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum ReactionContent {
    ThumbsUp,
    ThumbsDown,
    Laugh,
    Confused,
    Heart,
    Hooray,
    Rocket,
    Eyes,
}

impl ReactionContent {
    pub const ALL: [Self; 8] = [
        Self::ThumbsUp,
        Self::ThumbsDown,
        Self::Laugh,
        Self::Confused,
        Self::Heart,
        Self::Hooray,
        Self::Rocket,
        Self::Eyes,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ThumbsUp => "+1",
            Self::ThumbsDown => "-1",
            Self::Laugh => "laugh",
            Self::Confused => "confused",
            Self::Heart => "heart",
            Self::Hooray => "hooray",
            Self::Rocket => "rocket",
            Self::Eyes => "eyes",
        }
    }

    pub fn graphql_name(self) -> &'static str {
        match self {
            Self::ThumbsUp => "THUMBS_UP",
            Self::ThumbsDown => "THUMBS_DOWN",
            Self::Laugh => "LAUGH",
            Self::Confused => "CONFUSED",
            Self::Heart => "HEART",
            Self::Hooray => "HOORAY",
            Self::Rocket => "ROCKET",
            Self::Eyes => "EYES",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReviewReaction {
    pub content: ReactionContent,
    pub count: usize,
    pub viewer_has_reacted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReviewComment {
    pub id: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub pull_request_review_id: Option<String>,
    #[serde(default)]
    pub pull_request_review_node_id: Option<String>,
    pub author: String,
    pub author_avatar_url: Option<String>,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub position: Option<ReviewCommentPosition>,
    pub viewer_did_author: bool,
    pub viewer_can_update: bool,
    pub viewer_can_delete: bool,
    pub viewer_can_react: bool,
    pub reactions: Vec<ReviewReaction>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReviewThread {
    pub id: String,
    pub path: String,
    pub range: Option<ReviewCommentRange>,
    pub state: ReviewThreadState,
    pub comments: Vec<ReviewComment>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PullRequestReviewState {
    Pending,
    Commented,
    Approved,
    ChangesRequested,
    Dismissed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PullRequestReview {
    pub id: String,
    pub node_id: Option<String>,
    pub author: String,
    pub state: PullRequestReviewState,
    pub body: Option<String>,
    pub submitted_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CheckStatus {
    Queued,
    InProgress,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CheckConclusion {
    Success,
    Failure,
    Neutral,
    Cancelled,
    Skipped,
    TimedOut,
    ActionRequired,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CheckRun {
    pub id: Option<u64>,
    pub name: String,
    pub status: CheckStatus,
    pub conclusion: Option<CheckConclusion>,
    pub details_url: Option<String>,
    pub html_url: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WorkflowStatus {
    Queued,
    InProgress,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WorkflowConclusion {
    Success,
    Failure,
    Cancelled,
    Skipped,
    TimedOut,
    ActionRequired,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WorkflowState {
    Active,
    DisabledManually,
    DisabledInactivity,
    DisabledFork,
    Deleted,
    Unknown(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub state: WorkflowState,
    pub html_url: String,
    pub badge_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: u64,
    pub workflow_id: Option<u64>,
    pub name: String,
    pub workflow_name: Option<String>,
    pub status: WorkflowStatus,
    pub conclusion: Option<WorkflowConclusion>,
    pub head_branch: String,
    pub head_sha: String,
    pub event: String,
    pub url: String,
    pub html_url: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub run_number: Option<u64>,
    #[serde(default)]
    pub run_attempt: Option<u64>,
    #[serde(default)]
    pub actor_login: Option<String>,
    #[serde(default)]
    pub run_started_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowJob {
    pub id: u64,
    pub name: String,
    pub status: WorkflowStatus,
    pub conclusion: Option<WorkflowConclusion>,
    pub steps: Vec<WorkflowStep>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub name: String,
    pub number: u32,
    pub status: WorkflowStatus,
    pub conclusion: Option<WorkflowConclusion>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}
