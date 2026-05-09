use gpui::{AnyElement, Entity, IntoElement, StyleRefinement, div, prelude::*, px, rems, rgb};
use gpui_component::{
    StyledExt,
    input::InputState,
    text::{TextView, TextViewStyle},
};
use harbor_domain::ReviewComment;

use crate::workspace::{
    AppView, ReviewCommentUiError, ReviewReactionAction, review_comment_pending_sync,
};

use super::{
    avatars::render_review_comment_avatar,
    comment_actions::{
        ReviewCommentActionsMenuState, render_review_comment_actions_menu,
        render_review_comment_edit_composer, review_comment_action_visibility,
    },
    reactions::render_review_reactions,
};

#[derive(Clone)]
pub(in crate::panels::diff_view) struct ReviewCommentListRenderState<'a> {
    pub(in crate::panels::diff_view) active_review_comment_edit: Option<&'a str>,
    pub(in crate::panels::diff_view) review_comment_edit_input: Entity<InputState>,
    pub(in crate::panels::diff_view) edit_body_empty: bool,
    pub(in crate::panels::diff_view) is_submitting_edit: bool,
    pub(in crate::panels::diff_view) edit_error: Option<&'a ReviewCommentUiError>,
    pub(in crate::panels::diff_view) action_comment_id: Option<&'a str>,
    pub(in crate::panels::diff_view) comment_action_error: Option<&'a ReviewCommentUiError>,
    pub(in crate::panels::diff_view) reaction_action: Option<&'a ReviewReactionAction>,
    pub(in crate::panels::diff_view) reaction_error: Option<&'a ReviewCommentUiError>,
    pub(in crate::panels::diff_view) view_entity: Entity<AppView>,
}

pub(super) struct ReviewCommentRenderState<'a> {
    comment: &'a ReviewComment,
    separated: bool,
    thread_resolved: bool,
    ui_state: ReviewCommentUiState,
    review_comment_edit_input: Entity<InputState>,
    edit_body_empty: bool,
    edit_error: Option<String>,
    action_error: Option<String>,
    reaction_action: Option<&'a ReviewReactionAction>,
    reaction_error: Option<String>,
    view_entity: Entity<AppView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReviewCommentUiState {
    pub(crate) can_update: bool,
    pub(crate) can_delete: bool,
    pub(crate) show_actions: bool,
    pub(crate) active_edit: bool,
    pub(crate) edit_submitting: bool,
    pub(crate) action_running: bool,
}

impl<'a> ReviewCommentRenderState<'a> {
    pub(super) fn new(
        comment: &'a ReviewComment,
        separated: bool,
        thread_resolved: bool,
        list_state: &ReviewCommentListRenderState<'a>,
    ) -> Self {
        let ui_state = review_comment_ui_state(
            comment,
            list_state.active_review_comment_edit,
            list_state.is_submitting_edit,
            list_state.action_comment_id,
        );
        let edit_error = list_state
            .edit_error
            .filter(|error| error.comment_id == comment.id)
            .map(|error| error.message.clone());
        let action_error = list_state
            .comment_action_error
            .filter(|error| error.comment_id == comment.id)
            .map(|error| error.message.clone());
        let reaction_error = list_state
            .reaction_error
            .filter(|error| error.comment_id == comment.id)
            .map(|error| error.message.clone());

        Self {
            comment,
            separated,
            thread_resolved,
            ui_state,
            review_comment_edit_input: list_state.review_comment_edit_input.clone(),
            edit_body_empty: list_state.edit_body_empty,
            edit_error,
            action_error,
            reaction_action: list_state.reaction_action,
            reaction_error,
            view_entity: list_state.view_entity.clone(),
        }
    }
}

pub(super) fn render_review_comment_inline(state: ReviewCommentRenderState<'_>) -> AnyElement {
    let ReviewCommentRenderState {
        comment,
        separated,
        thread_resolved,
        ui_state,
        review_comment_edit_input,
        edit_body_empty,
        edit_error,
        action_error,
        reaction_action,
        reaction_error,
        view_entity,
    } = state;
    let comment_id = comment.id.clone();
    let comment_body = comment.body.clone();
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
                        .when(ui_state.show_actions, {
                            let view_entity = view_entity.clone();
                            let comment_id = comment_id.clone();
                            let comment_body = comment_body.clone();
                            move |element| {
                                element.child(render_review_comment_actions_menu(
                                    ReviewCommentActionsMenuState {
                                        comment_id: comment_id.clone(),
                                        comment_body: comment_body.clone(),
                                        can_update: ui_state.can_update,
                                        can_delete: ui_state.can_delete,
                                        active_edit: ui_state.active_edit,
                                        edit_submitting: ui_state.edit_submitting,
                                        action_running: ui_state.action_running,
                                        view_entity: view_entity.clone(),
                                    },
                                ))
                            }
                        }),
                )
                .when(!ui_state.active_edit, |element| {
                    element.child(render_review_comment_body(
                        &comment.id,
                        &comment.body,
                        body_color,
                    ))
                })
                .when(ui_state.active_edit, {
                    let view_entity = view_entity.clone();
                    let comment_id = comment_id.clone();
                    move |element| {
                        element.child(render_review_comment_edit_composer(
                            comment_id.clone(),
                            review_comment_edit_input.clone(),
                            edit_body_empty,
                            ui_state.edit_submitting,
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

fn render_review_comment_body(comment_id: &str, body: &str, color: gpui::Rgba) -> impl IntoElement {
    div().pt_2().text_xs().text_color(color).child(
        TextView::markdown(
            format!("review-comment-body-{comment_id}"),
            review_comment_body_markdown(body),
        )
        .style(review_comment_markdown_style())
        .selectable(true),
    )
}

fn review_comment_markdown_style() -> TextViewStyle {
    TextViewStyle::default()
        .paragraph_gap(rems(0.25))
        .heading_font_size(|level, size| match level {
            1..=2 => size,
            _ => size * 0.9,
        })
        .code_block(
            StyleRefinement::default()
                .bg(rgb(0x0b1118))
                .p_1()
                .text_size(px(11.0)),
        )
}

pub(crate) fn review_comment_body_markdown(body: &str) -> String {
    if body.trim().is_empty() {
        "empty comment".to_string()
    } else {
        body.to_string()
    }
}

pub(crate) fn review_comment_ui_state(
    comment: &ReviewComment,
    active_review_comment_edit: Option<&str>,
    is_submitting_edit: bool,
    action_comment_id: Option<&str>,
) -> ReviewCommentUiState {
    let (can_update, can_delete) = review_comment_action_visibility(comment);
    let active_edit = active_review_comment_edit == Some(comment.id.as_str());

    ReviewCommentUiState {
        can_update,
        can_delete,
        show_actions: can_update || can_delete,
        active_edit,
        edit_submitting: active_edit && is_submitting_edit,
        action_running: action_comment_id == Some(comment.id.as_str()),
    }
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
