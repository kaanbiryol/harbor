use gpui::Context;

use crate::workspace::{
    AppView, async_updates::AppViewAsyncUpdateExt,
    pull_request_detail_loaders::SelectedPullRequestLoad,
    review_data_loaders::selected_pull_request_matches,
};

impl AppView {
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
        self.tasks.push_pull_request_detail_task(cx.spawn({
            let repo = load.repo;
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
                        if !selected_pull_request_matches(view, &repo, number) {
                            return;
                        }

                        match result {
                            Ok(commits) => {
                                let count = commits.len();
                                view.detail_state.replace_commits(commits);
                                view.detail_state.apply_commits_success();
                                view.status = format!("Loaded {count} commits for PR #{number}");
                            }
                            Err(error) => {
                                view.detail_state.clear_commits();
                                view.detail_state.apply_commits_failure(error.to_string());
                                view.status = format!("Failed to load commits for PR #{number}");
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
