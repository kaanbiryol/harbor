use gpui::{AppContext, Context, ScrollStrategy};
use gpui_component::ActiveTheme;
use harbor_domain::{RepoId, ReviewThreadState};
use harbor_github::{GhCliTransport, GitHubClient};

use crate::{
    actions::PanelTab,
    diff::{parse_files, parse_unified_diff_with_syntax},
    panels::checks_summary_from_runs,
    workspace::{
        AppView,
        review_data_loaders::{pending_review_rest_id, selected_pull_request_matches},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PullRequestDetailFetchPolicy {
    PreferCache,
    Refresh,
}

#[derive(Clone, Debug)]
struct SelectedPullRequestLoad {
    repo: RepoId,
    owner: String,
    name: String,
    number: u64,
    head_sha: String,
}

impl AppView {
    pub(super) fn load_selected_pull_request(&mut self, cx: &mut Context<Self>) {
        self.load_selected_pull_request_with_policy(PullRequestDetailFetchPolicy::PreferCache, cx);
    }

    pub(super) fn refresh_selected_pull_request(&mut self, cx: &mut Context<Self>) {
        self.load_selected_pull_request_with_policy(PullRequestDetailFetchPolicy::Refresh, cx);
    }

    fn load_selected_pull_request_with_policy(
        &mut self,
        fetch_policy: PullRequestDetailFetchPolicy,
        cx: &mut Context<Self>,
    ) {
        let Some(pull_request) = self.selected_pull_request().cloned() else {
            return;
        };
        let repo = pull_request.repo;
        let load = SelectedPullRequestLoad {
            owner: repo.owner.clone(),
            name: repo.name.clone(),
            repo,
            number: pull_request.number,
            head_sha: pull_request.head_sha,
        };

        if fetch_policy == PullRequestDetailFetchPolicy::PreferCache
            && self.restore_selected_pull_request_detail_snapshot(cx)
        {
            return;
        }

        self.reset_selected_pull_request_detail_state(load.number);
        let review_data_generation = self.next_review_data_generation();

        self.spawn_pull_request_metadata_loader(load.clone(), cx);
        self.spawn_pull_request_files_loader(load.clone(), cx);
        self.spawn_pull_request_checks_loader(load.clone(), cx);
        self.spawn_pull_request_workflows_loader(load.clone(), cx);
        self.spawn_pull_request_initial_review_loader(load, review_data_generation, cx);
    }

    fn reset_selected_pull_request_detail_state(&mut self, number: u64) {
        self.is_loading_details = true;
        self.is_loading_files = true;
        self.is_loading_checks = true;
        self.is_loading_workflows = true;
        self.is_loading_reviews = true;
        self.details_error = None;
        self.files_error = None;
        self.checks_error = None;
        self.workflows_error = None;
        self.reviews_error = None;
        self.logs_error = None;
        self.action_error = None;
        self.pr_action_error = None;
        self.pr_detail_tasks.clear();
        self.files.clear();
        self.diffs.clear();
        self.collapsed_file_tree_folders.clear();
        self.reviewed_file_paths.clear();
        self.reset_changed_file_filters();
        self.owned_file_paths.clear();
        self.check_runs.clear();
        self.workflow_runs.clear();
        self.workflow_jobs.clear();
        self.pull_request_reviews.clear();
        self.review_threads.clear();
        self.clear_review_composer_state();
        self.review_comment_error = None;
        self.pending_review_error = None;
        self.log_chunk = None;
        self.active_file = 0;
        self.active_hunk = 0;
        self.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.review_list_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        self.log_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.status = format!("Loading PR #{number} details and changed files");
    }

    fn spawn_pull_request_metadata_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        cx: &mut Context<Self>,
    ) {
        self.pr_detail_tasks.push(cx.spawn({
            let repo = load.repo;
            let owner = load.owner;
            let name = load.name;
            let number = load.number;

            async move |this, cx| {
                let result = GitHubClient::new(GhCliTransport)
                    .get_pull_request(&owner, &name, number)
                    .await;

                if let Err(error) = this.update(cx, move |view, cx| {
                    if !selected_pull_request_matches(view, &repo, number) {
                        return;
                    }

                    view.is_loading_details = false;
                    match result {
                        Ok(detail) => {
                            if let Some(selected) = view.pull_requests.get_mut(view.selected_pr) {
                                let review_decision = selected.review_decision;
                                *selected = detail;
                                if selected.review_decision.is_none() {
                                    selected.review_decision = review_decision;
                                }
                            }
                            view.details_error = None;
                            view.status = format!("Loaded PR #{number} details");
                        }
                        Err(error) => {
                            view.details_error = Some(error.to_string());
                            view.status = format!("Failed to load PR #{number} details");
                        }
                    }

                    view.cache_current_pull_request_detail_snapshot();
                    cx.notify();
                }) {
                    eprintln!("failed to update pull request detail state: {error}");
                }
            }
        }));
    }

    fn spawn_pull_request_files_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        cx: &mut Context<Self>,
    ) {
        self.pr_detail_tasks.push(cx.spawn({
            let repo = load.repo;
            let owner = load.owner;
            let name = load.name;
            let number = load.number;
            let highlight_theme = cx.theme().highlight_theme.clone();

            async move |this, cx| {
                let result = GitHubClient::new(GhCliTransport)
                    .list_pull_request_files(&owner, &name, number)
                    .await
                    .map(|files| {
                        let diffs = parse_files(&files);
                        (files, diffs)
                    });
                let files_for_syntax = result.as_ref().ok().map(|(files, _)| files.clone());
                let update_repo = repo.clone();

                if let Err(error) = this.update(cx, move |view, cx| {
                    if !selected_pull_request_matches(view, &update_repo, number) {
                        return;
                    }

                    view.is_loading_files = false;
                    match result {
                        Ok((files, diffs)) => {
                            let count = files.len();
                            view.files = files;
                            view.diffs = diffs;
                            view.active_file = 0;
                            view.active_hunk = 0;
                            view.reset_changed_file_filters();
                            view.prune_reviewed_file_paths();
                            view.ensure_active_file_visible(cx);
                            view.clear_review_composer_state();
                            view.refresh_owned_file_filters(cx);
                            let row_index = view
                                .file_tree_row_index_for_file(view.active_file, cx)
                                .unwrap_or(0);
                            view.file_list_scroll
                                .scroll_to_item(row_index, ScrollStrategy::Top);
                            view.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                            view.files_error = None;
                            view.status = format!("Loaded {count} changed files for PR #{number}");
                        }
                        Err(error) => {
                            view.files.clear();
                            view.diffs.clear();
                            view.collapsed_file_tree_folders.clear();
                            view.reviewed_file_paths.clear();
                            view.reset_changed_file_filters();
                            view.owned_file_paths.clear();
                            view.active_file = 0;
                            view.active_hunk = 0;
                            view.clear_review_composer_state();
                            view.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                            view.diff_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                            view.files_error = Some(error.to_string());
                            view.status = format!("Failed to load changed files for PR #{number}");
                        }
                    }

                    view.cache_current_pull_request_detail_snapshot();
                    cx.notify();
                }) {
                    eprintln!("failed to update pull request file state: {error}");
                }

                let Some(files_for_syntax) = files_for_syntax else {
                    return;
                };

                for (file_index, file) in files_for_syntax.into_iter().enumerate() {
                    let file_path = file.path.clone();
                    let Some(patch) = file.patch.clone() else {
                        continue;
                    };
                    let highlight_theme = highlight_theme.clone();
                    let highlighted_diff = cx
                        .background_spawn(async move {
                            parse_unified_diff_with_syntax(&file, &patch, &highlight_theme)
                        })
                        .await;

                    let update_repo = repo.clone();
                    if let Err(error) = this.update(cx, move |view, cx| {
                        if !selected_pull_request_matches(view, &update_repo, number) {
                            return;
                        }
                        if view.files.get(file_index).map(|file| file.path.as_str())
                            != Some(file_path.as_str())
                        {
                            return;
                        }

                        if let Some(diff) = view.diffs.get_mut(file_index) {
                            *diff = Some(highlighted_diff);
                        }
                        view.cache_current_pull_request_detail_snapshot();
                        cx.notify();
                    }) {
                        eprintln!("failed to update pull request syntax highlight state: {error}");
                    }
                }
            }
        }));
    }

    fn spawn_pull_request_checks_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        cx: &mut Context<Self>,
    ) {
        self.pr_detail_tasks.push(cx.spawn({
            let repo = load.repo;
            let owner = load.owner;
            let name = load.name;
            let number = load.number;
            let head_sha = load.head_sha;

            async move |this, cx| {
                let result = if head_sha.is_empty() {
                    Ok(Vec::new())
                } else {
                    GitHubClient::new(GhCliTransport)
                        .list_check_runs(&owner, &name, &head_sha)
                        .await
                };

                if let Err(error) = this.update(cx, move |view, cx| {
                    if !selected_pull_request_matches(view, &repo, number) {
                        return;
                    }

                    view.is_loading_checks = false;
                    match result {
                        Ok(check_runs) => {
                            let count = check_runs.len();
                            let summary = checks_summary_from_runs(&check_runs);
                            view.check_runs = check_runs;
                            view.checks_error = None;

                            if let Some(selected) = view.pull_requests.get_mut(view.selected_pr) {
                                selected.checks_summary = summary;
                            }

                            view.status = format!("Loaded {count} check runs for PR #{number}");
                        }
                        Err(error) => {
                            view.check_runs.clear();
                            view.checks_error = Some(error.to_string());
                            view.status = format!("Failed to load checks for PR #{number}");
                        }
                    }

                    view.cache_current_pull_request_detail_snapshot();
                    cx.notify();
                }) {
                    eprintln!("failed to update pull request checks state: {error}");
                }
            }
        }));
    }

    fn spawn_pull_request_workflows_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        cx: &mut Context<Self>,
    ) {
        self.pr_detail_tasks.push(cx.spawn({
            let repo = load.repo;
            let owner = load.owner;
            let name = load.name;
            let number = load.number;
            let head_sha = load.head_sha;

            async move |this, cx| {
                let result = if head_sha.is_empty() {
                    Ok(Vec::new())
                } else {
                    GitHubClient::new(GhCliTransport)
                        .list_workflow_runs_for_head(&owner, &name, &head_sha)
                        .await
                };

                if let Err(error) = this.update(cx, move |view, cx| {
                    if !selected_pull_request_matches(view, &repo, number) {
                        return;
                    }

                    view.is_loading_workflows = false;
                    match result {
                        Ok(workflow_runs) => {
                            let count = workflow_runs.len();
                            view.workflow_runs = workflow_runs;
                            view.workflows_error = None;
                            view.status = format!("Loaded {count} workflow runs for PR #{number}");

                            if view.active_tab == PanelTab::Logs
                                && view.logs_error.is_none()
                                && !view.workflow_runs.is_empty()
                            {
                                view.load_selected_workflow_logs(cx);
                            }
                        }
                        Err(error) => {
                            view.workflow_runs.clear();
                            view.workflows_error = Some(error.to_string());
                            view.status = format!("Failed to load workflow runs for PR #{number}");
                        }
                    }

                    view.cache_current_pull_request_detail_snapshot();
                    cx.notify();
                }) {
                    eprintln!("failed to update pull request workflow state: {error}");
                }
            }
        }));
    }

    fn spawn_pull_request_initial_review_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        review_data_generation: u64,
        cx: &mut Context<Self>,
    ) {
        self.pr_detail_tasks.push(cx.spawn({
            let repo = load.repo;
            let owner = load.owner;
            let name = load.name;
            let number = load.number;

            async move |this, cx| {
                let client = GitHubClient::new(GhCliTransport);
                let current_user_result = client.current_user().await;
                let pull_request_reviews_result = client
                    .list_pull_request_reviews(&owner, &name, number)
                    .await;
                let pending_review_comment_count_result =
                    if let Ok(reviews) = pull_request_reviews_result.as_ref() {
                        if let Some(review_id) = pending_review_rest_id(
                            reviews,
                            current_user_result.as_ref().ok().map(String::as_str),
                        ) {
                            Some(
                                client
                                    .pull_request_review_comment_count(
                                        &owner, &name, number, &review_id,
                                    )
                                    .await,
                            )
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                let review_threads_result =
                    client.list_review_threads(&owner, &name, number).await;

                if let Err(error) = this.update(cx, move |view, cx| {
                    if !selected_pull_request_matches(view, &repo, number) {
                        return;
                    }
                    if view.review_data_generation() != review_data_generation {
                        return;
                    }

                    view.is_loading_reviews = false;
                    let mut loaded_review_thread_count = None;
                    let current_user_login = match current_user_result {
                        Ok(login) => {
                            view.reviews_error = None;
                            Some(login)
                        }
                        Err(error) => {
                            view.reviews_error =
                                Some(format!("Failed to detect current user: {error}"));
                            None
                        }
                    };

                    let reviews = match pull_request_reviews_result {
                        Ok(reviews) => Some(reviews),
                        Err(error) => {
                            view.pull_request_reviews.clear();
                            let message = format!("Failed to load review history: {error}");
                            view.reviews_error = Some(match view.reviews_error.take() {
                                Some(existing) => format!("{existing}; {message}"),
                                None => message,
                            });
                            None
                        }
                    };
                    let pending_review_comment_count = match pending_review_comment_count_result {
                        Some(Ok(count)) => Some(count),
                        Some(Err(error)) => {
                            let message =
                                format!("Failed to count pending review comments: {error}");
                            view.reviews_error = Some(match view.reviews_error.take() {
                                Some(existing) => format!("{existing}; {message}"),
                                None => message,
                            });
                            None
                        }
                        None => None,
                    };

                    match (reviews, review_threads_result) {
                        (Some(reviews), Ok(review_threads)) => {
                            let thread_count = review_threads.len();
                            view.apply_loaded_review_data(
                                reviews,
                                review_threads,
                                current_user_login,
                                pending_review_comment_count,
                            );
                            view.refresh_owned_file_filters(cx);
                            loaded_review_thread_count = Some(thread_count);
                        }
                        (None, Ok(review_threads)) => {
                            let thread_count = review_threads.len();
                            view.replace_loaded_review_threads(review_threads);
                            let unresolved_count = view
                                .review_threads
                                .iter()
                                .filter(|thread| thread.state == ReviewThreadState::Unresolved)
                                .count();
                            if let Some(selected) = view.pull_requests.get_mut(view.selected_pr) {
                                selected.unresolved_threads = unresolved_count;
                            }
                            loaded_review_thread_count = Some(thread_count);
                        }
                        (Some(reviews), Err(error)) => {
                            view.apply_loaded_review_data(
                                reviews,
                                Vec::new(),
                                current_user_login,
                                pending_review_comment_count,
                            );
                            view.refresh_owned_file_filters(cx);
                            let message = format!("Failed to load review threads: {error}");
                            view.reviews_error = Some(match view.reviews_error.take() {
                                Some(existing) => format!("{existing}; {message}"),
                                None => message,
                            });
                        }
                        (None, Err(error)) => {
                            let message = format!("Failed to load review threads: {error}");
                            view.reviews_error = Some(match view.reviews_error.take() {
                                Some(existing) => format!("{existing}; {message}"),
                                None => message,
                            });
                        }
                    }

                    view.status = match (view.reviews_error.as_ref(), loaded_review_thread_count) {
                        (None, Some(count)) => {
                            format!("Loaded review history and {count} threads for PR #{number}")
                        }
                        (None, None) => format!("Loaded review history for PR #{number}"),
                        (Some(_), Some(count)) => {
                            format!("Loaded {count} review threads for PR #{number}, with review warnings")
                        }
                        (Some(_), None) => format!("Failed to load review data for PR #{number}"),
                    };

                    view.cache_current_pull_request_detail_snapshot();
                    cx.notify();
                }) {
                    eprintln!("failed to update pull request review state: {error}");
                }
            }
        }));
    }
}
