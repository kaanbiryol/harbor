use harbor_domain::{CheckConclusion, CheckRun, CheckStatus};
use serde::Deserialize;
use serde_json::Value;

use crate::{GitHubError, Result};

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

pub fn check_runs_from_value(value: Value) -> Result<Vec<CheckRun>> {
    let response: ApiCheckRunsResponse =
        serde_json::from_value(value).map_err(|error| GitHubError::Mapping(error.to_string()))?;

    Ok(response
        .check_runs
        .into_iter()
        .map(ApiCheckRun::into_domain)
        .collect())
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
