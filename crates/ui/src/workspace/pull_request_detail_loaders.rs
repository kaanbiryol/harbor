use gpui::Context;
use harbor_domain::{MergeState, PullRequest, RepoId};

use crate::{
    actions::PanelTab,
    workspace::{
        AppView,
        review_data_loaders::{ReviewDataLoadMode, ReviewDataLoadTarget},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PullRequestDetailFetchPolicy {
    PreferCache,
    Refresh,
}

impl PullRequestDetailFetchPolicy {
    fn load_scope(self) -> PullRequestDetailLoadScope {
        match self {
            Self::PreferCache => PullRequestDetailLoadScope::ActivePanel,
            Self::Refresh => PullRequestDetailLoadScope::Full,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PullRequestDetailLoadScope {
    ActivePanel,
    Full,
}

#[derive(Clone, Debug)]
pub(super) struct SelectedPullRequestLoad {
    pub(super) repo: RepoId,
    pub(super) owner: String,
    pub(super) name: String,
    pub(super) number: u64,
    pub(super) head_sha: String,
}

impl SelectedPullRequestLoad {
    fn from_pull_request(pull_request: &PullRequest) -> Self {
        let repo = pull_request.repo.clone();

        Self {
            owner: repo.owner.clone(),
            name: repo.name.clone(),
            repo,
            number: pull_request.number,
            head_sha: pull_request.head_sha.clone(),
        }
    }
}

impl AppView {
    pub(crate) fn replace_selected_pull_request_preserving_row_fields(
        &mut self,
        mut detail: PullRequest,
    ) {
        let Some(selected) = self
            .pull_requests
            .get_mut(self.selection_state.pull_request_index())
        else {
            return;
        };

        if detail.review_decision.is_none() {
            detail.review_decision = selected.review_decision;
        }
        if detail.merge_state.is_none() || detail.merge_state == Some(MergeState::Unknown) {
            detail.merge_state = selected.merge_state;
        }
        detail.checks_summary = selected.checks_summary;
        detail.unresolved_threads = selected.unresolved_threads;

        *selected = detail;
    }

    pub(super) fn load_selected_pull_request(&mut self, cx: &mut Context<Self>) {
        self.load_selected_pull_request_with_policy(PullRequestDetailFetchPolicy::PreferCache, cx);
    }

    pub(super) fn refresh_selected_pull_request(&mut self, cx: &mut Context<Self>) {
        self.load_selected_pull_request_with_policy(PullRequestDetailFetchPolicy::Refresh, cx);
    }

    pub(crate) fn refresh_selected_pull_request_metadata_only(&mut self, cx: &mut Context<Self>) {
        let Some(pull_request) = self.selected_pull_request().cloned() else {
            return;
        };

        self.detail_state.mark_details_stale();
        self.spawn_pull_request_metadata_loader(
            SelectedPullRequestLoad::from_pull_request(&pull_request),
            cx,
        );
    }

    fn load_selected_pull_request_with_policy(
        &mut self,
        fetch_policy: PullRequestDetailFetchPolicy,
        cx: &mut Context<Self>,
    ) {
        let Some(pull_request) = self.selected_pull_request().cloned() else {
            return;
        };
        let load = SelectedPullRequestLoad::from_pull_request(&pull_request);

        if fetch_policy == PullRequestDetailFetchPolicy::PreferCache
            && self.restore_selected_pull_request_detail_snapshot(cx)
        {
            return;
        }

        self.reset_selected_pull_request_detail_state(load.number);
        let defer_review_load_until_cache = should_defer_review_load_until_cache(
            fetch_policy,
            self.repository_state.store().is_some(),
            self.active_tab,
        );
        if defer_review_load_until_cache {
            self.review_state.start_reviews_load();
        }
        if fetch_policy == PullRequestDetailFetchPolicy::PreferCache {
            self.spawn_cached_selected_pull_request_detail_loader(
                load.clone(),
                defer_review_load_until_cache,
                cx,
            );
        }

        self.spawn_pull_request_metadata_loader(load.clone(), cx);
        self.spawn_pull_request_files_loader(load.clone(), cx);

        match fetch_policy.load_scope() {
            PullRequestDetailLoadScope::ActivePanel => self.load_active_panel_data_if_needed(cx),
            PullRequestDetailLoadScope::Full => {
                self.spawn_pull_request_checks_loader(load.clone(), cx);
                self.spawn_pull_request_workflows_loader(load.clone(), cx);
                self.spawn_selected_review_data_loader(load, ReviewDataLoadMode::Initial, cx);
            }
        }
    }

    pub(super) fn load_active_panel_data_if_needed(&mut self, cx: &mut Context<Self>) {
        let Some(pull_request) = self.selected_pull_request().cloned() else {
            return;
        };
        let load = SelectedPullRequestLoad::from_pull_request(&pull_request);

        if self.detail_state.should_load_details() {
            self.spawn_pull_request_metadata_loader(load.clone(), cx);
        }
        if self.detail_state.should_load_files() {
            self.spawn_pull_request_files_loader(load.clone(), cx);
        }

        match self.active_tab {
            PanelTab::Diff | PanelTab::Review => {
                if self.review_state.should_load_reviews() {
                    self.spawn_selected_review_data_loader(load, ReviewDataLoadMode::Initial, cx);
                }
            }
            PanelTab::Checks => {
                if self.detail_state.should_load_checks() {
                    self.spawn_pull_request_checks_loader(load, cx);
                }
            }
            PanelTab::Actions => {
                if self.detail_state.should_load_workflows() {
                    self.spawn_pull_request_workflows_loader(load, cx);
                }
            }
            PanelTab::Logs => {
                if self.detail_state.should_load_workflows() {
                    self.spawn_pull_request_workflows_loader(load, cx);
                } else if !self.detail_state.log_state.is_loading()
                    && self.detail_state.log_state.chunk().is_none()
                    && self.detail_state.log_state.error().is_none()
                {
                    self.load_selected_workflow_logs(cx);
                }
            }
        }
    }

    fn reset_selected_pull_request_detail_state(&mut self, number: u64) {
        self.set_detail_loading(false);
        self.clear_detail_loaded_state();
        self.clear_detail_errors();
        self.clear_log_error();
        self.clear_action_errors();
        self.tasks.clear_pull_request_detail_tasks();
        self.clear_changed_file_state();
        self.clear_workflow_state();
        self.clear_review_data_state();
        self.clear_review_submission_errors();
        self.clear_log_content();
        self.reset_diff_selection();
        self.reset_detail_scrolls();
        self.status = format!("Loading PR #{number} details and changed files");
    }

    fn spawn_selected_review_data_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        mode: ReviewDataLoadMode,
        cx: &mut Context<Self>,
    ) {
        let review_data_generation = self.next_review_data_generation();
        self.spawn_review_data_loader(
            ReviewDataLoadTarget::new(
                load.repo,
                load.number,
                load.head_sha,
                review_data_generation,
            ),
            mode,
            cx,
        );
    }
}

fn should_defer_review_load_until_cache(
    fetch_policy: PullRequestDetailFetchPolicy,
    has_store: bool,
    active_tab: PanelTab,
) -> bool {
    fetch_policy == PullRequestDetailFetchPolicy::PreferCache
        && has_store
        && matches!(active_tab, PanelTab::Diff | PanelTab::Review)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defers_review_loading_until_cache_for_cache_backed_review_tabs() {
        assert!(should_defer_review_load_until_cache(
            PullRequestDetailFetchPolicy::PreferCache,
            true,
            PanelTab::Diff
        ));
        assert!(should_defer_review_load_until_cache(
            PullRequestDetailFetchPolicy::PreferCache,
            true,
            PanelTab::Review
        ));
    }

    #[test]
    fn does_not_defer_review_loading_without_cache_or_for_non_review_tabs() {
        assert!(!should_defer_review_load_until_cache(
            PullRequestDetailFetchPolicy::Refresh,
            true,
            PanelTab::Diff
        ));
        assert!(!should_defer_review_load_until_cache(
            PullRequestDetailFetchPolicy::PreferCache,
            false,
            PanelTab::Diff
        ));

        for tab in [PanelTab::Checks, PanelTab::Actions, PanelTab::Logs] {
            assert!(!should_defer_review_load_until_cache(
                PullRequestDetailFetchPolicy::PreferCache,
                true,
                tab
            ));
        }
    }
}
