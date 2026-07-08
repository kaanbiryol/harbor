use gpui::{AppContext, Context};
use harbor_domain::RepoId;

use crate::workspace::{
    AppView, PullRequestInboxCacheKey, PullRequestInboxMode, async_updates::AppViewAsyncUpdateExt,
};

impl AppView {
    pub(super) fn prefetch_pull_request_inbox_counts(
        &mut self,
        repo: RepoId,
        active_mode: PullRequestInboxMode,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.prefetch_inbox_counts {
            return;
        }

        let modes = PullRequestInboxMode::ALL
            .into_iter()
            .filter(|mode| force || *mode != active_mode)
            .filter(|mode| {
                force
                    || self
                        .pull_request_inbox
                        .snapshot_count(&PullRequestInboxCacheKey::new(repo.clone(), *mode))
                        .is_none()
            })
            .collect::<Vec<_>>();

        if modes.is_empty() {
            return;
        }

        let github_api = self.github_api.clone();
        let task = cx.background_spawn(async move {
            let mut counts = Vec::with_capacity(modes.len());
            let mut errors = Vec::new();

            for mode in modes {
                match github_api
                    .count_repository_pull_requests(&repo, mode.list_filter())
                    .await
                {
                    Ok(count) => counts.push((mode, count)),
                    Err(error) => errors.push((mode, error.to_string())),
                }
            }

            (repo, counts, errors)
        });

        cx.spawn(async move |this, cx| {
            let (repo, counts, errors) = task.await;

            this.update_or_log(
                cx,
                "failed to update pull request inbox counts",
                move |view, cx| {
                    for (mode, error) in errors {
                        tracing::warn!(
                            repository = %repo.full_name(),
                            mode = mode.key(),
                            %error,
                            "failed to load pull request inbox count"
                        );
                    }

                    for (mode, count) in counts {
                        view.pull_request_inbox
                            .insert_count(PullRequestInboxCacheKey::new(repo.clone(), mode), count);
                    }

                    cx.notify();
                },
            );
        })
        .detach();
    }
}
