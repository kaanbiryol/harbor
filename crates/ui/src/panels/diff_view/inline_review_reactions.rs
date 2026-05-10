use gpui::{Anchor, AnyElement, Entity, IntoElement, div, prelude::*, px, rgb};
use gpui_component::{
    Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    popover::{Popover, PopoverState},
};
use harbor_domain::{ReactionContent, ReviewComment};

use crate::workspace::{AppView, ReviewReactionAction, review_reaction};

pub(super) fn render_review_reactions(
    comment: &ReviewComment,
    reaction_action: Option<&ReviewReactionAction>,
    view_entity: Entity<AppView>,
) -> impl IntoElement {
    let visible_reactions = visible_review_reaction_contents(comment);
    let has_visible_reactions = !visible_reactions.is_empty();
    let can_add_reaction = comment.viewer_can_react;

    div().when(has_visible_reactions || can_add_reaction, |element| {
        element
            .pt_2()
            .flex()
            .items_center()
            .gap_1()
            .children(visible_reactions.into_iter().map(|content| {
                render_review_reaction_button(
                    comment,
                    content,
                    reaction_action,
                    view_entity.clone(),
                )
            }))
            .when(can_add_reaction, |element| {
                element.child(render_add_reaction_popover(comment, view_entity.clone()))
            })
    })
}

fn render_review_reaction_button(
    comment: &ReviewComment,
    content: ReactionContent,
    reaction_action: Option<&ReviewReactionAction>,
    view_entity: Entity<AppView>,
) -> AnyElement {
    let reaction = review_reaction(comment, content);
    let count = reaction.map_or(0, |reaction| reaction.count);
    let viewer_has_reacted = reaction.is_some_and(|reaction| reaction.viewer_has_reacted);
    let running = reaction_action
        .is_some_and(|action| action.comment_id == comment.id && action.content == content);
    let comment_id = comment.id.clone();
    let label = review_reaction_button_label(content, count);
    let button = Button::new(format!("reaction-{comment_id}-{}", content.label()))
        .label(label)
        .xsmall()
        .disabled(!comment.viewer_can_react || running)
        .on_click({
            let view_entity = view_entity.clone();
            let comment_id = comment_id.clone();
            move |_, _, cx| {
                view_entity.update(cx, |view, cx| {
                    view.toggle_review_comment_reaction(comment_id.clone(), content, cx);
                });
            }
        });

    if viewer_has_reacted {
        button.primary().into_any_element()
    } else {
        button.ghost().into_any_element()
    }
}

fn render_add_reaction_popover(
    comment: &ReviewComment,
    view_entity: Entity<AppView>,
) -> impl IntoElement {
    let comment_id = comment.id.clone();

    Popover::new(format!("add-reaction-{comment_id}"))
        .appearance(false)
        .anchor(Anchor::TopRight)
        .trigger(
            Button::new(format!("add-reaction-trigger-{comment_id}"))
                .icon(IconName::Plus)
                .xsmall()
                .compact()
                .ghost()
                .tooltip("Add reaction"),
        )
        .content({
            let view_entity = view_entity.clone();
            move |_, _window, popover_cx| {
                let popover = popover_cx.entity().clone();
                let (comment, reaction_action) = {
                    let view = view_entity.read(popover_cx);
                    (
                        view.review_comment(&comment_id).cloned(),
                        view.review_reaction_action.clone(),
                    )
                };
                let Some(comment) = comment else {
                    return div()
                        .w(px(256.0))
                        .border_1()
                        .border_color(rgb(0x343b44))
                        .bg(rgb(0x171b20))
                        .p_2()
                        .text_xs()
                        .text_color(rgb(0x9aa4b2))
                        .child("Comment is no longer loaded")
                        .into_any_element();
                };

                div()
                    .w(px(256.0))
                    .border_1()
                    .border_color(rgb(0x343b44))
                    .bg(rgb(0x171b20))
                    .p_2()
                    .shadow_lg()
                    .child(div().grid().grid_cols(4).gap_1().children(
                        ReactionContent::ALL.into_iter().map(|content| {
                            render_review_reaction_picker_button(
                                &comment,
                                content,
                                reaction_action.as_ref(),
                                popover.clone(),
                                view_entity.clone(),
                            )
                        }),
                    ))
                    .into_any_element()
            }
        })
}

fn render_review_reaction_picker_button(
    comment: &ReviewComment,
    content: ReactionContent,
    reaction_action: Option<&ReviewReactionAction>,
    popover: Entity<PopoverState>,
    view_entity: Entity<AppView>,
) -> AnyElement {
    let reaction = review_reaction(comment, content);
    let viewer_has_reacted = reaction.is_some_and(|reaction| reaction.viewer_has_reacted);
    let running = reaction_action
        .is_some_and(|action| action.comment_id == comment.id && action.content == content);
    let comment_id = comment.id.clone();
    let button = Button::new(format!("reaction-picker-{comment_id}-{}", content.label()))
        .label(review_reaction_emoji(content))
        .xsmall()
        .disabled(!comment.viewer_can_react || running)
        .on_click({
            let view_entity = view_entity.clone();
            let comment_id = comment_id.clone();
            let popover = popover.clone();
            move |_, window, cx| {
                view_entity.update(cx, |view, cx| {
                    view.toggle_review_comment_reaction(comment_id.clone(), content, cx);
                });
                popover.update(cx, |popover, cx| {
                    popover.dismiss(window, cx);
                });
            }
        });

    if viewer_has_reacted {
        button.primary().into_any_element()
    } else {
        button.ghost().into_any_element()
    }
}

pub(crate) fn visible_review_reaction_contents(comment: &ReviewComment) -> Vec<ReactionContent> {
    ReactionContent::ALL
        .into_iter()
        .filter(|content| {
            review_reaction(comment, *content)
                .is_some_and(|reaction| reaction.count > 0 || reaction.viewer_has_reacted)
        })
        .collect()
}

pub(crate) fn review_reaction_button_label(content: ReactionContent, count: usize) -> String {
    if count == 0 {
        review_reaction_emoji(content).to_string()
    } else {
        format!("{} {count}", review_reaction_emoji(content))
    }
}

pub(crate) fn review_reaction_emoji(content: ReactionContent) -> &'static str {
    match content {
        ReactionContent::ThumbsUp => "👍",
        ReactionContent::ThumbsDown => "👎",
        ReactionContent::Laugh => "😄",
        ReactionContent::Confused => "😕",
        ReactionContent::Heart => "❤️",
        ReactionContent::Hooray => "🎉",
        ReactionContent::Rocket => "🚀",
        ReactionContent::Eyes => "👀",
    }
}

#[cfg(test)]
mod tests {
    use harbor_domain::ReviewReaction;

    use super::*;

    #[test]
    fn labels_review_reaction_buttons() {
        assert_eq!(
            review_reaction_button_label(ReactionContent::ThumbsUp, 0),
            "👍"
        );
        assert_eq!(
            review_reaction_button_label(ReactionContent::Heart, 3),
            "❤️ 3"
        );
        assert_eq!(review_reaction_emoji(ReactionContent::Rocket), "🚀");
    }

    #[test]
    fn shows_only_active_review_reactions_inline() {
        let mut comment = review_comment();
        comment.reactions = vec![ReviewReaction {
            content: ReactionContent::Heart,
            count: 2,
            viewer_has_reacted: false,
        }];

        assert_eq!(
            visible_review_reaction_contents(&comment),
            vec![ReactionContent::Heart]
        );
    }

    fn review_comment() -> ReviewComment {
        ReviewComment {
            id: "comment".to_string(),
            author: "octocat".to_string(),
            author_avatar_url: None,
            body: "Looks good".to_string(),
            created_at: chrono::DateTime::parse_from_rfc3339("2026-05-01T10:00:00Z")
                .expect("valid test timestamp")
                .with_timezone(&chrono::Utc),
            updated_at: None,
            position: None,
            viewer_did_author: false,
            viewer_can_update: false,
            viewer_can_delete: false,
            viewer_can_react: true,
            reactions: Vec::new(),
        }
    }
}
