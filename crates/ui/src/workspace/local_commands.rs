use std::path::PathBuf;

use gpui::{AppContext, Context, PathPromptOptions, Window};
use harbor_domain::RepoId;

use crate::{
    actions::{CheckoutPullRequest, ChooseLocalCheckout},
    workspace::{AppView, async_updates::AppViewAsyncUpdateExt},
};

mod open_with;

impl AppView {
    pub(super) fn choose_local_checkout(
        &mut self,
        _: &ChooseLocalCheckout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repository) = self.current_repository().cloned() else {
            self.status = "Select a repository before choosing a local checkout".to_string();
            cx.notify();
            return;
        };

        let selected_path = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(format!("Select local checkout for {}", repository.full_name()).into()),
        });
        let view = cx.entity().clone();

        cx.spawn_in(window, async move |_, window| {
            let Ok(Ok(Some(paths))) = selected_path.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };

            if let Err(error) = window.update(|_, cx| {
                view.update(cx, |view, cx| {
                    view.validate_and_store_local_checkout(repository, path, cx);
                })
            }) {
                crate::workspace::log_entity_update_error(
                    "failed to start local checkout validation",
                    error,
                );
            }
        })
        .detach();
    }

    fn validate_and_store_local_checkout(
        &mut self,
        repository: RepoId,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        let store = self.repository_state.store();
        let repository_for_task = repository.clone();
        let owner = repository.owner.clone();
        let repo_name = repository.name.clone();
        let path_for_status = path.display().to_string();

        self.status = format!(
            "Validating local checkout for {} at {path_for_status}",
            repository.full_name()
        );
        cx.notify();

        let task = cx.background_spawn(async move {
            let local_repository = harbor_git::validate_repository_path(&path, &owner, &repo_name)
                .map_err(|error| error.to_string())?;

            if let Some(store) = store {
                store
                    .set_repository_local_path(&repository_for_task, &local_repository.repo_path)
                    .await
                    .map_err(|error| error.to_string())?;
            }

            Ok::<PathBuf, String>(local_repository.repo_path)
        });

        self.tasks.set_local_task(cx.spawn(async move |this, cx| {
            let result = task.await;

            this.update_or_log(
                cx,
                "failed to update local checkout state",
                move |view, cx| {
                    match result {
                        Ok(repo_path) => {
                            view.set_repository_local_path(repository.clone(), repo_path.clone());
                            view.repository_state.clear_error();
                            view.status = format!(
                                "Saved local checkout for {} at {}",
                                repository.full_name(),
                                repo_path.display()
                            );
                            view.refresh_owned_file_filters(cx);
                        }
                        Err(error) => {
                            view.repository_state.set_error(error.clone());
                            view.status = format!("Failed to save local checkout: {error}");
                        }
                    }

                    view.tasks.clear_local_task();
                    cx.notify();
                },
            );
        }));
    }

    pub(super) fn checkout_pr(
        &mut self,
        _: &CheckoutPullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pr) = self.selected_pull_request().cloned() else {
            self.status = "Select a pull request before checkout".to_string();
            cx.notify();
            return;
        };

        let Some(repo_path) = self.repository_state.local_path(&pr.repo).cloned() else {
            self.status = format!(
                "Choose a local checkout for {} before checkout",
                pr.repo.full_name()
            );
            cx.notify();
            return;
        };

        self.status = format!("Preparing PR #{} worktree", pr.number);
        cx.notify();

        let task = cx.background_spawn(async move {
            harbor_git::create_or_update_pr_worktree(
                &repo_path,
                &pr.repo.owner,
                &pr.repo.name,
                pr.number,
            )
            .map(|path| format!("Prepared PR #{} worktree at {}", pr.number, path.display()))
            .map_err(|error| error.to_string())
        });

        self.tasks.set_local_task(cx.spawn(async move |this, cx| {
            let result = task.await;

            this.update_or_log(cx, "failed to update checkout state", move |view, cx| {
                view.status = match result {
                    Ok(status) => status,
                    Err(error) => format!("Failed to prepare PR worktree: {error}"),
                };
                view.tasks.clear_local_task();
                cx.notify();
            });
        }));
    }
}
