use gpui::{AnyElement, Entity, IntoElement, div, prelude::*, px};
use gpui_component::{StyledExt, input::InputState, tooltip::Tooltip};
use harbor_domain::ReviewComment;

use crate::{
    date_time::{full_time_label_with_edit, natural_time_label_with_edit},
    panels::review_markdown::render_review_markdown_body,
    visual::color,
    workspace::{AppView, ReviewCommentUiError, ReviewReactionAction, review_comment_pending_sync},
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
    is_reply: bool,
    show_thread_rail: bool,
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
        is_reply: bool,
        show_thread_rail: bool,
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
            is_reply,
            show_thread_rail,
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
        is_reply,
        show_thread_rail,
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
    let author = comment.author.clone();
    let time_label = review_comment_time_label(comment);
    let time_tooltip = review_comment_time_tooltip(comment);
    let author_color = if thread_resolved {
        color::text_muted()
    } else {
        color::text_primary()
    };
    let metadata_color = if thread_resolved {
        color::text_disabled()
    } else {
        color::text_muted()
    };
    let body_color = if thread_resolved {
        color::text_disabled()
    } else {
        color::text_secondary()
    };
    let thread_rail_color = if thread_resolved {
        color::border_subtle()
    } else {
        color::border()
    };

    div()
        .pt_2()
        .when(is_reply, |element| element.mt_1().ml(px(28.0)))
        .flex()
        .gap_2()
        .child(render_review_comment_gutter(
            comment,
            show_thread_rail,
            thread_rail_color,
        ))
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
                                .child(render_review_comment_author_link(
                                    comment_id.clone(),
                                    author,
                                    author_color,
                                ))
                                .child(
                                    div()
                                        .id(format!("review-comment-time-{comment_id}"))
                                        .text_color(metadata_color)
                                        .tooltip(move |window, cx| {
                                            Tooltip::new(time_tooltip.clone()).build(window, cx)
                                        })
                                        .child(time_label),
                                )
                                .when(review_comment_pending_sync(comment), |element| {
                                    element.child(
                                        div()
                                            .rounded_xs()
                                            .border_1()
                                            .border_color(color::border_strong())
                                            .bg(color::row_selected_subtle())
                                            .px_1()
                                            .text_color(color::accent())
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
                            .text_color(color::danger())
                            .child(error),
                    )
                })
                .when_some(reaction_error, |element, error| {
                    element.child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(color::danger())
                            .child(error),
                    )
                }),
        )
        .into_any_element()
}

fn render_review_comment_gutter(
    comment: &ReviewComment,
    show_thread_rail: bool,
    rail_color: gpui::Rgba,
) -> impl IntoElement {
    div()
        .relative()
        .min_h(px(28.0))
        .w(px(20.0))
        .flex_none()
        .child(render_review_comment_avatar(comment))
        .when(show_thread_rail, |element| {
            element.child(
                div()
                    .absolute()
                    .top(px(24.0))
                    .bottom(px(-8.0))
                    .left(px(9.0))
                    .w(px(2.0))
                    .bg(rail_color),
            )
        })
}

fn render_review_comment_body(comment_id: &str, body: &str, color: gpui::Rgba) -> impl IntoElement {
    div()
        .pt_2()
        .text_xs()
        .text_color(color)
        .child(render_review_markdown_body(
            format!("review-comment-body-{comment_id}"),
            body,
        ))
}

fn render_review_comment_author_link(
    comment_id: String,
    author: String,
    color: gpui::Rgba,
) -> impl IntoElement {
    let profile_url = review_comment_author_profile_url(&author);

    div()
        .id(format!("review-comment-author-link-{comment_id}"))
        .font_medium()
        .text_color(color)
        .cursor_pointer()
        .hover(|element| element.text_color(color::accent_hover()))
        .on_click(move |_, _, cx| {
            cx.open_url(&profile_url);
        })
        .child(author)
}

fn review_comment_author_profile_url(author: &str) -> String {
    format!("https://github.com/{author}")
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
    natural_time_label_with_edit(comment.created_at, comment.updated_at)
}

fn review_comment_time_tooltip(comment: &ReviewComment) -> String {
    full_time_label_with_edit(comment.created_at, comment.updated_at)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use crate::date_time::natural_time_label_with_edit_at;
    use crate::test_fixtures::review_comment;

    use super::*;

    #[test]
    fn derives_inline_review_comment_ui_state() {
        let mut comment = review_comment();
        comment.viewer_can_update = true;

        let state = review_comment_ui_state(&comment, Some("comment"), true, Some("other"));

        assert!(state.can_update);
        assert!(!state.can_delete);
        assert!(state.show_actions);
        assert!(state.active_edit);
        assert!(state.edit_submitting);
        assert!(!state.action_running);

        let state = review_comment_ui_state(&comment, None, true, Some("comment"));

        assert!(!state.active_edit);
        assert!(!state.edit_submitting);
        assert!(state.action_running);
    }

    #[test]
    fn preserves_review_comment_markdown_body() {
        use crate::panels::review_markdown::review_markdown_body;

        assert_eq!(
            review_markdown_body("**bold**\n\n- list item"),
            "**bold**\n\n- list item"
        );
        assert_eq!(review_markdown_body(" \n\t "), "empty comment");
        assert_eq!(
            review_markdown_body("```suggestion\nlet value = 1;\n```"),
            "```text\nlet value = 1;\n```"
        );
    }

    #[test]
    fn builds_review_comment_author_profile_url() {
        assert_eq!(
            review_comment_author_profile_url("octocat"),
            "https://github.com/octocat"
        );
    }

    #[test]
    fn formats_review_comment_time_as_relative_label() {
        let mut comment = review_comment();
        comment.created_at = Utc
            .with_ymd_and_hms(2026, 6, 14, 13, 42, 0)
            .single()
            .expect("valid timestamp");
        let now = Utc
            .with_ymd_and_hms(2026, 7, 5, 13, 42, 0)
            .single()
            .expect("valid timestamp");

        assert_eq!(
            natural_time_label_with_edit_at(comment.created_at, comment.updated_at, now),
            "3 weeks ago"
        );
    }

    #[test]
    fn marks_edited_review_comment_time_label() {
        let mut comment = review_comment();
        comment.updated_at = Some(comment.created_at + Duration::minutes(5));
        let now = comment.created_at + Duration::minutes(30);

        assert_eq!(
            natural_time_label_with_edit_at(comment.created_at, comment.updated_at, now),
            "30 minutes ago (edited)"
        );
    }

    #[test]
    fn formats_review_comment_time_tooltip_as_absolute_label() {
        let mut comment = review_comment();
        comment.created_at = Utc
            .with_ymd_and_hms(2026, 6, 14, 13, 42, 0)
            .single()
            .expect("valid timestamp");

        assert_eq!(
            review_comment_time_tooltip(&comment),
            "2026-06-14 13:42 UTC"
        );
    }
}
