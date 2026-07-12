use gpui::{
    Anchor, AnyElement, Context, Div, Entity, IntoElement, Rgba, div, list, prelude::*, px, rgb,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt,
    avatar::Avatar,
    button::{Button, ButtonVariants},
    input::Input,
    list::ListItem,
    popover::Popover,
    scroll::ScrollableElement,
    spinner::Spinner,
};
use harbor_domain::{
    Label, MergeState, PullRequest, PullRequestPerson, PullRequestTeam, ReviewDecision,
    ReviewThread,
};

use crate::{
    actions::{PanelTab, PullRequestMetadataField},
    github::{avatar_initial, avatar_url},
    icons::Octicon,
    panels::{
        overview_markdown_body, render_review_markdown_state, review_markdown_body,
        review_thread_diff_preview,
    },
    visual::{Tone, color, tone_colors},
    workspace::{AppView, ReviewRuntimeState},
};

const OVERVIEW_SIDEBAR_WIDTH: f32 = 280.0;

#[path = "pull_request_overview/model.rs"]
mod model;
#[path = "pull_request_overview/timeline.rs"]
mod timeline;

use model::*;
use timeline::*;

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
                            .id("pull-request-overview-sidebar-scroll")
                            .debug_selector(|| "pull-request-overview-sidebar".to_string())
                            .w(px(OVERVIEW_SIDEBAR_WIDTH))
                            .h_full()
                            .min_h_0()
                            .flex_none()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .overflow_y_scroll()
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
        let (merge_label, merge_description, merge_tone) = merge_readiness(pr);
        let (status_label, status_description, status_tone) = pull_request_readiness(pr);
        let unresolved_tone = if pr.unresolved_threads == 0 {
            Tone::Success
        } else {
            Tone::Warning
        };
        let checks_tone = if pr.checks_summary.failed > 0 {
            Tone::Danger
        } else if pr.checks_summary.pending > 0 {
            Tone::Warning
        } else {
            Tone::Success
        };
        let checks_label = if pr.checks_summary.failed > 0 {
            format!("{} failed", pr.checks_summary.failed)
        } else if pr.checks_summary.pending > 0 {
            format!("{} pending", pr.checks_summary.pending)
        } else {
            format!("{} passed", pr.checks_summary.passed)
        };
        let checks_summary_title = if pr.checks_summary.failed > 0 {
            "Checks need attention"
        } else if pr.checks_summary.pending > 0 {
            "Checks running"
        } else {
            "Checks passed"
        };
        let (conflicts_label, conflicts_tone) = if pr.merge_state == Some(MergeState::Dirty) {
            ("Conflicts", Tone::Danger)
        } else {
            ("No conflicts", Tone::Success)
        };
        let pull_request_url = pr.url.clone();
        let close_pull_request_url = pr.url.clone();

        render_overview_card("PR status")
            .debug_selector(|| "pull-request-merge-readiness".to_string())
            .gap_0()
            .child(render_readiness_status(
                status_label,
                status_description,
                status_tone,
            ))
            .child(render_readiness_section_title("Readiness checklist"))
            .child(render_readiness_row(
                "pull-request-review-readiness-row",
                "Review",
                review_readiness_description(pr.review_decision),
                review_label,
                Octicon::Eye,
                review_tone,
                false,
            ))
            .child(render_readiness_row(
                "pull-request-merge-readiness-row",
                "Merge",
                merge_description,
                merge_label,
                Octicon::CheckCircle,
                merge_tone,
                false,
            ))
            .child(
                div()
                    .debug_selector(|| "pull-request-unresolved-conversations".to_string())
                    .child(
                        render_readiness_row(
                            "pull-request-unresolved-conversations-row",
                            "Conversations",
                            "Resolve open threads",
                            format!("{} open", pr.unresolved_threads),
                            Octicon::CommentDiscussion,
                            unresolved_tone,
                            true,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.select_panel_tab(PanelTab::Review, cx);
                        })),
                    ),
            )
            .child(render_readiness_section_title("Summary"))
            .child(render_summary_row(
                "pull-request-checks-summary-row",
                checks_summary_title,
                checks_label,
                checks_tone,
            ))
            .child(render_summary_row(
                "pull-request-conflicts-summary-row",
                conflicts_label,
                if conflicts_tone == Tone::Success {
                    "Up to date"
                } else {
                    "Resolve to merge"
                },
                conflicts_tone,
            ))
            .child(
                div()
                    .pt_2()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        Button::new("pull-request-draft-action")
                            .label(if pr.is_draft {
                                "Mark ready for review"
                            } else {
                                "Convert to draft"
                            })
                            .xsmall()
                            .link()
                            .on_click(move |_, _, cx| cx.open_url(&pull_request_url)),
                    )
                    .child(
                        Button::new("close-pull-request-action")
                            .label("Close pull request")
                            .xsmall()
                            .link()
                            .on_click(move |_, _, cx| cx.open_url(&close_pull_request_url)),
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
            .debug_selector(|| "pull-request-people-card".to_string())
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
        let query = input.read(cx).value().trim().to_lowercase();
        let input_is_empty = query.is_empty();
        let selected_pull_request = self.selected_pull_request();
        let mut choices: Vec<(String, Option<String>, Option<String>)> = match field {
            PullRequestMetadataField::Reviewer => {
                self.pull_request_metadata_options
                    .options
                    .reviewers
                    .iter()
                    .filter(|person| {
                        selected_pull_request.is_none_or(|pull_request| {
                            !person.login.eq_ignore_ascii_case(&pull_request.author)
                                && !pull_request.requested_reviewers.iter().any(|reviewer| {
                                    reviewer.login.eq_ignore_ascii_case(&person.login)
                                })
                        })
                    })
                    .map(|person| (person.login.clone(), person.avatar_url.clone(), None))
                    .collect()
            }
            PullRequestMetadataField::Assignee => self
                .pull_request_metadata_options
                .options
                .assignees
                .iter()
                .filter(|person| {
                    selected_pull_request.is_none_or(|pull_request| {
                        !pull_request
                            .assignees
                            .iter()
                            .any(|assignee| assignee.login.eq_ignore_ascii_case(&person.login))
                    })
                })
                .map(|person| (person.login.clone(), person.avatar_url.clone(), None))
                .collect(),
            PullRequestMetadataField::Label => self
                .pull_request_metadata_options
                .options
                .labels
                .iter()
                .filter(|label| {
                    selected_pull_request.is_none_or(|pull_request| {
                        !pull_request
                            .labels
                            .iter()
                            .any(|existing| existing.name.eq_ignore_ascii_case(&label.name))
                    })
                })
                .map(|label| (label.name.clone(), None, label.color.clone()))
                .collect(),
        };
        if !query.is_empty() {
            choices.retain(|(name, _, _)| name.to_lowercase().contains(&query));
        }
        choices.truncate(20);
        let choices_loading = self.pull_request_metadata_options.loading;
        let choices_error = self.pull_request_metadata_options.error.clone();
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
                        let view = view.clone();
                        move |open, window, cx| {
                            if *open {
                                input.update(cx, |input, cx| input.focus(window, cx));
                                view.update(cx, |view, cx| {
                                    view.load_pull_request_metadata_options(window, cx);
                                });
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
                        let mut content = div()
                            .w(px(280.0))
                            .border_1()
                            .border_color(color::border_strong())
                            .bg(color::elevated_background())
                            .shadow_lg()
                            .p_2()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(Input::new(&input).small().cleanable(true));
                        if choices_loading {
                            content = content.child(
                                div()
                                    .px_2()
                                    .py_2()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_xs()
                                    .text_color(color::text_muted())
                                    .child(Spinner::new().small())
                                    .child("Loading choices..."),
                            );
                        } else if let Some(choices_error) = choices_error.clone() {
                            content = content.child(
                                div()
                                    .px_2()
                                    .text_xs()
                                    .text_color(color::danger())
                                    .child(choices_error),
                            );
                        } else if choices.is_empty() {
                            content = content.child(
                                div()
                                    .px_2()
                                    .py_2()
                                    .text_xs()
                                    .text_color(color::text_muted())
                                    .child(if input_is_empty {
                                        "No available choices"
                                    } else {
                                        "No matching choices"
                                    }),
                            );
                        } else {
                            content = content.child(
                                div().max_h(px(240.0)).overflow_y_scrollbar().children(
                                    choices.iter().enumerate().map(
                                        |(index, (name, avatar_url, label_color))| {
                                            let name = name.clone();
                                            let selected_name = name.clone();
                                            let input = input.clone();
                                            let view = view.clone();
                                            div()
                                                .id(format!("metadata-{field_name}-choice-{index}"))
                                                .px_2()
                                                .py_1()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .rounded_sm()
                                                .cursor_pointer()
                                                .hover(|element| element.bg(color::row_hover()))
                                                .when_some(avatar_url.clone(), |element, url| {
                                                    element.child(
                                                        Avatar::new().src(url).size(px(20.0)),
                                                    )
                                                })
                                                .when_some(
                                                    label_color
                                                        .as_deref()
                                                        .and_then(parse_label_color),
                                                    |element, color| {
                                                        element.child(
                                                            div().size_3().rounded_full().bg(color),
                                                        )
                                                    },
                                                )
                                                .child(
                                                    div()
                                                        .min_w_0()
                                                        .truncate()
                                                        .text_sm()
                                                        .child(name),
                                                )
                                                .on_click(move |_, window, cx| {
                                                    input.update(cx, |input, cx| {
                                                        input.set_value(&selected_name, window, cx);
                                                    });
                                                    view.update(cx, |view, cx| {
                                                        view.add_pull_request_metadata(
                                                            field, window, cx,
                                                        );
                                                    });
                                                })
                                        },
                                    ),
                                ),
                            );
                        }
                        content
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
    description: &'static str,
    value: impl Into<String>,
    icon: Octicon,
    tone: Tone,
    navigable: bool,
) -> ListItem {
    let colors = tone_colors(tone);
    let value = value.into();

    ListItem::new(id)
        .w_full()
        .h(px(52.0))
        .px_0()
        .py_0()
        .rounded_none()
        .disabled(!navigable)
        .when(label != "Review", |element| {
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
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .text_color(color::text_primary())
                                .child(label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(color::text_muted())
                                .child(description),
                        ),
                ),
        )
        .suffix(move |_, _| {
            div()
                .flex()
                .items_center()
                .text_xs()
                .font_medium()
                .text_color(colors.text)
                .child(value.clone())
        })
}

fn render_readiness_status(label: &'static str, description: &'static str, tone: Tone) -> Div {
    let colors = tone_colors(tone);

    div()
        .py_3()
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .size(px(44.0))
                .flex_none()
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(colors.background)
                .child(
                    Icon::new(Octicon::CodeSquare)
                        .size(px(16.0))
                        .text_color(colors.text),
                ),
        )
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_medium()
                        .text_color(colors.text)
                        .child(label),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child(description),
                ),
        )
}

fn render_readiness_section_title(title: &'static str) -> Div {
    div()
        .mt_2()
        .pt_3()
        .pb_1()
        .border_t_1()
        .border_color(color::border_subtle())
        .text_sm()
        .font_semibold()
        .text_color(color::text_primary())
        .child(title)
}

fn render_summary_row(
    id: &'static str,
    label: &'static str,
    value: impl Into<String>,
    tone: Tone,
) -> impl IntoElement {
    let colors = tone_colors(tone);

    div()
        .id(id)
        .h(px(36.0))
        .flex()
        .items_center()
        .gap_2()
        .child(
            Icon::new(Octicon::CheckCircle)
                .xsmall()
                .text_color(colors.text),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_xs()
                .text_color(color::text_secondary())
                .child(label),
        )
        .child(
            div()
                .text_xs()
                .text_color(color::text_muted())
                .child(value.into()),
        )
}

fn pull_request_readiness(pr: &PullRequest) -> (&'static str, &'static str, Tone) {
    if pr.merge_state == Some(MergeState::Dirty) {
        ("Conflicts", "Resolve conflicts to merge.", Tone::Danger)
    } else if pr.checks_summary.failed > 0 {
        ("Checks failed", "Fix failing checks.", Tone::Danger)
    } else if pr.checks_summary.pending > 0 {
        ("Checks pending", "Waiting for checks.", Tone::Warning)
    } else if pr.is_draft {
        ("Draft", "Not ready for review.", Tone::Neutral)
    } else if pr.review_decision == Some(ReviewDecision::ChangesRequested) {
        (
            "Changes requested",
            "Address review feedback.",
            Tone::Danger,
        )
    } else if pr.review_decision != Some(ReviewDecision::Approved) {
        (
            "Review required",
            "Approval needed to merge.",
            Tone::Warning,
        )
    } else if pr.unresolved_threads > 0 {
        (
            "Conversations open",
            "Resolve threads to merge.",
            Tone::Warning,
        )
    } else {
        ("Ready", "Ready to merge.", Tone::Success)
    }
}

fn review_readiness_description(decision: Option<ReviewDecision>) -> &'static str {
    match decision {
        Some(ReviewDecision::Approved) => "Approvals received",
        Some(ReviewDecision::ChangesRequested) => "Changes were requested",
        Some(ReviewDecision::ReviewRequired) | None => "1 approval required",
    }
}

fn review_readiness(decision: Option<ReviewDecision>) -> (&'static str, Tone) {
    match decision {
        Some(ReviewDecision::Approved) => ("Approved", Tone::Success),
        Some(ReviewDecision::ChangesRequested) => ("Changes requested", Tone::Danger),
        Some(ReviewDecision::ReviewRequired) | None => ("Pending", Tone::Warning),
    }
}

fn merge_readiness(pr: &PullRequest) -> (&'static str, &'static str, Tone) {
    match pr.merge_state {
        Some(MergeState::Dirty) => ("Conflicts", "Resolve merge conflicts", Tone::Danger),
        Some(MergeState::Blocked) => ("Blocked", "Requirements not met", Tone::Danger),
        Some(MergeState::Behind) => ("Behind", "Update branch", Tone::Warning),
        Some(MergeState::Unknown) | None => ("Unknown", "Status unavailable", Tone::Neutral),
        Some(MergeState::Clean) if pr.review_decision != Some(ReviewDecision::Approved) => {
            ("Blocked", "Waiting for approval", Tone::Warning)
        }
        Some(MergeState::Clean) if pr.unresolved_threads > 0 => {
            ("Blocked", "Resolve open threads", Tone::Warning)
        }
        Some(MergeState::Clean)
            if pr.checks_summary.failed > 0 || pr.checks_summary.pending > 0 =>
        {
            ("Blocked", "Waiting for checks", Tone::Warning)
        }
        Some(MergeState::Clean) => ("Ready", "Requirements met", Tone::Success),
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
        .or_else(|| avatar_url(&person.login));

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
        MergeState, PullRequestComment, PullRequestReview, PullRequestReviewState, ReviewDecision,
        ReviewThreadState,
    };

    use super::{
        OverviewTimelineItem, merge_readiness, overview_panel_items, overview_review_visible,
        overview_thread_expanded, overview_thread_item_index, overview_timeline_items,
        parse_label_color, pull_request_readiness, sync_overview_list_items,
    };
    use crate::test_fixtures::{pull_request, review_thread, test_time};
    use crate::visual::Tone;
    use crate::workspace::AppView;

    #[test]
    fn parses_github_label_colors() {
        assert!(parse_label_color("34d399").is_some());
        assert!(parse_label_color("#34d399").is_some());
        assert!(parse_label_color("bad").is_none());
        assert!(parse_label_color("zzzzzz").is_none());
    }

    #[test]
    fn clean_merge_state_still_requires_approval_and_resolved_conversations() {
        let mut pull_request = pull_request();
        pull_request.merge_state = Some(MergeState::Clean);
        pull_request.review_decision = None;
        pull_request.unresolved_threads = 5;

        assert_eq!(merge_readiness(&pull_request).0, "Blocked");
        assert_eq!(pull_request_readiness(&pull_request).0, "Review required");

        pull_request.review_decision = Some(ReviewDecision::Approved);
        assert_eq!(merge_readiness(&pull_request).0, "Blocked");
        assert_eq!(
            pull_request_readiness(&pull_request),
            (
                "Conversations open",
                "Resolve threads to merge.",
                Tone::Warning,
            )
        );

        pull_request.unresolved_threads = 0;
        assert_eq!(merge_readiness(&pull_request).0, "Ready");
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
