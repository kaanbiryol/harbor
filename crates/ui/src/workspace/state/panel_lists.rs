use gpui::ListState;

pub(crate) struct PanelListState {
    pub(crate) review: ListState,
    pub(crate) commits: ListState,
    pub(crate) checks: ListState,
    pub(crate) action_workflows: ListState,
    pub(crate) action_runs: ListState,
}

impl PanelListState {
    pub(crate) fn new(
        review: ListState,
        commits: ListState,
        checks: ListState,
        action_workflows: ListState,
        action_runs: ListState,
    ) -> Self {
        Self {
            review,
            commits,
            checks,
            action_workflows,
            action_runs,
        }
    }
}
