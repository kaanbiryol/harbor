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

#[cfg(test)]
mod tests {
    use harbor_domain::{CheckConclusion, CheckStatus};
    use serde_json::json;

    use super::check_runs_from_value;

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
}
