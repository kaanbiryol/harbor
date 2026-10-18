use gpui::ListState;

pub(crate) struct PanelListState {
    pub(crate) commits: ListState,
    pub(crate) checks: ListState,
    pub(crate) action_workflows: ListState,
    pub(crate) action_runs: ListState,
}

impl PanelListState {
    pub(crate) fn new(
        commits: ListState,
        checks: ListState,
        action_workflows: ListState,
        action_runs: ListState,
    ) -> Self {
        Self {
            commits,
            checks,
            action_workflows,
            action_runs,
        }
    }
}
