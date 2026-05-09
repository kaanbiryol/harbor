use gpui::{AppContext, Context, ScrollStrategy};
use gpui_component::ActiveTheme;
use harbor_domain::RepoId;
use harbor_github::{GhCliTransport, GitHubClient};

use crate::{
    actions::PanelTab,
    diff::{parse_files, parse_unified_diff_with_syntax},
    panels::checks_summary_from_runs,
    workspace::{
        AppView,
        review_data_loaders::{
            ReviewDataLoadMode, ReviewDataLoadTarget, selected_pull_request_matches,
        },
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
        self.spawn_review_data_loader(
            ReviewDataLoadTarget::new(load.repo, load.number, review_data_generation),
            ReviewDataLoadMode::Initial,
            cx,
        );
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
        self.diff_selection.file_index = 0;
        self.diff_selection.hunk_index = 0;
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
                    crate::workspace::log_entity_update_error(
                        "failed to update pull request detail state",
                        error,
                    );
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
                            view.diff_selection.file_index = 0;
                            view.diff_selection.hunk_index = 0;
                            view.reset_changed_file_filters();
                            view.prune_reviewed_file_paths();
                            view.ensure_active_file_visible(cx);
                            view.clear_review_composer_state();
                            view.refresh_owned_file_filters(cx);
                            let row_index = view
                                .file_tree_row_index_for_file(view.diff_selection.file_index, cx)
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
                            view.diff_selection.file_index = 0;
                            view.diff_selection.hunk_index = 0;
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
                    crate::workspace::log_entity_update_error(
                        "failed to update pull request file state",
                        error,
                    );
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
                        crate::workspace::log_entity_update_error(
                            "failed to update pull request syntax highlight state",
                            error,
                        );
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
                    crate::workspace::log_entity_update_error(
                        "failed to update pull request checks state",
                        error,
                    );
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
                    crate::workspace::log_entity_update_error(
                        "failed to update pull request workflow state",
                        error,
                    );
                }
            }
        }));
    }
}
