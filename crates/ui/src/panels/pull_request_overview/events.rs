use crate::gpui_component::StyledExt;
use gpui::{AnyElement, Context, div, prelude::*, px};
use harbor_domain::{PullRequestComment, PullRequestCommit, PullRequestReview};

use crate::{
    date_time::{
        full_time_label, full_time_label_with_edit, natural_time_label,
        natural_time_label_with_edit,
    },
    icons::Octicon,
    panels::{
        people::{render_avatar, render_time_metadata},
        render_status_pill, review_time_label, review_time_tooltip,
    },
    visual::{color, tone_colors},
    workspace::AppView,
};

pub(super) fn render_overview_commit_event(
    commit: &PullRequestCommit,
    index: usize,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let subject = commit
        .message
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    let short_sha: String = commit.sha.chars().take(7).collect();
    let sha = commit.sha.clone();

    render_timeline_row(
        render_avatar(&commit.author, commit.author_avatar_url.as_deref(), 24.0),
        div()
            .debug_selector({
                let sha = commit.sha.clone();
                move || format!("overview-commit-{sha}")
            })
            .id(("overview-commit", index))
            .w_full()
            .min_w_0()
            .flex()
            .items_center()
            .gap_1()
            .text_xs()
            .cursor_pointer()
            .on_click(cx.listener(move |view, _, _, cx| {
                view.select_commit(sha.clone(), cx);
            }))
            .child(
                div()
                    .font_semibold()
                    .text_color(color::text_primary())
                    .child(commit.author.clone()),
            )
            .child(div().text_color(color::text_secondary()).child("committed"))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .truncate()
                    .text_color(color::text_primary())
                    .child(subject),
            )
            .child(
                div()
                    .font_family("monospace")
                    .text_color(color::text_muted())
                    .child(short_sha),
            )
            .when_some(commit.authored_at, |element, authored_at| {
                element.child(render_time_metadata(
                    format!("overview-commit-time-{}", commit.sha),
                    natural_time_label(authored_at),
                    Some(full_time_label(authored_at)),
                    color::text_muted(),
                ))
            })
            .into_any_element(),
        true,
    )
}

use super::timeline::{overview_review_state, render_timeline_icon, render_timeline_row};

pub(super) fn render_overview_comment_event(
    comment: &PullRequestComment,
    index: usize,
    markdown: AnyElement,
) -> AnyElement {
    let time_label = natural_time_label_with_edit(comment.created_at, comment.updated_at);
    let time_tooltip = full_time_label_with_edit(comment.created_at, comment.updated_at);

    render_timeline_row(
        render_avatar(&comment.author, comment.author_avatar_url.as_deref(), 24.0),
        div()
            .id(("overview-comment", index))
            .w_full()
            .min_w_0()
            .rounded_sm()
            .border_1()
            .border_color(color::border())
            .bg(color::content_background())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(color::border_subtle())
                    .text_xs()
                    .child(
                        div()
                            .font_semibold()
                            .text_color(color::text_primary())
                            .child(comment.author.clone()),
                    )
                    .child(div().text_color(color::text_muted()).child("commented"))
                    .child(render_time_metadata(
                        format!("overview-comment-time-{}", comment.id),
                        time_label,
                        Some(time_tooltip),
                        color::text_muted(),
                    )),
            )
            .child(
                div()
                    .px_3()
                    .py_3()
                    .text_sm()
                    .text_color(color::text_secondary())
                    .child(markdown),
            )
            .into_any_element(),
        true,
    )
}

pub(super) fn render_overview_review_event(
    review: &PullRequestReview,
    index: usize,
    markdown: Option<AnyElement>,
) -> AnyElement {
    let selector = format!("overview-review-{}", review.id);
    let (action, status, tone) = overview_review_state(review.state);
    let colors = tone_colors(tone);

    render_timeline_row(
        render_timeline_icon(Octicon::Eye, tone),
        div()
            .debug_selector(move || selector.clone())
            .id(("overview-review", index))
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .min_h(px(24.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_xs()
                            .child(
                                div()
                                    .font_semibold()
                                    .text_color(color::text_primary())
                                    .child(review.author.clone()),
                            )
                            .child(div().text_color(color::text_secondary()).child(action))
                            .child(render_time_metadata(
                                format!("overview-review-time-{}", review.id),
                                review_time_label(review),
                                review_time_tooltip(review),
                                color::text_muted(),
                            )),
                    )
                    .child(render_status_pill(status, tone)),
            )
            .when_some(markdown, |element, markdown| {
                element.child(
                    div()
                        .rounded_sm()
                        .border_1()
                        .border_color(color::border())
                        .bg(colors.background)
                        .px_3()
                        .py_2()
                        .text_sm()
                        .text_color(color::text_secondary())
                        .child(markdown),
                )
            })
            .into_any_element(),
        true,
    )
}
