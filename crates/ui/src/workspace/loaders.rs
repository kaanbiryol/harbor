use gpui::{AppContext, Context, ScrollStrategy};
use harbor_domain::{PullRequest, RepoId};
use harbor_sync::{
    PullRequestInboxPageInfo, PullRequestInboxRefresh, PullRequestInboxRefreshRequest,
    cache_pull_request_inbox_refresh, detect_pull_request_changes, refresh_pull_request_inbox,
};

use crate::workspace::{
    AppView, PullRequestInboxCacheKey, PullRequestInboxMode,
    async_updates::AppViewAsyncUpdateExt,
    pull_request_inbox_refresh::{
        PullRequestInboxRefreshIntent, append_pull_request_page, pull_request_inbox_failed_status,
        pull_request_inbox_loaded_more_status, pull_request_inbox_loaded_status,
        pull_request_inbox_loading_status,
    },
};

impl AppView {
    pub(super) fn load_pull_requests(&mut self, repo: RepoId, cx: &mut Context<Self>) {
        self.load_repository_pull_requests(
            repo,
            self.pull_request_inbox.mode(),
            PullRequestInboxRefreshIntent::PreferCache,
            cx,
        );
    }

    pub(super) fn load_repository_pull_requests_from_cache(
        &mut self,
        repo: RepoId,
        mode: PullRequestInboxMode,
        cx: &mut Context<Self>,
    ) {
        self.load_repository_pull_requests(
            repo,
            mode,
            PullRequestInboxRefreshIntent::PreferCache,
            cx,
        );
    }

    pub(super) fn refresh_pull_requests(&mut self, repo: RepoId, cx: &mut Context<Self>) {
        self.load_repository_pull_requests(
            repo,
            self.pull_request_inbox.mode(),
            PullRequestInboxRefreshIntent::ManualRefresh,
            cx,
        );
    }

    pub(crate) fn refresh_pull_requests_light(&mut self, repo: RepoId, cx: &mut Context<Self>) {
        self.load_repository_pull_requests(
            repo,
            self.pull_request_inbox.mode(),
            PullRequestInboxRefreshIntent::LightRefresh,
            cx,
        );
    }

    pub(super) fn reload_pull_request_inbox(&mut self, cx: &mut Context<Self>) {
        if let Some(repo) = self.repository_state.configured_repo_cloned() {
            self.mark_active_inbox_stale();
            self.refresh_pull_requests(repo, cx);
        } else {
            self.status =
                "Select a repository from the header before loading pull requests".to_string();
            cx.notify();
        }
    }

    pub(super) fn load_more_pull_requests(&mut self, cx: &mut Context<Self>) {
        if self.pull_request_inbox.is_loading() || self.pull_request_inbox.is_loading_more() {
            return;
        }

        let Some(repo) = self.repository_state.configured_repo_cloned() else {
            self.status =
                "Select a repository from the header before loading pull requests".to_string();
            cx.notify();
            return;
        };
        let Some(page_cursor) = self.pull_request_inbox.next_page_cursor() else {
            self.status = format!(
                "All {} are loaded",
                self.pull_request_inbox.mode().status_label()
            );
            cx.notify();
            return;
        };

        let mode = self.pull_request_inbox.mode();
        let key = PullRequestInboxCacheKey::new(repo.clone(), mode);
        let github_api = self.github_api.clone();
        let store = self.repository_state.store();
        let previous_pull_requests = self.pull_requests.clone();

        self.pull_request_inbox.start_loading_more();
        self.status = format!(
            "Loading more {} from {}",
            mode.status_label(),
            repo.full_name()
        );

        self.tasks
            .set_pull_request_list_task(cx.spawn(async move |this, cx| {
                let refresh = refresh_pull_request_inbox(
                    github_api.as_ref(),
                    PullRequestInboxRefreshRequest {
                        store: store.as_ref(),
                        repository: &repo,
                        mode,
                        page_cursor: Some(page_cursor),
                        previous_pull_requests: &previous_pull_requests,
                        force_enrichment: false,
                    },
                )
                .await;

                this.update_or_log(
                    cx,
                    "failed to update additional pull request inbox rows",
                    move |view, cx| {
                        if view.current_pull_request_inbox_key().as_ref() != Some(&key) {
                            return;
                        }

                        match refresh {
                            Ok(PullRequestInboxRefresh::Modified {
                                pull_requests,
                                page_info,
                                enrichment_error,
                            }) => {
                                view.pull_request_inbox.apply_load_more_success();
                                let appended_count = append_pull_request_page(
                                    &mut view.pull_requests,
                                    pull_requests,
                                );
                                view.pull_request_inbox.set_page_info(page_info.clone());
                                view.update_pull_request_inbox_count(key, &page_info);
                                view.status = pull_request_inbox_loaded_more_status(
                                    &repo,
                                    mode,
                                    appended_count,
                                    view.pull_requests.len(),
                                    &page_info,
                                );
                                if let Some(error) = enrichment_error {
                                    view.status =
                                        format!("{}; rich refresh failed: {error}", view.status);
                                }
                                view.cache_current_pull_request_inbox_snapshot();
                            }
                            Ok(PullRequestInboxRefresh::NotModified) => {
                                view.pull_request_inbox.apply_load_more_success();
                                view.status = format!(
                                    "{} from {} unchanged",
                                    mode.status_label(),
                                    repo.full_name()
                                );
                            }
                            Err(error) => {
                                view.pull_request_inbox
                                    .apply_load_more_failure(error.to_string());
                                view.status = format!(
                                    "Failed to load more {} from {}",
                                    mode.status_label(),
                                    repo.full_name()
                                );
                            }
                        }

                        cx.notify();
                    },
                );
            }));
    }

    fn load_repository_pull_requests(
        &mut self,
        repo: RepoId,
        mode: PullRequestInboxMode,
        refresh_intent: PullRequestInboxRefreshIntent,
        cx: &mut Context<Self>,
    ) {
        if !self.github_api.has_auth() {
            self.show_github_sign_in_required();
            cx.notify();
            return;
        }

        let key = PullRequestInboxCacheKey::new(repo.clone(), mode);
        let same_inbox = self
            .current_pull_request_inbox_key()
            .as_ref()
            .is_some_and(|current_key| current_key == &key);

        self.cache_current_pull_request_inbox_snapshot();

        if refresh_intent.prefetches_counts() {
            self.prefetch_pull_request_inbox_counts(
                repo.clone(),
                mode,
                refresh_intent.refreshes_counts(),
                cx,
            );
        }

        if refresh_intent.uses_cache() && self.restore_pull_request_inbox_snapshot(key.clone(), cx)
        {
            self.record_recent_repository(repo.clone(), cx);
            self.spawn_pull_request_inbox_refresh(repo, mode, key, false, cx);
            return;
        }

        self.repository_state.select_repository(repo.clone());
        self.pull_request_inbox.set_mode(mode);
        if !same_inbox {
            self.pull_request_inbox.clear_page_info();
        }
        self.ensure_sync_loop(cx);
        if refresh_intent != PullRequestInboxRefreshIntent::LightRefresh {
            self.record_recent_repository(repo.clone(), cx);
        }
        self.pull_request_inbox.start_loading();

        if refresh_intent.resets_detail_state() {
            self.clear_detail_errors();
            self.clear_log_error();
            self.clear_action_errors();
            self.tasks.clear_pull_request_detail_tasks();
            self.clear_review_data_state();
            self.clear_review_submission_errors();
            self.collapsed_file_tree_folders.clear();
            self.reviewed_file_paths.clear();
            self.reset_changed_file_filters();
            self.owned_file_paths.clear();
            self.clear_detail_loaded_state();
            self.set_detail_loading(false);
            self.set_log_loading(false);
        }

        self.status = pull_request_inbox_loading_status(&repo, mode);

        self.spawn_pull_request_inbox_refresh(
            repo,
            mode,
            key,
            refresh_intent.force_enrichment(),
            cx,
        );
    }

    pub(crate) fn spawn_pull_request_inbox_refresh(
        &mut self,
        repo: RepoId,
        mode: PullRequestInboxMode,
        key: PullRequestInboxCacheKey,
        force_enrichment: bool,
        cx: &mut Context<Self>,
    ) {
        self.pull_request_inbox.start_loading();
        self.mark_sync_attempt(mode.active_sync_target());
        let github_api = self.github_api.clone();
        let store = self.repository_state.store();
        let previous_pull_requests = self.pull_requests.clone();

        self.tasks
            .set_pull_request_list_task(cx.spawn(async move |this, cx| {
                let refresh = refresh_pull_request_inbox(
                    github_api.as_ref(),
                    PullRequestInboxRefreshRequest {
                        store: store.as_ref(),
                        repository: &repo,
                        mode,
                        page_cursor: None,
                        previous_pull_requests: &previous_pull_requests,
                        force_enrichment,
                    },
                )
                .await;
                let cache_result =
                    cache_pull_request_inbox_refresh(store.as_ref(), &repo, mode, &refresh).await;

                this.update_or_log(
                    cx,
                    "failed to update pull request inbox state",
                    move |view, cx| {
                        if view.current_pull_request_inbox_key().as_ref() != Some(&key) {
                            return;
                        }

                        view.pull_request_inbox.apply_success();
                        if let Err(error) = cache_result {
                            view.repository_state.set_error(error);
                        }

                        match refresh {
                            Ok(PullRequestInboxRefresh::NotModified) => {
                                view.mark_sync_success(mode.active_sync_target());
                                view.status = format!(
                                    "{} from {} unchanged",
                                    mode.status_label(),
                                    repo.full_name()
                                );
                            }
                            Ok(PullRequestInboxRefresh::Modified {
                                pull_requests,
                                page_info,
                                enrichment_error,
                            }) => {
                                view.mark_sync_success(mode.active_sync_target());
                                view.pull_request_inbox.set_page_info(page_info.clone());
                                let count = pull_requests.len();
                                let status = pull_request_inbox_loaded_status(
                                    &repo, mode, count, &page_info,
                                );
                                let change_events = detect_pull_request_changes(
                                    &previous_pull_requests,
                                    &pull_requests,
                                );
                                view.apply_loaded_pull_request_inbox(
                                    repo.clone(),
                                    mode,
                                    pull_requests,
                                    page_info,
                                    true,
                                    cx,
                                );
                                view.status = enrichment_error
                                    .map(|error| format!("{status}; rich refresh failed: {error}"))
                                    .unwrap_or(status);
                                view.handle_pull_request_change_events(change_events, cx);
                            }
                            Err(error) => {
                                view.mark_sync_failure(mode.active_sync_target());
                                let mut status = pull_request_inbox_failed_status(&repo, mode);
                                view.set_detail_loading(false);
                                view.set_log_loading(false);
                                view.pull_request_inbox.apply_failure(error.to_string());
                                if !view.pull_requests.is_empty() {
                                    status = format!("{status}; showing existing data");
                                } else {
                                    view.clear_changed_file_state();
                                    view.clear_workflow_state();
                                    view.clear_review_data_state();
                                    view.clear_detail_loaded_state();
                                    view.clear_review_submission_errors();
                                    view.clear_log_content();
                                    view.selection_state.reset_pull_request_index();
                                    view.reset_diff_selection();
                                    view.pr_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                                    view.reset_detail_scrolls();
                                }
                                view.status = status;
                            }
                        }

                        cx.notify();
                    },
                );
            }));
    }

    fn prefetch_pull_request_inbox_counts(
        &mut self,
        repo: RepoId,
        active_mode: PullRequestInboxMode,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.prefetch_inbox_counts {
            return;
        }

        let modes = PullRequestInboxMode::ALL
            .into_iter()
            .filter(|mode| *mode != active_mode)
            .filter(|mode| {
                force
                    || self
                        .pull_request_inbox
                        .snapshot_count(&PullRequestInboxCacheKey::new(repo.clone(), *mode))
                        .is_none()
            })
            .collect::<Vec<_>>();

        if modes.is_empty() {
            return;
        }

        let github_api = self.github_api.clone();
        let task = cx.background_spawn(async move {
            let mut counts = Vec::with_capacity(modes.len());
            let mut errors = Vec::new();

            for mode in modes {
                match github_api
                    .count_repository_pull_requests(&repo, mode.list_filter())
                    .await
                {
                    Ok(count) => counts.push((mode, count)),
                    Err(error) => errors.push((mode, error.to_string())),
                }
            }

            (repo, counts, errors)
        });

        cx.spawn(async move |this, cx| {
            let (repo, counts, errors) = task.await;

            this.update_or_log(
                cx,
                "failed to update pull request inbox counts",
                move |view, cx| {
                    for (mode, error) in errors {
                        tracing::warn!(
                            repository = %repo.full_name(),
                            mode = mode.key(),
                            %error,
                            "failed to load pull request inbox count"
                        );
                    }

                    for (mode, count) in counts {
                        view.pull_request_inbox
                            .insert_count(PullRequestInboxCacheKey::new(repo.clone(), mode), count);
                    }

                    cx.notify();
                },
            );
        })
        .detach();
    }

    fn apply_loaded_pull_request_inbox(
        &mut self,
        repo: RepoId,
        mode: PullRequestInboxMode,
        pull_requests: Vec<PullRequest>,
        page_info: PullRequestInboxPageInfo,
        load_selected_detail: bool,
        cx: &mut Context<Self>,
    ) {
        let previous_selected = self
            .selected_pull_request()
            .map(|pull_request| (pull_request.number, pull_request.head_sha.clone()));
        let previous_key = self.current_pull_request_inbox_key();
        let key = PullRequestInboxCacheKey::new(repo.clone(), mode);
        let same_inbox = previous_key
            .as_ref()
            .is_some_and(|previous_key| previous_key == &key);

        self.repository_state.select_repository(repo);
        self.pull_request_inbox.set_mode(mode);
        self.pull_requests = pull_requests;
        self.pull_request_inbox.set_page_info(page_info.clone());
        self.update_pull_request_inbox_count(key, &page_info);

        let selected_pr = previous_selected
            .as_ref()
            .and_then(|(number, _)| {
                self.pull_requests
                    .iter()
                    .position(|pull_request| pull_request.number == *number)
            })
            .unwrap_or(0);
        self.selection_state
            .restore_pull_request_index(selected_pr, self.pull_requests.len());

        let selected_head_unchanged =
            previous_selected
                .as_ref()
                .is_some_and(|(number, previous_head_sha)| {
                    self.selected_pull_request().is_some_and(|pull_request| {
                        pull_request.number == *number
                            && pull_request.head_sha == *previous_head_sha
                    })
                });

        if !same_inbox || !selected_head_unchanged {
            self.clear_changed_file_state();
            self.clear_workflow_state();
            self.clear_review_data_state();
            self.clear_detail_loaded_state();
            self.clear_review_submission_errors();
            self.clear_log_content();
            self.reset_diff_selection();
            self.reset_detail_scrolls();
        }

        self.pr_list_scroll
            .scroll_to_item(self.selected_pull_request_index(), ScrollStrategy::Center);

        if load_selected_detail && (!same_inbox || !selected_head_unchanged) {
            self.load_selected_pull_request(cx);
        } else {
            self.cache_current_pull_request_inbox_snapshot();
        }
    }

    fn update_pull_request_inbox_count(
        &mut self,
        key: PullRequestInboxCacheKey,
        page_info: &PullRequestInboxPageInfo,
    ) {
        if let Some(total_count) = page_info.total_count {
            self.pull_request_inbox.insert_count(key, total_count);
        } else if !page_info.has_next_page() {
            self.pull_request_inbox
                .insert_count(key, self.pull_requests.len());
        }
    }
}
