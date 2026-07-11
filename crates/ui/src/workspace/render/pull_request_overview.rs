use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use gpui::{
    Anchor, AnyElement, Context, Div, Entity, IntoElement, Rgba, SharedString, div, list,
    prelude::*, px, rgb,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt,
    avatar::Avatar,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    list::ListItem,
    popover::Popover,
    scroll::ScrollableElement,
    spinner::Spinner,
    tooltip::Tooltip,
};
use harbor_domain::{
    Label, MergeState, PullRequest, PullRequestComment, PullRequestPerson, PullRequestReview,
    PullRequestReviewState, PullRequestTeam, ReviewDecision, ReviewThread, ReviewThreadState,
};

use crate::{
    actions::{PanelTab, PullRequestMetadataField},
    date_time::{
        full_time_label, full_time_label_with_edit, natural_time_label,
        natural_time_label_with_edit,
    },
    icons::Octicon,
    panels::{
        ReviewDiffPreview, overview_markdown_body, render_review_diff_preview,
        render_review_markdown_state, render_review_reactions, render_status_pill,
        review_markdown_body,
        review_thread_chrome::{
            ReviewThreadActionIds, ReviewThreadActionsChrome, ReviewThreadActionsState,
            ReviewThreadReplyComposerChrome, ReviewThreadReplyComposerIds,
            ReviewThreadReplyComposerState, render_review_thread_actions,
            render_review_thread_reply_composer, review_thread_ui_state,
        },
        review_thread_diff_preview,
    },
    visual::{Tone, color, tone_colors},
    workspace::{
        AppView, ReviewCommentUiError, ReviewReactionAction, ReviewRuntimeState,
        ReviewThreadUiError,
    },
};

const OVERVIEW_SIDEBAR_WIDTH: f32 = 280.0;

impl AppView {
    pub(super) fn render_pull_request_overview_panel(
        &mut self,
        pr: Option<&PullRequest>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(pr) = pr else {
            return div()
                .text_sm()
                .text_color(color::text_muted())
                .child("Select a pull request to see its overview")
                .into_any_element();
        };
        let panel_items = overview_panel_items(
            &self.review_state.pull_request_reviews,
            &self.review_state.pull_request_comments,
            &self.review_state.review_threads,
            self.review_state.reviews_loading(),
            self.review_state.reviews_error(),
        );
        let panel_item_keys = panel_items.iter().map(OverviewPanelItem::key).collect();
        sync_overview_list_items(
            &self.overview_list_state,
            &mut self.overview_list_item_keys,
            panel_item_keys,
        );
        let panel_items_for_render = panel_items.clone();

        div()
            .debug_selector(|| "pull-request-overview-panel".to_string())
            .image_cache(gpui::retain_all("pull-request-overview-avatar-cache"))
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .min_h_0()
                    .flex()
                    .items_stretch()
                    .gap_3()
                    .w_full()
                    .min_w_0()
                    .child(
                        div()
                            .debug_selector(|| "pull-request-overview-timeline".to_string())
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .child(
                                list(
                                    self.overview_list_state.clone(),
                                    cx.processor(move |view, index: usize, _window, cx| {
                                        let Some(item) = panel_items_for_render.get(index) else {
                                            return div().into_any_element();
                                        };

                                        match item {
                                            OverviewPanelItem::Description => view
                                                .selected_pull_request()
                                                .cloned()
                                                .map(|pr| {
                                                    div()
                                                        .w_full()
                                                        .pb_3()
                                                        .child(
                                                            view.render_description_card(&pr, cx),
                                                        )
                                                        .into_any_element()
                                                })
                                                .unwrap_or_else(|| div().into_any_element()),
                                            OverviewPanelItem::Comment { id } => view
                                                .review_state
                                                .pull_request_comments
                                                .iter()
                                                .find(|comment| comment.id == *id)
                                                .cloned()
                                                .map(|comment| {
                                                    let markdown = view.render_overview_markdown(
                                                        format!(
                                                            "overview-comment-body-{}",
                                                            comment.id
                                                        ),
                                                        &comment.body,
                                                        cx,
                                                    );
                                                    render_overview_comment_event(
                                                        &comment, index, markdown,
                                                    )
                                                })
                                                .unwrap_or_else(|| div().into_any_element()),
                                            OverviewPanelItem::Review { id } => view
                                                .review_state
                                                .pull_request_reviews
                                                .iter()
                                                .find(|review| review.id == *id)
                                                .cloned()
                                                .map(|review| {
                                                    let markdown = review
                                                        .body
                                                        .as_deref()
                                                        .map(str::trim)
                                                        .filter(|body| !body.is_empty())
                                                        .map(|body| {
                                                            view.render_overview_markdown(
                                                                format!(
                                                                    "overview-review-body-{}",
                                                                    review.id
                                                                ),
                                                                body,
                                                                cx,
                                                            )
                                                        });
                                                    render_overview_review_event(
                                                        &review, index, markdown,
                                                    )
                                                })
                                                .unwrap_or_else(|| div().into_any_element()),
                                            OverviewPanelItem::Thread { id } => view
                                                .review_state
                                                .review_threads
                                                .iter()
                                                .find(|thread| thread.id == *id)
                                                .cloned()
                                                .map(|thread| {
                                                    let expanded = overview_thread_expanded(
                                                        thread.state,
                                                        view.overview_thread_expansion_overrides
                                                            .get(&thread.id)
                                                            .copied(),
                                                    );
                                                    view.render_overview_thread_event(
                                                        &thread, index, expanded, cx,
                                                    )
                                                })
                                                .unwrap_or_else(|| div().into_any_element()),
                                            OverviewPanelItem::Message(message) => {
                                                render_timeline_message(message)
                                            }
                                            OverviewPanelItem::Composer => view
                                                .selected_pull_request()
                                                .cloned()
                                                .map(|pr| {
                                                    view.render_overview_comment_composer(&pr, cx)
                                                })
                                                .unwrap_or_else(|| div().into_any_element()),
                                        }
                                    }),
                                )
                                .size_full(),
                            ),
                    )
                    .child(
                        div()
                            .debug_selector(|| "pull-request-overview-sidebar".to_string())
                            .w(px(OVERVIEW_SIDEBAR_WIDTH))
                            .min_h_0()
                            .flex_none()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .overflow_y_scrollbar()
                            .child(self.render_merge_readiness_card(pr, cx))
                            .child(self.render_people_card(pr, cx))
                            .child(self.render_labels_card(pr, cx)),
                    ),
            )
            .into_any_element()
    }
}

impl AppView {
    fn ensure_overview_markdown_state(
        &mut self,
        key: String,
        body: &str,
        cx: &mut Context<Self>,
    ) -> Entity<gpui_component::text::TextViewState> {
        let source = review_markdown_body(body);
        if let Some(entry) = self.overview_markdown_states.get_mut(&key) {
            if entry.source != source {
                entry.source.clone_from(&source);
                entry
                    .state
                    .update(cx, |state, cx| state.set_text(&source, cx));
            }
            return entry.state.clone();
        }

        let state = cx.new(|cx| gpui_component::text::TextViewState::markdown(&source, cx));
        self.overview_markdown_states.insert(
            key,
            super::super::OverviewMarkdownState {
                source,
                state: state.clone(),
            },
        );
        state
    }

    fn render_overview_markdown(
        &mut self,
        key: String,
        body: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = self.ensure_overview_markdown_state(key, body, cx);
        render_review_markdown_state(&state).into_any_element()
    }

    fn toggle_overview_thread_expansion(&mut self, thread_id: &str, cx: &mut Context<Self>) {
        let Some(thread) = self
            .review_state
            .review_threads
            .iter()
            .find(|thread| thread.id == thread_id)
        else {
            return;
        };
        let expanded = overview_thread_expanded(
            thread.state,
            self.overview_thread_expansion_overrides
                .get(thread_id)
                .copied(),
        );
        self.overview_thread_expansion_overrides
            .insert(thread_id.to_string(), !expanded);
        self.remeasure_overview_thread_item(thread_id);
        cx.notify();
    }

    pub(crate) fn remeasure_overview_thread_item(&self, thread_id: &str) {
        let panel_items = overview_panel_items(
            &self.review_state.pull_request_reviews,
            &self.review_state.pull_request_comments,
            &self.review_state.review_threads,
            self.review_state.reviews_loading(),
            self.review_state.reviews_error(),
        );
        let Some(index) = overview_thread_item_index(&panel_items, thread_id) else {
            return;
        };

        self.overview_list_state.remeasure_items(index..index + 1);
    }

    pub(crate) fn remeasure_overview_thread_item_for_comment(&self, comment_id: &str) {
        let Some(thread_id) = self
            .review_state
            .review_threads
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
        let previous_reviews = self.review_state.pull_request_reviews.clone();
        let previous_comments = self.review_state.pull_request_comments.clone();
        let previous_threads = self.review_state.review_threads.clone();
        let result = update(&mut self.review_state);
        let panel_items = overview_panel_items(
            &self.review_state.pull_request_reviews,
            &self.review_state.pull_request_comments,
            &self.review_state.review_threads,
            self.review_state.reviews_loading(),
            self.review_state.reviews_error(),
        );
        let panel_item_keys = panel_items.iter().map(OverviewPanelItem::key).collect();
        sync_overview_list_items(
            &self.overview_list_state,
            &mut self.overview_list_item_keys,
            panel_item_keys,
        );

        for (index, item) in panel_items.iter().enumerate() {
            let changed = match item {
                OverviewPanelItem::Comment { id } => {
                    previous_comments.iter().find(|comment| comment.id == *id)
                        != self
                            .review_state
                            .pull_request_comments
                            .iter()
                            .find(|comment| comment.id == *id)
                }
                OverviewPanelItem::Review { id } => {
                    previous_reviews.iter().find(|review| review.id == *id)
                        != self
                            .review_state
                            .pull_request_reviews
                            .iter()
                            .find(|review| review.id == *id)
                }
                OverviewPanelItem::Thread { id } => {
                    previous_threads.iter().find(|thread| thread.id == *id)
                        != self
                            .review_state
                            .review_threads
                            .iter()
                            .find(|thread| thread.id == *id)
                }
                OverviewPanelItem::Description
                | OverviewPanelItem::Message(_)
                | OverviewPanelItem::Composer => false,
            };
            if changed {
                self.overview_list_state.remeasure_items(index..index + 1);
            }
        }

        result
    }

    fn render_overview_thread_event(
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
                render_overview_thread_comment(
                    comment,
                    comment_index,
                    &thread.id,
                    markdown,
                    reaction_action.as_ref(),
                    reaction_error.as_ref(),
                    view_entity.clone(),
                )
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

    fn render_overview_comment_composer(
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
                .current_user_login
                .clone()
                .unwrap_or_else(|| pr.author.clone()),
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

    fn render_merge_readiness_card(&self, pr: &PullRequest, cx: &mut Context<Self>) -> AnyElement {
        let (review_label, review_tone) = review_readiness(pr.review_decision);
        let (merge_label, merge_tone) = merge_readiness(pr.merge_state);
        let unresolved_tone = if pr.unresolved_threads == 0 {
            Tone::Success
        } else {
            Tone::Warning
        };

        render_overview_card("Merge readiness")
            .debug_selector(|| "pull-request-merge-readiness".to_string())
            .gap_0()
            .child(render_readiness_row(
                "pull-request-review-readiness-row",
                "Review",
                review_label,
                Octicon::Eye,
                review_tone,
                false,
                false,
            ))
            .child(render_readiness_row(
                "pull-request-merge-readiness-row",
                "Merge",
                merge_label,
                Octicon::CheckCircle,
                merge_tone,
                true,
                false,
            ))
            .child(
                div()
                    .debug_selector(|| "pull-request-unresolved-conversations".to_string())
                    .child(
                        render_readiness_row(
                            "pull-request-unresolved-conversations-row",
                            "Conversations",
                            format!("{} unresolved", pr.unresolved_threads),
                            Octicon::CommentDiscussion,
                            unresolved_tone,
                            true,
                            true,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.select_panel_tab(PanelTab::Review, cx);
                        })),
                    ),
            )
            .into_any_element()
    }

    fn render_description_card(&mut self, pr: &PullRequest, cx: &mut Context<Self>) -> AnyElement {
        let editing = self.pull_request_description_editing;
        let saving = self
            .action_runtime
            .pull_request_description_action_running();
        let error = self
            .action_runtime
            .pull_request_description_action_error()
            .map(str::to_string);
        let description_input = self.pull_request_description_input.clone();
        let description = if editing {
            None
        } else {
            Some(self.render_pull_request_description(pr, cx))
        };

        div()
            .debug_selector(|| "pull-request-overview-description".to_string())
            .w_full()
            .min_w_0()
            .rounded_sm()
            .border_1()
            .border_color(color::border())
            .bg(color::content_background())
            .p_4()
            .child(
                div()
                    .pb_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .text_color(color::text_primary())
                            .child("Description"),
                    )
                    .when(!editing, |element| {
                        element.child(
                            Button::new("edit-pull-request-description")
                                .icon(Octicon::Pencil)
                                .xsmall()
                                .secondary()
                                .tooltip("Edit description if your GitHub permissions allow it")
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.start_pull_request_description_edit(window, cx);
                                })),
                        )
                    }),
            )
            .when_some(description, |element, description| {
                element.child(description)
            })
            .when(editing, |element| {
                element
                    .child(Input::new(&description_input))
                    .when_some(error, |element, error| {
                        element.child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(color::danger())
                                .child(error),
                        )
                    })
                    .child(
                        div()
                            .pt_3()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("cancel-pull-request-description")
                                    .label("Cancel")
                                    .small()
                                    .outline()
                                    .disabled(saving)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.cancel_pull_request_description_edit(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("save-pull-request-description")
                                    .label("Save")
                                    .small()
                                    .loading(saving)
                                    .disabled(saving)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.save_pull_request_description(window, cx);
                                    })),
                            ),
                    )
            })
            .into_any_element()
    }

    fn render_pull_request_description(
        &mut self,
        pr: &PullRequest,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(body) = pr
            .body
            .as_deref()
            .map(str::trim)
            .filter(|body| !body.is_empty())
        else {
            return div()
                .text_sm()
                .text_color(color::text_muted())
                .child("No description")
                .into_any_element();
        };
        let markdown = self.render_overview_markdown(
            format!("pull-request-description-{}", pr.number),
            &overview_markdown_body(body),
            cx,
        );

        div()
            .min_w_0()
            .pr_1()
            .text_sm()
            .text_color(color::text_secondary())
            .child(markdown)
            .into_any_element()
    }

    fn render_people_card(&self, pr: &PullRequest, cx: &mut Context<Self>) -> AnyElement {
        let author = PullRequestPerson {
            login: pr.author.clone(),
            avatar_url: None,
        };

        render_overview_card("People")
            .child(render_overview_section(
                "Author",
                div()
                    .debug_selector(|| "pull-request-author".to_string())
                    .child(render_people_row(std::slice::from_ref(&author)))
                    .into_any_element(),
            ))
            .child(render_overview_section(
                "Reviewers",
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .min_h(px(28.0))
                    .child(if has_review_requests(pr) {
                        render_review_requests_row(&pr.requested_reviewers, &pr.requested_teams)
                    } else {
                        render_empty_value("No reviewers requested")
                    })
                    .child(self.render_metadata_add_control(PullRequestMetadataField::Reviewer, cx))
                    .into_any_element(),
            ))
            .child(render_overview_section(
                "Assignees",
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .min_h(px(28.0))
                    .child(if pr.assignees.is_empty() {
                        render_empty_value("No assignees")
                    } else {
                        render_people_row(&pr.assignees)
                    })
                    .child(self.render_metadata_add_control(PullRequestMetadataField::Assignee, cx))
                    .into_any_element(),
            ))
            .into_any_element()
    }

    fn render_labels_card(&self, pr: &PullRequest, cx: &mut Context<Self>) -> AnyElement {
        render_overview_card("Labels")
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .min_h(px(28.0))
                    .child(if pr.labels.is_empty() {
                        render_empty_value("No labels")
                    } else {
                        render_labels_row(&pr.labels)
                    })
                    .child(self.render_metadata_add_control(PullRequestMetadataField::Label, cx)),
            )
            .into_any_element()
    }

    fn render_metadata_add_control(
        &self,
        field: PullRequestMetadataField,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let input = self.pull_request_metadata_input(field);
        let input_is_empty = input.read(cx).value().trim().is_empty();
        let action_running = self.action_runtime.pull_request_metadata_action_running();
        let action_field = self.action_runtime.pull_request_metadata_field();
        let field_running = action_running && action_field == Some(field);
        let error = (action_field == Some(field))
            .then(|| {
                self.action_runtime
                    .pull_request_metadata_action_error()
                    .map(str::to_string)
            })
            .flatten();
        let view = cx.entity().clone();
        let field_name = field.name();
        div()
            .debug_selector(move || format!("add-{field_name}-control"))
            .flex_none()
            .child(
                Popover::new(format!("add-{field_name}-popover"))
                    .appearance(false)
                    .anchor(Anchor::TopRight)
                    .on_open_change({
                        let input = input.clone();
                        move |open, window, cx| {
                            if *open {
                                input.update(cx, |input, cx| input.focus(window, cx));
                            }
                        }
                    })
                    .trigger(
                        Button::new(format!("open-add-{field_name}"))
                            .icon(Octicon::Plus)
                            .small()
                            .compact()
                            .outline()
                            .tooltip(format!("Add {field_name}")),
                    )
                    .content(move |_, _window, _popover_cx| {
                        div()
                            .w(px(280.0))
                            .border_1()
                            .border_color(color::border_strong())
                            .bg(color::elevated_background())
                            .shadow_lg()
                            .p_2()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(Input::new(&input).small().cleanable(true))
                            .when_some(error.clone(), |element, error| {
                                element
                                    .child(div().text_xs().text_color(color::danger()).child(error))
                            })
                            .child(
                                div().flex().justify_end().child(
                                    Button::new(format!("add-pull-request-{field_name}"))
                                        .icon(Octicon::Plus)
                                        .label("Add")
                                        .small()
                                        .loading(field_running)
                                        .disabled(action_running || input_is_empty)
                                        .on_click({
                                            let view = view.clone();
                                            move |_, window, cx| {
                                                view.update(cx, |view, cx| {
                                                    view.add_pull_request_metadata(
                                                        field, window, cx,
                                                    );
                                                });
                                            }
                                        }),
                                ),
                            )
                    }),
            )
            .into_any_element()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum OverviewPanelItem {
    Description,
    Comment { id: String },
    Review { id: String },
    Thread { id: String },
    Message(OverviewTimelineMessage),
    Composer,
}

impl OverviewPanelItem {
    fn key(&self) -> String {
        match self {
            Self::Description => "description".to_string(),
            Self::Comment { id } => format!("comment:{id}"),
            Self::Review { id } => format!("review:{id}"),
            Self::Thread { id } => format!("thread:{id}"),
            Self::Message(OverviewTimelineMessage::Loading) => "message:loading".to_string(),
            Self::Message(OverviewTimelineMessage::Empty) => "message:empty".to_string(),
            Self::Message(OverviewTimelineMessage::Error(_)) => "message:error".to_string(),
            Self::Composer => "composer".to_string(),
        }
    }
}

fn sync_overview_list_items(
    list_state: &gpui::ListState,
    previous_keys: &mut Vec<String>,
    next_keys: Vec<String>,
) {
    let current_item_count = list_state.item_count();
    if current_item_count != previous_keys.len() {
        if current_item_count == 0 {
            list_state.reset(next_keys.len());
        } else {
            list_state.splice(0..current_item_count, next_keys.len());
        }
        *previous_keys = next_keys;
        return;
    }

    if previous_keys == &next_keys {
        return;
    }

    let prefix_len = previous_keys
        .iter()
        .zip(&next_keys)
        .take_while(|(previous, next)| previous == next)
        .count();
    let mut previous_suffix_start = previous_keys.len();
    let mut next_suffix_start = next_keys.len();
    while previous_suffix_start > prefix_len
        && next_suffix_start > prefix_len
        && previous_keys[previous_suffix_start - 1] == next_keys[next_suffix_start - 1]
    {
        previous_suffix_start -= 1;
        next_suffix_start -= 1;
    }

    list_state.splice(
        prefix_len..previous_suffix_start,
        next_suffix_start - prefix_len,
    );
    *previous_keys = next_keys;
}

fn overview_thread_item_index(items: &[OverviewPanelItem], thread_id: &str) -> Option<usize> {
    items
        .iter()
        .position(|item| matches!(item, OverviewPanelItem::Thread { id } if id == thread_id))
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum OverviewTimelineMessage {
    Loading,
    Empty,
    Error(String),
}

fn overview_panel_items(
    reviews: &[PullRequestReview],
    comments: &[PullRequestComment],
    threads: &[ReviewThread],
    loading: bool,
    error: Option<&str>,
) -> Vec<OverviewPanelItem> {
    let timeline_items = overview_timeline_items(reviews, comments, threads);
    let mut items = Vec::with_capacity(timeline_items.len() + 3);
    items.push(OverviewPanelItem::Description);

    if let Some(error) = error {
        items.push(OverviewPanelItem::Message(OverviewTimelineMessage::Error(
            error.to_string(),
        )));
    }

    if timeline_items.is_empty() && error.is_none() {
        items.push(OverviewPanelItem::Message(if loading {
            OverviewTimelineMessage::Loading
        } else {
            OverviewTimelineMessage::Empty
        }));
    } else {
        items.extend(timeline_items.into_iter().map(|item| match item {
            OverviewTimelineItem::Comment(comment) => OverviewPanelItem::Comment {
                id: comment.id.clone(),
            },
            OverviewTimelineItem::Review(review) => OverviewPanelItem::Review {
                id: review.id.clone(),
            },
            OverviewTimelineItem::Thread(thread) => OverviewPanelItem::Thread {
                id: thread.id.clone(),
            },
        }));
    }

    items.push(OverviewPanelItem::Composer);
    items
}

#[derive(Clone, Copy)]
enum OverviewTimelineItem<'a> {
    Comment(&'a PullRequestComment),
    Review(&'a PullRequestReview),
    Thread(&'a ReviewThread),
}

impl OverviewTimelineItem<'_> {
    fn time(self) -> Option<DateTime<Utc>> {
        match self {
            Self::Comment(comment) => Some(comment.created_at),
            Self::Review(review) => review.submitted_at,
            Self::Thread(thread) => thread
                .comments
                .iter()
                .map(|comment| comment.created_at)
                .min(),
        }
    }
}

fn overview_timeline_items<'a>(
    reviews: &'a [PullRequestReview],
    comments: &'a [PullRequestComment],
    threads: &'a [ReviewThread],
) -> Vec<OverviewTimelineItem<'a>> {
    let mut items = Vec::with_capacity(reviews.len() + comments.len() + threads.len());
    items.extend(comments.iter().map(OverviewTimelineItem::Comment));
    items.extend(
        reviews
            .iter()
            .filter(|review| overview_review_visible(review))
            .map(OverviewTimelineItem::Review),
    );
    items.extend(
        threads
            .iter()
            .filter(|thread| !thread.comments.is_empty())
            .map(OverviewTimelineItem::Thread),
    );
    items.sort_by(|left, right| compare_timeline_times(left.time(), right.time()));
    items
}

fn overview_review_visible(review: &PullRequestReview) -> bool {
    match review.state {
        PullRequestReviewState::Pending => false,
        PullRequestReviewState::Commented => review
            .body
            .as_deref()
            .is_some_and(|body| !body.trim().is_empty()),
        PullRequestReviewState::Approved
        | PullRequestReviewState::ChangesRequested
        | PullRequestReviewState::Dismissed => true,
    }
}

fn compare_timeline_times(left: Option<DateTime<Utc>>, right: Option<DateTime<Utc>>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn render_overview_comment_event(
    comment: &PullRequestComment,
    index: usize,
    markdown: AnyElement,
) -> AnyElement {
    let person = PullRequestPerson {
        login: comment.author.clone(),
        avatar_url: comment.author_avatar_url.clone(),
    };
    let time_label = natural_time_label_with_edit(comment.created_at, comment.updated_at);
    let time_tooltip = full_time_label_with_edit(comment.created_at, comment.updated_at);

    render_timeline_row(
        render_person_avatar_with_size(&person, 24.0),
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
                    .child(render_timeline_time(
                        format!("overview-comment-time-{}", comment.id),
                        time_label,
                        time_tooltip,
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

fn render_overview_review_event(
    review: &PullRequestReview,
    index: usize,
    markdown: Option<AnyElement>,
) -> AnyElement {
    let selector = format!("overview-review-{}", review.id);
    let (action, status, tone) = overview_review_state(review.state);
    let time_label = review
        .submitted_at
        .map(natural_time_label)
        .unwrap_or_else(|| "not submitted".to_string());
    let time_tooltip = review.submitted_at.map(full_time_label);
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
                            .child(
                                div()
                                    .id(format!("overview-review-time-{}", review.id))
                                    .text_color(color::text_muted())
                                    .when_some(time_tooltip, |element, tooltip| {
                                        element.tooltip(move |window, cx| {
                                            Tooltip::new(tooltip.clone()).build(window, cx)
                                        })
                                    })
                                    .child(time_label),
                            ),
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

struct OverviewThreadRenderState<'a> {
    thread: &'a ReviewThread,
    index: usize,
    expanded: bool,
    active_review_thread_reply: Option<&'a str>,
    reply_input: Entity<InputState>,
    reply_body_empty: bool,
    is_submitting_reply: bool,
    reply_error: Option<&'a ReviewThreadUiError>,
    action_thread_id: Option<&'a str>,
    action_error: Option<&'a ReviewThreadUiError>,
    diff_preview: Option<ReviewDiffPreview>,
    mono_font_family: SharedString,
    comments: Vec<AnyElement>,
    view_entity: Entity<AppView>,
}

fn render_overview_thread(state: OverviewThreadRenderState<'_>) -> AnyElement {
    let OverviewThreadRenderState {
        thread,
        index,
        expanded,
        active_review_thread_reply,
        reply_input,
        reply_body_empty,
        is_submitting_reply,
        reply_error,
        action_thread_id,
        action_error,
        diff_preview,
        mono_font_family,
        comments,
        view_entity,
    } = state;
    let (status, tone, icon) = match thread.state {
        ReviewThreadState::Unresolved => ("unresolved", Tone::Warning, Octicon::CommentDiscussion),
        ReviewThreadState::Resolved => ("resolved", Tone::Success, Octicon::CheckCircle),
        ReviewThreadState::Outdated => ("outdated", Tone::Neutral, Octicon::Clock),
    };
    let comment_label = match thread.comments.len() {
        1 => "1 comment".to_string(),
        count => format!("{count} comments"),
    };
    let ui_state = review_thread_ui_state(
        thread,
        active_review_thread_reply,
        reply_body_empty,
        is_submitting_reply,
        action_thread_id,
    );
    let reply_error = reply_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let action_error = action_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let diff_preview = expanded.then_some(diff_preview).flatten();
    let path = thread.path.clone();
    let thread_id = thread.id.clone();
    let selector = format!("overview-thread-card-{}", thread.id);
    let node_selector = format!("overview-thread-node-{}", thread.id);
    let toggle_selector = format!("overview-thread-toggle-{}", thread.id);
    let reply_field_thread_id = thread_id.clone();
    let reply_field_view = view_entity.clone();
    let actions_view = view_entity.clone();
    let composer_view = view_entity.clone();
    let toggle_view = view_entity.clone();
    let toggle_thread_id = thread_id.clone();

    render_timeline_row_with_node_offset(
        div()
            .debug_selector(move || node_selector.clone())
            .child(render_timeline_icon(icon, tone))
            .into_any_element(),
        div()
            .debug_selector(move || selector.clone())
            .w_full()
            .min_w_0()
            .rounded_sm()
            .border_1()
            .border_color(color::border())
            .bg(color::content_background())
            .child(
                div()
                    .debug_selector(move || toggle_selector.clone())
                    .w_full()
                    .child(
                        ListItem::new(("overview-thread", index))
                            .h(px(40.0))
                            .w_full()
                            .py_0()
                            .rounded_none()
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Icon::new(Octicon::File)
                                            .xsmall()
                                            .text_color(color::text_muted()),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex_1()
                                            .truncate()
                                            .text_xs()
                                            .font_medium()
                                            .text_color(color::text_primary())
                                            .child(path),
                                    ),
                            )
                            .suffix(move |_, _| {
                                div()
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(color::text_muted())
                                            .child(comment_label.clone()),
                                    )
                                    .child(render_status_pill(status, tone))
                                    .child(
                                        Icon::new(if expanded {
                                            Octicon::ChevronDown
                                        } else {
                                            Octicon::ChevronRight
                                        })
                                        .xsmall()
                                        .text_color(color::text_muted()),
                                    )
                            })
                            .on_click(move |_, _, cx| {
                                toggle_view.update(cx, |view, cx| {
                                    view.toggle_overview_thread_expansion(&toggle_thread_id, cx);
                                });
                            }),
                    ),
            )
            .when_some(diff_preview, move |element, preview| {
                let selector = format!("overview-thread-diff-{thread_id}");
                element.child(
                    div()
                        .debug_selector(move || selector.clone())
                        .w_full()
                        .min_w_0()
                        .border_t_1()
                        .border_color(color::border_subtle())
                        .px_3()
                        .py_2()
                        .child(render_review_diff_preview(
                            preview,
                            mono_font_family.clone(),
                        )),
                )
            })
            .when(expanded, |element| {
                element.child(
                    div()
                        .w_full()
                        .min_w_0()
                        .border_t_1()
                        .border_color(color::border_subtle())
                        .children(comments),
                )
            })
            .when(expanded && !ui_state.active_reply, |element| {
                element.child(
                    div()
                        .w_full()
                        .min_w_0()
                        .border_t_1()
                        .border_color(color::border_subtle())
                        .px_3()
                        .py_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new(format!("overview-reply-field-{reply_field_thread_id}"))
                                .debug_selector({
                                    let selector =
                                        format!("overview-reply-field-{reply_field_thread_id}");
                                    move || selector.clone()
                                })
                                .child(div().w_full().text_left().child("Reply..."))
                                .small()
                                .outline()
                                .min_w_0()
                                .flex_1()
                                .justify_start()
                                .disabled(ui_state.reply_button_disabled)
                                .on_click(move |_, window, cx| {
                                    reply_field_view.update(cx, |view, cx| {
                                        view.open_review_thread_reply(
                                            reply_field_thread_id.clone(),
                                            window,
                                            cx,
                                        );
                                    });
                                }),
                        )
                        .child(div().flex_none().child(render_review_thread_actions(
                            ReviewThreadActionsState {
                                ids: ReviewThreadActionIds::overview(&thread.id),
                                thread_id: thread.id.clone(),
                                active_reply: ui_state.active_reply,
                                reply_button_disabled: ui_state.reply_button_disabled,
                                is_resolved: ui_state.is_resolved,
                                action_running: ui_state.action_running,
                                can_toggle_resolution: ui_state.can_toggle_resolution,
                                show_reply_button: false,
                                show_toggle_icon: false,
                                chrome: ReviewThreadActionsChrome::Inline,
                                view_entity: actions_view.clone(),
                            },
                        ))),
                )
            })
            .when(expanded && ui_state.active_reply, |element| {
                element.child(
                    div()
                        .w_full()
                        .min_w_0()
                        .border_t_1()
                        .border_color(color::border_subtle())
                        .px_3()
                        .py_2()
                        .child(render_review_thread_reply_composer(
                            ReviewThreadReplyComposerState {
                                ids: ReviewThreadReplyComposerIds::overview(&thread.id),
                                thread_id: thread.id.clone(),
                                input: reply_input.clone(),
                                input_height: px(64.0),
                                disabled: ui_state.reply_disabled,
                                submitting: ui_state.reply_submitting,
                                error: reply_error.clone(),
                                chrome: ReviewThreadReplyComposerChrome::Panel,
                                view_entity: composer_view.clone(),
                            },
                        ))
                        .child(div().pt_2().child(render_review_thread_actions(
                            ReviewThreadActionsState {
                                ids: ReviewThreadActionIds::overview(&thread.id),
                                thread_id: thread.id.clone(),
                                active_reply: ui_state.active_reply,
                                reply_button_disabled: ui_state.reply_button_disabled,
                                is_resolved: ui_state.is_resolved,
                                action_running: ui_state.action_running,
                                can_toggle_resolution: ui_state.can_toggle_resolution,
                                show_reply_button: false,
                                show_toggle_icon: false,
                                chrome: ReviewThreadActionsChrome::Inline,
                                view_entity: actions_view.clone(),
                            },
                        ))),
                )
            })
            .when_some(action_error, |element, error| {
                element.child(
                    div()
                        .px_3()
                        .pb_2()
                        .text_xs()
                        .text_color(color::danger())
                        .child(error),
                )
            })
            .into_any_element(),
        true,
        8.0,
    )
}

fn render_overview_thread_comment(
    comment: &harbor_domain::ReviewComment,
    index: usize,
    thread_id: &str,
    markdown: AnyElement,
    reaction_action: Option<&ReviewReactionAction>,
    reaction_error: Option<&ReviewCommentUiError>,
    view_entity: Entity<AppView>,
) -> AnyElement {
    let person = PullRequestPerson {
        login: comment.author.clone(),
        avatar_url: comment.author_avatar_url.clone(),
    };
    let time_label = natural_time_label_with_edit(comment.created_at, comment.updated_at);
    let time_tooltip = full_time_label_with_edit(comment.created_at, comment.updated_at);
    let selector = format!("overview-thread-comment-{thread_id}-{index}");
    let reaction_error = reaction_error
        .filter(|error| error.comment_id == comment.id)
        .map(|error| error.message.clone());

    div()
        .debug_selector(move || selector.clone())
        .w_full()
        .min_w_0()
        .flex()
        .items_start()
        .gap_2()
        .px_3()
        .py_3()
        .when(index > 0, |element| {
            element.border_t_1().border_color(color::border_subtle())
        })
        .child(render_person_avatar_with_size(&person, 24.0))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_xs()
                        .child(
                            div()
                                .font_semibold()
                                .text_color(color::text_primary())
                                .child(comment.author.clone()),
                        )
                        .child(render_timeline_time(
                            format!("overview-thread-comment-time-{}", comment.id),
                            time_label,
                            time_tooltip,
                        )),
                )
                .child(
                    div()
                        .pt_2()
                        .text_sm()
                        .text_color(color::text_secondary())
                        .child(markdown),
                )
                .child(render_review_reactions(
                    comment,
                    reaction_action,
                    view_entity,
                ))
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

fn overview_thread_expanded(state: ReviewThreadState, override_expanded: Option<bool>) -> bool {
    override_expanded.unwrap_or(state == ReviewThreadState::Unresolved)
}

fn overview_review_state(state: PullRequestReviewState) -> (&'static str, &'static str, Tone) {
    match state {
        PullRequestReviewState::Pending => ("started a review", "pending", Tone::Warning),
        PullRequestReviewState::Commented => ("reviewed changes", "commented", Tone::Info),
        PullRequestReviewState::Approved => ("approved changes", "approved", Tone::Success),
        PullRequestReviewState::ChangesRequested => {
            ("requested changes", "changes requested", Tone::Danger)
        }
        PullRequestReviewState::Dismissed => ("had a review dismissed", "dismissed", Tone::Neutral),
    }
}

fn render_timeline_message(message: &OverviewTimelineMessage) -> AnyElement {
    let (node, label, text_color) = match message {
        OverviewTimelineMessage::Loading => (
            Spinner::new().small().into_any_element(),
            "Loading activity...".to_string(),
            color::text_muted(),
        ),
        OverviewTimelineMessage::Empty => (
            Icon::new(Octicon::CommentDiscussion)
                .xsmall()
                .text_color(color::text_muted())
                .into_any_element(),
            "No comments or reviews yet".to_string(),
            color::text_muted(),
        ),
        OverviewTimelineMessage::Error(error) => (
            Icon::new(Octicon::Alert)
                .xsmall()
                .text_color(color::danger())
                .into_any_element(),
            error.clone(),
            color::danger(),
        ),
    };

    render_timeline_row(
        div()
            .size(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .border_1()
            .border_color(color::border())
            .bg(color::panel_background())
            .child(node)
            .into_any_element(),
        div()
            .w_full()
            .min_h(px(24.0))
            .flex()
            .items_center()
            .text_xs()
            .text_color(text_color)
            .child(label)
            .into_any_element(),
        true,
    )
}

fn render_timeline_row(node: AnyElement, content: AnyElement, show_tail: bool) -> AnyElement {
    render_timeline_row_with_node_offset(node, content, show_tail, 0.0)
}

fn render_timeline_row_with_node_offset(
    node: AnyElement,
    content: AnyElement,
    show_tail: bool,
    node_top: f32,
) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .items_stretch()
        .child(
            div()
                .relative()
                .w(px(36.0))
                .flex_none()
                .flex()
                .justify_center()
                .when(show_tail, |element| {
                    element.child(
                        div()
                            .absolute()
                            .top(px(12.0 + node_top))
                            .bottom(px(-12.0))
                            .left(px(17.5))
                            .w(px(1.0))
                            .bg(color::border()),
                    )
                })
                .child(div().pt(px(node_top)).child(node)),
        )
        .child(div().w_full().min_w_0().flex_1().pb_3().child(content))
        .into_any_element()
}

fn render_timeline_icon(icon: Octicon, tone: Tone) -> AnyElement {
    let colors = tone_colors(tone);

    div()
        .size(px(24.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_1()
        .border_color(color::border())
        .bg(colors.background)
        .child(Icon::new(icon).xsmall().text_color(colors.text))
        .into_any_element()
}

fn render_timeline_time(id: String, label: String, tooltip: String) -> impl IntoElement {
    div()
        .id(id)
        .text_color(color::text_muted())
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        .child(label)
}

fn render_overview_card(title: &'static str) -> Div {
    div()
        .rounded_sm()
        .border_1()
        .border_color(color::border())
        .bg(color::content_background())
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_semibold()
                .text_color(color::text_primary())
                .child(title),
        )
}

fn render_readiness_row(
    id: &'static str,
    label: &'static str,
    value: impl Into<String>,
    icon: Octicon,
    tone: Tone,
    divided: bool,
    navigable: bool,
) -> ListItem {
    let colors = tone_colors(tone);
    let value = value.into();

    ListItem::new(id)
        .w_full()
        .h(px(40.0))
        .px_0()
        .py_0()
        .rounded_none()
        .disabled(!navigable)
        .when(divided, |element| {
            element.border_t_1().border_color(color::border_subtle())
        })
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .child(Icon::new(icon).xsmall().text_color(colors.text))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(color::text_secondary())
                        .child(label),
                ),
        )
        .suffix(move |_, _| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .font_medium()
                .text_color(colors.text)
                .child(value.clone())
                .when(navigable, |element| {
                    element.child(
                        Icon::new(Octicon::ChevronRight)
                            .xsmall()
                            .text_color(color::text_muted()),
                    )
                })
        })
}

fn review_readiness(decision: Option<ReviewDecision>) -> (&'static str, Tone) {
    match decision {
        Some(ReviewDecision::Approved) => ("Approved", Tone::Success),
        Some(ReviewDecision::ChangesRequested) => ("Changes requested", Tone::Danger),
        Some(ReviewDecision::ReviewRequired) => ("Review required", Tone::Warning),
        None => ("Not reviewed", Tone::Info),
    }
}

fn merge_readiness(state: Option<MergeState>) -> (&'static str, Tone) {
    match state {
        Some(MergeState::Clean) => ("Ready", Tone::Success),
        Some(MergeState::Dirty) => ("Conflicts", Tone::Danger),
        Some(MergeState::Blocked) => ("Blocked", Tone::Danger),
        Some(MergeState::Behind) => ("Behind", Tone::Warning),
        Some(MergeState::Unknown) | None => ("Unknown", Tone::Neutral),
    }
}

fn render_empty_value(label: &'static str) -> AnyElement {
    div()
        .text_xs()
        .text_color(color::text_muted())
        .child(label)
        .into_any_element()
}

fn render_overview_section(title: &'static str, body: AnyElement) -> impl IntoElement {
    div()
        .w_full()
        .min_w_0()
        .pt_3()
        .border_t_1()
        .border_color(color::border_subtle())
        .child(
            div()
                .pb_1()
                .text_xs()
                .font_medium()
                .text_color(color::text_muted())
                .child(title),
        )
        .child(body)
}

fn render_people_row(people: &[PullRequestPerson]) -> AnyElement {
    render_wrapping_row(people.iter().map(render_person_chip).collect())
}

fn render_review_requests_row(
    reviewers: &[PullRequestPerson],
    teams: &[PullRequestTeam],
) -> AnyElement {
    let mut chips = Vec::with_capacity(reviewers.len() + teams.len());
    chips.extend(reviewers.iter().map(render_person_chip));
    chips.extend(teams.iter().map(render_team_chip));

    render_wrapping_row(chips)
}

fn render_labels_row(labels: &[Label]) -> AnyElement {
    render_wrapping_row(labels.iter().map(render_label_chip).collect())
}

fn render_wrapping_row(children: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_1()
        .min_w_0()
        .children(children)
        .into_any_element()
}

fn render_person_chip(person: &PullRequestPerson) -> AnyElement {
    let login = person.login.clone();
    let selector = format!("pull-request-person-{login}");
    render_chip()
        .debug_selector(move || selector.clone())
        .child(render_person_avatar(person))
        .child(render_chip_label(login))
        .into_any_element()
}

fn render_team_chip(team: &PullRequestTeam) -> AnyElement {
    let label = if team.name.trim().is_empty() {
        team.slug.clone()
    } else {
        team.name.clone()
    };

    render_chip()
        .child(render_team_avatar(&label))
        .child(render_chip_label(label))
        .into_any_element()
}

fn render_label_chip(label: &Label) -> AnyElement {
    let selector = format!("pull-request-label-{}", label.name);
    let swatch = label
        .color
        .as_deref()
        .and_then(parse_label_color)
        .unwrap_or_else(|| tone_colors(Tone::Neutral).text);

    render_chip()
        .debug_selector(move || selector.clone())
        .child(div().size(px(8.0)).flex_none().rounded_full().bg(swatch))
        .child(render_chip_label(label.name.clone()))
        .into_any_element()
}

fn render_chip_label(label: String) -> impl IntoElement {
    div().flex_none().max_w(px(188.0)).truncate().child(label)
}

fn render_chip() -> Div {
    div()
        .flex_none()
        .max_w(px(220.0))
        .flex()
        .items_center()
        .gap_1()
        .rounded_xs()
        .border_1()
        .border_color(color::border())
        .bg(color::panel_background())
        .px_1()
        .py_0p5()
        .text_xs()
        .text_color(color::text_secondary())
}

fn render_person_avatar(person: &PullRequestPerson) -> AnyElement {
    render_person_avatar_with_size(person, 16.0)
}

fn render_person_avatar_with_size(person: &PullRequestPerson, size: f32) -> AnyElement {
    let avatar_url = person
        .avatar_url
        .clone()
        .or_else(|| github_avatar_url_for_login(&person.login));

    if let Some(avatar_url) = avatar_url {
        return Avatar::new()
            .src(avatar_url)
            .name(person.login.clone())
            .with_size(px(size))
            .into_any_element();
    }

    render_fallback_avatar(&person.login, size).into_any_element()
}

fn render_team_avatar(label: &str) -> AnyElement {
    render_fallback_avatar(label, 16.0).into_any_element()
}

fn render_fallback_avatar(label: &str, size: f32) -> impl IntoElement {
    div()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .rounded_full()
        .border_1()
        .border_color(color::border_strong())
        .bg(color::row_selected_subtle())
        .text_size(px((size * 0.52).max(9.0)))
        .line_height(px(size))
        .font_semibold()
        .text_color(color::accent())
        .child(avatar_initial(label))
}

fn avatar_initial(label: &str) -> String {
    label
        .trim()
        .chars()
        .find(|character| character.is_alphanumeric())
        .map(|character| character.to_uppercase().collect())
        .unwrap_or_else(|| "?".to_string())
}

fn github_avatar_url_for_login(login: &str) -> Option<String> {
    let login = login.trim();

    if login.is_empty()
        || login.eq_ignore_ascii_case("ghost")
        || login.eq_ignore_ascii_case("you")
        || login.chars().any(char::is_whitespace)
    {
        None
    } else {
        Some(format!("https://github.com/{login}.png?size=48"))
    }
}

fn has_review_requests(pr: &PullRequest) -> bool {
    !pr.requested_reviewers.is_empty() || !pr.requested_teams.is_empty()
}

fn parse_label_color(color: &str) -> Option<Rgba> {
    let color = color.trim().trim_start_matches('#');
    if color.len() != 6 || !color.chars().all(|character| character.is_ascii_hexdigit()) {
        return None;
    }

    u32::from_str_radix(color, 16).ok().map(rgb)
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use gpui::{AppContext, ListAlignment, ListOffset, ListState, TestAppContext, px};
    use gpui_component::{Root, Theme, ThemeMode};
    use harbor_domain::{
        PullRequestComment, PullRequestReview, PullRequestReviewState, ReviewThreadState,
    };

    use super::{
        OverviewTimelineItem, avatar_initial, overview_panel_items, overview_review_visible,
        overview_thread_expanded, overview_thread_item_index, overview_timeline_items,
        parse_label_color, sync_overview_list_items,
    };
    use crate::test_fixtures::{review_thread, test_time};
    use crate::workspace::AppView;

    #[test]
    fn parses_github_label_colors() {
        assert!(parse_label_color("34d399").is_some());
        assert!(parse_label_color("#34d399").is_some());
        assert!(parse_label_color("bad").is_none());
        assert!(parse_label_color("zzzzzz").is_none());
    }

    #[test]
    fn derives_avatar_initials() {
        assert_eq!(avatar_initial("octocat"), "O");
        assert_eq!(avatar_initial(" team-reviewers"), "T");
        assert_eq!(avatar_initial(""), "?");
    }

    #[test]
    fn orders_timeline_activity_and_hides_pending_reviews() {
        let time = test_time();
        let comments = vec![PullRequestComment {
            id: "comment".to_string(),
            author: "octocat".to_string(),
            author_avatar_url: None,
            body: "comment".to_string(),
            created_at: time + Duration::minutes(2),
            updated_at: None,
        }];
        let reviews = vec![
            PullRequestReview {
                id: "submitted".to_string(),
                node_id: None,
                author: "reviewer".to_string(),
                state: PullRequestReviewState::Approved,
                body: None,
                submitted_at: Some(time + Duration::minutes(1)),
            },
            PullRequestReview {
                id: "pending".to_string(),
                node_id: None,
                author: "reviewer".to_string(),
                state: PullRequestReviewState::Pending,
                body: None,
                submitted_at: Some(time),
            },
            PullRequestReview {
                id: "empty-commented".to_string(),
                node_id: None,
                author: "reviewer".to_string(),
                state: PullRequestReviewState::Commented,
                body: None,
                submitted_at: Some(time),
            },
        ];
        let mut thread = review_thread(ReviewThreadState::Unresolved);
        thread.comments[0].created_at = time + Duration::minutes(3);
        let threads = vec![thread];

        let items = overview_timeline_items(&reviews, &comments, &threads);

        assert_eq!(items.len(), 3);
        assert!(matches!(items[0], OverviewTimelineItem::Review(_)));
        assert!(matches!(items[1], OverviewTimelineItem::Comment(_)));
        assert!(matches!(items[2], OverviewTimelineItem::Thread(_)));
    }

    #[test]
    fn keeps_only_meaningful_review_events() {
        let review = |state, body: Option<&str>| PullRequestReview {
            id: "review".to_string(),
            node_id: None,
            author: "reviewer".to_string(),
            state,
            body: body.map(str::to_string),
            submitted_at: Some(test_time()),
        };

        assert!(!overview_review_visible(&review(
            PullRequestReviewState::Pending,
            None
        )));
        assert!(!overview_review_visible(&review(
            PullRequestReviewState::Commented,
            None
        )));
        assert!(!overview_review_visible(&review(
            PullRequestReviewState::Commented,
            Some("  \n")
        )));
        assert!(overview_review_visible(&review(
            PullRequestReviewState::Commented,
            Some("Review summary")
        )));
        assert!(overview_review_visible(&review(
            PullRequestReviewState::Approved,
            None
        )));
        assert!(overview_review_visible(&review(
            PullRequestReviewState::ChangesRequested,
            None
        )));
    }

    #[test]
    fn unresolved_threads_start_expanded_and_completed_threads_start_collapsed() {
        assert!(overview_thread_expanded(
            ReviewThreadState::Unresolved,
            None
        ));
        assert!(!overview_thread_expanded(ReviewThreadState::Resolved, None));
        assert!(!overview_thread_expanded(ReviewThreadState::Outdated, None));
        assert!(overview_thread_expanded(
            ReviewThreadState::Resolved,
            Some(true)
        ));
        assert!(!overview_thread_expanded(
            ReviewThreadState::Unresolved,
            Some(false)
        ));
    }

    #[test]
    fn finds_thread_index_in_virtual_overview_items() {
        let thread = review_thread(ReviewThreadState::Unresolved);
        let items = overview_panel_items(&[], &[], &[thread], false, None);

        assert_eq!(overview_thread_item_index(&items, "thread-1"), Some(1));
        assert_eq!(overview_thread_item_index(&items, "missing"), None);
    }

    #[test]
    fn preserves_scroll_anchor_when_timeline_item_is_inserted_above() {
        let list_state = ListState::new(4, ListAlignment::Top, px(160.0));
        list_state.scroll_to(ListOffset {
            item_ix: 2,
            offset_in_item: px(18.0),
        });
        let mut previous_keys = vec![
            "description".to_string(),
            "comment:1".to_string(),
            "thread:1".to_string(),
            "composer".to_string(),
        ];
        let next_keys = vec![
            "description".to_string(),
            "review:1".to_string(),
            "comment:1".to_string(),
            "thread:1".to_string(),
            "composer".to_string(),
        ];

        sync_overview_list_items(&list_state, &mut previous_keys, next_keys.clone());

        assert_eq!(previous_keys, next_keys);
        assert_eq!(list_state.item_count(), 5);
        assert_eq!(list_state.logical_scroll_top().item_ix, 3);
        assert_eq!(list_state.logical_scroll_top().offset_in_item, px(18.0));
    }

    #[gpui::test]
    async fn overview_markdown_state_survives_virtual_row_recreation(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
        });

        let (_, _) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
            view.update(cx, |view, cx| {
                let first =
                    view.ensure_overview_markdown_state("comment:1".to_string(), "first body", cx);
                let reused =
                    view.ensure_overview_markdown_state("comment:1".to_string(), "first body", cx);
                let updated = view.ensure_overview_markdown_state(
                    "comment:1".to_string(),
                    "updated body",
                    cx,
                );

                assert_eq!(first.entity_id(), reused.entity_id());
                assert_eq!(first.entity_id(), updated.entity_id());
                assert_eq!(
                    view.overview_markdown_states["comment:1"].source,
                    "updated body"
                );
            });
            Root::new(view, window, cx)
        });
    }
}
