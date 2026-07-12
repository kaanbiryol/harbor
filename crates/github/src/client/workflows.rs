use harbor_domain::{CheckRun, Workflow, WorkflowJob, WorkflowRun};
use serde_json::json;

use crate::{GitHubTransport, Result, WorkflowRunPage, dto};

use super::GitHubClient;

const WORKFLOW_RUN_PAGE_SIZE: usize = 100;
const WORKFLOW_RUN_PAGE_SIZE_QUERY: &str = "100";

impl<T> GitHubClient<T>
where
    T: GitHubTransport,
{
    pub async fn list_workflows(&self, owner: &str, repo: &str) -> Result<Vec<Workflow>> {
        let path = format!("/repos/{owner}/{repo}/actions/workflows");
        let response = self
            .transport
            .rest_get(&path, &[("per_page", "100")])
            .await?;

        dto::workflows_from_value(response)
    }

    pub async fn list_repository_workflow_run_page(
        &self,
        owner: &str,
        repo: &str,
        page: usize,
    ) -> Result<WorkflowRunPage> {
        let page = page.max(1);
        let page_string = page.to_string();
        let path = format!("/repos/{owner}/{repo}/actions/runs");
        let mut query = vec![("per_page", WORKFLOW_RUN_PAGE_SIZE_QUERY)];
        if page > 1 {
            query.push(("page", page_string.as_str()));
        }
        let response = self.transport.rest_get(&path, &query).await?;

        dto::workflow_run_page_from_value(response, page, WORKFLOW_RUN_PAGE_SIZE)
    }

    pub async fn list_workflow_run_page_for_workflow(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: u64,
        page: usize,
    ) -> Result<WorkflowRunPage> {
        let page = page.max(1);
        let page_string = page.to_string();
        let path = format!("/repos/{owner}/{repo}/actions/workflows/{workflow_id}/runs");
        let mut query = vec![("per_page", WORKFLOW_RUN_PAGE_SIZE_QUERY)];
        if page > 1 {
            query.push(("page", page_string.as_str()));
        }
        let response = self.transport.rest_get(&path, &query).await?;

        dto::workflow_run_page_from_value(response, page, WORKFLOW_RUN_PAGE_SIZE)
    }

    pub async fn list_check_runs(
        &self,
        owner: &str,
        repo: &str,
        git_ref: &str,
    ) -> Result<Vec<CheckRun>> {
        let path = format!("/repos/{owner}/{repo}/commits/{git_ref}/check-runs");
        let response = self
            .transport
            .rest_get(&path, &[("per_page", "100")])
            .await?;

        dto::check_runs_from_value(response)
    }

    pub async fn list_workflow_runs_for_head(
        &self,
        owner: &str,
        repo: &str,
        head_sha: &str,
    ) -> Result<Vec<WorkflowRun>> {
        let path = format!("/repos/{owner}/{repo}/actions/runs");
        let response = self
            .transport
            .rest_get(&path, &[("head_sha", head_sha), ("per_page", "50")])
            .await?;

        dto::workflow_runs_from_value(response)
    }

    pub async fn list_workflow_jobs_for_run(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
    ) -> Result<Vec<WorkflowJob>> {
        let path = format!("/repos/{owner}/{repo}/actions/runs/{run_id}/jobs");
        let response = self
            .transport
            .rest_get(&path, &[("per_page", "100")])
            .await?;

        dto::workflow_jobs_from_value(response)
    }

    pub async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String> {
        self.transport.workflow_run_log(owner, repo, run_id).await
    }

    pub async fn rerun_workflow_run(&self, owner: &str, repo: &str, run_id: u64) -> Result<()> {
        let path = format!("/repos/{owner}/{repo}/actions/runs/{run_id}/rerun");
        self.transport.rest_post(&path, json!({})).await?;

        Ok(())
    }

    pub async fn rerun_failed_jobs(&self, owner: &str, repo: &str, run_id: u64) -> Result<()> {
        let path = format!("/repos/{owner}/{repo}/actions/runs/{run_id}/rerun-failed-jobs");
        self.transport.rest_post(&path, json!({})).await?;

        Ok(())
    }

    pub async fn dispatch_workflow(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: u64,
        git_ref: &str,
    ) -> Result<()> {
        let path = format!("/repos/{owner}/{repo}/actions/workflows/{workflow_id}/dispatches");
        self.transport
            .rest_post(&path, json!({ "ref": git_ref }))
            .await?;

        Ok(())
    }
}
