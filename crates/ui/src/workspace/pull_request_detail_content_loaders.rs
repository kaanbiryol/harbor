use gpui::{AppContext, Context, ScrollStrategy};
use gpui_component::ActiveTheme;
use harbor_sync::SyncTarget;

use crate::{
    diff::{parse_files, parse_unified_diff_with_syntax},
    workspace::{
        AppView, async_updates::AppViewAsyncUpdateExt,
        pull_request_detail_loaders::SelectedPullRequestLoad,
        review_data_loaders::selected_pull_request_matches,
    },
};

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
        self.tasks.push_pull_request_detail_task(cx.spawn({
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
        self.tasks.push_pull_request_detail_task(cx.spawn({
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
                                view.detail_state.replace_diff_files(files, diffs);
                                view.reset_diff_selection();
                                view.reset_changed_file_filters();
                                view.sync_reviewed_file_paths_from_files();
                                view.ensure_active_file_visible(cx);
                                view.clear_review_composer_state();
                                view.sync_diff_list_items(cx);
                                view.refresh_owned_file_filters(cx);
                                let row_index = view
                                    .file_tree_row_index_for_file(view.active_file_index(), cx)
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
                                view.detail_state.clear_diff_files();
                                view.collapsed_file_tree_folders.clear();
                                view.reviewed_file_paths.clear();
                                view.reset_changed_file_filters();
                                view.owned_file_paths.clear();
                                view.reset_diff_selection();
                                view.clear_review_composer_state();
                                view.sync_diff_list_items(cx);
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
                                .files()
                                .get(file_index)
                                .map(|file| file.path.as_str())
                                != Some(file_path.as_str())
                            {
                                return;
                            }

                            if view
                                .detail_state
                                .replace_parsed_diff(file_index, highlighted_diff)
                            {
                                view.cache_current_pull_request_detail_snapshot();
                                cx.notify();
                            }
                        },
                    );
                }
            }
        }));
    }
}
