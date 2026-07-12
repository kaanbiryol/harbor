use async_trait::async_trait;
use harbor_domain::{Workflow, WorkflowJob, WorkflowRun};
use harbor_github::Result;

#[async_trait]
pub trait GitHubWorkflowApi: Send + Sync {
    async fn list_workflows(&self, owner: &str, repo: &str) -> Result<Vec<Workflow>>;
    async fn list_repository_workflow_runs(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<WorkflowRun>>;
    async fn list_workflow_runs_for_workflow(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: u64,
    ) -> Result<Vec<WorkflowRun>>;
    async fn list_workflow_jobs_for_run(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
    ) -> Result<Vec<WorkflowJob>>;
    async fn workflow_run_log(&self, owner: &str, repo: &str, run_id: u64) -> Result<String>;
}

#[async_trait]
pub trait GitHubWorkflowMutationApi: Send + Sync {
    async fn dispatch_workflow(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: u64,
        git_ref: &str,
    ) -> Result<()>;
    async fn rerun_failed_jobs(&self, owner: &str, repo: &str, run_id: u64) -> Result<()>;
}
