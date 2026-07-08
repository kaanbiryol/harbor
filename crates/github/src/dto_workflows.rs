use harbor_domain::{
    Workflow, WorkflowConclusion, WorkflowJob, WorkflowRun, WorkflowState, WorkflowStatus,
    WorkflowStep,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{GitHubError, Result};

#[derive(Debug, Deserialize)]
struct ApiWorkflowsResponse {
    #[serde(default)]
    workflows: Vec<ApiWorkflow>,
}

#[derive(Debug, Deserialize)]
struct ApiWorkflow {
    id: u64,
    name: String,
    path: String,
    state: String,
    html_url: String,
    #[serde(default)]
    badge_url: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
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
    #[serde(default)]
    run_number: Option<u64>,
    #[serde(default)]
    run_attempt: Option<u64>,
    name: String,
    #[serde(default)]
    display_title: Option<String>,
    status: String,
    conclusion: Option<String>,
    #[serde(default)]
    head_branch: Option<String>,
    head_sha: String,
    event: String,
    url: String,
    html_url: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    run_started_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    actor: Option<ApiWorkflowActor>,
}

#[derive(Debug, Deserialize)]
struct ApiWorkflowActor {
    login: String,
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

pub fn workflows_from_value(value: Value) -> Result<Vec<Workflow>> {
    let response: ApiWorkflowsResponse =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;

    Ok(response
        .workflows
        .into_iter()
        .map(ApiWorkflow::into_domain)
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

impl ApiWorkflow {
    fn into_domain(self) -> Workflow {
        Workflow {
            id: self.id,
            name: self.name,
            path: self.path,
            state: map_workflow_state(&self.state),
            html_url: self.html_url,
            badge_url: self.badge_url,
            created_at: self.created_at,
            updated_at: self.updated_at,
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
            head_branch: self.head_branch.unwrap_or_default(),
            head_sha: self.head_sha,
            event: self.event,
            url: self.url,
            html_url: self.html_url,
            created_at: self.created_at,
            updated_at: self.updated_at,
            run_number: self.run_number,
            run_attempt: self.run_attempt,
            actor_login: self.actor.map(|actor| actor.login),
            run_started_at: self.run_started_at,
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

fn map_workflow_state(state: &str) -> WorkflowState {
    match state {
        "active" => WorkflowState::Active,
        "disabled_manually" => WorkflowState::DisabledManually,
        "disabled_inactivity" => WorkflowState::DisabledInactivity,
        "disabled_fork" => WorkflowState::DisabledFork,
        "deleted" => WorkflowState::Deleted,
        _ => WorkflowState::Unknown(state.to_string()),
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
    use harbor_domain::{WorkflowConclusion, WorkflowState, WorkflowStatus};
    use serde_json::json;

    use super::{workflow_jobs_from_value, workflow_runs_from_value, workflows_from_value};

    #[test]
    fn maps_workflows() {
        let value = json!({
            "total_count": 1,
            "workflows": [
                {
                    "id": 901,
                    "name": "CI",
                    "path": ".github/workflows/ci.yml",
                    "state": "active",
                    "html_url": "https://github.com/acme/app/blob/main/.github/workflows/ci.yml",
                    "badge_url": "https://github.com/acme/app/workflows/CI/badge.svg",
                    "created_at": "2026-05-01T10:00:00Z",
                    "updated_at": "2026-05-02T10:00:00Z"
                }
            ]
        });

        let workflows = workflows_from_value(value).unwrap();

        assert_eq!(workflows.len(), 1);
        assert_eq!(workflows[0].id, 901);
        assert_eq!(workflows[0].name, "CI");
        assert_eq!(workflows[0].state, WorkflowState::Active);
        assert_eq!(
            workflows[0].badge_url.as_deref(),
            Some("https://github.com/acme/app/workflows/CI/badge.svg")
        );
    }

    #[test]
    fn maps_workflow_runs() {
        let value = json!({
            "total_count": 1,
            "workflow_runs": [
                {
                    "id": 2001,
                    "workflow_id": 901,
                    "run_number": 33,
                    "run_attempt": 2,
                    "name": "CI",
                    "display_title": "run tests",
                    "status": "completed",
                    "conclusion": "failure",
                    "head_branch": "feature/test",
                    "head_sha": "abc123",
                    "event": "pull_request",
                    "url": "https://api.github.com/repos/acme/app/actions/runs/2001",
                    "html_url": "https://github.com/acme/app/actions/runs/2001",
                    "actor": { "login": "octocat" },
                    "run_started_at": "2026-05-01T10:01:00Z",
                    "created_at": "2026-05-01T10:00:00Z",
                    "updated_at": "2026-05-01T10:05:00Z"
                }
            ]
        });

        let workflow_runs = workflow_runs_from_value(value).unwrap();

        assert_eq!(workflow_runs.len(), 1);
        assert_eq!(workflow_runs[0].workflow_id, Some(901));
        assert_eq!(workflow_runs[0].run_number, Some(33));
        assert_eq!(workflow_runs[0].run_attempt, Some(2));
        assert_eq!(workflow_runs[0].name, "run tests");
        assert_eq!(workflow_runs[0].workflow_name.as_deref(), Some("CI"));
        assert_eq!(workflow_runs[0].actor_login.as_deref(), Some("octocat"));
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
}
