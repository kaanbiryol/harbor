use gpui::Context;
use harbor_domain::checks_summary_from_runs;
use harbor_sync::SyncTarget;

use crate::{
    actions::PanelTab,
    workspace::{
        AppView, async_updates::AppViewAsyncUpdateExt,
        pull_request_detail_loaders::SelectedPullRequestLoad,
        review_data_loaders::selected_pull_request_matches,
    },
};

impl AppView {
    pub(super) fn spawn_pull_request_checks_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        cx: &mut Context<Self>,
    ) {
        if !self.detail_state.should_load_checks() {
            return;
        }

        self.detail_state.start_checks_load();
        let github_api = self.github_api.clone();
        let store = self.repository_state.store();
        self.tasks.push_selected_pull_request_task(cx.spawn({
            let repo = load.repo;
            let owner = load.owner;
            let name = load.name;
            let number = load.number;
            let head_sha = load.head_sha;

            async move |this, cx| {
                let result = if head_sha.is_empty() {
                    Ok(Vec::new())
                } else {
                    github_api.list_check_runs(&owner, &name, &head_sha).await
                };
                let cache_result = match (&store, result.as_ref()) {
                    (Some(store), Ok(check_runs)) => store
                        .save_pull_request_check_runs(&repo, number, &head_sha, check_runs)
                        .await
                        .map_err(|error| error.to_string()),
                    (Some(store), Err(error)) => store
                        .record_sync_failure(
                            &harbor_storage::detail_target_key(
                                &repo,
                                number,
                                harbor_storage::PullRequestDetailSection::CheckRuns,
                            ),
                            &error.to_string(),
                        )
                        .await
                        .map_err(|error| error.to_string()),
                    (None, _) => Ok(()),
                };

                this.update_or_log(
                    cx,
                    "failed to update pull request checks state",
                    move |view, cx| {
                        if !selected_pull_request_matches(view, &repo, number) {
                            return;
                        }

                        if let Err(error) = cache_result {
                            view.repository_state.set_error(error);
                        }
                        match result {
                            Ok(check_runs) => {
                                view.mark_sync_success(SyncTarget::SelectedPullRequestChecks);
                                let count = check_runs.len();
                                let summary = checks_summary_from_runs(&check_runs);
                                view.detail_state.replace_check_runs(check_runs);
                                view.detail_state.apply_checks_success();

                                if let Some(selected) = view
                                    .pull_requests
                                    .get_mut(view.selection_state.pull_request_index())
                                {
                                    selected.checks_summary = summary;
                                }

                                view.status = format!("Loaded {count} check runs for PR #{number}");
                            }
                            Err(error) => {
                                view.mark_sync_failure(SyncTarget::SelectedPullRequestChecks);
                                view.detail_state.clear_check_runs();
                                view.detail_state.apply_checks_failure(error.to_string());
                                view.status = format!("Failed to load checks for PR #{number}");
                            }
                        }

                        view.cache_current_pull_request_detail_snapshot();
                        cx.notify();
                    },
                );
            }
        }));
    }

    pub(super) fn spawn_pull_request_workflows_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        cx: &mut Context<Self>,
    ) {
        if !self.detail_state.should_load_workflows() {
            return;
        }

        self.detail_state.start_workflows_load();
        let github_api = self.github_api.clone();
        let store = self.repository_state.store();
        self.tasks.push_selected_pull_request_task(cx.spawn({
            let repo = load.repo;
            let owner = load.owner;
            let name = load.name;
            let number = load.number;
            let head_sha = load.head_sha;

            async move |this, cx| {
                let result = if head_sha.is_empty() {
                    Ok(Vec::new())
                } else {
                    github_api
                        .list_workflow_runs_for_head(&owner, &name, &head_sha)
                        .await
                };
                let cache_result = match (&store, result.as_ref()) {
                    (Some(store), Ok(workflow_runs)) => store
                        .save_pull_request_workflow_runs(&repo, number, &head_sha, workflow_runs)
                        .await
                        .map_err(|error| error.to_string()),
                    (Some(store), Err(error)) => store
                        .record_sync_failure(
                            &harbor_storage::detail_target_key(
                                &repo,
                                number,
                                harbor_storage::PullRequestDetailSection::WorkflowRuns,
                            ),
                            &error.to_string(),
                        )
                        .await
                        .map_err(|error| error.to_string()),
                    (None, _) => Ok(()),
                };

                this.update_or_log(
                    cx,
                    "failed to update pull request workflow state",
                    move |view, cx| {
                        if !selected_pull_request_matches(view, &repo, number) {
                            return;
                        }

                        if let Err(error) = cache_result {
                            view.repository_state.set_error(error);
                        }
                        match result {
                            Ok(workflow_runs) => {
                                view.mark_sync_success(SyncTarget::SelectedPullRequestWorkflows);
                                let count = workflow_runs.len();
                                view.detail_state.replace_workflow_runs(workflow_runs);
                                view.detail_state.apply_workflows_success();
                                view.status =
                                    format!("Loaded {count} workflow runs for PR #{number}");

                                if view.active_tab == PanelTab::Logs
                                    && view.detail_state.log_state.error().is_none()
                                    && !view.detail_state.workflow_runs().is_empty()
                                {
                                    view.load_selected_workflow_logs(cx);
                                }
                            }
                            Err(error) => {
                                view.mark_sync_failure(SyncTarget::SelectedPullRequestWorkflows);
                                view.detail_state.clear_workflow_runs();
                                view.detail_state.apply_workflows_failure(error.to_string());
                                view.status =
                                    format!("Failed to load workflow runs for PR #{number}");
                            }
                        }

                        view.cache_current_pull_request_detail_snapshot();
                        cx.notify();
                    },
                );
            }
        }));
    }
}
