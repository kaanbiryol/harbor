use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{CheckConclusion, CheckStatus};

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

impl From<WorkflowStatus> for CheckStatus {
    fn from(status: WorkflowStatus) -> Self {
        match status {
            WorkflowStatus::Queued => Self::Queued,
            WorkflowStatus::InProgress => Self::InProgress,
            WorkflowStatus::Completed => Self::Completed,
        }
    }
}

impl From<WorkflowConclusion> for CheckConclusion {
    fn from(conclusion: WorkflowConclusion) -> Self {
        match conclusion {
            WorkflowConclusion::Success => Self::Success,
            WorkflowConclusion::Failure => Self::Failure,
            WorkflowConclusion::Cancelled => Self::Cancelled,
            WorkflowConclusion::Skipped => Self::Skipped,
            WorkflowConclusion::TimedOut => Self::TimedOut,
            WorkflowConclusion::ActionRequired => Self::ActionRequired,
        }
    }
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
    pub run_number: Option<u64>,
    pub run_attempt: Option<u64>,
    pub actor_login: Option<String>,
    pub run_started_at: Option<DateTime<Utc>>,
}

impl WorkflowRun {
    pub fn label(&self) -> &str {
        self.workflow_name.as_deref().unwrap_or(&self.name)
    }

    pub fn failed(&self) -> bool {
        matches!(
            (self.status, self.conclusion),
            (
                WorkflowStatus::Completed,
                Some(
                    WorkflowConclusion::Failure
                        | WorkflowConclusion::Cancelled
                        | WorkflowConclusion::TimedOut
                        | WorkflowConclusion::ActionRequired
                )
            )
        )
    }
}

pub fn workflow_outcome_label(
    status: WorkflowStatus,
    conclusion: Option<WorkflowConclusion>,
) -> &'static str {
    match (status, conclusion) {
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Success)) => "passed",
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Skipped)) => "skipped",
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Cancelled)) => "cancelled",
        (WorkflowStatus::Completed, Some(WorkflowConclusion::TimedOut)) => "timed out",
        (WorkflowStatus::Completed, Some(WorkflowConclusion::ActionRequired)) => "action required",
        (WorkflowStatus::Completed, Some(WorkflowConclusion::Failure) | None) => "failed",
        (WorkflowStatus::InProgress, _) => "running",
        (WorkflowStatus::Queued, _) => "queued",
    }
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
