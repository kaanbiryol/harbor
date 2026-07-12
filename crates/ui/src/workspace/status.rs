use crate::actions::PullRequestMetadataField;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum LoadStatus {
    #[default]
    Idle,
    Loading,
    Loaded,
    Failed(String),
}

impl LoadStatus {
    pub(crate) fn start(&mut self) {
        *self = Self::Loading;
    }

    pub(crate) fn succeed(&mut self) {
        *self = Self::Loaded;
    }

    pub(crate) fn fail(&mut self, error: impl Into<String>) {
        *self = Self::Failed(error.into());
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::Idle;
    }

    pub(crate) fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    pub(crate) fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded)
    }

    pub(crate) fn is_finished(&self) -> bool {
        matches!(self, Self::Loaded | Self::Failed(_))
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            Self::Failed(error) => Some(error),
            Self::Idle | Self::Loading | Self::Loaded => None,
        }
    }

    pub(crate) fn clear_error(&mut self) {
        if matches!(self, Self::Failed(_)) {
            self.reset();
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
enum ActionStatus {
    #[default]
    Idle,
    Running,
    Failed(String),
}

impl ActionStatus {
    fn start(&mut self) {
        *self = Self::Running;
    }

    fn succeed(&mut self) {
        *self = Self::Idle;
    }

    fn fail(&mut self, error: impl Into<String>) {
        *self = Self::Failed(error.into());
    }

    fn clear_error(&mut self) {
        if matches!(self, Self::Failed(_)) {
            *self = Self::Idle;
        }
    }

    fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    fn error(&self) -> Option<&str> {
        match self {
            Self::Failed(error) => Some(error),
            Self::Idle | Self::Running => None,
        }
    }
}

#[derive(Default)]
pub(crate) struct ActionRuntimeState {
    workflow_action: ActionStatus,
    pull_request_action: ActionStatus,
    pull_request_description_action: ActionStatus,
    pull_request_metadata_action: ActionStatus,
    pull_request_metadata_field: Option<PullRequestMetadataField>,
}

impl ActionRuntimeState {
    pub(crate) fn workflow_action_running(&self) -> bool {
        self.workflow_action.is_running()
    }

    pub(crate) fn workflow_action_error(&self) -> Option<&str> {
        self.workflow_action.error()
    }

    pub(crate) fn start_workflow_action(&mut self) {
        self.workflow_action.start();
    }

    pub(crate) fn finish_workflow_action_success(&mut self) {
        self.workflow_action.succeed();
    }

    pub(crate) fn finish_workflow_action_failure(&mut self, error: impl Into<String>) {
        self.workflow_action.fail(error);
    }

    pub(crate) fn set_workflow_action_error(&mut self, error: impl Into<String>) {
        self.workflow_action.fail(error);
    }

    pub(crate) fn pull_request_action_running(&self) -> bool {
        self.pull_request_action.is_running()
    }

    pub(crate) fn pull_request_action_error(&self) -> Option<&str> {
        self.pull_request_action.error()
    }

    pub(crate) fn start_pull_request_action(&mut self) {
        self.pull_request_action.start();
    }

    pub(crate) fn finish_pull_request_action(&mut self) {
        self.pull_request_action.succeed();
    }

    pub(crate) fn finish_pull_request_action_failure(&mut self, error: impl Into<String>) {
        self.pull_request_action.fail(error);
    }

    pub(crate) fn set_pull_request_action_error(&mut self, error: impl Into<String>) {
        self.pull_request_action.fail(error);
    }

    pub(crate) fn pull_request_description_action_running(&self) -> bool {
        self.pull_request_description_action.is_running()
    }

    pub(crate) fn pull_request_description_action_error(&self) -> Option<&str> {
        self.pull_request_description_action.error()
    }

    pub(crate) fn start_pull_request_description_action(&mut self) {
        self.pull_request_description_action.start();
    }

    pub(crate) fn finish_pull_request_description_action(&mut self) {
        self.pull_request_description_action.succeed();
    }

    pub(crate) fn finish_pull_request_description_action_failure(
        &mut self,
        error: impl Into<String>,
    ) {
        self.pull_request_description_action.fail(error);
    }

    pub(crate) fn clear_pull_request_description_action_error(&mut self) {
        self.pull_request_description_action.clear_error();
    }

    pub(crate) fn pull_request_metadata_action_running(&self) -> bool {
        self.pull_request_metadata_action.is_running()
    }

    pub(crate) fn pull_request_metadata_action_error(&self) -> Option<&str> {
        self.pull_request_metadata_action.error()
    }

    pub(crate) fn pull_request_metadata_field(&self) -> Option<PullRequestMetadataField> {
        self.pull_request_metadata_field
    }

    pub(crate) fn start_pull_request_metadata_action(&mut self, field: PullRequestMetadataField) {
        self.pull_request_metadata_field = Some(field);
        self.pull_request_metadata_action.start();
    }

    pub(crate) fn finish_pull_request_metadata_action(&mut self) {
        self.pull_request_metadata_action.succeed();
        self.pull_request_metadata_field = None;
    }

    pub(crate) fn finish_pull_request_metadata_action_failure(&mut self, error: impl Into<String>) {
        self.pull_request_metadata_action.fail(error);
    }

    pub(crate) fn clear_errors(&mut self) {
        self.workflow_action.clear_error();
        self.pull_request_action.clear_error();
        self.pull_request_description_action.clear_error();
        self.pull_request_metadata_action.clear_error();
        self.pull_request_metadata_field = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_status_transitions_and_error_access() {
        let mut status = LoadStatus::default();
        assert!(!status.is_loading());
        assert!(!status.is_loaded());
        assert_eq!(status.error(), None);

        status.start();
        assert!(status.is_loading());
        assert!(!status.is_finished());

        status.fail("network");
        assert!(!status.is_loading());
        assert!(status.is_finished());
        assert_eq!(status.error(), Some("network"));

        status.succeed();
        assert!(status.is_loaded());
        assert_eq!(status.error(), None);

        status.reset();
        assert_eq!(status, LoadStatus::Idle);
    }

    #[test]
    fn action_runtime_keeps_running_and_errors_exclusive() {
        let mut state = ActionRuntimeState::default();
        assert!(!state.workflow_action_running());
        assert_eq!(state.workflow_action_error(), None);
        assert!(!state.pull_request_action_running());
        assert_eq!(state.pull_request_action_error(), None);
        assert!(!state.pull_request_metadata_action_running());
        assert_eq!(state.pull_request_metadata_action_error(), None);
        assert!(!state.pull_request_description_action_running());
        assert_eq!(state.pull_request_description_action_error(), None);

        state.set_workflow_action_error("missing workflow");
        assert!(!state.workflow_action_running());
        assert_eq!(state.workflow_action_error(), Some("missing workflow"));

        state.start_workflow_action();
        assert!(state.workflow_action_running());
        assert_eq!(state.workflow_action_error(), None);

        state.finish_workflow_action_failure("dispatch failed");
        assert!(!state.workflow_action_running());
        assert_eq!(state.workflow_action_error(), Some("dispatch failed"));

        state.start_pull_request_action();
        assert!(state.pull_request_action_running());
        assert_eq!(state.pull_request_action_error(), None);

        state.finish_pull_request_action();
        assert!(!state.pull_request_action_running());
        assert_eq!(state.pull_request_action_error(), None);

        state.start_pull_request_description_action();
        assert!(state.pull_request_description_action_running());
        state.finish_pull_request_description_action_failure("description failed");
        assert_eq!(
            state.pull_request_description_action_error(),
            Some("description failed")
        );

        state.start_pull_request_metadata_action(PullRequestMetadataField::Reviewer);
        assert!(state.pull_request_metadata_action_running());
        assert_eq!(
            state.pull_request_metadata_field(),
            Some(PullRequestMetadataField::Reviewer)
        );
        state.finish_pull_request_metadata_action_failure("reviewer failed");
        assert_eq!(
            state.pull_request_metadata_action_error(),
            Some("reviewer failed")
        );

        state.clear_errors();
        assert_eq!(state.workflow_action_error(), None);
        assert_eq!(state.pull_request_action_error(), None);
        assert_eq!(state.pull_request_description_action_error(), None);
        assert_eq!(state.pull_request_metadata_action_error(), None);
        assert_eq!(state.pull_request_metadata_field(), None);
    }
}
