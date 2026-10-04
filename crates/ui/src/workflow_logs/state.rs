use std::{collections::HashMap, sync::Arc};

use gpui::{Task, UniformListScrollHandle};
use harbor_domain::WorkflowJob;
use harbor_logs::LogChunk;

pub(crate) struct WorkflowLogState {
    pub(crate) navigation_jobs: HashMap<u64, Result<Arc<Vec<WorkflowJob>>, String>>,
    pub(crate) navigation_task: Option<Task<()>>,
    chunk: Option<Arc<LogChunk>>,
    pub(crate) selected_run: Option<u64>,
    pub(crate) selected_section: Option<(String, Option<String>)>,
    pub(crate) visible_lines: Arc<Vec<usize>>,
    pub(crate) search_input: Option<gpui::Entity<crate::gpui_component::input::InputState>>,
    pub(crate) search_subscription: Option<gpui::Subscription>,
    pub(crate) filter_task: Option<Task<()>>,
    pub(crate) show_timestamps: bool,
    pub(crate) navigation_scroll: UniformListScrollHandle,
    pub(crate) next_failure: usize,
    pub(crate) widest_line: Option<usize>,
    pub(crate) is_filtering: bool,
    task: Option<Task<()>>,
    pub(crate) list_scroll: UniformListScrollHandle,
    is_loading: bool,
    error: Option<String>,
}

impl WorkflowLogState {
    pub(crate) fn new() -> Self {
        Self {
            navigation_jobs: HashMap::new(),
            navigation_task: None,
            chunk: None,
            selected_run: None,
            selected_section: None,
            visible_lines: Arc::new(Vec::new()),
            search_input: None,
            search_subscription: None,
            filter_task: None,
            show_timestamps: false,
            navigation_scroll: UniformListScrollHandle::new(),
            next_failure: 0,
            widest_line: None,
            is_filtering: false,
            task: None,
            list_scroll: UniformListScrollHandle::new(),
            is_loading: false,
            error: None,
        }
    }

    pub(crate) fn chunk(&self) -> Option<&LogChunk> {
        self.chunk.as_deref()
    }

    pub(crate) fn clear_navigation(&mut self) {
        self.navigation_task = None;
        self.navigation_jobs.clear();
    }

    pub(crate) fn restore_chunk(&mut self, chunk: Option<Arc<LogChunk>>) {
        let chunk = chunk.filter(|chunk| chunk.is_current_format());
        self.visible_lines =
            Arc::new((0..chunk.as_ref().map_or(0, |chunk| chunk.lines.len())).collect());
        self.selected_section = None;
        self.selected_run = chunk.as_ref().map(|chunk| chunk.run_id);
        self.filter_task = None;
        self.is_filtering = false;
        self.chunk = chunk;
    }

    pub(crate) fn shared_chunk(&self) -> Option<Arc<LogChunk>> {
        self.chunk.clone()
    }

    pub(crate) fn set_task(&mut self, task: Task<()>) {
        self.task = Some(task);
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.is_loading
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn has_error(&self) -> bool {
        self.error.is_some()
    }

    pub(crate) fn start_loading(&mut self) {
        self.is_loading = true;
        self.error = None;
    }

    pub(crate) fn clear_content(&mut self) {
        self.task = None;
        self.is_loading = false;
        self.selected_run = None;
        self.widest_line = None;
        self.chunk = None;
        self.visible_lines = Arc::new(Vec::new());
        self.selected_section = None;
        self.filter_task = None;
        self.is_filtering = false;
    }

    pub(crate) fn apply_jobs_failure(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    pub(crate) fn apply_log_success(&mut self, chunk: LogChunk) {
        self.visible_lines = Arc::new((0..chunk.lines.len()).collect());
        self.chunk = Some(Arc::new(chunk));
        self.is_loading = false;
    }

    pub(crate) fn apply_log_failure(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
        self.is_loading = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reloads_legacy_cached_logs_and_keeps_current_output() {
        let mut state = WorkflowLogState::new();
        state.restore_chunk(Some(Arc::new(LogChunk::default())));
        assert!(state.chunk().is_none());
        assert!(state.visible_lines.is_empty());

        state.restore_chunk(Some(Arc::new(harbor_logs::parse_workflow_log(
            42, "output",
        ))));
        assert_eq!(state.chunk().unwrap().run_id, 42);
        assert_eq!(state.visible_lines.as_ref(), &[0]);
    }
}
