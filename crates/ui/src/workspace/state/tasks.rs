use std::collections::HashMap;

use gpui::Task;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum SelectedPullRequestTaskKind {
    Cache,
    Metadata,
    Files,
    Checks,
    Commits,
    Workflows,
    Reviews,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum RepositoryActionsTaskKind {
    Workflows,
    Runs,
}

#[derive(Default)]
pub(crate) struct WorkspaceTasks {
    pull_request_list_task: Option<Task<()>>,
    selected_pull_request_tasks: HashMap<SelectedPullRequestTaskKind, Task<()>>,
    repository_actions_tasks: HashMap<RepositoryActionsTaskKind, Task<()>>,
    repository_task: Option<Task<()>>,
    local_task: Option<Task<()>>,
    external_app_availability_task: Option<Task<()>>,
    sync_task: Option<Task<()>>,
    auth_task: Option<Task<()>>,
}

impl WorkspaceTasks {
    pub(crate) fn cancel_selected_pull_request_tasks(&mut self) {
        self.selected_pull_request_tasks.clear();
    }

    pub(crate) fn cancel_repository_actions_tasks(&mut self) {
        self.repository_actions_tasks.clear();
    }

    pub(crate) fn set_pull_request_list_task(&mut self, task: Task<()>) {
        self.pull_request_list_task = Some(task);
    }

    pub(crate) fn set_selected_pull_request_task(
        &mut self,
        kind: SelectedPullRequestTaskKind,
        task: Task<()>,
    ) {
        self.selected_pull_request_tasks.insert(kind, task);
    }

    pub(crate) fn set_repository_actions_task(
        &mut self,
        kind: RepositoryActionsTaskKind,
        task: Task<()>,
    ) {
        self.repository_actions_tasks.insert(kind, task);
    }

    pub(crate) fn cancel_pull_request_list_task(&mut self) {
        self.pull_request_list_task = None;
    }

    pub(crate) fn set_repository_task(&mut self, task: Task<()>) {
        self.repository_task = Some(task);
    }

    pub(crate) fn set_local_task(&mut self, task: Task<()>) {
        self.local_task = Some(task);
    }

    pub(crate) fn clear_local_task(&mut self) {
        self.local_task = None;
    }

    pub(crate) fn set_external_app_availability_task(&mut self, task: Task<()>) {
        self.external_app_availability_task = Some(task);
    }

    pub(crate) fn clear_external_app_availability_task(&mut self) {
        self.external_app_availability_task = None;
    }

    pub(crate) fn set_sync_task(&mut self, task: Task<()>) {
        self.sync_task = Some(task);
    }

    pub(crate) fn set_auth_task(&mut self, task: Task<()>) {
        self.auth_task = Some(task);
    }

    #[cfg(test)]
    pub(crate) fn has_auth_task(&self) -> bool {
        self.auth_task.is_some()
    }

    pub(crate) fn sync_task_running(&self) -> bool {
        self.sync_task.is_some()
    }

    pub(crate) fn local_task_running(&self) -> bool {
        self.local_task.is_some()
    }
}
