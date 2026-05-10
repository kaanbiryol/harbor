use gpui::Context;
use harbor_domain::ReactionContent;

use crate::workspace::{
    AppView, ReviewCommentUiError, ReviewReactionAction,
    async_updates::AppViewAsyncUpdateExt,
    reviews::{ReviewReactionKey, review_reaction},
};

impl AppView {
    pub(crate) fn toggle_review_comment_reaction(
        &mut self,
        comment_id: String,
        content: ReactionContent,
        cx: &mut Context<Self>,
    ) {
        if self.review_reaction_action.is_some() {
            self.status = "A review reaction action is already running".to_string();
            cx.notify();
            return;
        }

        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_reaction_error = Some(ReviewCommentUiError {
                comment_id,
                message: "Select a pull request before reacting".to_string(),
            });
            self.status = "Select a pull request before reacting".to_string();
            cx.notify();
            return;
        };

        let Some(comment) = self.review_comment(&comment_id) else {
            self.review_reaction_error = Some(ReviewCommentUiError {
                comment_id,
                message: "Review comment is no longer loaded".to_string(),
            });
            self.status = "Review comment is no longer loaded".to_string();
            cx.notify();
            return;
        };

        if !comment.viewer_can_react {
            self.review_reaction_error = Some(ReviewCommentUiError {
                comment_id,
                message: "GitHub does not allow you to react to this comment".to_string(),
            });
            self.status = "GitHub does not allow you to react to this comment".to_string();
            cx.notify();
            return;
        }

        let had_reacted =
            review_reaction(comment, content).is_some_and(|reaction| reaction.viewer_has_reacted);
        let viewer_has_reacted = !had_reacted;
        let reaction_key = ReviewReactionKey::new(comment_id.clone(), content);
        self.set_review_comment_reaction(&comment_id, content, viewer_has_reacted);
        self.review_reaction_overrides
            .insert(reaction_key.clone(), viewer_has_reacted);
        self.review_reaction_action = Some(ReviewReactionAction {
            comment_id: comment_id.clone(),
            content,
        });
        self.review_reaction_error = None;
        self.status = format!("Updating reaction on PR #{}", pr.number);
        cx.notify();
        let github_api = self.github_api.clone();

        cx.spawn(async move |this, cx| {
            let result = if had_reacted {
                github_api
                    .remove_review_comment_reaction(&comment_id, content)
                    .await
            } else {
                github_api
                    .add_review_comment_reaction(&comment_id, content)
                    .await
            };

            this.update_or_log(
                cx,
                "failed to update review reaction state",
                move |view, cx| {
                    view.review_reaction_action = None;

                    match result {
                        Ok(()) => {
                            view.review_reaction_error = None;
                            view.status = format!("Updated reaction on PR #{}", pr.number);
                            view.load_selected_review_data(cx);
                        }
                        Err(error) => {
                            view.review_reaction_overrides.remove(&reaction_key);
                            view.set_review_comment_reaction(&comment_id, content, had_reacted);
                            let message = format!("Failed to update reaction: {error}");
                            view.review_reaction_error = Some(ReviewCommentUiError {
                                comment_id,
                                message: message.clone(),
                            });
                            view.status = message;
                        }
                    }

                    cx.notify();
                },
            );
        })
        .detach();
    }
}
