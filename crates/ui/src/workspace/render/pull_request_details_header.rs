use gpui::{Anchor, Context, IntoElement, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants, DropdownButton},
    checkbox::Checkbox,
    clipboard::Clipboard,
};
use harbor_domain::{MergeMethod, MergeQueueState, PullRequest};

use crate::{
    actions::{
        MergePullRequest, MergePullRequestWithMergeCommit, OpenApproveCommentDialog,
        OpenRequestChangesCommentDialog, PullRequestAction, PullRequestActionKind,
        RebasePullRequest,
    },
    panels::{
        merge_blocker, merge_when_ready_blocker, merge_without_requirements_blocker,
        review_action_blocker,
    },
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
        let bypass_available = pr.merge_capabilities.viewer_can_merge_as_admin;
        let bypass_enabled = bypass_available && self.merge_bypass_enabled;
        let show_bypass = bypass_available
            && (pr.merge_capabilities.queue_state != MergeQueueState::Disabled
                || merge_blocker(pr).is_some());
        let merge_action_blocker = if bypass_enabled {
            merge_without_requirements_blocker(pr)
        } else {
            match pr.merge_capabilities.queue_state {
                MergeQueueState::Unknown => {
                    Some("Loading merge queue settings from GitHub".to_string())
                }
                MergeQueueState::Enabled | MergeQueueState::Queued => merge_when_ready_blocker(pr),
                MergeQueueState::Disabled => merge_blocker(pr),
            }
        };
        let merge_action_disabled = pull_request_action_running || merge_action_blocker.is_some();
        let merge_tooltip = merge_action_blocker.unwrap_or_else(|| {
            if bypass_enabled {
                "Merge immediately without waiting for requirements".to_string()
            } else if pr.merge_capabilities.queue_state == MergeQueueState::Enabled {
                "Add pull request to the merge queue when requirements are met".to_string()
            } else {
                "Merge pull request".to_string()
            }
        });
        let pull_request_url = pr.url.clone();
        let pull_request_link = pr.url.clone();
        let pull_request_number = pr.number;
        let branch_name = pr.head_ref.clone();
        let head_sha = pr.head_sha.clone();
        let short_head_sha = short_commit_sha(&head_sha);
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
                    .compact()
                    .tooltip(approve_tooltip)
                    .loading(review_action_running)
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
                    dropdown.outline().opacity(0.58)
                } else {
                    dropdown.success().outline()
                }
            })
            .when(show_bypass, |actions| {
                actions.child(
                    Checkbox::new("bypass-merge-requirements")
                        .small()
                        .checked(bypass_enabled)
                        .disabled(pull_request_action_running)
                        .tooltip("Merge without waiting for requirements to be met (bypass rules)")
                        .child(
                            div()
                                .text_xs()
                                .text_color(color::danger())
                                .child("bypass requirements"),
                        )
                        .on_click(cx.listener(|view, enabled: &bool, _, cx| {
                            view.set_merge_bypass_enabled(*enabled, cx);
                        })),
                )
            })
            .child({
                if !bypass_enabled
                    && matches!(
                        pr.merge_capabilities.queue_state,
                        MergeQueueState::Enabled | MergeQueueState::Queued
                    )
                {
                    let label = if pr.merge_capabilities.queue_state == MergeQueueState::Queued {
                        "queued to merge"
                    } else if pr.merge_capabilities.auto_merge_enabled {
                        "merge when ready enabled"
                    } else {
                        "merge when ready"
                    };
                    let button = Button::new("merge-pr-when-ready")
                        .label(label)
                        .small()
                        .tooltip(merge_tooltip)
                        .loading(merge_action_running)
                        .disabled(merge_action_disabled)
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.run_pull_request_action(
                                PullRequestAction::MergeWhenReady,
                                window,
                                cx,
                            );
                        }));
                    if merge_action_disabled {
                        button.outline().opacity(0.58).into_any_element()
                    } else {
                        button.success().into_any_element()
                    }
                } else {
                    let button = Button::new("merge-pr-primary")
                        .label(merge_method_button_label(MergeMethod::Squash))
                        .small()
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
                        .compact()
                        .tooltip(merge_tooltip)
                        .loading(merge_action_running)
                        .disabled(merge_action_disabled)
                        .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                            menu.menu_with_check_and_disabled(
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
                            )
                        });

                    if merge_action_disabled {
                        dropdown.outline().opacity(0.58).into_any_element()
                    } else {
                        dropdown.success().into_any_element()
                    }
                }
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
