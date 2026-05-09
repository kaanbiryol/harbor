use gpui::{Context, Window};

use crate::actions::*;
use crate::workspace::AppView;

impl AppView {
    pub(super) fn select_next(
        &mut self,
        _: &SelectNextPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.pull_requests.is_empty() {
            let next = (self.selected_pr + 1) % self.pull_requests.len();
            self.select_pull_request(next, cx);
        } else {
            self.status = "No pull requests to select".to_string();
            cx.notify();
        }
    }

    pub(super) fn select_previous(
        &mut self,
        _: &SelectPreviousPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.pull_requests.is_empty() {
            let previous = if self.selected_pr == 0 {
                self.pull_requests.len() - 1
            } else {
                self.selected_pr - 1
            };
            self.select_pull_request(previous, cx);
        } else {
            self.status = "No pull requests to select".to_string();
            cx.notify();
        }
    }

    pub(super) fn open_selected(
        &mut self,
        _: &OpenSelectedPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let behavior = open_selected_pull_request_behavior(
            self.selected_pull_request_number(),
            !self.files.is_empty(),
            self.is_loading_details,
            self.is_loading_files,
            self.is_loading_reviews,
        );

        match behavior {
            OpenSelectedPullRequestBehavior::NoSelection => {
                self.status = "No pull request selected".to_string();
                cx.notify();
            }
            OpenSelectedPullRequestBehavior::ShowDetails { number, refresh } => {
                self.repository_switcher_open = false;
                self.pull_request_switcher_open = false;
                self.file_filter_popover_open = false;
                self.pull_request_inbox_visible = false;
                self.active_tab = PanelTab::Diff;
                self.status = format!("Opened PR #{number} details");

                if refresh {
                    self.refresh_selected_pull_request(cx);
                } else {
                    cx.notify();
                }
            }
        }
    }

    pub(super) fn refresh_selected(
        &mut self,
        _: &RefreshSelectedPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_pull_request_number().is_some() {
            self.refresh_selected_pull_request(cx);
        } else if let Some(repo) = self.configured_repo.clone() {
            self.refresh_pull_requests(repo, cx);
        } else {
            self.status =
                "Select a repository from the header before refreshing pull requests".to_string();
            cx.notify();
        }
    }

    pub(super) fn open_in_browser(
        &mut self,
        _: &OpenPullRequestInBrowser,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pr) = self.selected_pull_request() else {
            self.status = "No pull request selected".to_string();
            cx.notify();
            return;
        };

        let url = pr.url.clone();
        let number = pr.number;
        cx.open_url(&url);
        self.status = format!("Opened PR #{number} in browser");
        cx.notify();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OpenSelectedPullRequestBehavior {
    NoSelection,
    ShowDetails { number: u64, refresh: bool },
}

pub(crate) fn open_selected_pull_request_behavior(
    selected_pull_request_number: Option<u64>,
    has_loaded_files: bool,
    is_loading_details: bool,
    is_loading_files: bool,
    is_loading_reviews: bool,
) -> OpenSelectedPullRequestBehavior {
    let Some(number) = selected_pull_request_number else {
        return OpenSelectedPullRequestBehavior::NoSelection;
    };
    let refresh =
        !has_loaded_files && !is_loading_details && !is_loading_files && !is_loading_reviews;

    OpenSelectedPullRequestBehavior::ShowDetails { number, refresh }
}
