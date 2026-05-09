use std::path::{Path, PathBuf};

use gpui::{AppContext, Context, PathPromptOptions, Window};
use harbor_domain::{DiffFile, FileStatus, PullRequest, RepoId};
use harbor_git::{ExternalApp, ExternalAppKind, OpenTarget};

use crate::{
    actions::{
        CheckoutPullRequest, ChooseLocalCheckout, OpenWithCursor, OpenWithFinder, OpenWithGhostty,
        OpenWithTerminal, OpenWithVsCode, OpenWithWarp, OpenWithXcode, OpenWithZed,
    },
    workspace::AppView,
};

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

    pub(super) fn open_with_vs_code(
        &mut self,
        _: &OpenWithVsCode,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::VsCode, cx);
    }

    pub(super) fn open_with_cursor(
        &mut self,
        _: &OpenWithCursor,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Cursor, cx);
    }

    pub(super) fn open_with_zed(
        &mut self,
        _: &OpenWithZed,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Zed, cx);
    }

    pub(super) fn open_with_finder(
        &mut self,
        _: &OpenWithFinder,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Finder, cx);
    }

    pub(super) fn open_with_terminal(
        &mut self,
        _: &OpenWithTerminal,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Terminal, cx);
    }

    pub(super) fn open_with_ghostty(
        &mut self,
        _: &OpenWithGhostty,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Ghostty, cx);
    }

    pub(super) fn open_with_warp(
        &mut self,
        _: &OpenWithWarp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Warp, cx);
    }

    pub(super) fn open_with_xcode(
        &mut self,
        _: &OpenWithXcode,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Xcode, cx);
    }

    fn validate_and_store_local_checkout(
        &mut self,
        repository: RepoId,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        let store = self.repository_store.clone();
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

        self.local_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;

            if let Err(error) = this.update(cx, move |view, cx| {
                match result {
                    Ok(repo_path) => {
                        view.set_repository_local_path(repository.clone(), repo_path.clone());
                        view.repository_error = None;
                        view.status = format!(
                            "Saved local checkout for {} at {}",
                            repository.full_name(),
                            repo_path.display()
                        );
                        view.refresh_owned_file_filters(cx);
                    }
                    Err(error) => {
                        view.repository_error = Some(error.clone());
                        view.status = format!("Failed to save local checkout: {error}");
                    }
                }

                view.local_task = None;
                cx.notify();
            }) {
                crate::workspace::log_entity_update_error(
                    "failed to update local checkout state",
                    error,
                );
            }
        }));
    }

    fn open_with_app(&mut self, app: ExternalApp, cx: &mut Context<Self>) {
        let Some(pr) = self.selected_pull_request().cloned() else {
            self.status = "Select a pull request before using Open With".to_string();
            cx.notify();
            return;
        };

        let Some(repo_path) = self.repository_local_paths.get(&pr.repo).cloned() else {
            self.status = format!(
                "Choose a local checkout for {} before opening with {}",
                pr.repo.full_name(),
                app.label()
            );
            cx.notify();
            return;
        };

        if !self.external_app_is_available(app) {
            self.status = if self.is_loading_external_app_availability() {
                "Detecting installed applications before opening".to_string()
            } else {
                format!("{} is not installed", app.label())
            };
            cx.notify();
            return;
        }

        let active_file = self.active_file().cloned();
        let app_label = app.label();
        self.status = format!("Preparing PR #{} worktree for {app_label}", pr.number);
        cx.notify();

        let task = cx.background_spawn(async move {
            let worktree_path = harbor_git::create_or_update_pr_worktree(
                &repo_path,
                &pr.repo.owner,
                &pr.repo.name,
                pr.number,
            )
            .map_err(|error| error.to_string())?;
            let (target, target_status) =
                open_target_for_app(app, &worktree_path, active_file.as_ref());

            harbor_git::open_external_app(app, target).map_err(|error| error.to_string())?;

            Ok::<String, String>(open_with_status(
                app,
                &pr,
                active_file.as_ref(),
                target_status,
            ))
        });

        self.local_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;

            if let Err(error) = this.update(cx, move |view, cx| {
                view.status = match result {
                    Ok(status) => status,
                    Err(error) => format!("Failed to open with {app_label}: {error}"),
                };
                view.local_task = None;
                cx.notify();
            }) {
                crate::workspace::log_entity_update_error(
                    "failed to update open-with state",
                    error,
                );
            }
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

        let Some(repo_path) = self.repository_local_paths.get(&pr.repo).cloned() else {
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

        self.local_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;

            if let Err(error) = this.update(cx, move |view, cx| {
                view.status = match result {
                    Ok(status) => status,
                    Err(error) => format!("Failed to prepare PR worktree: {error}"),
                };
                view.local_task = None;
                cx.notify();
            }) {
                crate::workspace::log_entity_update_error("failed to update checkout state", error);
            }
        }));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OpenTargetStatus {
    Root,
    ActiveFile,
    RemovedFile,
    MissingFile,
}

pub(crate) fn open_target_for_app(
    app: ExternalApp,
    worktree_path: &Path,
    active_file: Option<&DiffFile>,
) -> (OpenTarget, OpenTargetStatus) {
    if app.kind() == ExternalAppKind::Terminal {
        return (
            OpenTarget::Directory(worktree_path.to_path_buf()),
            OpenTargetStatus::Root,
        );
    }

    let Some(file) = active_file else {
        return (
            OpenTarget::Directory(worktree_path.to_path_buf()),
            OpenTargetStatus::Root,
        );
    };

    if file.status == FileStatus::Removed {
        return (
            OpenTarget::Directory(worktree_path.to_path_buf()),
            OpenTargetStatus::RemovedFile,
        );
    }

    let file_path = worktree_path.join(&file.path);
    if !file_path.exists() {
        return (
            OpenTarget::Directory(worktree_path.to_path_buf()),
            OpenTargetStatus::MissingFile,
        );
    }

    if app.kind() == ExternalAppKind::Finder {
        (OpenTarget::Reveal(file_path), OpenTargetStatus::ActiveFile)
    } else {
        (OpenTarget::File(file_path), OpenTargetStatus::ActiveFile)
    }
}

fn open_with_status(
    app: ExternalApp,
    pr: &PullRequest,
    active_file: Option<&DiffFile>,
    target_status: OpenTargetStatus,
) -> String {
    match target_status {
        OpenTargetStatus::ActiveFile => {
            let path = active_file
                .map(|file| file.path.as_str())
                .unwrap_or("active file");
            format!("Opened {path} from PR #{} in {}", pr.number, app.label())
        }
        OpenTargetStatus::Root => {
            format!("Opened PR #{} worktree in {}", pr.number, app.label())
        }
        OpenTargetStatus::RemovedFile => {
            format!(
                "Opened PR #{} worktree in {}; selected file was removed",
                pr.number,
                app.label()
            )
        }
        OpenTargetStatus::MissingFile => {
            format!(
                "Opened PR #{} worktree in {}; active file was unavailable",
                pr.number,
                app.label()
            )
        }
    }
}
