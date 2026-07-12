use gpui::{AnyElement, Context, Entity, IntoElement, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::Input,
};
use harbor_domain::{PullRequest, PullRequestPerson, ReviewThread};

use crate::{
    panels::{render_review_markdown_state, review_markdown_body, review_thread_diff_preview},
    visual::color,
    workspace::{AppView, ReviewRuntimeState, state::OverviewMarkdownState},
};

use super::{model::*, sidebar::render_person_avatar_with_size, timeline::*};

impl AppView {
    pub(super) fn ensure_overview_markdown_state(
        &mut self,
        key: String,
        body: &str,
        cx: &mut Context<Self>,
    ) -> Entity<gpui_component::text::TextViewState> {
        let source = review_markdown_body(body);
        if let Some(entry) = self.overview_state.markdown_states.get_mut(&key) {
            if entry.source != source {
                entry.source.clone_from(&source);
                entry
                    .state
                    .update(cx, |state, cx| state.set_text(&source, cx));
            }
            return entry.state.clone();
        }

        let state = cx.new(|cx| gpui_component::text::TextViewState::markdown(&source, cx));
        self.overview_state.markdown_states.insert(
            key,
            OverviewMarkdownState {
                source,
                state: state.clone(),
            },
        );
        state
    }

    pub(super) fn render_overview_markdown(
        &mut self,
        key: String,
        body: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = self.ensure_overview_markdown_state(key, body, cx);
        render_review_markdown_state(&state).into_any_element()
    }

    pub(super) fn toggle_overview_thread_expansion(
        &mut self,
        thread_id: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(thread) = self
            .review_state
            .review_threads()
            .iter()
            .find(|thread| thread.id == thread_id)
        else {
            return;
        };
        let expanded = overview_thread_expanded(
            thread.state,
            self.overview_state
                .thread_expansion_overrides
                .get(thread_id)
                .copied(),
        );
        self.overview_state
            .thread_expansion_overrides
            .insert(thread_id.to_string(), !expanded);
        self.remeasure_overview_thread_item(thread_id);
        cx.notify();
    }

    pub(crate) fn remeasure_overview_thread_item(&self, thread_id: &str) {
        let panel_items = overview_panel_items(
            self.detail_state.commits(),
            self.review_state.pull_request_reviews(),
            self.review_state.pull_request_comments(),
            self.review_state.review_threads(),
            self.review_state.reviews_loading(),
            self.review_state.reviews_error(),
        );
        let Some(index) = overview_thread_item_index(&panel_items, thread_id) else {
            return;
        };

        self.overview_state
            .list_state
            .remeasure_items(index..index + 1);
    }

    pub(crate) fn remeasure_overview_thread_item_for_comment(&self, comment_id: &str) {
        let Some(thread_id) = self
            .review_state
            .review_threads()
            .iter()
            .find(|thread| {
                thread
                    .comments
                    .iter()
                    .any(|comment| comment.id == comment_id)
            })
            .map(|thread| thread.id.as_str())
        else {
            return;
        };

        self.remeasure_overview_thread_item(thread_id);
    }

    pub(crate) fn update_overview_review_data<R>(
        &mut self,
        update: impl FnOnce(&mut ReviewRuntimeState) -> R,
    ) -> R {
        let previous_reviews = self.review_state.pull_request_reviews().to_vec();
        let previous_comments = self.review_state.pull_request_comments().to_vec();
        let previous_threads = self.review_state.review_threads().to_vec();
        let result = update(&mut self.review_state);
        let panel_items = overview_panel_items(
            self.detail_state.commits(),
            self.review_state.pull_request_reviews(),
            self.review_state.pull_request_comments(),
            self.review_state.review_threads(),
            self.review_state.reviews_loading(),
            self.review_state.reviews_error(),
        );
        let panel_item_keys = panel_items.iter().map(OverviewPanelItem::key).collect();
        sync_overview_list_items(
            &self.overview_state.list_state,
            &mut self.overview_state.list_item_keys,
            panel_item_keys,
        );

        for (index, item) in panel_items.iter().enumerate() {
            let changed = match item {
                OverviewPanelItem::Commit { .. } => false,
                OverviewPanelItem::Comment { id } => {
                    previous_comments.iter().find(|comment| comment.id == *id)
                        != self
                            .review_state
                            .pull_request_comments()
                            .iter()
                            .find(|comment| comment.id == *id)
                }
                OverviewPanelItem::Review { id } => {
                    previous_reviews.iter().find(|review| review.id == *id)
                        != self
                            .review_state
                            .pull_request_reviews()
                            .iter()
                            .find(|review| review.id == *id)
                }
                OverviewPanelItem::Thread { id } => {
                    previous_threads.iter().find(|thread| thread.id == *id)
                        != self
                            .review_state
                            .review_threads()
                            .iter()
                            .find(|thread| thread.id == *id)
                }
                OverviewPanelItem::Description
                | OverviewPanelItem::Message(_)
                | OverviewPanelItem::Composer => false,
            };
            if changed {
                self.overview_state
                    .list_state
                    .remeasure_items(index..index + 1);
            }
        }

        result
    }

    pub(super) fn render_overview_thread_event(
        &mut self,
        thread: &ReviewThread,
        index: usize,
        expanded: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let reply_input = self
            .review_state
            .review_composer_state
            .thread_reply_input
            .clone();
        let reply_body_empty = reply_input.read(cx).value().trim().is_empty();

        let reaction_action = self.review_state.review_reaction_action().cloned();
        let reaction_error = self.review_state.review_reaction_error().cloned();
        let active_comment_edit = self
            .review_state
            .review_composer_state
            .active_comment_edit()
            .map(str::to_string);
        let comment_edit_input = self
            .review_state
            .review_composer_state
            .comment_edit_input
            .clone();
        let edit_body_empty = comment_edit_input.read(cx).value().trim().is_empty();
        let is_submitting_edit = self.review_state.is_submitting_review_comment_edit();
        let edit_error = self.review_state.review_comment_edit_error().cloned();
        let action_comment_id = self
            .review_state
            .review_comment_action_comment_id()
            .map(str::to_string);
        let action_error = self.review_state.review_comment_action_error().cloned();
        let view_entity = cx.entity().clone();
        let comments = thread
            .comments
            .iter()
            .enumerate()
            .map(|(comment_index, comment)| {
                let markdown = self.render_overview_markdown(
                    format!("overview-thread-comment-body-{}", comment.id),
                    &comment.body,
                    cx,
                );
                render_overview_thread_comment(OverviewThreadCommentRenderState {
                    comment,
                    index: comment_index,
                    thread_id: &thread.id,
                    markdown,
                    reaction_action: reaction_action.as_ref(),
                    reaction_error: reaction_error.as_ref(),
                    active_comment_edit: active_comment_edit.as_deref(),
                    comment_edit_input: comment_edit_input.clone(),
                    edit_body_empty,
                    is_submitting_edit,
                    edit_error: edit_error.as_ref(),
                    action_comment_id: action_comment_id.as_deref(),
                    action_error: action_error.as_ref(),
                    view_entity: view_entity.clone(),
                })
            })
            .collect();

        render_overview_thread(OverviewThreadRenderState {
            thread,
            index,
            expanded,
            active_review_thread_reply: self
                .review_state
                .review_composer_state
                .active_thread_reply(),
            reply_input,
            reply_body_empty,
            is_submitting_reply: self.review_state.is_submitting_review_thread_reply(),
            reply_error: self.review_state.review_thread_reply_error(),
            action_thread_id: self.review_state.review_thread_action_thread_id(),
            action_error: self.review_state.review_thread_action_error(),
            diff_preview: review_thread_diff_preview(
                thread,
                self.detail_state.files(),
                self.detail_state.diffs(),
            ),
            mono_font_family: cx.theme().mono_font_family.clone(),
            comments,
            view_entity,
        })
    }

    pub(super) fn render_overview_comment_composer(
        &self,
        pr: &PullRequest,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let input = self.overview_comment_input.clone();
        let input_empty = input.read(cx).value().trim().is_empty();
        let action_running = self.action_runtime.pull_request_action_running();
        let action_error = self
            .action_runtime
            .pull_request_action_error()
            .map(str::to_string);
        let commenter = PullRequestPerson {
            login: self
                .review_state
                .current_user_login()
                .map_or_else(|| pr.author.clone(), str::to_string),
            avatar_url: None,
        };

        render_timeline_row(
            render_person_avatar_with_size(&commenter, 24.0),
            div()
                .debug_selector(|| "pull-request-overview-comment-composer".to_string())
                .w_full()
                .min_w_0()
                .rounded_sm()
                .border_1()
                .border_color(color::border())
                .bg(color::content_background())
                .p_2()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(color::text_primary())
                        .child("Add a comment"),
                )
                .child(
                    div()
                        .debug_selector(|| "pull-request-overview-comment-input".to_string())
                        .child(
                            Input::new(&input)
                                .small()
                                .w_full()
                                .h(px(64.0))
                                .appearance(false)
                                .bordered(true)
                                .focus_bordered(true),
                        ),
                )
                .when_some(action_error, |element, error| {
                    element.child(div().text_xs().text_color(color::danger()).child(error))
                })
                .child(
                    div().flex().justify_end().child(
                        Button::new("submit-overview-comment")
                            .label("Comment")
                            .small()
                            .primary()
                            .loading(action_running)
                            .disabled(action_running || input_empty)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.submit_overview_comment(window, cx);
                            })),
                    ),
                )
                .into_any_element(),
            false,
        )
    }
}
