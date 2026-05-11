use gpui::{AppContext, Context, ScrollStrategy};
use gpui_component::ActiveTheme;
use harbor_domain::{MergeState, PullRequest, RepoId};
use harbor_sync::SyncTarget;

use crate::{
    actions::PanelTab,
    diff::{ParsedDiff, parse_files, parse_unified_diff_with_syntax},
    panels::checks_summary_from_runs,
    workspace::{
        AppView,
        async_updates::AppViewAsyncUpdateExt,
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
struct SelectedPullRequestLoad {
    repo: RepoId,
    owner: String,
    name: String,
    number: u64,
    head_sha: String,
}

#[derive(Clone, Debug, Default)]
struct CachedSelectedPullRequestDetail {
    metadata: Option<PullRequest>,
    files: Option<(Vec<harbor_domain::DiffFile>, Vec<Option<ParsedDiff>>)>,
    check_runs: Option<Vec<harbor_domain::CheckRun>>,
    workflow_runs: Option<Vec<harbor_domain::WorkflowRun>>,
    review_data: Option<(
        Vec<harbor_domain::PullRequestReview>,
        Vec<harbor_domain::ReviewThread>,
    )>,
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
        let Some(selected) = self.pull_requests.get_mut(self.selected_pr) else {
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
        if fetch_policy == PullRequestDetailFetchPolicy::PreferCache {
            self.spawn_cached_selected_pull_request_detail_loader(load.clone(), cx);
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
                } else if !self.detail_state.log_state.is_loading
                    && self.detail_state.log_state.chunk.is_none()
                    && self.detail_state.log_state.error.is_none()
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

    fn spawn_cached_selected_pull_request_detail_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.repository_state.repository_store.clone() else {
            return;
        };

        let task = cx.background_spawn({
            let repo = load.repo.clone();
            let head_sha = load.head_sha.clone();
            async move {
                let metadata = store
                    .load_pull_request_metadata(&repo, load.number, &head_sha)
                    .await?;
                let files = store
                    .load_pull_request_files(&repo, load.number, &head_sha)
                    .await?
                    .map(|files| {
                        let diffs = parse_files(&files);
                        (files, diffs)
                    });
                let check_runs = store
                    .load_pull_request_check_runs(&repo, load.number, &head_sha)
                    .await?;
                let workflow_runs = store
                    .load_pull_request_workflow_runs(&repo, load.number, &head_sha)
                    .await?;
                let review_data = store
                    .load_pull_request_reviews(&repo, load.number, &head_sha)
                    .await?;

                harbor_storage::Result::Ok(CachedSelectedPullRequestDetail {
                    metadata,
                    files,
                    check_runs,
                    workflow_runs,
                    review_data,
                })
            }
        });

        self.tasks
            .pr_detail_tasks
            .push(cx.spawn(async move |this, cx| {
                let result = task.await;

                this.update_or_log(
                    cx,
                    "failed to update cached pull request detail state",
                    move |view, cx| {
                        if !selected_pull_request_matches(view, &load.repo, load.number) {
                            return;
                        }

                        let Ok(cached) = result else {
                            return;
                        };
                        let mut applied_any = false;

                        if let Some(metadata) = cached.metadata
                            && !view.detail_state.details_loaded()
                        {
                            view.replace_selected_pull_request_preserving_row_fields(metadata);
                            applied_any = true;
                        }

                        if let Some((files, diffs)) = cached.files
                            && view.detail_state.files.is_empty()
                        {
                            view.detail_state.files = files;
                            view.detail_state.diffs = diffs;
                            view.ensure_active_file_visible(cx);
                            applied_any = true;
                        }

                        if let Some(check_runs) = cached.check_runs
                            && view.detail_state.check_runs.is_empty()
                        {
                            let summary = checks_summary_from_runs(&check_runs);
                            view.detail_state.check_runs = check_runs;
                            if let Some(selected) = view.pull_requests.get_mut(view.selected_pr) {
                                selected.checks_summary = summary;
                            }
                            applied_any = true;
                        }

                        if let Some(workflow_runs) = cached.workflow_runs
                            && view.detail_state.workflow_runs.is_empty()
                        {
                            view.detail_state.workflow_runs = workflow_runs;
                            applied_any = true;
                        }

                        if let Some((reviews, threads)) = cached.review_data
                            && view.review_state.pull_request_reviews.is_empty()
                            && view.review_state.review_threads.is_empty()
                        {
                            view.review_state.pull_request_reviews = reviews;
                            view.replace_loaded_review_threads(threads);
                            view.refresh_owned_file_filters(cx);
                            applied_any = true;
                        }

                        if applied_any {
                            view.status = format!("Showing cached PR #{} details", load.number);
                            cx.notify();
                        }
                    },
                );
            }));
    }

    fn spawn_pull_request_metadata_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        cx: &mut Context<Self>,
    ) {
        if !self.detail_state.should_load_details() {
            return;
        }

        self.detail_state.start_details_load();
        let github_api = self.github_api.clone();
        let store = self.repository_state.repository_store.clone();
        self.tasks.pr_detail_tasks.push(cx.spawn({
            let repo = load.repo;
            let owner = load.owner;
            let name = load.name;
            let number = load.number;

            async move |this, cx| {
                let result = github_api.get_pull_request(&owner, &name, number).await;
                let cache_result = match (&store, result.as_ref()) {
                    (Some(store), Ok(detail)) => store
                        .save_pull_request_metadata(detail)
                        .await
                        .map_err(|error| error.to_string()),
                    (Some(store), Err(error)) => store
                        .record_sync_failure(
                            &harbor_storage::detail_target_key(
                                &repo,
                                number,
                                harbor_storage::PullRequestDetailSection::Metadata,
                            ),
                            &error.to_string(),
                        )
                        .await
                        .map_err(|error| error.to_string()),
                    (None, _) => Ok(()),
                };

                this.update_or_log(
                    cx,
                    "failed to update pull request detail state",
                    move |view, cx| {
                        if !selected_pull_request_matches(view, &repo, number) {
                            return;
                        }

                        if let Err(error) = cache_result {
                            view.repository_state.set_error(error);
                        }
                        match result {
                            Ok(detail) => {
                                view.mark_sync_success(SyncTarget::SelectedPullRequestMetadata);
                                view.replace_selected_pull_request_preserving_row_fields(detail);
                                view.detail_state.apply_details_success();
                                view.status = format!("Loaded PR #{number} details");
                            }
                            Err(error) => {
                                view.mark_sync_failure(SyncTarget::SelectedPullRequestMetadata);
                                view.detail_state.apply_details_failure(error.to_string());
                                view.status = format!("Failed to load PR #{number} details");
                            }
                        }

                        view.cache_current_pull_request_detail_snapshot();
                        cx.notify();
                    },
                );
            }
        }));
    }

    fn spawn_pull_request_files_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        cx: &mut Context<Self>,
    ) {
        if !self.detail_state.should_load_files() {
            return;
        }

        self.detail_state.start_files_load();
        let github_api = self.github_api.clone();
        let store = self.repository_state.repository_store.clone();
        self.tasks.pr_detail_tasks.push(cx.spawn({
            let repo = load.repo;
            let owner = load.owner;
            let name = load.name;
            let number = load.number;
            let head_sha = load.head_sha;
            let highlight_theme = cx.theme().highlight_theme.clone();

            async move |this, cx| {
                let result = match github_api
                    .list_pull_request_files(&owner, &name, number)
                    .await
                {
                    Ok(files) => Ok(cx
                        .background_spawn(async move {
                            let diffs = parse_files(&files);
                            (files, diffs)
                        })
                        .await),
                    Err(error) => Err(error),
                };
                let cache_result = match (&store, result.as_ref()) {
                    (Some(store), Ok((files, _))) => store
                        .save_pull_request_files(&repo, number, &head_sha, files)
                        .await
                        .map_err(|error| error.to_string()),
                    (Some(store), Err(error)) => store
                        .record_sync_failure(
                            &harbor_storage::detail_target_key(
                                &repo,
                                number,
                                harbor_storage::PullRequestDetailSection::Files,
                            ),
                            &error.to_string(),
                        )
                        .await
                        .map_err(|error| error.to_string()),
                    (None, _) => Ok(()),
                };
                let files_for_syntax = result.as_ref().ok().map(|(files, _)| files.clone());
                let update_repo = repo.clone();

                this.update_or_log(
                    cx,
                    "failed to update pull request file state",
                    move |view, cx| {
                        if !selected_pull_request_matches(view, &update_repo, number) {
                            return;
                        }

                        if let Err(error) = cache_result {
                            view.repository_state.set_error(error);
                        }
                        match result {
                            Ok((files, diffs)) => {
                                let count = files.len();
                                view.detail_state.files = files;
                                view.detail_state.diffs = diffs;
                                view.reset_diff_selection();
                                view.reset_changed_file_filters();
                                view.prune_reviewed_file_paths();
                                view.ensure_active_file_visible(cx);
                                view.clear_review_composer_state();
                                view.refresh_owned_file_filters(cx);
                                let row_index = view
                                    .file_tree_row_index_for_file(
                                        view.diff_selection.file_index,
                                        cx,
                                    )
                                    .unwrap_or(0);
                                view.file_list_scroll
                                    .scroll_to_item(row_index, ScrollStrategy::Top);
                                view.reset_diff_list_scroll();
                                view.detail_state.apply_files_success();
                                view.status =
                                    format!("Loaded {count} changed files for PR #{number}");
                            }
                            Err(error) => {
                                let error = error.to_string();
                                view.detail_state.files.clear();
                                view.detail_state.diffs.clear();
                                view.collapsed_file_tree_folders.clear();
                                view.reviewed_file_paths.clear();
                                view.reset_changed_file_filters();
                                view.owned_file_paths.clear();
                                view.reset_diff_selection();
                                view.clear_review_composer_state();
                                view.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                                view.reset_diff_list_scroll();
                                view.detail_state.apply_files_failure(error);
                                view.status =
                                    format!("Failed to load changed files for PR #{number}");
                            }
                        }

                        view.cache_current_pull_request_detail_snapshot();
                        cx.notify();
                    },
                );

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
                    this.update_or_log(
                        cx,
                        "failed to update pull request syntax highlight state",
                        move |view, cx| {
                            if !selected_pull_request_matches(view, &update_repo, number) {
                                return;
                            }
                            if view
                                .detail_state
                                .files
                                .get(file_index)
                                .map(|file| file.path.as_str())
                                != Some(file_path.as_str())
                            {
                                return;
                            }

                            if let Some(diff) = view.detail_state.diffs.get_mut(file_index) {
                                *diff = Some(highlighted_diff);
                            }
                            view.cache_current_pull_request_detail_snapshot();
                            cx.notify();
                        },
                    );
                }
            }
        }));
    }

    fn spawn_pull_request_checks_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        cx: &mut Context<Self>,
    ) {
        if !self.detail_state.should_load_checks() {
            return;
        }

        self.detail_state.start_checks_load();
        let github_api = self.github_api.clone();
        let store = self.repository_state.repository_store.clone();
        self.tasks.pr_detail_tasks.push(cx.spawn({
            let repo = load.repo;
            let owner = load.owner;
            let name = load.name;
            let number = load.number;
            let head_sha = load.head_sha;

            async move |this, cx| {
                let result = if head_sha.is_empty() {
                    Ok(Vec::new())
                } else {
                    github_api.list_check_runs(&owner, &name, &head_sha).await
                };
                let cache_result = match (&store, result.as_ref()) {
                    (Some(store), Ok(check_runs)) => store
                        .save_pull_request_check_runs(&repo, number, &head_sha, check_runs)
                        .await
                        .map_err(|error| error.to_string()),
                    (Some(store), Err(error)) => store
                        .record_sync_failure(
                            &harbor_storage::detail_target_key(
                                &repo,
                                number,
                                harbor_storage::PullRequestDetailSection::CheckRuns,
                            ),
                            &error.to_string(),
                        )
                        .await
                        .map_err(|error| error.to_string()),
                    (None, _) => Ok(()),
                };

                this.update_or_log(
                    cx,
                    "failed to update pull request checks state",
                    move |view, cx| {
                        if !selected_pull_request_matches(view, &repo, number) {
                            return;
                        }

                        if let Err(error) = cache_result {
                            view.repository_state.set_error(error);
                        }
                        match result {
                            Ok(check_runs) => {
                                view.mark_sync_success(SyncTarget::SelectedPullRequestChecks);
                                let count = check_runs.len();
                                let summary = checks_summary_from_runs(&check_runs);
                                view.detail_state.check_runs = check_runs;
                                view.detail_state.apply_checks_success();

                                if let Some(selected) = view.pull_requests.get_mut(view.selected_pr)
                                {
                                    selected.checks_summary = summary;
                                }

                                view.status = format!("Loaded {count} check runs for PR #{number}");
                            }
                            Err(error) => {
                                view.mark_sync_failure(SyncTarget::SelectedPullRequestChecks);
                                view.detail_state.check_runs.clear();
                                view.detail_state.apply_checks_failure(error.to_string());
                                view.status = format!("Failed to load checks for PR #{number}");
                            }
                        }

                        view.cache_current_pull_request_detail_snapshot();
                        cx.notify();
                    },
                );
            }
        }));
    }

    fn spawn_pull_request_workflows_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        cx: &mut Context<Self>,
    ) {
        if !self.detail_state.should_load_workflows() {
            return;
        }

        self.detail_state.start_workflows_load();
        let github_api = self.github_api.clone();
        let store = self.repository_state.repository_store.clone();
        self.tasks.pr_detail_tasks.push(cx.spawn({
            let repo = load.repo;
            let owner = load.owner;
            let name = load.name;
            let number = load.number;
            let head_sha = load.head_sha;

            async move |this, cx| {
                let result = if head_sha.is_empty() {
                    Ok(Vec::new())
                } else {
                    github_api
                        .list_workflow_runs_for_head(&owner, &name, &head_sha)
                        .await
                };
                let cache_result = match (&store, result.as_ref()) {
                    (Some(store), Ok(workflow_runs)) => store
                        .save_pull_request_workflow_runs(&repo, number, &head_sha, workflow_runs)
                        .await
                        .map_err(|error| error.to_string()),
                    (Some(store), Err(error)) => store
                        .record_sync_failure(
                            &harbor_storage::detail_target_key(
                                &repo,
                                number,
                                harbor_storage::PullRequestDetailSection::WorkflowRuns,
                            ),
                            &error.to_string(),
                        )
                        .await
                        .map_err(|error| error.to_string()),
                    (None, _) => Ok(()),
                };

                this.update_or_log(
                    cx,
                    "failed to update pull request workflow state",
                    move |view, cx| {
                        if !selected_pull_request_matches(view, &repo, number) {
                            return;
                        }

                        if let Err(error) = cache_result {
                            view.repository_state.set_error(error);
                        }
                        match result {
                            Ok(workflow_runs) => {
                                view.mark_sync_success(SyncTarget::SelectedPullRequestWorkflows);
                                let count = workflow_runs.len();
                                view.detail_state.workflow_runs = workflow_runs;
                                view.detail_state.apply_workflows_success();
                                view.status =
                                    format!("Loaded {count} workflow runs for PR #{number}");

                                if view.active_tab == PanelTab::Logs
                                    && view.detail_state.log_state.error.is_none()
                                    && !view.detail_state.workflow_runs.is_empty()
                                {
                                    view.load_selected_workflow_logs(cx);
                                }
                            }
                            Err(error) => {
                                view.mark_sync_failure(SyncTarget::SelectedPullRequestWorkflows);
                                view.detail_state.workflow_runs.clear();
                                view.detail_state.apply_workflows_failure(error.to_string());
                                view.status =
                                    format!("Failed to load workflow runs for PR #{number}");
                            }
                        }

                        view.cache_current_pull_request_detail_snapshot();
                        cx.notify();
                    },
                );
            }
        }));
    }
}
