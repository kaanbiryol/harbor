use futures::{StreamExt, stream};
use gpui::{AppContext, Context, ScrollStrategy};
use harbor_logs::{
    filter_log_lines, first_failed_section, joined_log_text, parse_workflow_log,
    parse_workflow_log_with_jobs, widest_line_position,
};
use std::sync::Arc;

use super::WorkflowLogView;

impl WorkflowLogView {
    pub(crate) fn select_workflow_log_section(
        &mut self,
        run_id: u64,
        section: Option<(String, Option<String>)>,
        cx: &mut Context<Self>,
    ) {
        if !self.runs.as_slice().iter().any(|run| run.id == run_id) {
            return;
        }
        if self.selected_workflow_run_for_logs().map(|run| run.id) != Some(run_id) {
            self.state.clear_content();
            self.jobs = Arc::new(Vec::new());
        }
        self.state.selected_run = Some(run_id);
        self.state.selected_section = section;
        if self
            .state
            .chunk()
            .is_some_and(|chunk| chunk.run_id == run_id)
        {
            self.filter_workflow_logs(cx);
        } else {
            self.load_selected_workflow_logs(cx);
        }
    }

    pub(crate) fn load_workflow_log_navigation(&mut self, cx: &mut Context<Self>) {
        if self.state.navigation_task.is_some() {
            return;
        }
        let repo = self.repository.clone();
        let selected_run = self.selected_workflow_run_for_logs().map(|run| run.id);
        if let Some(run_id) = selected_run
            && self
                .state
                .chunk()
                .is_some_and(|chunk| chunk.run_id == run_id)
            && !self.state.navigation_jobs.contains_key(&run_id)
        {
            let jobs = self.jobs.clone();
            self.state.navigation_jobs.insert(run_id, Ok(jobs));
        }
        let runs = self
            .runs
            .as_slice()
            .iter()
            .filter(|run| Some(run.id) != selected_run)
            .filter(|run| !self.state.navigation_jobs.contains_key(&run.id))
            .map(|run| (run.id, run.run_attempt))
            .collect::<Vec<_>>();
        if runs.is_empty() {
            return;
        }
        let github_api = self.github_api.clone();
        self.state.navigation_task = Some(cx.spawn(async move |this, cx| {
            let executor = cx.background_executor().clone();
            let mut requests = stream::iter(runs)
                .map(|(run_id, attempt)| {
                    let github_api = github_api.clone();
                    let repo = repo.clone();
                    executor.spawn(async move {
                        let jobs = github_api
                            .list_workflow_jobs_for_run(&repo.owner, &repo.name, run_id, attempt)
                            .await
                            .map(Arc::new)
                            .map_err(|error| error.to_string());
                        (run_id, jobs)
                    })
                })
                .buffer_unordered(4);
            while let Some((run_id, jobs)) = requests.next().await {
                this.update(cx, |view, cx| {
                    view.state.navigation_jobs.insert(run_id, jobs);
                    cx.notify();
                })
                .unwrap_or_else(
                    |error| tracing::debug!(%error, "failed to update log check navigation"),
                );
            }
            this.update(cx, |view, _| view.state.navigation_task = None)
                .unwrap_or_else(|error| tracing::debug!(%error, "log navigation closed"));
        }));
    }

    pub(crate) fn load_selected_workflow_logs(&mut self, cx: &mut Context<Self>) {
        let repo = self.repository.clone();
        let Some(run) = self.selected_workflow_run_for_logs().cloned() else {
            let message = "No workflow run is available for the selected PR head";
            self.state.apply_log_failure(message);
            self.status = message.to_string();
            cx.notify();
            return;
        };

        if self.state.is_loading() {
            self.status = format!("Already loading logs for {}", run.label());
            cx.notify();
            return;
        }

        let refresh = self
            .state
            .chunk()
            .is_some_and(|chunk| chunk.run_id == run.id);
        if !refresh {
            let section = if self.state.selected_run == Some(run.id) {
                self.state.selected_section.take()
            } else {
                None
            };
            self.state.clear_content();
            self.state.selected_run = Some(run.id);
            self.state.selected_section = section;
            self.jobs = Arc::new(Vec::new());
        }
        self.state.start_loading();
        self.status = format!("Loading logs for {}", run.label());

        cx.notify();
        let owner = repo.owner.clone();
        let name = repo.name.clone();
        let run_id = run.id;
        let run_label = run.label().to_string();
        let github_api = self.github_api.clone();

        self.state.set_task(cx.spawn(async move |this, cx| {
            let jobs_result = cx
                .background_spawn({
                    let github_api = github_api.clone();
                    let owner = owner.clone();
                    let name = name.clone();
                    async move {
                        github_api
                            .list_workflow_jobs_for_run(&owner, &name, run_id, run.run_attempt)
                            .await
                            .map(Arc::new)
                    }
                })
                .await;
            let jobs_for_logs = jobs_result.as_ref().cloned().unwrap_or_default();
            let all_skipped = jobs_result.as_ref().is_ok_and(|jobs| {
                !jobs.is_empty()
                    && jobs.iter().all(|job| {
                        job.conclusion == Some(harbor_domain::WorkflowConclusion::Skipped)
                    })
            });
            this.update(cx, move |view, cx| {
                if view.selected_workflow_run_for_logs().map(|run| run.id) != Some(run_id) {
                    return;
                }
                match jobs_result {
                    Ok(jobs) => {
                        view.state.navigation_jobs.insert(run_id, Ok(jobs.clone()));
                        view.jobs = jobs;
                    }
                    Err(error) => {
                        view.state
                            .navigation_jobs
                            .insert(run_id, Err(error.to_string()));
                        view.state
                            .apply_jobs_failure(format!("Failed to load workflow jobs: {error}"));
                    }
                }
                cx.notify();
            })
            .unwrap_or_else(|error| tracing::debug!(%error, "failed to update workflow jobs"));
            let log_result = cx
                .background_spawn(async move {
                    if all_skipped {
                        return Ok(parse_workflow_log(run_id, ""));
                    }
                    github_api
                        .workflow_run_log(&owner, &name, run_id, run.run_attempt)
                        .await
                        .map(|text| parse_workflow_log_with_jobs(run_id, &text, &jobs_for_logs))
                })
                .await;

            this.update(cx, move |view, cx| {
                if view.selected_workflow_run_for_logs().map(|run| run.id) != Some(run_id) {
                    return;
                }

                match log_result {
                    Ok(chunk) => {
                        view.state.apply_log_success(chunk);
                        if !refresh && view.state.selected_section.is_none() {
                            view.state.selected_section = first_failed_section(&view.jobs);
                        }
                        view.filter_workflow_logs_with_scroll(!refresh, cx);
                        if !view.state.has_error() {
                            view.status = format!("Loaded logs for {run_label}");
                        } else {
                            view.status = format!("Loaded logs for {run_label}, but jobs failed");
                        }
                    }
                    Err(error) => {
                        let message = workflow_log_error_message(&error);
                        view.state.apply_log_failure(message.clone());
                        view.status = message;
                    }
                }

                view.publish(cx);
                cx.notify();
            })
            .unwrap_or_else(|error| tracing::debug!(%error, "failed to update workflow log state"));
        }));
    }
}

impl WorkflowLogView {
    pub(crate) fn filter_workflow_logs(&mut self, cx: &mut Context<Self>) {
        self.filter_workflow_logs_with_scroll(true, cx);
    }

    fn filter_workflow_logs_with_scroll(&mut self, reset_scroll: bool, cx: &mut Context<Self>) {
        let state = &mut self.state;
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
                    let indices = filter_log_lines(&chunk, section.as_ref(), &query);
                    let widest = widest_line_position(&chunk, &indices);
                    (indices, widest)
                })
                .await;
            this.update(cx, |view, cx| {
                let state = &mut view.state;
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
            })
            .unwrap_or_else(|error| tracing::debug!(%error, "failed to filter workflow logs"));
        }));
        cx.notify();
    }
}

impl WorkflowLogView {
    pub(crate) fn export_workflow_logs(&mut self, download: bool, cx: &mut Context<Self>) {
        let state = &self.state;
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
                        this.update(cx, |view, cx| {
                            view.status = format!("Could not choose a log export path: {error:?}");
                            cx.notify();
                        })
                        .unwrap_or_else(
                            |error| tracing::debug!(%error, "failed to report log export error"),
                        );
                        return;
                    }
                }
            } else {
                None
            };
            let result = cx
                .background_spawn(async move {
                    let text = joined_log_text(&chunk, &indices);
                    if let Some(path) = path {
                        std::fs::write(path, text).map(|()| None)
                    } else {
                        Ok(Some(text))
                    }
                })
                .await;
            this.update(cx, |view, cx| {
                match result {
                    Ok(Some(text)) => {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                        view.status = "Logs copied".into();
                    }
                    Ok(None) => view.status = "Logs downloaded".into(),
                    Err(error) => view.status = format!("Failed to download logs: {error}"),
                }
                view.publish(cx);
                cx.notify();
            })
            .unwrap_or_else(|error| tracing::debug!(%error, "failed to report log export"));
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
