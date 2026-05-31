use gpui::Task;

#[derive(Default)]
pub(crate) struct WorkspaceTasks {
    pr_list_task: Option<Task<()>>,
    pr_detail_tasks: Vec<Task<()>>,
    repository_task: Option<Task<()>>,
    local_task: Option<Task<()>>,
    external_app_availability_task: Option<Task<()>>,
    sync_task: Option<Task<()>>,
    auth_task: Option<Task<()>>,
}

impl WorkspaceTasks {
    pub(crate) fn clear_pull_request_detail_tasks(&mut self) {
        self.pr_detail_tasks.clear();
    }

    pub(crate) fn set_pull_request_list_task(&mut self, task: Task<()>) {
        self.pr_list_task = Some(task);
    }

    pub(crate) fn push_pull_request_detail_task(&mut self, task: Task<()>) {
        self.pr_detail_tasks.push(task);
    }

    pub(crate) fn clear_pull_request_list_task(&mut self) {
        self.pr_list_task = None;
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
