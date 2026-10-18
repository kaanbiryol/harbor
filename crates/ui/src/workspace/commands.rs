use gpui::{ClipboardItem, Context, Window};

use crate::actions::*;
use crate::workspace::AppView;

impl AppView {
    pub(super) fn select_next(
        &mut self,
        _: &SelectNextPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let visible_indices = self.visible_pull_request_indices();
        if !visible_indices.is_empty() {
            let current_position = visible_indices
                .iter()
                .position(|index| *index == self.selected_pull_request_index())
                .unwrap_or(visible_indices.len().saturating_sub(1));
            let next = visible_indices[(current_position + 1) % visible_indices.len()];
            self.select_pull_request(next, cx);
        } else if self.has_active_pull_request_filters() {
            self.status = "No pull requests match filters".to_string();
            cx.notify();
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
        let visible_indices = self.visible_pull_request_indices();
        if !visible_indices.is_empty() {
            let current_position = visible_indices
                .iter()
                .position(|index| *index == self.selected_pull_request_index())
                .unwrap_or(0);
            let previous_position = if current_position == 0 {
                visible_indices.len() - 1
            } else {
                current_position - 1
            };
            self.select_pull_request(visible_indices[previous_position], cx);
        } else if self.has_active_pull_request_filters() {
            self.status = "No pull requests match filters".to_string();
            cx.notify();
        } else {
            self.status = "No pull requests to select".to_string();
            cx.notify();
        }
    }

    pub(super) fn refresh_selected(
        &mut self,
        _: &RefreshSelectedPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_tab == PanelTab::Actions {
            self.refresh_repository_actions(cx);
        }

        if self.selected_pull_request_number().is_some() {
            self.refresh_selected_pull_request(cx);
        } else if let Some(repo) = self.repository_state.configured_repo_cloned() {
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

    pub(crate) fn copy_to_clipboard(
        &mut self,
        text: String,
        description: &str,
        cx: &mut Context<Self>,
    ) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.status = format!("Copied {description}");
        cx.notify();
    }

    pub(crate) fn open_pull_request_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(pr) = self.pull_requests.get(index) else {
            return;
        };
        cx.open_url(&pr.url);
        self.status = format!("Opened PR #{} in browser", pr.number);
        cx.notify();
    }

    pub(crate) fn copy_pull_request_link_at(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(pr) = self.pull_requests.get(index) {
            let description = format!("link to PR #{}", pr.number);
            self.copy_to_clipboard(pr.url.clone(), &description, cx);
        }
    }

    pub(crate) fn copy_pull_request_branch_at(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(pr) = self.pull_requests.get(index) {
            let description = format!("branch {}", pr.head_ref);
            self.copy_to_clipboard(pr.head_ref.clone(), &description, cx);
        }
    }

    pub(crate) fn checkout_pull_request_at(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.pull_requests.len() {
            return;
        }
        self.select_pull_request(index, cx);
        self.checkout_pr(&CheckoutPullRequest, window, cx);
    }

    pub(crate) fn open_commit_on_github(&mut self, sha: &str, cx: &mut Context<Self>) {
        let Some(pr) = self.selected_pull_request() else {
            return;
        };
        cx.open_url(&format!("{}/commits/{sha}", pr.url));
        self.status = format!("Opened commit {} on GitHub", short_commit_sha(sha));
        cx.notify();
    }

    pub(crate) fn copy_commit_sha(&mut self, sha: &str, cx: &mut Context<Self>) {
        let description = format!("commit {}", short_commit_sha(sha));
        self.copy_to_clipboard(sha.to_string(), &description, cx);
    }
}

fn short_commit_sha(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}
