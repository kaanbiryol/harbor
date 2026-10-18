use crate::gpui_component::{
    ActiveTheme, Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants, DropdownButton},
    clipboard::Clipboard,
    popover::Popover,
};
use gpui::{Anchor, Context, IntoElement, div, prelude::*, px};
use harbor_domain::{
    MergeMethod, PullRequest,
    pull_request_signals::{
        MergeActionPlan, MergeBypassPlacement, MergeControl, merge_action_plan,
    },
};

use crate::{
    actions::{
        MergePullRequest, MergePullRequestWithMergeCommit, OpenApproveCommentDialog,
        OpenRequestChangesCommentDialog, PullRequestAction, PullRequestActionKind,
        RebasePullRequest, ToggleMergeRequirementsBypass,
    },
    dropdown::{dropdown_menu_item, dropdown_menu_surface},
    icons::Octicon,
    panels::{ImmediateTooltip, PullRequestMergeQueueStatus, review_action_blocker},
    visual::{color, layout},
    workspace::{AppView, log_entity_update_error},
};

impl AppView {
    pub(super) fn render_pull_request_details_header(
        &self,
        pr: &PullRequest,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pull_request_action_running = self.action_runtime.pull_request_action_running();
        let pull_request_action_kind = self.action_runtime.pull_request_action_kind();
        let review_action_running = pull_request_action_kind == Some(PullRequestActionKind::Review);
        let merge_action_running = pull_request_action_kind == Some(PullRequestActionKind::Merge);
        let review_action_blocker = review_action_blocker(pr);
        let review_action_disabled = pull_request_action_running || review_action_blocker.is_some();
        let approve_tooltip = review_action_blocker
            .clone()
            .unwrap_or_else(|| "Approve pull request".to_string());
        let MergeActionPlan {
            bypass_enabled,
            control: merge_control,
            disabled: merge_action_disabled,
            tooltip: merge_tooltip,
            bypass_placement,
        } = merge_action_plan(pr, self.merge_bypass_enabled, pull_request_action_running);
        let pull_request_url = pr.url.clone();
        let pull_request_link = pr.url.clone();
        let pull_request_number = pr.number;
        let branch_name = pr.head_ref.clone();
        let head_sha = pr.head_sha.clone();
        let short_head_sha = short_commit_sha(&head_sha);
        let view = cx.entity().clone();
        let bypass_in_merge_menu = bypass_placement == Some(MergeBypassPlacement::MergeMenu);
        let show_merge_bypass_options = bypass_placement == Some(MergeBypassPlacement::Separate);
        let header_actions = div()
            .flex_none()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_end()
            .gap_2()
            .child({
                let dropdown = DropdownButton::new("review-pr")
                    .button(
                        Button::new("approve-pr-primary")
                            .label("approve")
                            .small()
                            .tooltip(approve_tooltip)
                            .loading(review_action_running)
                            .disabled(review_action_disabled)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.run_pull_request_action(
                                    PullRequestAction::Approve { body: None },
                                    window,
                                    cx,
                                );
                            })),
                    )
                    .small()
                    .disabled(review_action_disabled)
                    .dropdown_menu_with_anchor(Anchor::TopLeft, move |menu, _, _| {
                        menu.menu_with_disabled(
                            "Approve with comment...",
                            Box::new(OpenApproveCommentDialog),
                            review_action_disabled,
                        )
                        .menu_with_disabled(
                            "Request changes...",
                            Box::new(OpenRequestChangesCommentDialog),
                            review_action_disabled,
                        )
                    });

                if review_action_disabled {
                    dropdown.outline()
                } else {
                    dropdown.success().outline()
                }
            })
            .child({
                let merge_control = match merge_control {
                    MergeControl::QueueStatus(queue_status) => {
                        render_merge_queue_status(queue_status).into_any_element()
                    }
                    MergeControl::MergeWhenReady => {
                        let button = Button::new("merge-pr-when-ready")
                            .label("merge when ready")
                            .small()
                            .tooltip(merge_tooltip.clone())
                            .loading(merge_action_running)
                            .disabled(merge_action_disabled)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.run_pull_request_action(
                                    PullRequestAction::MergeWhenReady,
                                    window,
                                    cx,
                                );
                            }));
                        if bypass_in_merge_menu {
                            DropdownButton::new("merge-pr-when-ready")
                                .button(button)
                                .small()
                                .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                                    menu.menu_with_check_and_disabled(
                                        "Bypass merge requirements",
                                        bypass_enabled,
                                        Box::new(ToggleMergeRequirementsBypass),
                                        pull_request_action_running,
                                    )
                                })
                                .success()
                                .into_any_element()
                        } else if merge_action_disabled {
                            button.outline().into_any_element()
                        } else {
                            button.success().into_any_element()
                        }
                    }
                    MergeControl::Merge => {
                        let button = Button::new("merge-pr-primary")
                            .label(merge_method_button_label(MergeMethod::Squash))
                            .small()
                            .tooltip(merge_tooltip)
                            .loading(merge_action_running)
                            .disabled(merge_action_disabled)
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.run_pull_request_action(
                                    PullRequestAction::Merge {
                                        method: MergeMethod::Squash,
                                        bypass_requirements: bypass_enabled,
                                    },
                                    window,
                                    cx,
                                );
                            }));
                        let dropdown = DropdownButton::new("merge-pr")
                            .button(button)
                            .small()
                            .disabled(merge_action_disabled)
                            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                                let menu = menu
                                    .menu_with_check_and_disabled(
                                        MergeMethod::Merge.label(),
                                        false,
                                        Box::new(MergePullRequestWithMergeCommit),
                                        merge_action_disabled,
                                    )
                                    .menu_with_check_and_disabled(
                                        MergeMethod::Squash.label(),
                                        true,
                                        Box::new(MergePullRequest),
                                        merge_action_disabled,
                                    )
                                    .menu_with_check_and_disabled(
                                        MergeMethod::Rebase.label(),
                                        false,
                                        Box::new(RebasePullRequest),
                                        merge_action_disabled,
                                    );

                                if bypass_in_merge_menu {
                                    menu.separator().menu_with_check_and_disabled(
                                        "Bypass merge requirements",
                                        bypass_enabled,
                                        Box::new(ToggleMergeRequirementsBypass),
                                        pull_request_action_running,
                                    )
                                } else {
                                    menu
                                }
                            });

                        if merge_action_disabled {
                            dropdown.outline().into_any_element()
                        } else {
                            dropdown.success().into_any_element()
                        }
                    }
                };

                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(merge_control)
                    .when(show_merge_bypass_options, |controls| {
                        controls.child(render_merge_bypass_options(
                            bypass_enabled,
                            pull_request_action_running,
                            view,
                        ))
                    })
            });

        let header_content = div()
            .px_3()
            .pt_2()
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .items_start()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .id(("pull-request-title-link", pr.number))
                                            .min_w_0()
                                            .flex_initial()
                                            .truncate()
                                            .text_size(px(layout::PULL_REQUEST_TITLE_FONT_SIZE))
                                            .font_medium()
                                            .text_color(color::accent())
                                            .cursor_pointer()
                                            .hover(|element| {
                                                element.text_color(color::accent_hover())
                                            })
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                cx.open_url(&pull_request_url);
                                                view.status = format!(
                                                    "Opened PR #{pull_request_number} in browser"
                                                );
                                                cx.notify();
                                            }))
                                            .child(format!("#{} {}", pr.number, pr.title)),
                                    )
                                    .child(div().flex_none().child(render_copy_button(
                                        format!("copy-pr-link-{}", pr.number),
                                        "Copy pull request link",
                                        pull_request_link,
                                        "Copied PR link".to_string(),
                                        cx,
                                    ))),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "pull-request-header-metadata".to_string())
                                    .pt_1()
                                    .flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap_2()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(color::text_muted())
                                    .child(
                                        div()
                                            .flex_none()
                                            .font_medium()
                                            .text_color(color::text_secondary())
                                            .child(pr.author.clone()),
                                    )
                                    .child(div().flex_none().child("·"))
                                    .child(
                                        div()
                                            .min_w_0()
                                            .max_w(px(260.))
                                            .truncate()
                                            .text_color(color::accent())
                                            .child(branch_name.clone()),
                                    )
                                    .child(div().flex_none().child("→"))
                                    .child(
                                        div()
                                            .flex_none()
                                            .text_color(color::accent())
                                            .child(pr.base_ref.clone()),
                                    )
                                    .child(render_copy_button(
                                        format!("copy-pr-base-branch-{}", pr.number),
                                        "Copy base branch name",
                                        pr.base_ref.clone(),
                                        format!("Copied branch {}", pr.base_ref),
                                        cx,
                                    ))
                                    .child(div().flex_none().child("·"))
                                    .child(
                                        div()
                                            .flex_none()
                                            .font_family(cx.theme().mono_font_family.clone())
                                            .child(short_head_sha.clone()),
                                    )
                                    .child(render_copy_button(
                                        format!("copy-pr-sha-{}", pr.number),
                                        "Copy commit SHA",
                                        head_sha,
                                        format!("Copied commit {short_head_sha}"),
                                        cx,
                                    )),
                            )
                            .when_some(
                                self.detail_state.details_error().map(str::to_string),
                                |element, error| {
                                    element.child(
                                        div()
                                            .pt_2()
                                            .text_xs()
                                            .text_color(color::danger())
                                            .child(error),
                                    )
                                },
                            ),
                    )
                    .child(header_actions),
            )
            .when_some(
                self.review_state.pending_review_cloned(),
                |element, pending_review| {
                    element.child(self.render_pending_review_bar(pending_review, cx))
                },
            )
            .when_some(
                self.action_runtime
                    .pull_request_action_error()
                    .map(str::to_string),
                |element, error| {
                    element.child(
                        div()
                            .pt_2()
                            .text_xs()
                            .text_color(color::danger())
                            .child(error),
                    )
                },
            );

        div()
            .debug_selector(|| "pull-request-workspace-header".to_string())
            .flex_none()
            .border_1()
            .border_color(color::border())
            .bg(color::panel_background())
            .child(header_content)
            .child(self.render_panel_tabs(cx))
    }
}

fn render_merge_queue_status(status: PullRequestMergeQueueStatus) -> impl IntoElement {
    let icon = match status {
        PullRequestMergeQueueStatus::Waiting => Octicon::Clock,
        PullRequestMergeQueueStatus::Queued => Octicon::GitPullRequest,
    };
    Button::new("pull-request-merge-queue-status")
        .label("Queued")
        .icon(icon)
        .small()
        .warning()
        .outline()
        .tooltip(status.description())
}

fn render_merge_bypass_options(
    bypass_enabled: bool,
    action_running: bool,
    view: gpui::Entity<AppView>,
) -> impl IntoElement {
    Popover::new("pull-request-merge-options")
        .appearance(false)
        .anchor(Anchor::TopRight)
        .trigger(ImmediateTooltip::new(
            "pull-request-merge-options-tooltip",
            "Merge options",
            Button::new("pull-request-merge-options-trigger")
                .icon(Octicon::KebabHorizontal)
                .xsmall()
                .compact()
                .ghost()
                .disabled(action_running),
        ))
        .content(move |_, _window, popover_cx| {
            let popover = popover_cx.entity().clone();
            let view = view.clone();

            dropdown_menu_surface(popover_cx, 232.0).p_0p5().child(
                dropdown_menu_item(
                    "toggle-merge-requirements-bypass",
                    "Bypass merge requirements",
                    None,
                    bypass_enabled,
                    bypass_enabled,
                    action_running,
                )
                .on_click(move |_, window, cx| {
                    view.update(cx, |view, cx| {
                        view.set_merge_bypass_enabled(!bypass_enabled, cx);
                    });
                    popover.update(cx, |popover, cx| {
                        popover.dismiss(window, cx);
                    });
                }),
            )
        })
}

fn merge_method_button_label(method: MergeMethod) -> &'static str {
    match method {
        MergeMethod::Merge => "merge",
        MergeMethod::Squash => "squash and merge",
        MergeMethod::Rebase => "rebase and merge",
    }
}

fn render_copy_button(
    id: String,
    tooltip: &'static str,
    clipboard_value: String,
    status: String,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let view = cx.weak_entity();

    Clipboard::new(id)
        .tooltip(tooltip)
        .value(clipboard_value)
        .on_copied(move |_, _, cx| {
            if let Err(error) = view.update(cx, |view, cx| {
                view.status = status.clone();
                cx.notify();
            }) {
                log_entity_update_error("failed to update clipboard copy status", error);
            }
        })
}

fn short_commit_sha(sha: &str) -> String {
    sha.chars().take(7).collect()
}

#[cfg(test)]
mod tests {
    use super::short_commit_sha;

    #[test]
    fn short_commit_sha_limits_full_hashes_to_seven_characters() {
        assert_eq!(
            short_commit_sha("ffe970011a044b2d6aa767d1608993b9c94d690e"),
            "ffe9700"
        );
    }

    #[test]
    fn short_commit_sha_preserves_short_hashes() {
        assert_eq!(short_commit_sha("abc123"), "abc123");
    }
}
