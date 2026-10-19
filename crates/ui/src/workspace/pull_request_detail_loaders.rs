use gpui::{AppContext, Context};
use harbor_domain::{MergeState, PullRequest, RepoId};

use crate::{
    actions::PanelTab,
    workspace::{
        AppView, PullRequestDetailCacheKey, SelectedPullRequestTaskKind,
        async_updates::AppViewAsyncUpdateExt,
        reviews::{ReviewDataLoadMode, ReviewDataLoadTarget},
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

    pub(super) fn detail_key(&self) -> PullRequestDetailCacheKey {
        PullRequestDetailCacheKey::new(self.repo.clone(), self.number, self.head_sha.clone())
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
        let description_changed = detail.body != selected.body;
        let detail_key = PullRequestDetailCacheKey::new(
            selected.repo.clone(),
            selected.number,
            selected.head_sha.clone(),
        );

        if detail.head_sha == selected.head_sha {
            if detail.review_decision.is_none() {
                detail.review_decision = selected.review_decision;
            }
            if detail.merge_state.is_none() || detail.merge_state == Some(MergeState::Unknown) {
                detail.merge_state = selected.merge_state;
            }
            detail.checks_summary = selected.checks_summary;
            detail.merge_capabilities = selected.merge_capabilities;
            detail.unresolved_threads = selected.unresolved_threads;
        }
        if detail.created_at.is_none() {
            detail.created_at = selected.created_at;
        }

        *selected = detail;
        if description_changed {
            self.overview_state.invalidate_description(&detail_key);
        }
    }

    pub(super) fn apply_pull_request_metadata(
        &mut self,
        detail: PullRequest,
        cx: &mut Context<Self>,
    ) {
        let head_changed = self
            .selected_pull_request()
            .is_some_and(|selected| selected.head_sha != detail.head_sha);
        let reload_checks = !self.detail_state.should_load_checks();
        let reload_commits =
            self.detail_state.commits_loading() || self.detail_state.commits_finished();
        let reload_workflows = !self.detail_state.should_load_workflows();
        let reload_reviews =
            self.review_state.reviews_loading() || self.review_state.reviews_finished();
        let load = SelectedPullRequestLoad::from_pull_request(&detail);
        self.replace_selected_pull_request_preserving_row_fields(detail);

        if head_changed {
            self.reset_selected_pull_request_detail_state(load.number, cx);
        }
        self.detail_state.apply_details_success();
        self.load_selected_pull_request_stack(cx);
        self.mark_sync_success(harbor_sync::SyncTarget::SelectedPullRequestMetadata);

        if head_changed {
            self.load_active_panel_data_if_needed(cx);
            if reload_checks {
                self.spawn_pull_request_checks_loader(load.clone(), cx);
            }
            if reload_commits {
                self.spawn_pull_request_commits_loader(load.clone(), cx);
            }
            if reload_workflows {
                self.spawn_pull_request_workflows_loader(load.clone(), cx);
            }
            if reload_reviews && self.review_state.should_load_reviews() {
                self.spawn_selected_review_data_loader(load, ReviewDataLoadMode::Initial, cx);
            }
        }
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

        self.reset_selected_pull_request_detail_state(load.number, cx);
        let defer_review_load_until_cache = should_defer_review_load_until_cache(
            fetch_policy,
            self.cache_store().is_some(),
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
                self.spawn_pull_request_commits_loader(load.clone(), cx);
                self.spawn_pull_request_workflows_loader(load.clone(), cx);
                self.spawn_selected_review_data_loader(load, ReviewDataLoadMode::Initial, cx);
            }
        }
    }

    pub(super) fn load_active_panel_data_if_needed(&mut self, cx: &mut Context<Self>) {
        if self.active_tab == PanelTab::Actions {
            self.load_repository_actions_if_needed(cx);
        }

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
            PanelTab::Overview | PanelTab::Diff => {
                if self.review_state.should_load_reviews() {
                    self.spawn_selected_review_data_loader(
                        load.clone(),
                        ReviewDataLoadMode::Initial,
                        cx,
                    );
                }
                if self.active_tab == PanelTab::Overview && self.detail_state.should_load_commits()
                {
                    self.spawn_pull_request_commits_loader(load, cx);
                }
            }
            PanelTab::Checks => {
                if self.detail_state.should_load_checks() {
                    self.spawn_pull_request_checks_loader(load, cx);
                }
            }
            PanelTab::Commits => {
                if self.detail_state.should_load_commits() {
                    self.spawn_pull_request_commits_loader(load, cx);
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
                } else {
                    self.load_workflow_log_navigation(cx);
                }
            }
        }
    }

    fn reset_selected_pull_request_detail_state(&mut self, number: u64, cx: &mut Context<Self>) {
        self.tasks.cancel_selected_pull_request_tasks();
        self.set_detail_loading(false);
        self.clear_detail_errors();
        self.clear_action_errors();
        self.clear_selected_pull_request_detail_state();
        self.sync_diff_list_items(cx);
        self.status = format!("Loading PR #{number} details and changed files");
    }

    fn spawn_selected_review_data_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        mode: ReviewDataLoadMode,
        cx: &mut Context<Self>,
    ) {
        let review_data_generation = self.review_state.next_review_data_generation();
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

    pub(super) fn spawn_selected_pull_request_load<T: Send + 'static>(
        &mut self,
        kind: SelectedPullRequestTaskKind,
        detail_key: PullRequestDetailCacheKey,
        update_error_context: &'static str,
        load: impl Future<Output = T> + Send + 'static,
        apply: impl FnOnce(&mut AppView, T, &mut Context<AppView>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let auth_generation = self.auth_generation;
        self.tasks.set_selected_pull_request_task(
            kind,
            cx.spawn(async move |this, cx| {
                let result = cx.background_spawn(load).await;
                this.update_in_session_or_log(
                    cx,
                    auth_generation,
                    update_error_context,
                    move |view, cx| {
                        if !selected_pull_request_matches(view, &detail_key) {
                            return;
                        }
                        apply(view, result, cx);
                        view.cache_current_pull_request_detail_snapshot();
                        cx.notify();
                    },
                );
            }),
        );
    }
}

pub(super) fn selected_pull_request_matches(
    view: &AppView,
    detail_key: &PullRequestDetailCacheKey,
) -> bool {
    view.selected_pull_request_detail_key().as_ref() == Some(detail_key)
}

fn should_defer_review_load_until_cache(
    fetch_policy: PullRequestDetailFetchPolicy,
    has_store: bool,
    active_tab: PanelTab,
) -> bool {
    fetch_policy == PullRequestDetailFetchPolicy::PreferCache
        && has_store
        && matches!(active_tab, PanelTab::Overview | PanelTab::Diff)
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
            PanelTab::Overview
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

        for tab in [
            PanelTab::Commits,
            PanelTab::Checks,
            PanelTab::Actions,
            PanelTab::Logs,
        ] {
            assert!(!should_defer_review_load_until_cache(
                PullRequestDetailFetchPolicy::PreferCache,
                true,
                tab
            ));
        }
    }
}
