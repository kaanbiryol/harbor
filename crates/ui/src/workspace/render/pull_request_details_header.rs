use gpui::{Anchor, Context, IntoElement, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants, DropdownButton},
    clipboard::Clipboard,
};
use harbor_domain::{MergeMethod, PullRequest};

use crate::{
    actions::{
        MergePullRequest, MergePullRequestWithMergeCommit, OpenApproveCommentDialog,
        OpenRequestChangesCommentDialog, PullRequestAction, RebasePullRequest,
    },
    panels::{merge_blocker, render_merge_state, render_review_decision, review_action_blocker},
    visual::{Tone, color},
    workspace::{AppView, log_entity_update_error},
};

impl AppView {
    pub(super) fn render_pull_request_details_header(
        &self,
        pr: &PullRequest,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pull_request_action_running = self.action_runtime.pull_request_action_running();
        let review_action_blocker = review_action_blocker(pr);
        let merge_blocker = merge_blocker(pr);
        let review_action_disabled = pull_request_action_running || review_action_blocker.is_some();
        let merge_action_disabled = pull_request_action_running || merge_blocker.is_some();
        let approve_tooltip = review_action_blocker
            .clone()
            .unwrap_or_else(|| "Approve pull request".to_string());
        let merge_tooltip = merge_blocker
            .clone()
            .unwrap_or_else(|| "Merge pull request".to_string());
        let pull_request_url = pr.url.clone();
        let pull_request_link = pr.url.clone();
        let pull_request_number = pr.number;
        let repository_name = pr.repo.full_name();
        let branch_name = pr.head_ref.clone();
        let head_sha = pr.head_sha.clone();
        let short_head_sha = short_commit_sha(&head_sha);

        div()
            .px_3()
            .py_4()
            .border_1()
            .border_color(color::border())
            .bg(color::panel_background())
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_1()
                    .min_w_0()
                    .child(
                        div()
                            .id(("pull-request-title-link", pr.number))
                            .min_w_0()
                            .flex_1()
                            .text_size(px(15.0))
                            .font_medium()
                            .text_color(color::accent())
                            .cursor_pointer()
                            .hover(|element| element.text_color(color::accent_hover()))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                cx.open_url(&pull_request_url);
                                view.status =
                                    format!("Opened PR #{pull_request_number} in browser");
                                cx.notify();
                            }))
                            .child(format!("#{} {}", pr.number, pr.title)),
                    )
                    .child(render_copy_button(
                        format!("copy-pr-link-{}", pr.number),
                        "Copy pull request link",
                        pull_request_link,
                        "Copied PR link".to_string(),
                        cx,
                    )),
            )
            .child(
                div()
                    .pt_2()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .min_w_0()
                    .text_xs()
                    .text_color(color::text_muted())
                    .child(div().flex_none().child(repository_name))
                    .child(div().flex_none().child("/"))
                    .child(
                        div()
                            .min_w_0()
                            .max_w(px(220.))
                            .truncate()
                            .child(branch_name.clone()),
                    )
                    .child(render_copy_button(
                        format!("copy-pr-branch-{}", pr.number),
                        "Copy branch name",
                        branch_name.clone(),
                        format!("Copied branch {branch_name}"),
                        cx,
                    ))
                    .child(div().flex_none().child("/"))
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
            .when(self.detail_state.details_loading(), |element| {
                element.child(
                    div()
                        .pt_2()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child("Loading latest PR details..."),
                )
            })
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
            )
            .child(
                div()
                    .pt_3()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(render_review_decision(pr.review_decision))
                    .child(render_merge_state(pr.merge_state))
                    .child(crate::panels::render_status_pill(
                        format!("{} unresolved", pr.unresolved_threads),
                        if pr.unresolved_threads == 0 {
                            Tone::Neutral
                        } else {
                            Tone::Warning
                        },
                    )),
            )
            .child(
                div()
                    .pt_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(div().flex().items_center().gap_2().child({
                        DropdownButton::new("review-pr")
                            .button(
                                Button::new("approve-pr-primary")
                                    .label("approve")
                                    .small()
                                    .loading(pull_request_action_running)
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
                            .primary()
                            .tooltip(approve_tooltip.clone())
                            .loading(pull_request_action_running)
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
                            })
                    }))
                    .child({
                        let button = Button::new("merge-pr-primary")
                            .label(merge_method_button_label(MergeMethod::Squash))
                            .small()
                            .loading(pull_request_action_running)
                            .disabled(merge_action_disabled)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.run_pull_request_action(
                                    PullRequestAction::Merge(MergeMethod::Squash),
                                    window,
                                    cx,
                                );
                            }));
                        let dropdown = DropdownButton::new("merge-pr")
                            .button(button)
                            .small()
                            .compact()
                            .tooltip(merge_tooltip.clone())
                            .loading(pull_request_action_running)
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
                            dropdown.outline().opacity(0.58)
                        } else {
                            dropdown.primary()
                        }
                    }),
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
            )
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
