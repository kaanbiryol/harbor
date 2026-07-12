use gpui::{Context, ListOffset, px};
use harbor_domain::RepoId;

use crate::{
    actions::PanelTab,
    panels::workflow_run_label,
    workspace::{AppView, RepositoryActionsTaskKind, async_updates::AppViewAsyncUpdateExt},
};

impl AppView {
    pub(crate) fn load_repository_actions_if_needed(&mut self, cx: &mut Context<Self>) {
        if !self.github_api.has_auth() {
            self.show_github_sign_in_required();
            cx.notify();
            return;
        }

        let Some(repository) = self.current_repository().cloned() else {
            self.status = "Select a repository before loading Actions".to_string();
            cx.notify();
            return;
        };

        self.repository_actions_state
            .reset_for_repository(repository.clone());

        if self.repository_actions_state.should_load_workflows() {
            self.spawn_repository_workflows_loader(repository.clone(), cx);
        }

        if self.repository_actions_state.should_load_runs() {
            self.spawn_repository_workflow_runs_loader(repository, cx);
        }
    }

    pub(crate) fn refresh_repository_actions(&mut self, cx: &mut Context<Self>) {
        self.tasks.cancel_repository_actions_tasks();
        self.repository_actions_state.mark_workflows_stale();
        self.repository_actions_state.mark_runs_stale();
        self.load_repository_actions_if_needed(cx);
    }

    pub(crate) fn select_repository_actions_workflow(
        &mut self,
        workflow_id: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        if !self.repository_actions_state.select_workflow(workflow_id) {
            return;
        }

        self.panel_list_state.action_runs.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: px(0.0),
        });
        self.status = match self.repository_actions_state.selected_workflow() {
            Some(workflow) => format!("Showing runs for {}", workflow.name),
            None => "Showing runs from all workflows".to_string(),
        };
        self.load_repository_actions_if_needed(cx);
        cx.notify();
    }

    fn spawn_repository_workflows_loader(&mut self, repository: RepoId, cx: &mut Context<Self>) {
        self.repository_actions_state.start_workflows_load();
        self.status = format!("Loading workflows for {}", repository.full_name());
        let github_api = self.github_api.clone();

        self.tasks.set_repository_actions_task(
            RepositoryActionsTaskKind::Workflows,
            cx.spawn({
                let repository = repository.clone();

                async move |this, cx| {
                    let result = github_api
                        .list_workflows(&repository.owner, &repository.name)
                        .await;

                    this.update_or_log(
                        cx,
                        "failed to update repository workflow state",
                        move |view, cx| {
                            if view.current_repository() != Some(&repository) {
                                return;
                            }

                            match result {
                                Ok(workflows) => {
                                    let count = workflows.len();
                                    view.repository_actions_state.replace_workflows(workflows);
                                    view.repository_actions_state.apply_workflows_success();
                                    if view.active_tab == PanelTab::Actions {
                                        view.status = format!(
                                            "Loaded {count} workflows for {}",
                                            repository.full_name()
                                        );
                                    }
                                }
                                Err(error) => {
                                    view.repository_actions_state
                                        .apply_workflows_failure(error.to_string());
                                    if view.active_tab == PanelTab::Actions {
                                        view.status = format!(
                                            "Failed to load workflows for {}",
                                            repository.full_name()
                                        );
                                    }
                                }
                            }

                            cx.notify();
                        },
                    );
                }
            }),
        );
    }

    fn spawn_repository_workflow_runs_loader(
        &mut self,
        repository: RepoId,
        cx: &mut Context<Self>,
    ) {
        self.repository_actions_state.start_runs_load();
        let workflow_id = self.repository_actions_state.selected_workflow_id();
        self.status = match self.repository_actions_state.selected_workflow() {
            Some(workflow) => format!("Loading runs for {}", workflow.name),
            None => format!("Loading workflow runs for {}", repository.full_name()),
        };
        let github_api = self.github_api.clone();

        self.tasks.set_repository_actions_task(
            RepositoryActionsTaskKind::Runs,
            cx.spawn({
                let repository = repository.clone();

                async move |this, cx| {
                    let result = match workflow_id {
                        Some(workflow_id) => {
                            github_api
                                .list_workflow_runs_for_workflow(
                                    &repository.owner,
                                    &repository.name,
                                    workflow_id,
                                )
                                .await
                        }
                        None => {
                            github_api
                                .list_repository_workflow_runs(&repository.owner, &repository.name)
                                .await
                        }
                    };

                    this.update_or_log(
                        cx,
                        "failed to update repository workflow run state",
                        move |view, cx| {
                            if view.current_repository() != Some(&repository)
                                || view.repository_actions_state.selected_workflow_id()
                                    != workflow_id
                            {
                                return;
                            }

                            match result {
                                Ok(workflow_runs) => {
                                    let count = workflow_runs.len();
                                    let first_run = workflow_runs.first().map(workflow_run_label);
                                    view.repository_actions_state
                                        .replace_workflow_runs(workflow_runs);
                                    view.repository_actions_state.apply_runs_success();
                                    if view.active_tab == PanelTab::Actions {
                                        view.status = first_run.map_or_else(
                                            || format!("Loaded {count} workflow runs"),
                                            |run| {
                                                format!(
                                                    "Loaded {count} workflow runs; latest: {run}"
                                                )
                                            },
                                        );
                                    }
                                }
                                Err(error) => {
                                    view.repository_actions_state
                                        .apply_runs_failure(error.to_string());
                                    if view.active_tab == PanelTab::Actions {
                                        view.status = format!(
                                            "Failed to load workflow runs for {}",
                                            repository.full_name()
                                        );
                                    }
                                }
                            }

                            cx.notify();
                        },
                    );
                }
            }),
        );
    }
}
