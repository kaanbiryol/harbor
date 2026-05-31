use std::collections::HashMap;

use crate::{ReactionContent, ReviewComment, ReviewReaction, ReviewThread};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ReviewReactionKey {
    comment_id: String,
    content: ReactionContent,
}

impl ReviewReactionKey {
    pub fn new(comment_id: impl Into<String>, content: ReactionContent) -> Self {
        Self {
            comment_id: comment_id.into(),
            content,
        }
    }
}

pub fn apply_review_reaction_overrides(
    review_threads: &mut [ReviewThread],
    overrides: &HashMap<ReviewReactionKey, bool>,
) -> Vec<ReviewReactionKey> {
    if overrides.is_empty() {
        return Vec::new();
    }

    let mut settled_overrides = Vec::new();

    for thread in review_threads {
        for comment in &mut thread.comments {
            for (key, viewer_has_reacted) in overrides {
                if key.comment_id != comment.id {
                    continue;
                }

                let loaded_viewer_has_reacted = review_reaction(comment, key.content)
                    .is_some_and(|reaction| reaction.viewer_has_reacted);

                if loaded_viewer_has_reacted == *viewer_has_reacted {
                    settled_overrides.push(key.clone());
                } else {
                    set_review_comment_reaction_state(comment, key.content, *viewer_has_reacted);
                }
            }
        }
    }

    settled_overrides
}

pub fn set_review_comment_reaction_state(
    comment: &mut ReviewComment,
    content: ReactionContent,
    viewer_has_reacted: bool,
) {
    if let Some(reaction) = comment
        .reactions
        .iter_mut()
        .find(|reaction| reaction.content == content)
    {
        if reaction.viewer_has_reacted == viewer_has_reacted {
            return;
        }

        reaction.viewer_has_reacted = viewer_has_reacted;
        reaction.count = if viewer_has_reacted {
            reaction.count.saturating_add(1)
        } else {
            reaction.count.saturating_sub(1)
        };
    } else if viewer_has_reacted {
        comment.reactions.push(ReviewReaction {
            content,
            count: 1,
            viewer_has_reacted: true,
        });
    }

    comment
        .reactions
        .retain(|reaction| reaction.count > 0 || reaction.viewer_has_reacted);
}

pub fn review_reaction(
    comment: &ReviewComment,
    content: ReactionContent,
) -> Option<&ReviewReaction> {
    comment
        .reactions
        .iter()
        .find(|reaction| reaction.content == content)
}
