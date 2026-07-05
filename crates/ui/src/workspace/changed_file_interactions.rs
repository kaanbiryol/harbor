use std::collections::HashSet;

use gpui::{App, AppContext, Context, ListOffset, ScrollStrategy, px};
use harbor_domain::{DiffFile, FileViewedState};

use crate::{
    actions::PanelTab,
    diff::ParsedDiff,
    panels::{
        ContinuousDiffLayoutInput, continuous_diff_items, diff_file_item_index,
        sync_diff_list_state,
    },
    workspace::async_updates::AppViewAsyncUpdateExt,
};

use super::{
    AppView, ChangedFileFilters, ChangedFileTreeRow, ChangedFileTypeFilter, changed_file_tree_rows,
    changed_file_type_filters, codeowners::codeowners_owned_file_paths, log_entity_update_error,
    review_data_loaders::selected_pull_request_matches,
};

impl AppView {
    pub(crate) fn active_file(&self) -> Option<&DiffFile> {
        self.detail_state.files().get(self.active_file_index())
    }

    pub(crate) fn active_file_index(&self) -> usize {
        self.selection_state.file_index()
    }

    pub(crate) fn active_hunk_index(&self) -> usize {
        self.selection_state.hunk_index()
    }

    pub(crate) fn diff_files(&self) -> &[DiffFile] {
        self.detail_state.files()
    }

    pub(crate) fn parsed_diffs(&self) -> &[Option<ParsedDiff>] {
        self.detail_state.diffs()
    }

    pub(crate) fn reviewed_file_paths(&self) -> &HashSet<String> {
        &self.reviewed_file_paths
    }

    pub(crate) fn changed_file_tree_rows(&self, _cx: &App) -> Vec<ChangedFileTreeRow> {
        let filters = self.changed_file_filters();

        changed_file_tree_rows(
            self.detail_state.files(),
            &self.collapsed_file_tree_folders,
            &self.reviewed_file_paths,
            &filters,
        )
    }

    pub(crate) fn visible_file_indices(&self, cx: &App) -> Vec<usize> {
        self.changed_file_tree_rows(cx)
            .into_iter()
            .filter_map(|row| match row {
                ChangedFileTreeRow::File(file_row) => Some(file_row.file_index),
                ChangedFileTreeRow::Folder(_) => None,
            })
            .collect()
    }

    pub(crate) fn reviewed_file_count(&self) -> usize {
        self.detail_state
            .files()
            .iter()
            .filter(|file| self.reviewed_file_paths.contains(&file.path))
            .count()
    }

    pub(crate) fn changed_file_filters(&self) -> ChangedFileFilters {
        ChangedFileFilters {
            query: String::new(),
            excluded_file_types: self.excluded_file_type_filters.clone(),
            owned_by_current_user_only: self.show_files_owned_by_current_user,
            owned_file_paths: self.owned_file_paths.clone(),
        }
    }

    pub(crate) fn changed_file_type_filters(&self) -> Vec<ChangedFileTypeFilter> {
        changed_file_type_filters(self.detail_state.files(), &self.excluded_file_type_filters)
    }

    pub(crate) fn included_file_type_filter_count(&self) -> usize {
        self.changed_file_type_filters()
            .into_iter()
            .filter(|filter| filter.included)
            .count()
    }

    pub(crate) fn has_owned_file_filter_data(&self) -> bool {
        !self.owned_file_paths.is_empty()
    }

    pub(super) fn file_tree_row_index_for_file(
        &self,
        file_index: usize,
        cx: &App,
    ) -> Option<usize> {
        self.changed_file_tree_rows(cx)
            .into_iter()
            .position(|row| matches!(row, ChangedFileTreeRow::File(file_row) if file_row.file_index == file_index))
    }

    fn diff_item_index_for_file(&self, file_index: usize) -> Option<usize> {
        diff_file_item_index(&self.diff_list_items, file_index)
    }

    pub(super) fn sync_diff_list_items(&mut self, cx: &App) {
        let visible_file_indices = self.visible_file_indices(cx);
        let next_items = continuous_diff_items(ContinuousDiffLayoutInput {
            files: self.detail_state.files(),
            diffs: self.detail_state.diffs(),
            visible_file_indices: &visible_file_indices,
            reviewed_file_paths: &self.reviewed_file_paths,
            review_threads: &self.review_state.review_threads,
            review_composer: self.review_state.review_composer_state.inline_composer(),
        });
        sync_diff_list_state(&self.diff_list_state, &mut self.diff_list_items, next_items);
    }

    pub(super) fn scroll_diff_list_to_item(&mut self, item_index: usize) {
        self.diff_list_state.scroll_to(ListOffset {
            item_ix: item_index,
            offset_in_item: px(0.0),
        });
    }

    pub(super) fn reset_diff_list_scroll(&mut self) {
        self.scroll_diff_list_to_item(0);
    }

    pub(super) fn ensure_active_file_visible(&mut self, cx: &mut Context<Self>) {
        let visible_files = self.visible_file_indices(cx);
        if visible_files.is_empty() || visible_files.contains(&self.active_file_index()) {
            return;
        }

        if let Some(file_index) = visible_files.first().copied() {
            self.select_diff_file_index(file_index);
            self.clear_review_composer_state();
            self.sync_diff_list_items(cx);
            if let Some(row_index) = self.file_tree_row_index_for_file(file_index, cx) {
                self.file_list_scroll
                    .scroll_to_item(row_index, ScrollStrategy::Center);
            }
            if let Some(item_index) = self.diff_item_index_for_file(file_index) {
                self.scroll_diff_list_to_item(item_index);
            } else {
                self.reset_diff_list_scroll();
            }
        }
    }

    pub(super) fn prune_reviewed_file_paths(&mut self) {
        let file_paths = self
            .detail_state
            .files()
            .iter()
            .map(|file| file.path.clone())
            .collect::<HashSet<_>>();
        self.reviewed_file_paths
            .retain(|path| file_paths.contains(path));
        self.owned_file_paths
            .retain(|path| file_paths.contains(path));
    }

    pub(super) fn sync_reviewed_file_paths_from_files(&mut self) {
        self.reviewed_file_paths = self
            .detail_state
            .files()
            .iter()
            .filter(|file| file.viewed_state == FileViewedState::Viewed)
            .map(|file| file.path.clone())
            .collect();
        self.prune_reviewed_file_paths();
    }

    fn set_changed_file_reviewed_by_path(&mut self, path: &str, reviewed: bool) {
        if reviewed {
            self.reviewed_file_paths.insert(path.to_string());
            self.detail_state
                .set_file_viewed_state(path, FileViewedState::Viewed);
        } else {
            self.reviewed_file_paths.remove(path);
            self.detail_state
                .set_file_viewed_state(path, FileViewedState::Unviewed);
        }
    }

    pub(super) fn reset_changed_file_filters(&mut self) {
        self.excluded_file_type_filters.clear();
        self.show_files_owned_by_current_user = false;
    }

    pub(crate) fn select_file(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(path) = self
            .detail_state
            .files()
            .get(index)
            .map(|file| file.path.clone())
        {
            self.select_diff_file_index(index);
            self.active_tab = PanelTab::Diff;
            self.clear_review_composer_state();
            self.sync_diff_list_items(cx);
            if let Some(row_index) = self.file_tree_row_index_for_file(index, cx) {
                self.file_list_scroll
                    .scroll_to_item(row_index, ScrollStrategy::Center);
            }
            if let Some(item_index) = self.diff_item_index_for_file(index) {
                self.scroll_diff_list_to_item(item_index);
            }
            self.status = format!("Selected {path}");
        }

        cx.notify();
    }

    pub(crate) fn toggle_changed_file_folder(
        &mut self,
        folder_path: String,
        cx: &mut Context<Self>,
    ) {
        let status = if self.collapsed_file_tree_folders.remove(&folder_path) {
            format!("Expanded {folder_path}")
        } else {
            self.collapsed_file_tree_folders.insert(folder_path.clone());
            format!("Collapsed {folder_path}")
        };

        self.ensure_active_file_visible(cx);
        self.sync_diff_list_items(cx);
        self.status = status;
        cx.notify();
    }

    pub(crate) fn toggle_changed_file_reviewed(
        &mut self,
        file_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self
            .detail_state
            .files()
            .get(file_index)
            .map(|file| file.path.clone())
        else {
            self.status = "No changed file to mark reviewed".to_string();
            cx.notify();
            return;
        };

        let Some(pull_request) = self.selected_pull_request().cloned() else {
            self.status = "No pull request selected for file viewed sync".to_string();
            cx.notify();
            return;
        };
        if pull_request.node_id.is_empty() {
            self.status =
                "Cannot sync file viewed state without a pull request node ID".to_string();
            cx.notify();
            return;
        }

        let reviewed = !self.reviewed_file_paths.contains(&path);
        self.set_changed_file_reviewed_by_path(&path, reviewed);

        let github_api = self.github_api.clone();
        let repo = pull_request.repo.clone();
        let number = pull_request.number;
        let pull_request_node_id = pull_request.node_id.clone();
        let sync_path = path.clone();
        cx.spawn(async move |this, cx| {
            let result = if reviewed {
                github_api
                    .mark_pull_request_file_viewed(&pull_request_node_id, &sync_path)
                    .await
            } else {
                github_api
                    .unmark_pull_request_file_viewed(&pull_request_node_id, &sync_path)
                    .await
            };

            if let Err(error) = result {
                let error = error.to_string();
                this.update_or_log(
                    cx,
                    "failed to update pull request file viewed state",
                    move |view, cx| {
                        if !selected_pull_request_matches(view, &repo, number) {
                            return;
                        }

                        if view.reviewed_file_paths.contains(&sync_path) == reviewed {
                            view.set_changed_file_reviewed_by_path(&sync_path, !reviewed);
                            view.sync_diff_list_items(cx);
                        }
                        view.status = if reviewed {
                            format!("Failed to mark {sync_path} as reviewed: {error}")
                        } else {
                            format!("Failed to mark {sync_path} as unreviewed: {error}")
                        };
                        cx.notify();
                    },
                );
            }
        })
        .detach();

        let reviewed_count = self.reviewed_file_count();
        let total_count = self.detail_state.files().len();

        self.sync_diff_list_items(cx);
        self.status = if reviewed {
            format!("Marked {path} as reviewed ({reviewed_count}/{total_count})")
        } else {
            format!("Marked {path} as unreviewed ({reviewed_count}/{total_count})")
        };
        cx.notify();
    }

    pub(crate) fn toggle_changed_file_type_filter(
        &mut self,
        file_type: String,
        cx: &mut Context<Self>,
    ) {
        let included = if self.excluded_file_type_filters.remove(&file_type) {
            true
        } else {
            self.excluded_file_type_filters.insert(file_type.clone());
            false
        };
        let visible_count = self.visible_file_indices(cx).len();

        self.ensure_active_file_visible(cx);
        self.sync_diff_list_items(cx);
        self.status = if included {
            format!("Included {file_type} files ({visible_count} visible)")
        } else {
            format!("Excluded {file_type} files ({visible_count} visible)")
        };
        cx.notify();
    }

    pub(crate) fn include_all_changed_file_types(&mut self, cx: &mut Context<Self>) {
        self.excluded_file_type_filters.clear();
        self.ensure_active_file_visible(cx);
        self.sync_diff_list_items(cx);
        let visible_count = self.visible_file_indices(cx).len();
        self.status = format!("Included all file types ({visible_count} visible)");
        cx.notify();
    }

    pub(crate) fn show_all_changed_files(&mut self, cx: &mut Context<Self>) {
        self.show_files_owned_by_current_user = false;
        self.ensure_active_file_visible(cx);
        self.sync_diff_list_items(cx);
        let visible_count = self.visible_file_indices(cx).len();
        self.status = format!("Showing all changed files ({visible_count} visible)");
        cx.notify();
    }

    pub(crate) fn toggle_files_owned_by_current_user_filter(&mut self, cx: &mut Context<Self>) {
        if !self.has_owned_file_filter_data() {
            self.status = "No owned-file metadata is available for this pull request".to_string();
            cx.notify();
            return;
        }

        self.show_files_owned_by_current_user = !self.show_files_owned_by_current_user;
        self.ensure_active_file_visible(cx);
        self.sync_diff_list_items(cx);
        let visible_count = self.visible_file_indices(cx).len();

        self.status = if self.show_files_owned_by_current_user {
            format!("Showing {visible_count} files owned by you")
        } else {
            format!("Showing {visible_count} changed files")
        };
        cx.notify();
    }

    pub(crate) fn refresh_owned_file_filters(&mut self, cx: &mut Context<Self>) {
        let Some(current_user_login) = self.review_state.current_user_login.clone() else {
            self.owned_file_paths.clear();
            self.show_files_owned_by_current_user = false;
            self.sync_diff_list_items(cx);
            cx.notify();
            return;
        };
        let Some(repository_path) = self.current_repository_local_path().cloned() else {
            self.owned_file_paths.clear();
            self.show_files_owned_by_current_user = false;
            self.sync_diff_list_items(cx);
            cx.notify();
            return;
        };
        if self.detail_state.files().is_empty() {
            self.owned_file_paths.clear();
            self.show_files_owned_by_current_user = false;
            self.sync_diff_list_items(cx);
            cx.notify();
            return;
        }

        let files = self.detail_state.files().to_vec();
        let selected_repository = self.current_repository().cloned();
        let selected_pr_number = self.selected_pull_request_number();
        let task = cx.background_spawn(async move {
            codeowners_owned_file_paths(&repository_path, &files, &current_user_login)
        });

        cx.spawn(async move |this, cx| {
            let result = task.await;

            if let Err(error) = this.update(cx, move |view, cx| {
                if view.current_repository().cloned() != selected_repository
                    || view.selected_pull_request_number() != selected_pr_number
                {
                    return;
                }

                match result {
                    Ok(paths) => {
                        view.owned_file_paths = paths;
                    }
                    Err(_) => {
                        view.owned_file_paths.clear();
                    }
                }

                if !view.has_owned_file_filter_data() {
                    view.show_files_owned_by_current_user = false;
                }
                view.ensure_active_file_visible(cx);
                view.sync_diff_list_items(cx);
                cx.notify();
            }) {
                log_entity_update_error("failed to update file ownership filters", error);
            }
        })
        .detach();
    }
}
