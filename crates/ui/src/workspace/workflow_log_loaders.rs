use gpui::{Context, ScrollStrategy};
use harbor_github::{GhCliTransport, GitHubClient};
use harbor_logs::parse_workflow_log;

use crate::{actions::PanelTab, panels::workflow_run_label, workspace::AppView};

impl AppView {
    pub(crate) fn load_selected_workflow_logs(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self
            .selected_pull_request()
            .map(|pull_request| pull_request.repo.clone())
        else {
            self.logs_error =
                Some("Workflow logs require a selected pull request and GitHub CLI auth".into());
            self.status = self.logs_error.clone().unwrap_or_default();
            cx.notify();
            return;
        };
        let Some(run) = self.selected_workflow_run_for_logs().cloned() else {
            self.logs_error = Some("No workflow run is available for the selected PR head".into());
            self.status = self.logs_error.clone().unwrap_or_default();
            cx.notify();
            return;
        };

        if self.is_loading_logs {
            self.status = format!("Already loading logs for {}", workflow_run_label(&run));
            cx.notify();
            return;
        }

        self.active_tab = PanelTab::Logs;
        self.is_loading_logs = true;
        self.logs_error = None;
        self.workflow_jobs.clear();
        self.log_chunk = None;
        self.log_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.status = format!("Loading logs for {}", workflow_run_label(&run));

        let owner = repo.owner.clone();
        let name = repo.name.clone();
        let run_id = run.id;
        let run_label = workflow_run_label(&run);

        self.log_task = Some(cx.spawn(async move |this, cx| {
            let client = GitHubClient::new(GhCliTransport);
            let jobs_result = client
                .list_workflow_jobs_for_run(&owner, &name, run_id)
                .await;
            let log_result = client.workflow_run_log(&owner, &name, run_id).await;

            _ = this.update(cx, move |view, cx| {
                if view.selected_workflow_run_for_logs().map(|run| run.id) != Some(run_id) {
                    return;
                }

                view.is_loading_logs = false;

                match jobs_result {
                    Ok(jobs) => {
                        view.workflow_jobs = jobs;
                    }
                    Err(error) => {
                        view.workflow_jobs.clear();
                        view.logs_error = Some(format!("Failed to load workflow jobs: {error}"));
                    }
                }

                match log_result {
                    Ok(text) => {
                        view.log_chunk = Some(parse_workflow_log(run_id, &text));
                        if view.logs_error.is_none() {
                            view.status = format!("Loaded logs for {run_label}");
                        } else {
                            view.status = format!("Loaded logs for {run_label}, but jobs failed");
                        }
                    }
                    Err(error) => {
                        view.log_chunk = None;
                        let message = format!("Failed to load workflow logs: {error}");
                        view.logs_error = Some(message.clone());
                        view.status = message;
                    }
                }

                view.log_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                view.cache_current_pull_request_detail_snapshot();
                cx.notify();
            });
        }));
    }
}
