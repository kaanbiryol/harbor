use gpui::{App, KeyBinding, actions};

pub(crate) const KEY_CONTEXT: &str = "HarborWorkspace";
const KEY_BINDING_CONTEXT: &str = "HarborWorkspace && !Input";

actions!(
    harbor,
    [
        SelectNextPullRequest,
        SelectPreviousPullRequest,
        OpenSelectedPullRequest,
        CyclePanelTab,
        TogglePullRequestInbox,
        ToggleRepositorySwitcher,
        ClosePanel,
        RefreshSelectedPullRequest,
        CheckoutPullRequest,
        OpenPullRequestInBrowser,
        ApprovePullRequest,
        RequestChanges,
        MergePullRequest,
        OpenLogs,
        TriggerBuild,
        RerunFailedJobs,
        FilterCurrentList,
        SelectNextFile,
        SelectPreviousFile,
        SelectNextHunk,
        SelectPreviousHunk,
        CopyActiveFilePath,
        OpenActiveFileOnGitHub,
        ChooseLocalCheckout,
        OpenWithVsCode,
        OpenWithCursor,
        OpenWithZed,
        OpenWithFinder,
        OpenWithTerminal,
        OpenWithGhostty,
        OpenWithWarp,
        OpenWithXcode
    ]
);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("down", SelectNextPullRequest, Some(KEY_BINDING_CONTEXT)),
        KeyBinding::new("up", SelectPreviousPullRequest, Some(KEY_BINDING_CONTEXT)),
    ]);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PanelTab {
    Diff,
    Review,
    Checks,
    Actions,
    Logs,
}

impl PanelTab {
    pub(crate) const ALL: [Self; 5] = [
        Self::Diff,
        Self::Review,
        Self::Checks,
        Self::Actions,
        Self::Logs,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Diff => "Diff",
            Self::Review => "Review",
            Self::Checks => "Checks",
            Self::Actions => "Actions",
            Self::Logs => "Logs",
        }
    }

    pub(crate) fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|tab| *tab == self)
            .expect("active tab must be present");
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum WorkflowAction {
    DispatchBuild,
    RerunFailedJobs,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum PullRequestAction {
    Approve,
    RequestChanges,
    Merge,
}

#[derive(Clone, Debug)]
pub(crate) enum WorkflowActionRequest {
    DispatchBuild {
        owner: String,
        repo: String,
        workflow_id: u64,
        git_ref: String,
        workflow_name: String,
    },
    RerunFailedJobs {
        owner: String,
        repo: String,
        run_id: u64,
        workflow_name: String,
    },
}

impl WorkflowActionRequest {
    pub(crate) fn start_status(&self) -> String {
        match self {
            Self::DispatchBuild {
                workflow_name,
                git_ref,
                ..
            } => format!("Dispatching {workflow_name} on {git_ref}"),
            Self::RerunFailedJobs { workflow_name, .. } => {
                format!("Requesting failed job rerun for {workflow_name}")
            }
        }
    }

    pub(crate) fn success_status(&self) -> String {
        match self {
            Self::DispatchBuild {
                workflow_name,
                git_ref,
                ..
            } => format!("Dispatched {workflow_name} on {git_ref}"),
            Self::RerunFailedJobs { workflow_name, .. } => {
                format!("Requested failed job rerun for {workflow_name}")
            }
        }
    }

    pub(crate) fn failure_label(&self) -> &'static str {
        match self {
            Self::DispatchBuild { .. } => "dispatch workflow",
            Self::RerunFailedJobs { .. } => "rerun failed jobs",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum PullRequestActionRequest {
    Approve {
        owner: String,
        repo: String,
        number: u64,
    },
    RequestChanges {
        owner: String,
        repo: String,
        number: u64,
        body: String,
    },
    Merge {
        owner: String,
        repo: String,
        number: u64,
        head_sha: String,
    },
}

impl PullRequestActionRequest {
    pub(crate) fn number(&self) -> u64 {
        match self {
            Self::Approve { number, .. }
            | Self::RequestChanges { number, .. }
            | Self::Merge { number, .. } => *number,
        }
    }

    pub(crate) fn start_status(&self) -> String {
        match self {
            Self::Approve { .. } => format!("Approving PR #{}", self.number()),
            Self::RequestChanges { .. } => {
                format!("Requesting changes on PR #{}", self.number())
            }
            Self::Merge { .. } => format!("Merging PR #{}", self.number()),
        }
    }

    pub(crate) fn success_status(&self) -> String {
        match self {
            Self::Approve { .. } => format!("Approved PR #{}", self.number()),
            Self::RequestChanges { .. } => {
                format!("Requested changes on PR #{}", self.number())
            }
            Self::Merge { .. } => format!("Merged PR #{}", self.number()),
        }
    }

    pub(crate) fn failure_label(&self) -> &'static str {
        match self {
            Self::Approve { .. } => "approve pull request",
            Self::RequestChanges { .. } => "request changes",
            Self::Merge { .. } => "merge pull request",
        }
    }
}

pub(crate) const DEFAULT_REQUEST_CHANGES_BODY: &str = "Changes requested from Harbor.";
