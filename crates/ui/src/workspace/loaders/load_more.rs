use gpui::Context;
use harbor_sync::{
    PullRequestInboxRefresh, PullRequestInboxRefreshRequest, refresh_pull_request_inbox,
};

use crate::workspace::{
    AppView, PullRequestInboxCacheKey,
    async_updates::AppViewAsyncUpdateExt,
    pull_request_inbox_refresh::{append_pull_request_page, pull_request_inbox_loaded_more_status},
};

impl AppView {
    pub(in crate::workspace) fn load_more_pull_requests(&mut self, cx: &mut Context<Self>) {
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
}
