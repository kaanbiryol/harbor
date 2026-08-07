use gpui::{AppContext, Context};
use gpui_component::ActiveTheme;

use crate::{actions::PanelTab, diff::parse_files};

use crate::workspace::{
    AppView, PullRequestDetailCacheKey, SelectedPullRequestTaskKind,
    async_updates::AppViewAsyncUpdateExt, pull_request_detail_loaders::SelectedPullRequestLoad,
    review_data_loaders::selected_pull_request_matches,
};

impl AppView {
    pub(crate) fn show_full_pull_request_diff(&mut self, cx: &mut Context<Self>) {
        self.active_commit_sha = None;
        if self.restore_selected_pull_request_detail_snapshot(cx) {
            self.active_tab = PanelTab::Diff;
            self.status = "Showing full pull request diff".to_string();
            cx.notify();
            return;
        }

        self.detail_state.reset_for_selection();
        self.active_tab = PanelTab::Diff;
        self.load_active_panel_data_if_needed(cx);
        self.status = "Loading full pull request diff".to_string();
        cx.notify();
    }

    pub(crate) fn select_commit(&mut self, sha: String, cx: &mut Context<Self>) {
        let Some(pull_request) = self.selected_pull_request().cloned() else {
            return;
        };
        let github_api = self.github_api.clone();
        let detail_key = PullRequestDetailCacheKey::new(
            pull_request.repo.clone(),
            pull_request.number,
            pull_request.head_sha,
        );
        let owner = pull_request.repo.owner;
        let name = pull_request.repo.name;
        let short_sha: String = sha.chars().take(7).collect();
        let highlight_theme = cx.theme().highlight_theme.clone();

        self.active_tab = PanelTab::Diff;
        self.active_commit_sha = Some(sha.clone());
        self.detail_state.start_files_load();
        self.status = format!("Loading commit {short_sha}");
        self.tasks.set_selected_pull_request_task(
            SelectedPullRequestTaskKind::Files,
            cx.spawn(async move |this, cx| {
                let result = github_api.list_commit_files(&owner, &name, &sha).await;
                let result = match result {
                    Ok(files) => {
                        let diffs = cx
                            .background_spawn({
                                let files = files.clone();
                                async move { parse_files(&files) }
                            })
                            .await;
                        Ok((files, diffs))
                    }
                    Err(error) => Err(error),
                };
                let files_for_syntax = result.as_ref().ok().map(|(files, _)| files.clone());
                let update_detail_key = detail_key.clone();
                let update_sha = sha.clone();
                let applied =
                    this.update_or_log(cx, "failed to show commit diff", move |view, cx| {
                        if !selected_pull_request_matches(view, &update_detail_key)
                            || view.active_commit_sha.as_deref() != Some(update_sha.as_str())
                        {
                            return false;
                        }
                        match result {
                            Ok((files, diffs)) => {
                                view.detail_state.replace_diff_files(files, diffs);
                                view.detail_state.apply_files_success();
                                view.reset_diff_selection();
                                view.reset_changed_file_filters();
                                view.sync_reviewed_file_paths_from_files();
                                view.sync_diff_list_items(cx);
                                view.reset_diff_list_scroll();
                                view.status = format!("Showing commit {short_sha}");
                                cx.notify();
                                true
                            }
                            Err(error) => {
                                view.detail_state.apply_files_failure(error.to_string());
                                view.status = format!("Failed to load commit {short_sha}");
                                cx.notify();
                                false
                            }
                        }
                    });
                if applied != Some(true) {
                    return;
                }
                if let Some(files) = files_for_syntax {
                    Self::highlight_diff_files_progressively(
                        &this,
                        cx,
                        detail_key,
                        Some(sha),
                        files,
                        highlight_theme,
                    )
                    .await;
                }
            }),
        );
        cx.notify();
    }

    pub(super) fn spawn_pull_request_commits_loader(
        &mut self,
        load: SelectedPullRequestLoad,
        cx: &mut Context<Self>,
    ) {
        if !self.detail_state.should_load_commits() {
            return;
        }

        self.detail_state.start_commits_load();
        let github_api = self.github_api.clone();
        let detail_key = load.detail_key();
        self.tasks.set_selected_pull_request_task(
            SelectedPullRequestTaskKind::Commits,
            cx.spawn({
                let owner = load.owner;
                let name = load.name;
                let number = load.number;

                async move |this, cx| {
                    let result = github_api
                        .list_pull_request_commits(&owner, &name, number)
                        .await;
                    this.update_or_log(
                        cx,
                        "failed to update pull request commits",
                        move |view, cx| {
                            if !selected_pull_request_matches(view, &detail_key) {
                                return;
                            }

                            match result {
                                Ok(commits) => {
                                    let count = commits.len();
                                    view.detail_state.replace_commits(commits);
                                    view.detail_state.apply_commits_success();
                                    view.status =
                                        format!("Loaded {count} commits for PR #{number}");
                                }
                                Err(error) => {
                                    view.detail_state.clear_commits();
                                    view.detail_state.apply_commits_failure(error.to_string());
                                    view.status =
                                        format!("Failed to load commits for PR #{number}");
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
}
