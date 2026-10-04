use gpui::{AppContext, Context, Entity};

use super::AppView;
use crate::{
    actions::PanelTab,
    workflow_logs::{WorkflowLogUpdate, WorkflowLogView},
};

impl AppView {
    pub(super) fn ensure_workflow_log_view(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<Entity<WorkflowLogView>> {
        let key = self.selected_pull_request_detail_key()?;
        let runs = self.detail_state.shared_workflow_runs();
        if let Some(logs) = &self.workflow_logs {
            logs.update(cx, |logs, cx| {
                logs.code_font_size = self.preferences.value.code_font_size;
                logs.set_runs(runs, cx);
            });
            return Some(logs.clone());
        }
        let repository = self.selected_pull_request()?.repo.clone();
        let github_api = self.github_api_for_request();
        let input = self.workflow_log_input.clone();
        let font_size = self.preferences.value.code_font_size;
        let jobs = self.detail_state.shared_workflow_jobs();
        let chunk = self.detail_state.log_chunk.clone();
        let logs = cx.new(|cx| {
            let mut logs = WorkflowLogView::new(github_api, repository, input, font_size, cx);
            logs.set_runs(runs, cx);
            logs.jobs = jobs;
            logs.state.restore_chunk(chunk);
            logs.filter_workflow_logs(cx);
            logs
        });
        let auth_generation = self.auth_generation;
        self.workflow_log_subscription = Some(cx.subscribe(
            &logs,
            move |view, _, update: &WorkflowLogUpdate, cx| {
                if view.auth_generation != auth_generation
                    || view.selected_pull_request_detail_key().as_ref() != Some(&key)
                {
                    return;
                }
                view.detail_state.log_chunk = update.chunk.clone();
                view.detail_state.restore_workflow_jobs(update.jobs.clone());
                view.status = update.status.clone();
                view.cache_current_pull_request_detail_snapshot();
                cx.notify();
            },
        ));
        self.workflow_logs = Some(logs.clone());
        Some(logs)
    }

    pub(crate) fn load_selected_workflow_logs(&mut self, cx: &mut Context<Self>) {
        self.active_tab = PanelTab::Logs;
        if let Some(logs) = self.ensure_workflow_log_view(cx) {
            logs.update(cx, |logs, cx| logs.load_selected_workflow_logs(cx));
        }
    }

    pub(crate) fn load_workflow_log_navigation(&mut self, cx: &mut Context<Self>) {
        if let Some(logs) = self.ensure_workflow_log_view(cx) {
            logs.update(cx, |logs, cx| logs.load_if_needed(cx));
        }
    }
}
