use harbor_domain::{RepoId, Workflow, WorkflowRun};

use crate::workspace::status::LoadStatus;

pub(crate) struct RepositoryActionsUiState {
    repository: Option<RepoId>,
    workflows: Vec<Workflow>,
    workflow_runs: Vec<WorkflowRun>,
    selected_workflow_id: Option<u64>,
    workflows_load: LoadStatus,
    runs_load: LoadStatus,
}

impl RepositoryActionsUiState {
    pub(crate) fn new() -> Self {
        Self {
            repository: None,
            workflows: Vec::new(),
            workflow_runs: Vec::new(),
            selected_workflow_id: None,
            workflows_load: LoadStatus::Idle,
            runs_load: LoadStatus::Idle,
        }
    }

    pub(crate) fn reset_for_repository(&mut self, repository: RepoId) {
        if self.repository.as_ref() == Some(&repository) {
            return;
        }

        self.repository = Some(repository);
        self.workflows.clear();
        self.workflow_runs.clear();
        self.selected_workflow_id = None;
        self.workflows_load.reset();
        self.runs_load.reset();
    }

    pub(crate) fn clear(&mut self) {
        self.repository = None;
        self.workflows.clear();
        self.workflow_runs.clear();
        self.selected_workflow_id = None;
        self.workflows_load.reset();
        self.runs_load.reset();
    }

    pub(crate) fn select_workflow(&mut self, workflow_id: Option<u64>) -> bool {
        if self.selected_workflow_id == workflow_id {
            return false;
        }

        self.selected_workflow_id = workflow_id;
        self.workflow_runs.clear();
        self.runs_load.reset();
        true
    }

    pub(crate) fn selected_workflow_id(&self) -> Option<u64> {
        self.selected_workflow_id
    }

    pub(crate) fn selected_workflow(&self) -> Option<&Workflow> {
        self.selected_workflow_id.and_then(|workflow_id| {
            self.workflows
                .iter()
                .find(|workflow| workflow.id == workflow_id)
        })
    }

    pub(crate) fn workflows(&self) -> &[Workflow] {
        &self.workflows
    }

    pub(crate) fn workflow_runs(&self) -> &[WorkflowRun] {
        &self.workflow_runs
    }

    pub(crate) fn replace_workflows(&mut self, workflows: Vec<Workflow>) {
        self.workflows = workflows;
        if let Some(selected_workflow_id) = self.selected_workflow_id
            && !self
                .workflows
                .iter()
                .any(|workflow| workflow.id == selected_workflow_id)
        {
            self.selected_workflow_id = None;
            self.workflow_runs.clear();
            self.runs_load.reset();
        }
    }

    pub(crate) fn replace_workflow_runs(&mut self, workflow_runs: Vec<WorkflowRun>) {
        self.workflow_runs = workflow_runs;
    }

    pub(crate) fn should_load_workflows(&self) -> bool {
        !self.workflows_load.is_loading() && !self.workflows_load.is_finished()
    }

    pub(crate) fn should_load_runs(&self) -> bool {
        !self.runs_load.is_loading() && !self.runs_load.is_finished()
    }

    pub(crate) fn mark_workflows_stale(&mut self) {
        self.workflows_load.reset();
    }

    pub(crate) fn mark_runs_stale(&mut self) {
        self.runs_load.reset();
    }

    pub(crate) fn start_workflows_load(&mut self) {
        self.workflows_load.start();
    }

    pub(crate) fn start_runs_load(&mut self) {
        self.runs_load.start();
    }

    pub(crate) fn apply_workflows_success(&mut self) {
        self.workflows_load.succeed();
    }

    pub(crate) fn apply_runs_success(&mut self) {
        self.runs_load.succeed();
    }

    pub(crate) fn apply_workflows_failure(&mut self, error: impl Into<String>) {
        self.workflows_load.fail(error);
    }

    pub(crate) fn apply_runs_failure(&mut self, error: impl Into<String>) {
        self.runs_load.fail(error);
    }

    pub(crate) fn workflows_loading(&self) -> bool {
        self.workflows_load.is_loading()
    }

    pub(crate) fn runs_loading(&self) -> bool {
        self.runs_load.is_loading()
    }

    pub(crate) fn workflows_error(&self) -> Option<&str> {
        self.workflows_load.error()
    }

    pub(crate) fn runs_error(&self) -> Option<&str> {
        self.runs_load.error()
    }
}

#[cfg(test)]
mod tests {
    use harbor_domain::RepoId;

    use super::RepositoryActionsUiState;

    #[test]
    fn selecting_workflow_resets_run_load() {
        let mut state = RepositoryActionsUiState::new();
        state.reset_for_repository(RepoId::new("acme", "app"));
        state.start_runs_load();
        state.apply_runs_success();

        assert!(state.select_workflow(Some(42)));
        assert!(state.should_load_runs());
        assert_eq!(state.selected_workflow_id(), Some(42));
    }

    #[test]
    fn repository_change_clears_actions_state() {
        let mut state = RepositoryActionsUiState::new();
        state.reset_for_repository(RepoId::new("acme", "app"));
        state.select_workflow(Some(42));
        state.start_workflows_load();
        state.apply_workflows_success();

        state.reset_for_repository(RepoId::new("acme", "other"));

        assert!(state.workflows().is_empty());
        assert!(state.workflow_runs().is_empty());
        assert_eq!(state.selected_workflow_id(), None);
        assert!(state.should_load_workflows());
    }
}
