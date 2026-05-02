use gpui::{
    App, Context, FocusHandle, Focusable, IntoElement, KeyBinding, Render, Window, actions, div,
    prelude::*, px, rgb,
};
use gpui_component::{Sizable, button::Button};
use harbor_domain::{
    ChecksSummary, DiffFile, FileStatus, Label, MergeState, PullRequest, PullRequestState, RepoId,
    ReviewDecision,
};
use harbor_github::{GhCliTransport, GitHubClient};

const KEY_CONTEXT: &str = "HarborWorkspace";

actions!(
    harbor,
    [
        SelectNextPullRequest,
        SelectPreviousPullRequest,
        OpenSelectedPullRequest,
        CyclePanelTab,
        ToggleCommandPalette,
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
        FilterCurrentList
    ]
);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("j", SelectNextPullRequest, Some(KEY_CONTEXT)),
        KeyBinding::new("k", SelectPreviousPullRequest, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", OpenSelectedPullRequest, Some(KEY_CONTEXT)),
        KeyBinding::new("tab", CyclePanelTab, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-k", ToggleCommandPalette, Some(KEY_CONTEXT)),
        KeyBinding::new("cmd-p", ToggleCommandPalette, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", ClosePanel, Some(KEY_CONTEXT)),
        KeyBinding::new("r", RefreshSelectedPullRequest, Some(KEY_CONTEXT)),
        KeyBinding::new("c", CheckoutPullRequest, Some(KEY_CONTEXT)),
        KeyBinding::new("o", OpenPullRequestInBrowser, Some(KEY_CONTEXT)),
        KeyBinding::new("a", ApprovePullRequest, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-a", RequestChanges, Some(KEY_CONTEXT)),
        KeyBinding::new("m", MergePullRequest, Some(KEY_CONTEXT)),
        KeyBinding::new("l", OpenLogs, Some(KEY_CONTEXT)),
        KeyBinding::new("b", TriggerBuild, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-r", RerunFailedJobs, Some(KEY_CONTEXT)),
        KeyBinding::new("/", FilterCurrentList, Some(KEY_CONTEXT)),
    ]);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PanelTab {
    Diff,
    Checks,
    Actions,
    Logs,
}

impl PanelTab {
    const ALL: [Self; 4] = [Self::Diff, Self::Checks, Self::Actions, Self::Logs];

    fn label(self) -> &'static str {
        match self {
            Self::Diff => "Diff",
            Self::Checks => "Checks",
            Self::Actions => "Actions",
            Self::Logs => "Logs",
        }
    }

    fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|tab| *tab == self)
            .expect("active tab must be present");
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommandSpec {
    shortcut: &'static str,
    title: &'static str,
}

const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        shortcut: "cmd+k",
        title: "Open command palette",
    },
    CommandSpec {
        shortcut: "cmd+p",
        title: "Switch repository or pull request",
    },
    CommandSpec {
        shortcut: "j/k",
        title: "Move pull request selection",
    },
    CommandSpec {
        shortcut: "enter",
        title: "Open selected pull request",
    },
    CommandSpec {
        shortcut: "tab",
        title: "Cycle right panel",
    },
    CommandSpec {
        shortcut: "r",
        title: "Refresh selected pull request",
    },
    CommandSpec {
        shortcut: "l",
        title: "Open logs",
    },
    CommandSpec {
        shortcut: "shift+r",
        title: "Rerun failed jobs",
    },
];

pub struct AppView {
    focus_handle: FocusHandle,
    pull_requests: Vec<PullRequest>,
    files: Vec<DiffFile>,
    selected_pr: usize,
    active_tab: PanelTab,
    command_palette_open: bool,
    configured_repo: Option<RepoId>,
    is_loading_prs: bool,
    load_error: Option<String>,
    did_focus: bool,
    status: String,
}

impl AppView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let configured_repo = configured_repo_from_env();
        let pull_requests = if configured_repo.is_some() {
            Vec::new()
        } else {
            fake_pull_requests()
        };
        let files = if configured_repo.is_some() {
            Vec::new()
        } else {
            fake_files()
        };
        let status = configured_repo
            .as_ref()
            .map(|repo| format!("Loading open pull requests from {}", repo.full_name()))
            .unwrap_or_else(|| {
                "Using fake data. Set HARBOR_REPO=owner/repo to load GitHub PRs.".to_string()
            });

        let mut view = Self {
            focus_handle: cx.focus_handle(),
            pull_requests,
            files,
            selected_pr: 0,
            active_tab: PanelTab::Diff,
            command_palette_open: false,
            configured_repo,
            is_loading_prs: false,
            load_error: None,
            did_focus: false,
            status,
        };

        if let Some(repo) = view.configured_repo.clone() {
            view.load_pull_requests(repo, cx);
        }

        view
    }

    fn selected_pull_request(&self) -> Option<&PullRequest> {
        self.pull_requests.get(self.selected_pr)
    }

    fn selected_pr_label(&self) -> String {
        self.selected_pull_request()
            .map(|pr| format!("PR #{}", pr.number))
            .unwrap_or_else(|| "no selected pull request".to_string())
    }

    fn load_pull_requests(&mut self, repo: RepoId, cx: &mut Context<Self>) {
        self.is_loading_prs = true;
        self.load_error = None;
        self.status = format!("Loading open pull requests from {}", repo.full_name());

        let owner = repo.owner.clone();
        let name = repo.name.clone();

        cx.spawn(async move |this, cx| {
            let result = GitHubClient::new(GhCliTransport)
                .list_open_pull_requests(&owner, &name)
                .await;

            _ = this.update(cx, |view, cx| {
                view.is_loading_prs = false;

                match result {
                    Ok(pull_requests) => {
                        let count = pull_requests.len();
                        view.pull_requests = pull_requests;
                        view.files.clear();
                        view.selected_pr = 0;
                        view.load_error = None;
                        view.status =
                            format!("Loaded {count} open pull requests from {owner}/{name}");
                    }
                    Err(error) => {
                        view.pull_requests.clear();
                        view.files.clear();
                        view.selected_pr = 0;
                        view.load_error = Some(error.to_string());
                        view.status = format!("Failed to load pull requests from {owner}/{name}");
                    }
                }

                cx.notify();
            });
        })
        .detach();
    }

    fn select_next(&mut self, _: &SelectNextPullRequest, _: &mut Window, cx: &mut Context<Self>) {
        if !self.pull_requests.is_empty() {
            self.selected_pr = (self.selected_pr + 1) % self.pull_requests.len();
            self.status = format!("Selected {}", self.selected_pr_label());
        } else {
            self.status = "No pull requests to select".to_string();
        }

        cx.notify();
    }

    fn select_previous(
        &mut self,
        _: &SelectPreviousPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.pull_requests.is_empty() {
            self.selected_pr = if self.selected_pr == 0 {
                self.pull_requests.len() - 1
            } else {
                self.selected_pr - 1
            };
            self.status = format!("Selected {}", self.selected_pr_label());
        } else {
            self.status = "No pull requests to select".to_string();
        }

        cx.notify();
    }

    fn open_selected(
        &mut self,
        _: &OpenSelectedPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.status = format!("Opened {} in the local shell", self.selected_pr_label());
        cx.notify();
    }

    fn cycle_panel_tab(&mut self, _: &CyclePanelTab, _: &mut Window, cx: &mut Context<Self>) {
        self.active_tab = self.active_tab.next();
        self.status = format!("Switched to {} panel", self.active_tab.label());
        cx.notify();
    }

    fn toggle_command_palette(
        &mut self,
        _: &ToggleCommandPalette,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.command_palette_open = !self.command_palette_open;
        self.status = if self.command_palette_open {
            "Command palette opened".to_string()
        } else {
            "Command palette closed".to_string()
        };
        cx.notify();
    }

    fn close_panel(&mut self, _: &ClosePanel, _: &mut Window, cx: &mut Context<Self>) {
        self.command_palette_open = false;
        self.status = "Closed transient UI".to_string();
        cx.notify();
    }

    fn set_placeholder_status(&mut self, label: &str, cx: &mut Context<Self>) {
        self.status = format!(
            "{label} is wired as a command placeholder for {}",
            self.selected_pr_label()
        );
        cx.notify();
    }

    fn refresh_selected(
        &mut self,
        _: &RefreshSelectedPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(repo) = self.configured_repo.clone() {
            self.load_pull_requests(repo, cx);
        } else {
            self.set_placeholder_status("Refresh", cx);
        }
    }

    fn checkout_pr(&mut self, _: &CheckoutPullRequest, _: &mut Window, cx: &mut Context<Self>) {
        self.set_placeholder_status("Checkout", cx);
    }

    fn open_in_browser(
        &mut self,
        _: &OpenPullRequestInBrowser,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_placeholder_status("Open in browser", cx);
    }

    fn approve_pr(&mut self, _: &ApprovePullRequest, _: &mut Window, cx: &mut Context<Self>) {
        self.set_placeholder_status("Approve", cx);
    }

    fn request_changes(&mut self, _: &RequestChanges, _: &mut Window, cx: &mut Context<Self>) {
        self.set_placeholder_status("Request changes", cx);
    }

    fn merge_pr(&mut self, _: &MergePullRequest, _: &mut Window, cx: &mut Context<Self>) {
        self.set_placeholder_status("Merge", cx);
    }

    fn open_logs(&mut self, _: &OpenLogs, _: &mut Window, cx: &mut Context<Self>) {
        self.active_tab = PanelTab::Logs;
        self.set_placeholder_status("Open logs", cx);
    }

    fn trigger_build(&mut self, _: &TriggerBuild, _: &mut Window, cx: &mut Context<Self>) {
        self.set_placeholder_status("Trigger build", cx);
    }

    fn rerun_failed(&mut self, _: &RerunFailedJobs, _: &mut Window, cx: &mut Context<Self>) {
        self.set_placeholder_status("Rerun failed jobs", cx);
    }

    fn filter_current_list(
        &mut self,
        _: &FilterCurrentList,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_placeholder_status("Filter", cx);
    }
}

impl Focusable for AppView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.did_focus {
            window.focus(&self.focus_handle, cx);
            self.did_focus = true;
        }

        let selected_pr = self.selected_pull_request().cloned();

        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle(cx))
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::open_selected))
            .on_action(cx.listener(Self::cycle_panel_tab))
            .on_action(cx.listener(Self::toggle_command_palette))
            .on_action(cx.listener(Self::close_panel))
            .on_action(cx.listener(Self::refresh_selected))
            .on_action(cx.listener(Self::checkout_pr))
            .on_action(cx.listener(Self::open_in_browser))
            .on_action(cx.listener(Self::approve_pr))
            .on_action(cx.listener(Self::request_changes))
            .on_action(cx.listener(Self::merge_pr))
            .on_action(cx.listener(Self::open_logs))
            .on_action(cx.listener(Self::trigger_build))
            .on_action(cx.listener(Self::rerun_failed))
            .on_action(cx.listener(Self::filter_current_list))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x101214))
            .text_color(rgb(0xe6e8eb))
            .child(self.render_header())
            .when(self.command_palette_open, |element| {
                element.child(self.render_command_palette())
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .gap_2()
                    .p_2()
                    .child(self.render_inbox())
                    .child(self.render_details(selected_pr.as_ref()))
                    .child(self.render_panel(selected_pr.as_ref())),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(rgb(0x9aa4b2))
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .child(self.status.clone()),
            )
    }
}

impl AppView {
    fn render_header(&self) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .py_3()
            .border_1()
            .border_color(rgb(0x242a31))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().text_lg().child("Harbor"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x9aa4b2))
                            .child("native GitHub pull request cockpit"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("repo-switcher")
                            .label("cmd+p")
                            .small()
                            .outline(),
                    )
                    .child(Button::new("command-palette").label("cmd+k").small()),
            )
    }

    fn render_command_palette(&self) -> impl IntoElement {
        div()
            .mx_2()
            .mt_2()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x3a424c))
            .bg(rgb(0x171b20))
            .child(
                div()
                    .pb_2()
                    .text_sm()
                    .text_color(rgb(0xf1f5f9))
                    .child("Command palette placeholder"),
            )
            .children(COMMANDS.iter().map(|command| {
                div()
                    .flex()
                    .justify_between()
                    .py_1()
                    .text_sm()
                    .child(command.title)
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x9aa4b2))
                            .child(command.shortcut),
                    )
            }))
    }

    fn render_inbox(&self) -> impl IntoElement {
        div()
            .w(px(320.))
            .flex()
            .flex_col()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x242a31))
            .bg(rgb(0x15191e))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(rgb(0xf1f5f9))
                    .child("Pull request inbox")
                    .child(
                        div().pt_1().text_xs().text_color(rgb(0x9aa4b2)).child(
                            self.configured_repo
                                .as_ref()
                                .map(RepoId::full_name)
                                .unwrap_or_else(|| "fake data".to_string()),
                        ),
                    ),
            )
            .when(self.is_loading_prs, |element| {
                element.child(
                    div()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(rgb(0x9aa4b2))
                        .child("Loading open pull requests..."),
                )
            })
            .when(
                !self.is_loading_prs && self.load_error.is_some(),
                |element| {
                    element.child(
                        div()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(rgb(0xf87171))
                            .child(self.load_error.clone().unwrap_or_default()),
                    )
                },
            )
            .when(
                !self.is_loading_prs && self.load_error.is_none() && self.pull_requests.is_empty(),
                |element| {
                    element.child(
                        div()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(rgb(0x9aa4b2))
                            .child("No open pull requests"),
                    )
                },
            )
            .children(self.pull_requests.iter().enumerate().map(|(index, pr)| {
                let selected = index == self.selected_pr;
                div()
                    .px_3()
                    .py_2()
                    .border_1()
                    .border_color(rgb(0x20252b))
                    .when(selected, |element| element.bg(rgb(0x243244)))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .gap_2()
                            .text_sm()
                            .child(format!("#{} {}", pr.number, pr.title))
                            .child(render_checks_summary(pr.checks_summary)),
                    )
                    .child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(rgb(0x9aa4b2))
                            .child(format!(
                                "{} into {} by {}",
                                pr.head_ref, pr.base_ref, pr.author
                            )),
                    )
            }))
    }

    fn render_details(&self, pr: Option<&PullRequest>) -> impl IntoElement {
        let Some(pr) = pr else {
            return div()
                .w(px(360.))
                .flex()
                .flex_col()
                .rounded_md()
                .border_1()
                .border_color(rgb(0x242a31))
                .bg(rgb(0x15191e))
                .p_3()
                .text_sm()
                .text_color(rgb(0x9aa4b2))
                .child("Select a pull request to see details")
                .into_any_element();
        };

        div()
            .w(px(360.))
            .flex()
            .flex_col()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x242a31))
            .bg(rgb(0x15191e))
            .child(
                div()
                    .p_3()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .child(
                        div()
                            .text_sm()
                            .child(format!("#{} {}", pr.number, pr.title)),
                    )
                    .child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(rgb(0x9aa4b2))
                            .child(format!("{} / {}", pr.repo.full_name(), pr.head_sha)),
                    )
                    .child(
                        div()
                            .pt_2()
                            .flex()
                            .gap_2()
                            .child(render_review_decision(pr.review_decision))
                            .child(render_merge_state(pr.merge_state))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0xfbbf24))
                                    .child(format!("{} unresolved", pr.unresolved_threads)),
                            ),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(rgb(0x9aa4b2))
                    .child("Changed files"),
            )
            .when(self.files.is_empty(), |element| {
                element.child(
                    div()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(rgb(0x9aa4b2))
                        .child("Changed files load in the next milestone"),
                )
            })
            .children(self.files.iter().map(|file| {
                div()
                    .px_3()
                    .py_2()
                    .border_1()
                    .border_color(rgb(0x20252b))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_sm()
                            .child(file.path.clone())
                            .child(format!("+{} -{}", file.additions, file.deletions)),
                    )
                    .child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(rgb(0x9aa4b2))
                            .child(format!("{:?}", file.status)),
                    )
            }))
            .into_any_element()
    }

    fn render_panel(&self, pr: Option<&PullRequest>) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x242a31))
            .bg(rgb(0x15191e))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .p_2()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .children(PanelTab::ALL.iter().map(|tab| {
                        let active = *tab == self.active_tab;
                        div()
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .text_sm()
                            .when(active, |element| element.bg(rgb(0x243244)))
                            .child(tab.label())
                    })),
            )
            .child(
                div().flex_1().p_3().text_sm().child(match self.active_tab {
                    PanelTab::Diff => render_diff_panel(&self.files).into_any_element(),
                    PanelTab::Checks => {
                        render_checks_panel(pr.map(|pr| pr.checks_summary).unwrap_or_default())
                            .into_any_element()
                    }
                    PanelTab::Actions => render_actions_panel().into_any_element(),
                    PanelTab::Logs => render_logs_panel().into_any_element(),
                }),
            )
    }
}

fn render_checks_summary(summary: ChecksSummary) -> impl IntoElement {
    let color = if summary.failed > 0 {
        rgb(0xf87171)
    } else if summary.pending > 0 {
        rgb(0xfbbf24)
    } else {
        rgb(0x34d399)
    };

    div()
        .text_xs()
        .text_color(color)
        .child(format!("{}/{}", summary.passed, summary.total))
}

fn render_review_decision(decision: Option<ReviewDecision>) -> impl IntoElement {
    let label = match decision {
        Some(ReviewDecision::Approved) => "approved",
        Some(ReviewDecision::ChangesRequested) => "changes requested",
        Some(ReviewDecision::ReviewRequired) => "review required",
        None => "no review",
    };

    div().text_xs().text_color(rgb(0x93c5fd)).child(label)
}

fn render_merge_state(state: Option<MergeState>) -> impl IntoElement {
    let label = match state {
        Some(MergeState::Clean) => "mergeable",
        Some(MergeState::Dirty) => "dirty",
        Some(MergeState::Blocked) => "blocked",
        Some(MergeState::Behind) => "behind",
        Some(MergeState::Unknown) | None => "unknown",
    };

    div().text_xs().text_color(rgb(0x9aa4b2)).child(label)
}

fn render_diff_panel(files: &[DiffFile]) -> impl IntoElement {
    let patch = files
        .first()
        .and_then(|file| file.patch.as_deref())
        .unwrap_or("Diff unavailable via GitHub API. Local checkout fallback will be added.");

    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(rgb(0xf1f5f9))
                .child("Unified diff preview"),
        )
        .child(
            div()
                .rounded_sm()
                .border_1()
                .border_color(rgb(0x242a31))
                .bg(rgb(0x0c0f12))
                .p_3()
                .font_family("Menlo")
                .text_xs()
                .child(patch.to_string()),
        )
}

fn render_checks_panel(summary: ChecksSummary) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child("Checks summary")
        .child(format!("passed: {}", summary.passed))
        .child(format!("failed: {}", summary.failed))
        .child(format!("pending: {}", summary.pending))
        .child(format!("skipped: {}", summary.skipped))
}

fn render_actions_panel() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child("Workflow actions")
        .child("Rerun failed jobs and workflow_dispatch commands will be wired after real GitHub data.")
}

fn render_logs_panel() -> impl IntoElement {
    div().flex().flex_col().gap_2().child("Logs").child(
        "The log viewer will use chunked, virtualized rendering for large GitHub Actions output.",
    )
}

fn fake_pull_requests() -> Vec<PullRequest> {
    let repo = RepoId::new("sixt", "mobile-app");

    vec![
        PullRequest {
            repo: repo.clone(),
            number: 1842,
            title: "speed up pull request inbox refresh".to_string(),
            body: Some("Cache first, refresh in the background.".to_string()),
            author: "alex".to_string(),
            url: "https://github.com/sixt/mobile-app/pull/1842".to_string(),
            state: PullRequestState::Open,
            is_draft: false,
            head_ref: "feature/pr-cache".to_string(),
            base_ref: "main".to_string(),
            head_sha: "a1b2c3d".to_string(),
            review_decision: Some(ReviewDecision::ReviewRequired),
            merge_state: Some(MergeState::Clean),
            labels: vec![Label {
                name: "performance".to_string(),
                color: Some("34d399".to_string()),
            }],
            checks_summary: ChecksSummary {
                total: 18,
                passed: 16,
                failed: 0,
                pending: 2,
                skipped: 0,
            },
            unresolved_threads: 3,
        },
        PullRequest {
            repo: repo.clone(),
            number: 1837,
            title: "render failed action steps inline".to_string(),
            body: None,
            author: "maria".to_string(),
            url: "https://github.com/sixt/mobile-app/pull/1837".to_string(),
            state: PullRequestState::Open,
            is_draft: false,
            head_ref: "ci/failed-step-focus".to_string(),
            base_ref: "main".to_string(),
            head_sha: "d4e5f6a".to_string(),
            review_decision: Some(ReviewDecision::ChangesRequested),
            merge_state: Some(MergeState::Blocked),
            labels: vec![Label {
                name: "ci".to_string(),
                color: Some("fbbf24".to_string()),
            }],
            checks_summary: ChecksSummary {
                total: 21,
                passed: 18,
                failed: 2,
                pending: 1,
                skipped: 0,
            },
            unresolved_threads: 7,
        },
        PullRequest {
            repo,
            number: 1829,
            title: "add review thread domain model".to_string(),
            body: None,
            author: "kaan".to_string(),
            url: "https://github.com/sixt/mobile-app/pull/1829".to_string(),
            state: PullRequestState::Open,
            is_draft: true,
            head_ref: "review/thread-model".to_string(),
            base_ref: "main".to_string(),
            head_sha: "f7a8b9c".to_string(),
            review_decision: None,
            merge_state: Some(MergeState::Unknown),
            labels: vec![Label {
                name: "review".to_string(),
                color: Some("93c5fd".to_string()),
            }],
            checks_summary: ChecksSummary {
                total: 17,
                passed: 17,
                failed: 0,
                pending: 0,
                skipped: 0,
            },
            unresolved_threads: 0,
        },
    ]
}

fn configured_repo_from_env() -> Option<RepoId> {
    std::env::var("HARBOR_REPO")
        .ok()
        .or_else(|| std::env::var("GH_REPO").ok())
        .and_then(|value| parse_repo_id(&value))
}

fn parse_repo_id(value: &str) -> Option<RepoId> {
    let (owner, name) = value.split_once('/')?;

    if owner.is_empty() || name.is_empty() || name.contains('/') {
        None
    } else {
        Some(RepoId::new(owner, name))
    }
}

fn fake_files() -> Vec<DiffFile> {
    vec![
        DiffFile {
            path: "crates/ui/src/inbox.rs".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 42,
            deletions: 11,
            changes: 53,
            patch: Some(
                "@@ -14,6 +14,13 @@\n pub struct InboxState {\n+    selected: usize,\n+    visible_rows: Range<usize>,\n }\n+\n+impl InboxState {\n+    pub fn move_selection(&mut self, delta: i32) { /* fake diff */ }\n+}\n"
                    .to_string(),
            ),
        },
        DiffFile {
            path: "crates/github/src/transport.rs".to_string(),
            previous_path: None,
            status: FileStatus::Added,
            additions: 88,
            deletions: 0,
            changes: 88,
            patch: None,
        },
        DiffFile {
            path: "crates/logs/src/parser.rs".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 65,
            deletions: 22,
            changes: 87,
            patch: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::parse_repo_id;

    #[test]
    fn parses_owner_and_repo() {
        let repo = parse_repo_id("acme/app").unwrap();

        assert_eq!(repo.owner, "acme");
        assert_eq!(repo.name, "app");
    }

    #[test]
    fn rejects_invalid_repo_values() {
        assert!(parse_repo_id("").is_none());
        assert!(parse_repo_id("acme").is_none());
        assert!(parse_repo_id("/app").is_none());
        assert!(parse_repo_id("acme/").is_none());
        assert!(parse_repo_id("acme/app/extra").is_none());
    }
}
