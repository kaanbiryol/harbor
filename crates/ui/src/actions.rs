use gpui::{App, KeyBinding, actions};
use harbor_domain::MergeMethod;

use crate::icons::Octicon;

pub(crate) const KEY_CONTEXT: &str = "HarborWorkspace";
const KEY_BINDING_CONTEXT: &str = "HarborWorkspace && !Input";

actions!(
    harbor,
    [
        /// Selects the next pull request in the current list.
        SelectNextPullRequest,
        /// Selects the previous pull request in the current list.
        SelectPreviousPullRequest,
        /// Opens the selected pull request details.
        OpenSelectedPullRequest,
        /// Advances to the next workspace panel tab.
        CyclePanelTab,
        /// Toggles the pull request inbox panel.
        TogglePullRequestInbox,
        /// Toggles the repository switcher.
        ToggleRepositorySwitcher,
        /// Closes the active panel or popover.
        ClosePanel,
        /// Refreshes the selected pull request and related data.
        RefreshSelectedPullRequest,
        /// Checks out the selected pull request into a local worktree.
        CheckoutPullRequest,
        /// Opens the selected pull request in the browser.
        OpenPullRequestInBrowser,
        /// Approves the selected pull request.
        ApprovePullRequest,
        /// Requests changes on the selected pull request.
        RequestChanges,
        /// Opens the approval comment dialog.
        OpenApproveCommentDialog,
        /// Opens the request-changes comment dialog.
        OpenRequestChangesCommentDialog,
        /// Merges the selected pull request.
        MergePullRequest,
        /// Merges the selected pull request with a merge commit.
        MergePullRequestWithMergeCommit,
        /// Rebases and merges the selected pull request.
        RebasePullRequest,
        /// Opens logs for the selected workflow run.
        OpenLogs,
        /// Dispatches the selected workflow build.
        TriggerBuild,
        /// Reruns failed jobs for the selected workflow run.
        RerunFailedJobs,
        /// Focuses filtering for the current list.
        FilterCurrentList,
        /// Selects the next changed file.
        SelectNextFile,
        /// Selects the previous changed file.
        SelectPreviousFile,
        /// Selects the next diff section.
        SelectNextHunk,
        /// Selects the previous diff section.
        SelectPreviousHunk,
        /// Copies the active changed file path.
        CopyActiveFilePath,
        /// Opens the active changed file on GitHub.
        OpenActiveFileOnGitHub,
        /// Chooses a local checkout for the current repository.
        ChooseLocalCheckout,
        /// Opens the current target in VS Code.
        OpenWithVsCode,
        /// Opens the current target in Cursor.
        OpenWithCursor,
        /// Opens the current target in Zed.
        OpenWithZed,
        /// Opens or reveals the current target in Finder.
        OpenWithFinder,
        /// Opens the current target in Terminal.
        OpenWithTerminal,
        /// Opens the current target in Ghostty.
        OpenWithGhostty,
        /// Opens the current target in Warp.
        OpenWithWarp,
        /// Opens the current target in Xcode.
        OpenWithXcode,
        /// Starts GitHub sign in.
        SignInToGitHub,
        /// Uses the authenticated GitHub CLI session.
        UseGitHubCli,
        /// Signs out of GitHub.
        SignOutOfGitHub,
        /// Opens application settings.
        OpenSettings,
        /// Closes application settings.
        CloseSettings,
        /// Switches GitHub auth to OAuth device login.
        SwitchGitHubAuthToOAuth,
        /// Switches GitHub auth to the authenticated GitHub CLI session.
        SwitchGitHubAuthToGhCli
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

    pub(crate) fn icon(self) -> Octicon {
        match self {
            Self::Diff => Octicon::CodeSquare,
            Self::Review => Octicon::CommentDiscussion,
            Self::Checks => Octicon::CheckCircle,
            Self::Actions => Octicon::Gear,
            Self::Logs => Octicon::Terminal,
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

#[derive(Clone, Debug)]
pub(crate) enum PullRequestAction {
    Approve { body: Option<String> },
    RequestChanges { body: Option<String> },
    Merge(MergeMethod),
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
        body: Option<String>,
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
        method: MergeMethod,
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
