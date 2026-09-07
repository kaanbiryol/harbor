use gpui::{AppContext, Context, ScrollStrategy};
use harbor_logs::{parse_workflow_log, parse_workflow_log_with_jobs};
use std::sync::Arc;

use crate::{
    actions::PanelTab,
    panels::workflow_run_label,
    workspace::{AppView, async_updates::AppViewAsyncUpdateExt},
};

impl AppView {
    pub(crate) fn load_selected_workflow_logs(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self
            .selected_pull_request()
            .map(|pull_request| pull_request.repo.clone())
        else {
            let message = "Workflow logs require a selected pull request and GitHub auth";
            self.detail_state.log_state.apply_log_failure(message);
            self.status = message.to_string();
            cx.notify();
            return;
        };
        let Some(run) = self.selected_workflow_run_for_logs().cloned() else {
            let message = "No workflow run is available for the selected PR head";
            self.detail_state.log_state.apply_log_failure(message);
            self.status = message.to_string();
            cx.notify();
            return;
        };

        if self.detail_state.log_state.is_loading() {
            self.status = format!("Already loading logs for {}", workflow_run_label(&run));
            cx.notify();
            return;
        }

        self.active_tab = PanelTab::Logs;
        let refresh = self
            .detail_state
            .log_state
            .chunk()
            .is_some_and(|chunk| chunk.run_id == run.id);
        if !refresh {
            self.detail_state.log_state.clear_content();
            self.detail_state.log_state.selected_run = Some(run.id);
            self.detail_state.clear_workflow_jobs();
        }
        self.detail_state.log_state.start_loading();
        self.status = format!("Loading logs for {}", workflow_run_label(&run));

        cx.notify();
        let owner = repo.owner.clone();
        let name = repo.name.clone();
        let run_id = run.id;
        let run_label = workflow_run_label(&run);
        let github_api = self.github_api.clone();

        self.detail_state
            .log_state
            .set_task(cx.spawn(async move |this, cx| {
                let jobs_result = github_api
                    .list_workflow_jobs_for_run(&owner, &name, run_id, run.run_attempt)
                    .await.map(Arc::new);
                let jobs_for_logs = jobs_result.as_ref().cloned().unwrap_or_default();
                let all_skipped = jobs_result.as_ref().is_ok_and(|jobs| !jobs.is_empty() && jobs.iter().all(|job| job.conclusion == Some(harbor_domain::WorkflowConclusion::Skipped)));
                this.update_or_log(cx, "failed to update workflow jobs", move |view, cx| {
                    if view.selected_workflow_run_for_logs().map(|run| run.id) != Some(run_id) { return; }
                        match jobs_result {
                            Ok(jobs) => {
                                view.detail_state.restore_workflow_jobs(jobs);
                            }
                            Err(error) => {
                                view.detail_state.log_state.apply_jobs_failure(format!(
                                    "Failed to load workflow jobs: {error}"
                                ));
                            }
                        }
                    cx.notify();
                });
                let log_result = if all_skipped { Ok(parse_workflow_log(run_id, "")) } else { match github_api.workflow_run_log(&owner, &name, run_id, run.run_attempt).await {
                    Ok(text) => Ok(cx
                        .background_spawn(async move { parse_workflow_log_with_jobs(run_id, &text, &jobs_for_logs) })
                        .await),
                    Err(error) => Err(error),
                }};

                this.update_or_log(
                    cx,
                    "failed to update workflow log state",
                    move |view, cx| {
                        if view.selected_workflow_run_for_logs().map(|run| run.id) != Some(run_id) {
                            return;
                        }

                        match log_result {
                            Ok(chunk) => {
                                view.detail_state.log_state.apply_log_success(chunk);
                                let failed = view.detail_state.workflow_jobs().iter().find(|job| matches!(job.conclusion, Some(harbor_domain::WorkflowConclusion::Failure | harbor_domain::WorkflowConclusion::TimedOut)));
                                if !refresh { view.detail_state.log_state.selected_section = failed.map(|job| (job.name.clone(), job.steps.iter().find(|step| matches!(step.conclusion, Some(harbor_domain::WorkflowConclusion::Failure | harbor_domain::WorkflowConclusion::TimedOut))).map(|step| step.name.clone()))); }
                                view.filter_workflow_logs_with_scroll(!refresh, cx);
                                if !view.detail_state.log_state.has_error() {
                                    view.status = format!("Loaded logs for {run_label}");
                                } else {
                                    view.status =
                                        format!("Loaded logs for {run_label}, but jobs failed");
                                }
                            }
                            Err(error) => {
                                let message = workflow_log_error_message(&error);
                                view.detail_state
                                    .log_state
                                    .apply_log_failure(message.clone());
                                view.status = message;
                            }
                        }

                        view.cache_current_pull_request_detail_snapshot();
                        cx.notify();
                    },
                );
            }));
    }
}

impl AppView {
    pub(crate) fn filter_workflow_logs(&mut self, cx: &mut Context<Self>) {
        self.filter_workflow_logs_with_scroll(true, cx);
    }

    fn filter_workflow_logs_with_scroll(&mut self, reset_scroll: bool, cx: &mut Context<Self>) {
        let state = &mut self.detail_state.log_state;
        let Some(chunk) = state.shared_chunk() else {
            return;
        };
        let section = state.selected_section.clone();
        let query = state
            .search_input
            .as_ref()
            .map(|input| input.read(cx).value().to_lowercase())
            .unwrap_or_default();
        state.is_filtering = true;
        state.next_failure = 0;
        state.filter_task = Some(cx.spawn(async move |this, cx| {
            let (indices, widest_line) = cx
                .background_spawn(async move {
                    let indices = harbor_logs::filter_log_lines(&chunk, section.as_ref(), &query);
                    let widest = indices
                        .iter()
                        .enumerate()
                        .filter_map(|(position, index)| {
                            chunk
                                .lines
                                .get(*index)
                                .map(|line| (position, line.text.chars().count()))
                        })
                        .max_by_key(|(_, width)| *width)
                        .map(|(position, _)| position);
                    (indices, widest)
                })
                .await;
            this.update_or_log(cx, "failed to filter workflow logs", |view, cx| {
                let state = &mut view.detail_state.log_state;
                state.visible_lines = std::sync::Arc::new(indices);
                state.widest_line = widest_line;
                state.is_filtering = false;
                if reset_scroll {
                    state
                        .list_scroll
                        .0
                        .borrow()
                        .base_handle
                        .set_offset(gpui::point(gpui::px(0.), gpui::px(0.)));
                    state.list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}

impl AppView {
    pub(crate) fn export_workflow_logs(&mut self, download: bool, cx: &mut Context<Self>) {
        let state = &self.detail_state.log_state;
        let Some(chunk) = state.shared_chunk() else {
            return;
        };
        let indices = state.visible_lines.clone();
        let path_request = download
            .then(|| cx.prompt_for_new_path(std::path::Path::new("."), Some("workflow.log")));
        cx.spawn(async move |this, cx| {
            let path = if let Some(request) = path_request {
                match request.await {
                    Ok(Ok(Some(path))) => Some(path),
                    Ok(Ok(None)) => return,
                    error => {
                        this.update_or_log(cx, "failed to report log export error", |view, cx| {
                            view.status = format!("Could not choose a log export path: {error:?}");
                            cx.notify();
                        });
                        return;
                    }
                }
            } else {
                None
            };
            let result = cx
                .background_spawn(async move {
                    let text = indices
                        .iter()
                        .filter_map(|index| chunk.lines.get(*index))
                        .map(|line| line.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    if let Some(path) = path {
                        std::fs::write(path, text).map(|()| None)
                    } else {
                        Ok(Some(text))
                    }
                })
                .await;
            this.update_or_log(cx, "failed to report log export", |view, cx| {
                match result {
                    Ok(Some(text)) => {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                        view.status = "Logs copied".into();
                    }
                    Ok(None) => view.status = "Logs downloaded".into(),
                    Err(error) => view.status = format!("Failed to download logs: {error}"),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn workflow_log_error_message(error: &harbor_github::GitHubError) -> String {
    use harbor_github::GitHubError;
    match error {
        GitHubError::Unauthenticated | GitHubError::UnauthenticatedCli => {
            "Sign in to GitHub to load workflow logs.".into()
        }
        _ => {
            let message = error.to_string();
            let lower = message.to_ascii_lowercase();
            if lower.contains("403") || lower.contains("forbidden") {
                format!(
                    "Access to workflow logs was denied. Check repository and Actions permissions. {message}"
                )
            } else if lower.contains("404")
                || lower.contains("410")
                || lower.contains("log not found")
                || lower.contains("logs not found")
            {
                format!(
                    "Workflow logs are unavailable. They may have expired or been deleted, or this account may lack access. {message}"
                )
            } else {
                format!("Failed to load workflow logs: {message}")
            }
        }
    }
}
