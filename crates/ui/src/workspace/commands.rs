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

    pub(super) fn open_selected(
        &mut self,
        _: &OpenSelectedPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let behavior = open_selected_pull_request_behavior(
            self.selected_pull_request_number(),
            !self.detail_state.files().is_empty(),
            self.detail_state.details_loading(),
            self.detail_state.files_loading(),
            self.review_state.reviews_loading(),
        );

        match behavior {
            OpenSelectedPullRequestBehavior::NoSelection => {
                self.status = "No pull request selected".to_string();
                cx.notify();
            }
            OpenSelectedPullRequestBehavior::ShowDetails { number, refresh } => {
                self.repository_state.repository_switcher_open = false;
                self.pull_request_inbox_search_open = false;
                self.pull_request_filter_popover_open = false;
                self.file_filter_popover_open = false;
                self.review_action_comment_target = None;
                self.pull_request_inbox.set_visible(false);
                self.active_tab = PanelTab::Diff;
                self.status = format!("Opened PR #{number} details");

                if refresh {
                    self.load_selected_pull_request(cx);
                } else {
                    self.load_active_panel_data_if_needed(cx);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_selected_pull_request_details_without_selection() {
        assert_eq!(
            open_selected_pull_request_behavior(None, false, false, false, false),
            OpenSelectedPullRequestBehavior::NoSelection
        );
    }

    #[test]
    fn opens_selected_pull_request_details_and_refreshes_when_empty() {
        assert_eq!(
            open_selected_pull_request_behavior(Some(7), false, false, false, false),
            OpenSelectedPullRequestBehavior::ShowDetails {
                number: 7,
                refresh: true
            }
        );
    }

    #[test]
    fn opens_selected_pull_request_details_without_duplicate_refresh() {
        assert_eq!(
            open_selected_pull_request_behavior(Some(7), true, false, false, false),
            OpenSelectedPullRequestBehavior::ShowDetails {
                number: 7,
                refresh: false
            }
        );
        assert_eq!(
            open_selected_pull_request_behavior(Some(7), false, true, false, false),
            OpenSelectedPullRequestBehavior::ShowDetails {
                number: 7,
                refresh: false
            }
        );
        assert_eq!(
            open_selected_pull_request_behavior(Some(7), false, false, true, false),
            OpenSelectedPullRequestBehavior::ShowDetails {
                number: 7,
                refresh: false
            }
        );
        assert_eq!(
            open_selected_pull_request_behavior(Some(7), false, false, false, true),
            OpenSelectedPullRequestBehavior::ShowDetails {
                number: 7,
                refresh: false
            }
        );
    }
}
