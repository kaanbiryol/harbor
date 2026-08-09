use std::time::Duration;

use gpui::Context;

use crate::workspace::{AppView, async_updates::AppViewAsyncUpdateExt};

const PULL_REQUEST_SEARCH_DEBOUNCE: Duration = Duration::from_millis(250);
const PULL_REQUEST_SEARCH_PAGE_SIZE: usize = 25;

impl AppView {
    pub(crate) fn current_pull_request_search_query(&self, cx: &gpui::App) -> String {
        let input_query = self.pull_request_search_input.read(cx).value();
        let input_query = input_query.trim();
        if input_query.is_empty() {
            return String::new();
        }

        let filter_query = self.pull_request_filters.github_search_query();
        if filter_query.is_empty() {
            input_query.to_string()
        } else {
            format!("{input_query} {filter_query}")
        }
    }

    pub(crate) fn schedule_pull_request_search(&mut self, cx: &mut Context<Self>) {
        let query = self.current_pull_request_search_query(cx);
        let Some(repository) = self.current_repository().cloned() else {
            self.clear_pull_request_search();
            cx.notify();
            return;
        };

        if query.is_empty() {
            self.clear_pull_request_search();
            cx.notify();
            return;
        }

        let mode = self.pull_request_inbox.mode();
        let request_id = self.pull_request_search_state.start(query.clone());
        let github_api = self.github_api.clone();
        self.pull_request_switcher_selection = 0;
        self.status = format!("Searching {} on GitHub…", mode.status_label());

        self.tasks
            .set_pull_request_search_task(cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(PULL_REQUEST_SEARCH_DEBOUNCE)
                    .await;
                let result = github_api
                    .search_repository_pull_request_page(
                        &repository,
                        mode.list_filter(),
                        &query,
                        None,
                        PULL_REQUEST_SEARCH_PAGE_SIZE,
                    )
                    .await;

                this.update_or_log(
                    cx,
                    "failed to update pull request search",
                    move |view, cx| {
                        if view.current_repository() != Some(&repository)
                            || view.pull_request_inbox.mode() != mode
                            || !view.pull_request_search_state.matches(request_id, &query)
                        {
                            return;
                        }

                        match result {
                            Ok(page) => {
                                let result_count = page.pull_requests.len();
                                let total_count = page.total_count;
                                view.pull_request_search_state.apply_success(page);
                                view.status = match total_count {
                                    Some(total_count) => {
                                        format!("Found {total_count} matching pull requests")
                                    }
                                    None => format!("Found {result_count} matching pull requests"),
                                };
                            }
                            Err(error) => {
                                view.pull_request_search_state
                                    .apply_failure(error.to_string());
                                view.status = "Pull request search failed".to_string();
                            }
                        }
                        cx.notify();
                    },
                );
            }));
        cx.notify();
    }

    pub(crate) fn retry_pull_request_search(&mut self, cx: &mut Context<Self>) {
        self.schedule_pull_request_search(cx);
    }

    pub(crate) fn load_more_pull_request_search_results(&mut self, cx: &mut Context<Self>) {
        if self.pull_request_search_state.is_loading()
            || self.pull_request_search_state.is_loading_more()
        {
            return;
        }

        let Some(repository) = self.current_repository().cloned() else {
            return;
        };
        let Some(cursor) = self.pull_request_search_state.next_cursor() else {
            return;
        };
        let mode = self.pull_request_inbox.mode();
        let query = self.pull_request_search_state.query().to_string();
        let request_id = self.pull_request_search_state.request_id();
        let github_api = self.github_api.clone();
        self.pull_request_search_state.start_loading_more();

        self.tasks
            .set_pull_request_search_task(cx.spawn(async move |this, cx| {
                let result = github_api
                    .search_repository_pull_request_page(
                        &repository,
                        mode.list_filter(),
                        &query,
                        Some(cursor),
                        PULL_REQUEST_SEARCH_PAGE_SIZE,
                    )
                    .await;

                this.update_or_log(
                    cx,
                    "failed to update additional pull request search results",
                    move |view, cx| {
                        if view.current_repository() != Some(&repository)
                            || view.pull_request_inbox.mode() != mode
                            || !view.pull_request_search_state.matches(request_id, &query)
                        {
                            return;
                        }

                        match result {
                            Ok(page) => {
                                view.pull_request_search_state.apply_load_more_success(page)
                            }
                            Err(error) => view
                                .pull_request_search_state
                                .apply_load_more_failure(error.to_string()),
                        }
                        cx.notify();
                    },
                );
            }));
        cx.notify();
    }

    pub(crate) fn clear_pull_request_search(&mut self) {
        self.tasks.cancel_pull_request_search_task();
        self.pull_request_search_state.clear();
        self.pull_request_switcher_selection = 0;
    }
}
