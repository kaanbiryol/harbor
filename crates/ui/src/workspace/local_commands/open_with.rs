use std::path::Path;

use gpui::{AppContext, Context, Window};
use harbor_domain::{DiffFile, FileStatus, PullRequest};
use harbor_git::{ExternalApp, ExternalAppKind, OpenTarget};

use crate::{
    actions::{
        OpenWithCursor, OpenWithFinder, OpenWithGhostty, OpenWithTerminal, OpenWithVsCode,
        OpenWithWarp, OpenWithXcode, OpenWithZed,
    },
    workspace::{AppView, async_updates::AppViewAsyncUpdateExt},
};

impl AppView {
    pub(in crate::workspace) fn open_with_vs_code(
        &mut self,
        _: &OpenWithVsCode,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::VsCode, cx);
    }

    pub(in crate::workspace) fn open_with_cursor(
        &mut self,
        _: &OpenWithCursor,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Cursor, cx);
    }

    pub(in crate::workspace) fn open_with_zed(
        &mut self,
        _: &OpenWithZed,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Zed, cx);
    }

    pub(in crate::workspace) fn open_with_finder(
        &mut self,
        _: &OpenWithFinder,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Finder, cx);
    }

    pub(in crate::workspace) fn open_with_terminal(
        &mut self,
        _: &OpenWithTerminal,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Terminal, cx);
    }

    pub(in crate::workspace) fn open_with_ghostty(
        &mut self,
        _: &OpenWithGhostty,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Ghostty, cx);
    }

    pub(in crate::workspace) fn open_with_warp(
        &mut self,
        _: &OpenWithWarp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Warp, cx);
    }

    pub(in crate::workspace) fn open_with_xcode(
        &mut self,
        _: &OpenWithXcode,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_with_app(ExternalApp::Xcode, cx);
    }

    fn open_with_app(&mut self, app: ExternalApp, cx: &mut Context<Self>) {
        let Some(pr) = self.selected_pull_request().cloned() else {
            self.status = "Select a pull request before using Open With".to_string();
            cx.notify();
            return;
        };

        let Some(repo_path) = self.repository_state.local_path(&pr.repo).cloned() else {
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

        self.tasks.set_local_task(cx.spawn(async move |this, cx| {
            let result = task.await;

            this.update_or_log(cx, "failed to update open-with state", move |view, cx| {
                view.status = match result {
                    Ok(status) => status,
                    Err(error) => format!("Failed to open with {app_label}: {error}"),
                };
                view.tasks.clear_local_task();
                cx.notify();
            });
        }));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OpenTargetStatus {
    Root,
    ActiveFile,
    RemovedFile,
    MissingFile,
}

fn open_target_for_app(
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

#[cfg(test)]
mod tests {
    use harbor_domain::FileStatus;
    use harbor_git::{ExternalApp, OpenTarget};

    use super::*;
    use crate::test_fixtures::diff_file;

    #[test]
    fn opens_worktree_root_for_removed_local_files() {
        let root = std::path::Path::new("/tmp/harbor-worktree");
        let file = diff_file("src/deleted.rs", FileStatus::Removed);

        let (target, status) = open_target_for_app(ExternalApp::Zed, root, Some(&file));

        assert_eq!(target, OpenTarget::Directory(root.to_path_buf()));
        assert_eq!(status, OpenTargetStatus::RemovedFile);
    }
}
