use gpui::{AppContext, Context};
use harbor_domain::{
    CheckRun, DiffFile, PullRequest, PullRequestComment, PullRequestReview, ReviewThread,
    WorkflowRun,
};
use harbor_sync::SyncTarget;

use crate::{
    diff::{ParsedDiff, parse_files},
    panels::checks_summary_from_runs,
    workspace::{
        AppView, async_updates::AppViewAsyncUpdateExt,
        pull_request_detail_loaders::SelectedPullRequestLoad,
        review_data_loaders::selected_pull_request_matches,
    },
};

#[derive(Clone, Debug, Default)]
struct CachedSelectedPullRequestDetail {
    metadata: Option<PullRequest>,
    files: Option<(Vec<DiffFile>, Vec<Option<ParsedDiff>>)>,
    check_runs: Option<Vec<CheckRun>>,
    workflow_runs: Option<Vec<WorkflowRun>>,
    review_data: Option<(
        Vec<PullRequestReview>,
        Vec<PullRequestComment>,
        Vec<ReviewThread>,
    )>,
}

impl AppView {
    pub(super) fn spawn_cached_selected_pull_request_detail_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        defer_review_load_until_cache: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.repository_state.store() else {
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
            .push_pull_request_detail_task(cx.spawn(async move |this, cx| {
                let result = task.await;

                this.update_or_log(
                    cx,
                    "failed to update cached pull request detail state",
                    move |view, cx| {
                        if !selected_pull_request_matches(view, &load.repo, load.number) {
                            return;
                        }

                        let Ok(cached) = result else {
                            if defer_review_load_until_cache {
                                view.review_state.reset_reviews_load();
                                view.load_active_panel_data_if_needed(cx);
                                cx.notify();
                            }
                            return;
                        };
                        let mut applied_any = false;
                        let mut applied_review_data = false;

                        if let Some(metadata) = cached.metadata
                            && !view.detail_state.details_loaded()
                        {
                            view.replace_selected_pull_request_preserving_row_fields(metadata);
                            applied_any = true;
                        }

                        if let Some((files, diffs)) = cached.files
                            && view.detail_state.files().is_empty()
                        {
                            view.detail_state.replace_diff_files(files, diffs);
                            view.ensure_active_file_visible(cx);
                            view.sync_diff_list_items(cx);
                            applied_any = true;
                        }

                        if let Some(check_runs) = cached.check_runs
                            && view.detail_state.check_runs().is_empty()
                        {
                            let summary = checks_summary_from_runs(&check_runs);
                            view.detail_state.replace_check_runs(check_runs);
                            if let Some(selected) = view
                                .pull_requests
                                .get_mut(view.selection_state.pull_request_index())
                            {
                                selected.checks_summary = summary;
                            }
                            applied_any = true;
                        }

                        if let Some(workflow_runs) = cached.workflow_runs
                            && view.detail_state.workflow_runs().is_empty()
                        {
                            view.detail_state.replace_workflow_runs(workflow_runs);
                            applied_any = true;
                        }

                        if let Some((reviews, comments, threads)) = cached.review_data
                            && view.review_state.pull_request_reviews.is_empty()
                            && view.review_state.pull_request_comments.is_empty()
                            && view.review_state.review_threads.is_empty()
                        {
                            view.replace_reviews_and_loaded_threads(reviews, comments, threads);
                            view.sync_diff_list_items(cx);
                            view.review_state.apply_reviews_success();
                            view.mark_sync_success(SyncTarget::SelectedPullRequestReviews);
                            view.refresh_owned_file_filters(cx);
                            applied_any = true;
                            applied_review_data = true;
                        }

                        if defer_review_load_until_cache && !applied_review_data {
                            view.review_state.reset_reviews_load();
                            view.load_active_panel_data_if_needed(cx);
                        }

                        if applied_any {
                            view.status = format!("Showing cached PR #{} details", load.number);
                            cx.notify();
                        }
                    },
                );
            }));
    }
}
