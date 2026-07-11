use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use gpui::{AnyElement, Entity, IntoElement, SharedString, div, prelude::*, px};
use gpui_component::{
    Disableable, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::InputState,
    list::ListItem,
    spinner::Spinner,
    tooltip::Tooltip,
};
use harbor_domain::{
    PullRequestComment, PullRequestPerson, PullRequestReview, PullRequestReviewState, ReviewThread,
    ReviewThreadState,
};

use crate::{
    date_time::{
        full_time_label, full_time_label_with_edit, natural_time_label,
        natural_time_label_with_edit,
    },
    icons::Octicon,
    panels::{
        ReviewCommentActionsMenuState, ReviewDiffPreview, render_review_comment_actions_menu,
        render_review_comment_edit_composer, render_review_diff_preview, render_review_reactions,
        render_status_pill, review_comment_ui_state,
        review_thread_chrome::{
            ReviewThreadReplyComposerChrome, ReviewThreadReplyComposerIds,
            ReviewThreadReplyComposerState, render_review_thread_reply_composer,
            review_thread_ui_state,
        },
    },
    visual::{Tone, color, tone_colors},
    workspace::{AppView, ReviewCommentUiError, ReviewReactionAction, ReviewThreadUiError},
};

use super::render_person_avatar_with_size;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum OverviewPanelItem {
    Description,
    Comment { id: String },
    Review { id: String },
    Thread { id: String },
    Message(OverviewTimelineMessage),
    Composer,
}

impl OverviewPanelItem {
    pub(super) fn key(&self) -> String {
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

pub(super) fn sync_overview_list_items(
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

pub(super) fn overview_thread_item_index(
    items: &[OverviewPanelItem],
    thread_id: &str,
) -> Option<usize> {
    items
        .iter()
        .position(|item| matches!(item, OverviewPanelItem::Thread { id } if id == thread_id))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum OverviewTimelineMessage {
    Loading,
    Empty,
    Error(String),
}

pub(super) fn overview_panel_items(
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
pub(super) enum OverviewTimelineItem<'a> {
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

pub(super) fn overview_timeline_items<'a>(
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

pub(super) fn overview_review_visible(review: &PullRequestReview) -> bool {
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

fn compare_timeline_times(
    left: Option<DateTime<Utc>>,
    right: Option<DateTime<Utc>>,
) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

pub(super) fn render_overview_comment_event(
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

pub(super) fn render_overview_review_event(
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

pub(super) struct OverviewThreadRenderState<'a> {
    pub(super) thread: &'a ReviewThread,
    pub(super) index: usize,
    pub(super) expanded: bool,
    pub(super) active_review_thread_reply: Option<&'a str>,
    pub(super) reply_input: Entity<InputState>,
    pub(super) reply_body_empty: bool,
    pub(super) is_submitting_reply: bool,
    pub(super) reply_error: Option<&'a ReviewThreadUiError>,
    pub(super) action_thread_id: Option<&'a str>,
    pub(super) action_error: Option<&'a ReviewThreadUiError>,
    pub(super) diff_preview: Option<ReviewDiffPreview>,
    pub(super) mono_font_family: SharedString,
    pub(super) comments: Vec<AnyElement>,
    pub(super) view_entity: Entity<AppView>,
}

pub(super) fn render_overview_thread(state: OverviewThreadRenderState<'_>) -> AnyElement {
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
    let composer_view = view_entity.clone();
    let toggle_view = view_entity.clone();
    let toggle_thread_id = thread_id.clone();
    let resolution_view = view_entity.clone();
    let resolution_thread_id = thread_id.clone();

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
                                    .when(ui_state.can_toggle_resolution, |element| {
                                        let button = Button::new(format!(
                                            "overview-toggle-thread-{resolution_thread_id}"
                                        ))
                                        .label(if ui_state.is_resolved {
                                            "Reopen"
                                        } else {
                                            "Resolve"
                                        })
                                        .xsmall()
                                        .loading(ui_state.action_running)
                                        .disabled(ui_state.action_running);
                                        let button = if ui_state.is_resolved {
                                            button.success()
                                        } else {
                                            button.warning()
                                        };

                                        element.child(button.on_click({
                                            let resolution_view = resolution_view.clone();
                                            let resolution_thread_id = resolution_thread_id.clone();
                                            move |_, _, cx| {
                                                cx.stop_propagation();
                                                resolution_view.update(cx, |view, cx| {
                                                    view.set_review_thread_resolved(
                                                        resolution_thread_id.clone(),
                                                        !ui_state.is_resolved,
                                                        cx,
                                                    );
                                                });
                                            }
                                        }))
                                    })
                                    .when(!ui_state.can_toggle_resolution, |element| {
                                        element.child(render_status_pill(status, tone))
                                    })
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
                        ),
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
                        )),
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

pub(super) struct OverviewThreadCommentRenderState<'a> {
    pub(super) comment: &'a harbor_domain::ReviewComment,
    pub(super) index: usize,
    pub(super) thread_id: &'a str,
    pub(super) markdown: AnyElement,
    pub(super) reaction_action: Option<&'a ReviewReactionAction>,
    pub(super) reaction_error: Option<&'a ReviewCommentUiError>,
    pub(super) active_comment_edit: Option<&'a str>,
    pub(super) comment_edit_input: Entity<InputState>,
    pub(super) edit_body_empty: bool,
    pub(super) is_submitting_edit: bool,
    pub(super) edit_error: Option<&'a ReviewCommentUiError>,
    pub(super) action_comment_id: Option<&'a str>,
    pub(super) action_error: Option<&'a ReviewCommentUiError>,
    pub(super) view_entity: Entity<AppView>,
}

pub(super) fn render_overview_thread_comment(
    state: OverviewThreadCommentRenderState<'_>,
) -> AnyElement {
    let OverviewThreadCommentRenderState {
        comment,
        index,
        thread_id,
        markdown,
        reaction_action,
        reaction_error,
        active_comment_edit,
        comment_edit_input,
        edit_body_empty,
        is_submitting_edit,
        edit_error,
        action_comment_id,
        action_error,
        view_entity,
    } = state;
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
    let edit_error = edit_error
        .filter(|error| error.comment_id == comment.id)
        .map(|error| error.message.clone());
    let action_error = action_error
        .filter(|error| error.comment_id == comment.id)
        .map(|error| error.message.clone());
    let ui_state = review_comment_ui_state(
        comment,
        active_comment_edit,
        is_submitting_edit,
        action_comment_id,
    );

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
                        .justify_between()
                        .gap_1()
                        .text_xs()
                        .child(
                            div()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap_1()
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
                        .child(render_review_comment_actions_menu(
                            ReviewCommentActionsMenuState {
                                comment_id: comment.id.clone(),
                                thread_id: thread_id.to_string(),
                                comment_body: comment.body.clone(),
                                comment_url: comment.url.clone(),
                                can_update: ui_state.can_update,
                                can_delete: ui_state.can_delete,
                                active_edit: ui_state.active_edit,
                                edit_submitting: ui_state.edit_submitting,
                                action_running: ui_state.action_running,
                                view_entity: view_entity.clone(),
                            },
                        )),
                )
                .when(!ui_state.active_edit, |element| {
                    element.child(
                        div()
                            .pt_2()
                            .text_sm()
                            .text_color(color::text_secondary())
                            .child(markdown),
                    )
                })
                .when(ui_state.active_edit, |element| {
                    element.child(render_review_comment_edit_composer(
                        comment.id.clone(),
                        comment_edit_input,
                        edit_body_empty,
                        ui_state.edit_submitting,
                        edit_error,
                        view_entity.clone(),
                    ))
                })
                .when_some(action_error, |element, error| {
                    element.child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(color::danger())
                            .child(error),
                    )
                })
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

pub(super) fn overview_thread_expanded(
    state: ReviewThreadState,
    override_expanded: Option<bool>,
) -> bool {
    override_expanded.unwrap_or(state == ReviewThreadState::Unresolved)
}

fn overview_review_state(
    state: PullRequestReviewState,
) -> (&'static str, &'static str, Tone) {
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

pub(super) fn render_timeline_message(message: &OverviewTimelineMessage) -> AnyElement {
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

pub(super) fn render_timeline_row(
    node: AnyElement,
    content: AnyElement,
    show_tail: bool,
) -> AnyElement {
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
