use chrono::{DateTime, Utc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshKind {
    RepositoryPullRequests,
    SelectedPullRequest,
    WorkflowRun,
    Logs,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefreshRequest {
    pub kind: RefreshKind,
    pub requested_at: DateTime<Utc>,
}
