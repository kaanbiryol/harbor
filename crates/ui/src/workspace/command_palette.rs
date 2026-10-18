use std::{cell::Cell, rc::Rc};

use crate::gpui_component::{
    Root, WindowExt,
    command::{Command, CommandGroup, CommandItem},
};
use gpui::{Action, Context, Focusable, ParentElement, SharedString, Styled, Window, px};
use harbor_git::ExternalApp;

use crate::actions::*;
use crate::workspace::AppView;
use crate::workspace::render::header::open_with_action;

struct PaletteGroup {
    label: &'static str,
    commands: Vec<(SharedString, Box<dyn Action>)>,
}

fn palette_groups() -> Vec<PaletteGroup> {
    let navigation = PanelTab::ALL
        .into_iter()
        .map(|tab| (format!("Go to {}", tab.label()).into(), tab.select_action()))
        .chain(command_list([
            ("Search Pull Requests…", Box::new(OpenPullRequestSearch)),
            ("Switch Repository…", Box::new(ToggleRepositorySwitcher)),
            (
                "Toggle Pull Request Inbox",
                Box::new(TogglePullRequestInbox),
            ),
            ("Next File", Box::new(SelectNextFile)),
            ("Previous File", Box::new(SelectPreviousFile)),
            ("Next Hunk", Box::new(SelectNextHunk)),
            ("Previous Hunk", Box::new(SelectPreviousHunk)),
        ]))
        .collect();

    let pull_request = command_list([
        ("Refresh Pull Request", Box::new(RefreshSelectedPullRequest)),
        (
            "Open Pull Request in Browser",
            Box::new(OpenPullRequestInBrowser),
        ),
        ("Check Out Pull Request", Box::new(CheckoutPullRequest)),
        ("Approve", Box::new(ApprovePullRequest)),
        ("Approve with Comment…", Box::new(OpenApproveCommentDialog)),
        (
            "Request Changes…",
            Box::new(OpenRequestChangesCommentDialog),
        ),
        ("Squash and Merge", Box::new(MergePullRequest)),
        (
            "Merge with Merge Commit",
            Box::new(MergePullRequestWithMergeCommit),
        ),
        ("Rebase and Merge", Box::new(RebasePullRequest)),
        (
            "Toggle Merge Requirements Bypass",
            Box::new(ToggleMergeRequirementsBypass),
        ),
        ("Copy Active File Path", Box::new(CopyActiveFilePath)),
        (
            "Open Active File on GitHub",
            Box::new(OpenActiveFileOnGitHub),
        ),
    ])
    .collect();

    let open_with = command_list([("Choose Local Checkout…", Box::new(ChooseLocalCheckout))])
        .chain(ExternalApp::ALL.into_iter().map(|app| {
            (
                format!("Open in {}", app.label()).into(),
                open_with_action(app),
            )
        }))
        .collect();

    let application = command_list([
        ("Open Settings", Box::new(OpenSettings)),
        ("Sign In to GitHub", Box::new(SignInToGitHub)),
        ("Use GitHub CLI Session", Box::new(UseGitHubCli)),
        ("Sign Out of GitHub", Box::new(SignOutOfGitHub)),
    ])
    .collect();

    vec![
        PaletteGroup {
            label: "Navigation",
            commands: navigation,
        },
        PaletteGroup {
            label: "Pull Request",
            commands: pull_request,
        },
        PaletteGroup {
            label: "Open With",
            commands: open_with,
        },
        PaletteGroup {
            label: "Application",
            commands: application,
        },
    ]
}

fn command_list<const N: usize>(
    commands: [(&'static str, Box<dyn Action>); N],
) -> impl Iterator<Item = (SharedString, Box<dyn Action>)> {
    commands
        .into_iter()
        .map(|(label, action)| (SharedString::from(label), action))
}

impl AppView {
    pub(super) fn open_command_palette(
        &mut self,
        _: &OpenCommandPalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.root::<Root>().flatten().is_none() || window.has_active_dialog(cx) {
            return;
        }

        self.command_palette
            .update(cx, |palette, cx| palette.set_query("", window, cx));

        let palette = self.command_palette.clone();
        let workspace_focus = self.focus_handle.clone();
        let groups = Rc::new(palette_groups());
        let focus_on_mount = Rc::new(Cell::new(true));
        window.open_dialog(cx, move |dialog, _, _| {
            let palette = palette.clone();
            let workspace_focus = workspace_focus.clone();
            let groups = groups.clone();
            let focus_on_mount = focus_on_mount.clone();
            dialog
                .close_button(false)
                .w(px(560.))
                .p_0()
                .content(move |content, window, cx| {
                    if focus_on_mount.replace(false) {
                        let palette = palette.clone();
                        window.defer(cx, move |window, cx| {
                            palette.read(cx).focus_handle(cx).focus(window, cx);
                        });
                    }

                    let confirm_groups = groups.clone();
                    let workspace_focus = workspace_focus.clone();
                    let command = groups.iter().fold(
                        Command::new(&palette)
                            .bordered(false)
                            .placeholder("Type a command…")
                            .max_h(px(400.))
                            .on_confirm(move |index, window, cx| {
                                window.close_dialog(cx);
                                window.focus(&workspace_focus, cx);
                                // The palette's own dispatch runs from the dialog layer, which
                                // sits outside the workspace's action handlers, so re-dispatch
                                // from the workspace once it holds focus again.
                                if let Some((_, action)) = confirm_groups
                                    .get(index.section)
                                    .and_then(|group| group.commands.get(index.row))
                                {
                                    window.dispatch_action(action.boxed_clone(), cx);
                                }
                            }),
                        |command, group| {
                            command.group(CommandGroup::new().label(group.label).items(
                                group.commands.iter().map(|(label, action)| {
                                    CommandItem::new()
                                        .label(label.clone())
                                        .action(action.boxed_clone())
                                }),
                            ))
                        },
                    );
                    content.child(command)
                })
        });
    }
}
