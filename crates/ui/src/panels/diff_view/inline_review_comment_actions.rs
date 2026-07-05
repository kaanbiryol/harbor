use gpui::{Anchor, Entity, IntoElement, div, prelude::*, px};
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    popover::Popover,
};
use harbor_domain::ReviewComment;

use crate::{icons::Octicon, visual::color, workspace::AppView};

pub(super) struct ReviewCommentActionsMenuState {
    pub(super) comment_id: String,
    pub(super) comment_body: String,
    pub(super) can_update: bool,
    pub(super) can_delete: bool,
    pub(super) active_edit: bool,
    pub(super) edit_submitting: bool,
    pub(super) action_running: bool,
    pub(super) view_entity: Entity<AppView>,
}

pub(super) fn render_review_comment_actions_menu(
    state: ReviewCommentActionsMenuState,
) -> impl IntoElement {
    let ReviewCommentActionsMenuState {
        comment_id,
        comment_body,
        can_update,
        can_delete,
        active_edit,
        edit_submitting,
        action_running,
        view_entity,
    } = state;

    Popover::new(format!("comment-actions-{comment_id}"))
        .appearance(false)
        .anchor(Anchor::TopRight)
        .trigger(
            Button::new(format!("comment-actions-trigger-{comment_id}"))
                .icon(Octicon::KebabHorizontal)
                .xsmall()
                .compact()
                .ghost()
                .debug_selector({
                    let selector = format!("inline-review-comment-actions-{comment_id}");
                    move || selector.clone()
                })
                .tooltip("Comment actions"),
        )
        .content(move |_, _window, _popover_cx| {
            div()
                .w(px(160.0))
                .border_1()
                .border_color(color::border_strong())
                .bg(color::elevated_background())
                .p_1()
                .shadow_lg()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .when(can_update, {
                            let view_entity = view_entity.clone();
                            let comment_id = comment_id.clone();
                            let comment_body = comment_body.clone();
                            move |element| {
                                element.child(
                                    Button::new(format!("edit-comment-{comment_id}"))
                                        .icon(Octicon::Pencil)
                                        .label(if active_edit { "Editing" } else { "Edit" })
                                        .small()
                                        .ghost()
                                        .disabled(edit_submitting || action_running)
                                        .debug_selector({
                                            let selector =
                                                format!("inline-review-comment-edit-{comment_id}");
                                            move || selector.clone()
                                        })
                                        .on_click({
                                            let view_entity = view_entity.clone();
                                            let comment_id = comment_id.clone();
                                            let comment_body = comment_body.clone();
                                            move |_, window, cx| {
                                                view_entity.update(cx, |view, cx| {
                                                    view.open_review_comment_edit(
                                                        comment_id.clone(),
                                                        comment_body.clone(),
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            }
                                        }),
                                )
                            }
                        })
                        .when(can_delete, {
                            let view_entity = view_entity.clone();
                            let comment_id = comment_id.clone();
                            move |element| {
                                element.child(
                                    Button::new(format!("delete-comment-{comment_id}"))
                                        .icon(Octicon::Trash)
                                        .label("Delete")
                                        .small()
                                        .ghost()
                                        .loading(action_running)
                                        .disabled(action_running || edit_submitting)
                                        .on_click({
                                            let view_entity = view_entity.clone();
                                            let comment_id = comment_id.clone();
                                            move |_, _, cx| {
                                                view_entity.update(cx, |view, cx| {
                                                    view.delete_review_comment(
                                                        comment_id.clone(),
                                                        cx,
                                                    );
                                                });
                                            }
                                        }),
                                )
                            }
                        }),
                )
        })
}

pub(super) fn render_review_comment_edit_composer(
    comment_id: String,
    review_comment_edit_input: Entity<InputState>,
    edit_body_empty: bool,
    edit_submitting: bool,
    edit_error: Option<String>,
    view_entity: Entity<AppView>,
) -> impl IntoElement {
    div()
        .child(
            div()
                .mt_2()
                .w_full()
                .border_1()
                .border_color(color::border_strong())
                .bg(color::input_background())
                .px_2()
                .py_1()
                .child(
                    Input::new(&review_comment_edit_input)
                        .w_full()
                        .small()
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false),
                ),
        )
        .when_some(edit_error, |element, error| {
            element.child(
                div()
                    .pt_1()
                    .text_xs()
                    .text_color(color::danger())
                    .child(error),
            )
        })
        .child(
            div()
                .pt_1()
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .child(
                    Button::new(format!("cancel-comment-edit-{comment_id}"))
                        .label("Cancel")
                        .xsmall()
                        .ghost()
                        .disabled(edit_submitting)
                        .debug_selector({
                            let selector =
                                format!("inline-review-comment-edit-cancel-{comment_id}");
                            move || selector.clone()
                        })
                        .on_click({
                            let view_entity = view_entity.clone();
                            move |_, window, cx| {
                                view_entity.update(cx, |view, cx| {
                                    view.cancel_review_comment_edit(window, cx);
                                });
                            }
                        }),
                )
                .child(
                    Button::new(format!("save-comment-edit-{comment_id}"))
                        .label("Save")
                        .xsmall()
                        .primary()
                        .loading(edit_submitting)
                        .disabled(edit_body_empty || edit_submitting)
                        .debug_selector({
                            let selector = format!("inline-review-comment-edit-save-{comment_id}");
                            move || selector.clone()
                        })
                        .on_click({
                            let view_entity = view_entity.clone();
                            let comment_id = comment_id.clone();
                            move |_, _, cx| {
                                view_entity.update(cx, |view, cx| {
                                    view.submit_review_comment_edit(comment_id.clone(), cx);
                                });
                            }
                        }),
                ),
        )
}

pub(crate) fn review_comment_action_visibility(comment: &ReviewComment) -> (bool, bool) {
    (comment.viewer_can_update, comment.viewer_can_delete)
}

#[cfg(test)]
mod tests {
    use crate::test_fixtures::review_comment;

    use super::*;

    #[test]
    fn exposes_review_comment_action_visibility() {
        let mut comment = review_comment();

        assert_eq!(review_comment_action_visibility(&comment), (false, false));

        comment.viewer_can_update = true;
        comment.viewer_can_delete = true;

        assert_eq!(review_comment_action_visibility(&comment), (true, true));
    }
}
