use gpui::{AnyElement, Entity, IntoElement, div, prelude::*, px, rgb};
use gpui_component::{StyledExt, input::InputState};
use harbor_domain::ReviewComment;

use crate::workspace::{
    AppView, ReviewCommentUiError, ReviewReactionAction, review_comment_pending_sync,
};

use super::{
    avatars::render_review_comment_avatar,
    comment_actions::{
        render_review_comment_actions_menu, render_review_comment_edit_composer,
        review_comment_action_visibility,
    },
    reactions::render_review_reactions,
};

pub(super) fn render_review_comment_inline(
    comment: &ReviewComment,
    separated: bool,
    active_review_comment_edit: Option<&str>,
    review_comment_edit_input: Entity<InputState>,
    edit_body_empty: bool,
    is_submitting_edit: bool,
    edit_error: Option<&ReviewCommentUiError>,
    action_comment_id: Option<&str>,
    comment_action_error: Option<&ReviewCommentUiError>,
    reaction_action: Option<&ReviewReactionAction>,
    reaction_error: Option<&ReviewCommentUiError>,
    thread_resolved: bool,
    view_entity: Entity<AppView>,
) -> AnyElement {
    let comment_id = comment.id.clone();
    let comment_body = comment.body.clone();
    let active_edit = active_review_comment_edit == Some(comment.id.as_str());
    let edit_submitting = active_edit && is_submitting_edit;
    let action_running = action_comment_id == Some(comment.id.as_str());
    let edit_error = edit_error
        .filter(|error| error.comment_id == comment.id)
        .map(|error| error.message.clone());
    let action_error = comment_action_error
        .filter(|error| error.comment_id == comment.id)
        .map(|error| error.message.clone());
    let reaction_error = reaction_error
        .filter(|error| error.comment_id == comment.id)
        .map(|error| error.message.clone());
    let (can_update, can_delete) = review_comment_action_visibility(comment);
    let author_color = if thread_resolved {
        rgb(0xb7c0cd)
    } else {
        rgb(0xe5edf7)
    };
    let metadata_color = if thread_resolved {
        rgb(0x526176)
    } else {
        rgb(0x64748b)
    };
    let body_color = if thread_resolved {
        rgb(0x8996a8)
    } else {
        rgb(0xcbd5e1)
    };
    let separator_color = if thread_resolved {
        rgb(0x213040)
    } else {
        rgb(0x263241)
    };

    div()
        .pt_2()
        .when(separated, |element| {
            element.mt_2().border_t_1().border_color(separator_color)
        })
        .flex()
        .items_start()
        .gap_2()
        .child(render_review_comment_avatar(comment))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_xs()
                                .child(
                                    div()
                                        .font_medium()
                                        .text_color(author_color)
                                        .child(comment.author.clone()),
                                )
                                .child(
                                    div()
                                        .text_color(metadata_color)
                                        .child(review_comment_time_label(comment)),
                                )
                                .when(review_comment_pending_sync(comment), |element| {
                                    element.child(
                                        div()
                                            .rounded_xs()
                                            .border_1()
                                            .border_color(rgb(0x355071))
                                            .bg(rgb(0x101b2a))
                                            .px_1()
                                            .text_color(rgb(0x93c5fd))
                                            .child("syncing"),
                                    )
                                }),
                        )
                        .when(can_update || can_delete, {
                            let view_entity = view_entity.clone();
                            let comment_id = comment_id.clone();
                            let comment_body = comment_body.clone();
                            move |element| {
                                element.child(render_review_comment_actions_menu(
                                    comment_id.clone(),
                                    comment_body.clone(),
                                    can_update,
                                    can_delete,
                                    active_edit,
                                    edit_submitting,
                                    action_running,
                                    view_entity.clone(),
                                ))
                            }
                        }),
                )
                .when(!active_edit, |element| {
                    element.child(render_review_comment_body(&comment.body, body_color))
                })
                .when(active_edit, {
                    let view_entity = view_entity.clone();
                    let comment_id = comment_id.clone();
                    move |element| {
                        element.child(render_review_comment_edit_composer(
                            comment_id.clone(),
                            review_comment_edit_input.clone(),
                            edit_body_empty,
                            edit_submitting,
                            edit_error.clone(),
                            view_entity.clone(),
                        ))
                    }
                })
                .child(render_review_reactions(
                    comment,
                    reaction_action,
                    view_entity.clone(),
                ))
                .when_some(action_error, |element, error| {
                    element.child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(rgb(0xf87171))
                            .child(error),
                    )
                })
                .when_some(reaction_error, |element, error| {
                    element.child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(rgb(0xf87171))
                            .child(error),
                    )
                }),
        )
        .into_any_element()
}

fn render_review_comment_body(body: &str, color: gpui::Rgba) -> impl IntoElement {
    let lines: Vec<String> = body.lines().map(str::to_string).collect::<Vec<_>>();
    let lines = if lines.is_empty() {
        vec!["empty comment".to_string()]
    } else {
        lines
    };

    div()
        .pt_2()
        .text_xs()
        .text_color(color)
        .children(lines.into_iter().map(|line| {
            div().min_h(px(16.0)).child(if line.is_empty() {
                " ".to_string()
            } else {
                line
            })
        }))
}

fn review_comment_time_label(comment: &ReviewComment) -> String {
    let mut label = comment.created_at.format("%Y-%m-%d %H:%M").to_string();

    if comment
        .updated_at
        .is_some_and(|updated_at| updated_at != comment.created_at)
    {
        label.push_str(" edited");
    }

    label
}
