use std::sync::Arc;

use gpui::{Task, UniformListScrollHandle};
use harbor_logs::LogChunk;

pub(crate) struct WorkflowLogState {
    chunk: Option<Arc<LogChunk>>,
    task: Option<Task<()>>,
    pub(crate) list_scroll: UniformListScrollHandle,
    is_loading: bool,
    error: Option<String>,
}

impl WorkflowLogState {
    pub(crate) fn new() -> Self {
        Self {
            chunk: None,
            task: None,
            list_scroll: UniformListScrollHandle::new(),
            is_loading: false,
            error: None,
        }
    }

    pub(crate) fn chunk(&self) -> Option<&LogChunk> {
        self.chunk.as_deref()
    }

    pub(crate) fn restore_chunk(&mut self, chunk: Option<Arc<LogChunk>>) {
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
        self.chunk = None;
    }

    pub(crate) fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
    }

    pub(crate) fn clear_content(&mut self) {
        self.chunk = None;
    }

    pub(crate) fn clear_error(&mut self) {
        self.error = None;
    }

    pub(crate) fn apply_jobs_failure(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    pub(crate) fn apply_log_success(&mut self, chunk: LogChunk) {
        self.chunk = Some(Arc::new(chunk));
        self.is_loading = false;
    }

    pub(crate) fn apply_log_failure(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
        self.is_loading = false;
    }
}
