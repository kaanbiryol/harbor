use std::sync::Arc;

use gpui::{Context, Entity, EventEmitter, IntoElement, Render, Window};
use harbor_domain::{RepoId, WorkflowJob, WorkflowRun};
use harbor_github::GitHubWorkflowApi;
use harbor_logs::LogChunk;

use crate::gpui_component::input::{InputEvent, InputState};

mod loading;
mod state;
pub(crate) use state::WorkflowLogState;

pub(crate) struct WorkflowLogView {
    github_api: Arc<dyn GitHubWorkflowApi>,
    repository: RepoId,
    pub(crate) runs: Arc<Vec<WorkflowRun>>,
    pub(crate) jobs: Arc<Vec<WorkflowJob>>,
    pub(crate) state: WorkflowLogState,
    pub(crate) code_font_size: f32,
    status: String,
}

pub(crate) struct WorkflowLogUpdate {
    pub(crate) chunk: Option<Arc<LogChunk>>,
    pub(crate) jobs: Arc<Vec<WorkflowJob>>,
    pub(crate) status: String,
}

impl EventEmitter<WorkflowLogUpdate> for WorkflowLogView {}

impl WorkflowLogView {
    pub(crate) fn new(
        github_api: Arc<dyn GitHubWorkflowApi>,
        repository: RepoId,
        search_input: Entity<InputState>,
        code_font_size: f32,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut state = WorkflowLogState::new();
        state.search_subscription = Some(cx.subscribe(&search_input, |view, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                view.filter_workflow_logs(cx);
            }
        }));
        state.search_input = Some(search_input);
        Self {
            github_api,
            repository,
            state,
            code_font_size,
            runs: Arc::new(Vec::new()),
            jobs: Arc::new(Vec::new()),
            status: String::new(),
        }
    }

    pub(crate) fn set_runs(&mut self, runs: Arc<Vec<WorkflowRun>>, cx: &mut Context<Self>) {
        if Arc::ptr_eq(&self.runs, &runs) {
            return;
        }
        self.state.clear_navigation();
        if self
            .state
            .selected_run
            .is_some_and(|id| !runs.iter().any(|run| run.id == id))
        {
            self.state.clear_content();
            self.jobs = Arc::new(Vec::new());
        }
        self.runs = runs;
        cx.notify();
    }

    pub(crate) fn selected_workflow_run_for_logs(&self) -> Option<&WorkflowRun> {
        self.runs
            .iter()
            .find(|run| Some(run.id) == self.state.selected_run)
            .or_else(|| self.runs.iter().find(|run| run.failed()))
            .or_else(|| self.runs.first())
    }

    pub(crate) fn load_if_needed(&mut self, cx: &mut Context<Self>) {
        if !self.state.is_loading() && self.state.chunk().is_none() && self.state.error().is_none()
        {
            self.load_selected_workflow_logs(cx);
        }
        self.load_workflow_log_navigation(cx);
    }

    fn publish(&self, cx: &mut Context<Self>) {
        cx.emit(WorkflowLogUpdate {
            chunk: self.state.shared_chunk(),
            jobs: self.jobs.clone(),
            status: self.status.clone(),
        });
    }
}

impl Render for WorkflowLogView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::panels::render_logs_panel(self, cx)
    }
}
