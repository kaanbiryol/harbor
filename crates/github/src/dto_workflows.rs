use harbor_domain::{WorkflowConclusion, WorkflowJob, WorkflowRun, WorkflowStatus, WorkflowStep};
use serde::Deserialize;
use serde_json::Value;

use crate::{GitHubError, Result};

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
