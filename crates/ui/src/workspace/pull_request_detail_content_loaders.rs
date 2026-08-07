use std::sync::Arc;

use gpui::{AppContext, AsyncApp, Context, ScrollStrategy, WeakEntity};
use gpui_component::{ActiveTheme, highlighter::HighlightTheme};
use harbor_domain::DiffFile;
use harbor_sync::{SyncTarget, refresh_pull_request_files, refresh_pull_request_metadata};

use crate::{
    diff::{ParsedDiff, parse_files, parse_unified_diff_with_syntax},
    workspace::{
        AppView, PullRequestDetailCacheKey, SelectedPullRequestTaskKind,
        async_updates::AppViewAsyncUpdateExt, pull_request_detail_loaders::SelectedPullRequestLoad,
        review_data_loaders::selected_pull_request_matches,
    },
};

type PullRequestFilesResult = harbor_github::Result<(Vec<DiffFile>, Vec<Option<ParsedDiff>>)>;

struct DiffSyntaxUpdate {
    detail_key: PullRequestDetailCacheKey,
    commit_sha: Option<String>,
    file_index: usize,
    file_path: String,
    expected_patch: String,
    highlighted_diff: ParsedDiff,
}

impl AppView {
    pub(super) fn spawn_pull_request_metadata_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        cx: &mut Context<Self>,
    ) {
        if !self.detail_state.should_load_details() {
            return;
        }

        self.detail_state.start_details_load();
        let github_api = self.github_api.clone();
        let store = self.repository_state.store();
        let detail_key = load.detail_key();
        self.tasks.set_selected_pull_request_task(
            SelectedPullRequestTaskKind::Metadata,
            cx.spawn({
                let repo = load.repo;
                let number = load.number;

                async move |this, cx| {
                    let refresh = refresh_pull_request_metadata(
                        github_api.as_ref(),
                        store.as_ref(),
                        &repo,
                        number,
                    )
                    .await;

                    this.update_or_log(
                        cx,
                        "failed to update pull request detail state",
                        move |view, cx| {
                            if !selected_pull_request_matches(view, &detail_key) {
                                return;
                            }

                            if let Some(error) = refresh.cache_error {
                                view.repository_state.set_error(error);
                            }
                            match refresh.result {
                                Ok(detail) => {
                                    view.mark_sync_success(SyncTarget::SelectedPullRequestMetadata);
                                    view.replace_selected_pull_request_preserving_row_fields(
                                        detail,
                                    );
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
            }),
        );
    }

    pub(super) fn spawn_pull_request_files_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        cx: &mut Context<Self>,
    ) {
        if !self.detail_state.should_load_files() {
            return;
        }

        self.detail_state.start_files_load();
        let github_api = self.github_api.clone();
        let store = self.repository_state.store();
        let detail_key = load.detail_key();
        self.tasks.set_selected_pull_request_task(
            SelectedPullRequestTaskKind::Files,
            cx.spawn({
                let repo = load.repo;
                let number = load.number;
                let head_sha = load.head_sha;
                let highlight_theme = cx.theme().highlight_theme.clone();

                async move |this, cx| {
                    let refresh = refresh_pull_request_files(
                        github_api.as_ref(),
                        store.as_ref(),
                        &repo,
                        number,
                        &head_sha,
                    )
                    .await;
                    let result = match refresh.result {
                        Ok(files) => Ok(cx
                            .background_spawn(async move {
                                let diffs = parse_files(&files);
                                (files, diffs)
                            })
                            .await),
                        Err(error) => Err(error),
                    };
                    let update_detail_key = detail_key.clone();

                    let Some(files_for_syntax) = this
                        .update_or_log(
                            cx,
                            "failed to update pull request file state",
                            move |view, cx| {
                                view.apply_pull_request_files_result(
                                    &update_detail_key,
                                    result,
                                    refresh.cache_error,
                                    cx,
                                )
                            },
                        )
                        .flatten()
                    else {
                        return;
                    };

                    Self::highlight_diff_files_progressively(
                        &this,
                        cx,
                        detail_key,
                        None,
                        files_for_syntax,
                        highlight_theme,
                    )
                    .await;
                }
            }),
        );
    }

    fn apply_pull_request_files_result(
        &mut self,
        detail_key: &PullRequestDetailCacheKey,
        result: PullRequestFilesResult,
        cache_error: Option<String>,
        cx: &mut Context<Self>,
    ) -> Option<Vec<DiffFile>> {
        if !selected_pull_request_matches(self, detail_key) {
            return None;
        }
        let number = detail_key.number();

        if let Some(error) = cache_error {
            self.repository_state.set_error(error);
        }
        let files_for_syntax = result.as_ref().ok().map(|(files, _)| files.clone());
        match result {
            Ok((files, diffs)) => {
                let count = files.len();
                self.detail_state.replace_diff_files(files, diffs);
                self.reset_diff_selection();
                self.changed_files_state.expanded_diff_file_paths.clear();
                self.changed_files_state.collapsed_diff_file_paths.clear();
                self.reset_changed_file_filters();
                self.sync_reviewed_file_paths_from_files();
                self.ensure_active_file_visible(cx);
                self.clear_review_composer_state();
                self.sync_diff_list_items(cx);
                self.refresh_owned_file_filters(cx);
                let row_index = self
                    .file_tree_row_index_for_file(self.active_file_index(), cx)
                    .unwrap_or(0);
                self.file_list_scroll
                    .scroll_to_item(row_index, ScrollStrategy::Top);
                self.reset_diff_list_scroll();
                self.detail_state.apply_files_success();
                self.status = format!("Loaded {count} changed files for PR #{number}");
            }
            Err(error) => {
                self.detail_state.clear_diff_files();
                self.changed_files_state.collapsed_file_tree_folders.clear();
                self.changed_files_state.expanded_diff_file_paths.clear();
                self.changed_files_state.collapsed_diff_file_paths.clear();
                self.changed_files_state.reviewed_file_paths.clear();
                self.reset_changed_file_filters();
                self.changed_files_state.owned_file_paths.clear();
                self.reset_diff_selection();
                self.clear_review_composer_state();
                self.sync_diff_list_items(cx);
                self.file_list_scroll.scroll_to_item(0, ScrollStrategy::Top);
                self.reset_diff_list_scroll();
                self.detail_state.apply_files_failure(error.to_string());
                self.status = format!("Failed to load changed files for PR #{number}");
            }
        }

        self.cache_current_pull_request_detail_snapshot();
        cx.notify();
        files_for_syntax
    }

    pub(super) async fn highlight_diff_files_progressively(
        this: &WeakEntity<Self>,
        cx: &mut AsyncApp,
        detail_key: PullRequestDetailCacheKey,
        commit_sha: Option<String>,
        files: Vec<DiffFile>,
        highlight_theme: Arc<HighlightTheme>,
    ) {
        let mut syntax_updated = false;

        for (file_index, file) in files.into_iter().enumerate() {
            let file_path = file.path.clone();
            let Some(patch) = file.patch.clone() else {
                continue;
            };
            let expected_patch = patch.clone();
            let highlight_theme = highlight_theme.clone();
            let highlighted_diff = cx
                .background_spawn(async move {
                    parse_unified_diff_with_syntax(&file, &patch, &highlight_theme)
                })
                .await;
            let update = DiffSyntaxUpdate {
                detail_key: detail_key.clone(),
                commit_sha: commit_sha.clone(),
                file_index,
                file_path,
                expected_patch,
                highlighted_diff,
            };

            let Some(updated) = this
                .update_or_log(
                    cx,
                    "failed to update pull request syntax highlight state",
                    move |view, cx| view.apply_pull_request_file_syntax(update, cx),
                )
                .flatten()
            else {
                return;
            };
            syntax_updated |= updated;
        }

        if syntax_updated && commit_sha.is_none() {
            this.update_or_log(
                cx,
                "failed to cache pull request syntax highlight state",
                move |view, _| {
                    if view.diff_syntax_target_matches(&detail_key, None) {
                        view.cache_current_pull_request_detail_snapshot();
                    }
                },
            );
        }
    }

    fn apply_pull_request_file_syntax(
        &mut self,
        update: DiffSyntaxUpdate,
        cx: &mut Context<Self>,
    ) -> Option<bool> {
        if !self.diff_syntax_target_matches(&update.detail_key, update.commit_sha.as_deref()) {
            return None;
        }
        let file = self.detail_state.files().get(update.file_index)?;
        if file.path != update.file_path
            || file.patch.as_deref() != Some(update.expected_patch.as_str())
        {
            return None;
        }

        if self
            .detail_state
            .replace_parsed_diff(update.file_index, update.highlighted_diff)
        {
            cx.notify();
            Some(true)
        } else {
            None
        }
    }

    fn diff_syntax_target_matches(
        &self,
        detail_key: &PullRequestDetailCacheKey,
        commit_sha: Option<&str>,
    ) -> bool {
        selected_pull_request_matches(self, detail_key)
            && self.active_commit_sha.as_deref() == commit_sha
    }
}
