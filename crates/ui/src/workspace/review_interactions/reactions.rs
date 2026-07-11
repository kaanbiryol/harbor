use gpui::Context;
use harbor_domain::ReactionContent;

use crate::workspace::{
    AppView, ReviewReactionAction,
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
        if self.review_state.reaction_action_running() {
            self.status = "A review reaction action is already running".to_string();
            cx.notify();
            return;
        }

        let Some(pr) = self.selected_pull_request().cloned() else {
            self.review_state
                .set_review_reaction_error(comment_id, "Select a pull request before reacting");
            self.status = "Select a pull request before reacting".to_string();
            cx.notify();
            return;
        };

        let Some(comment) = self.review_comment(&comment_id) else {
            self.review_state
                .set_review_reaction_error(comment_id, "Review comment is no longer loaded");
            self.status = "Review comment is no longer loaded".to_string();
            cx.notify();
            return;
        };

        if !comment.viewer_can_react {
            self.review_state.set_review_reaction_error(
                comment_id,
                "GitHub does not allow you to react to this comment",
            );
            self.status = "GitHub does not allow you to react to this comment".to_string();
            cx.notify();
            return;
        }

        let had_reacted =
            review_reaction(comment, content).is_some_and(|reaction| reaction.viewer_has_reacted);
        let viewer_has_reacted = !had_reacted;
        let reaction_key = ReviewReactionKey::new(comment_id.clone(), content);
        self.set_review_comment_reaction(&comment_id, content, viewer_has_reacted);
        self.review_state
            .set_review_reaction_override(reaction_key.clone(), viewer_has_reacted);
        self.review_state
            .start_review_reaction_action(ReviewReactionAction {
                comment_id: comment_id.clone(),
                content,
            });
        self.remeasure_overview_thread_item_for_comment(&comment_id);
        self.status = format!("Updating reaction on PR #{}", pr.number);
        cx.notify();
        let github_api = self.github_api.clone();
        let overview_comment_id = comment_id.clone();

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
                    view.review_state.finish_review_reaction_action();

                    match result {
                        Ok(()) => {
                            view.review_state.clear_review_reaction_error();
                            view.status = format!("Updated reaction on PR #{}", pr.number);
                            view.load_selected_review_data(cx);
                        }
                        Err(error) => {
                            view.review_state
                                .remove_review_reaction_override(&reaction_key);
                            view.set_review_comment_reaction(&comment_id, content, had_reacted);
                            let message = format!("Failed to update reaction: {error}");
                            view.review_state
                                .set_review_reaction_error(comment_id, message.clone());
                            view.status = message;
                        }
                    }

                    view.remeasure_overview_thread_item_for_comment(&overview_comment_id);
                    cx.notify();
                },
            );
        })
        .detach();
    }
}
