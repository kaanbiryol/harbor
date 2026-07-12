use gpui::{AnyElement, Context, IntoElement, ListState, div, list, prelude::*, px};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt, avatar::Avatar, spinner::Spinner, tooltip::Tooltip,
};
use harbor_domain::{
    PullRequestComment, PullRequestReview, PullRequestReviewState, ReviewComment, ReviewThread,
    ReviewThreadState,
};

use crate::{
    date_time::{
        full_time_label, full_time_label_with_edit, natural_time_label,
        natural_time_label_with_edit,
    },
    github::{avatar_initial, avatar_url as github_avatar_url, profile_url},
    icons::Octicon,
    visual::{Tone, color, leading_truncated_path, tone_text},
    workspace::AppView,
};

use super::review_markdown::{render_review_markdown_body, review_markdown_body};
use super::review_thread_rows::{ReviewThreadRowRenderState, render_review_thread_row};
use super::{
    render_empty_panel_card, render_error_panel_card, render_metric_pill, render_status_pill,
    sync_virtual_list_item_count,
};

#[path = "review/diff_preview.rs"]
mod diff_preview;
#[path = "review/model.rs"]
mod model;

#[cfg(test)]
use diff_preview::ReviewDiffPreviewLine;
pub(crate) use diff_preview::{
    ReviewDiffPreview, render_review_diff_preview, review_thread_diff_preview,
};

#[cfg(test)]
use model::{ReviewConversationItemKind, review_conversation_items};
use model::{ReviewPanelItem, ReviewPanelSection, review_content_item_count, review_panel_items};

impl ReviewPanelSection {
    fn id(self) -> &'static str {
        match self {
            Self::NeedsAttention => "review-section-needs-attention",
            Self::Conversation => "review-section-conversation",
            Self::Resolved => "review-section-resolved",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::NeedsAttention => "Needs attention",
            Self::Conversation => "Conversation & activity",
            Self::Resolved => "Resolved & outdated",
        }
    }

    fn tone(self) -> Tone {
        match self {
            Self::NeedsAttention => Tone::Warning,
            Self::Conversation => Tone::Info,
            Self::Resolved => Tone::Neutral,
        }
    }

    fn count_label(self, item_count: usize) -> String {
        let noun = match self {
            Self::NeedsAttention | Self::Resolved if item_count == 1 => "thread",
            Self::NeedsAttention | Self::Resolved => "threads",
            Self::Conversation if item_count == 1 => "item",
            Self::Conversation => "items",
        };

        format!("{item_count} {noun}")
    }
}

pub(crate) struct ReviewPanelRenderInput<'a> {
    pub(crate) reviews: &'a [PullRequestReview],
    pub(crate) comments: &'a [PullRequestComment],
    pub(crate) threads: &'a [ReviewThread],
    pub(crate) is_loading: bool,
    pub(crate) error: Option<&'a str>,
    pub(crate) review_list_state: ListState,
}

pub(crate) fn render_review_panel(
    input: ReviewPanelRenderInput<'_>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let ReviewPanelRenderInput {
        reviews,
        comments,
        threads,
        is_loading,
        error,
        review_list_state,
    } = input;
    let (unresolved, resolved, outdated) = review_thread_counts(threads);
    let view_entity = cx.entity().clone();
    let review_items = review_panel_items(reviews, threads, comments);
    let review_item_count = review_content_item_count(&review_items);
    let has_review_items = !review_items.is_empty();
    sync_virtual_list_item_count(&review_list_state, review_items.len());
    let review_items_for_render = review_items.clone();

    div()
        .image_cache(gpui::retain_all("review-timeline-avatar-cache"))
        .id("review-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_2()
        .child(render_review_panel_header(review_item_count, is_loading))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .flex_wrap()
                .child(render_metric_pill("unresolved", unresolved, Tone::Warning))
                .child(render_metric_pill("resolved", resolved, Tone::Success))
                .child(render_metric_pill("outdated", outdated, Tone::Neutral)),
        )
        .when(is_loading && !has_review_items, |element| {
            element.child(render_empty_panel_card("Loading review comments..."))
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(render_error_panel_card(error))
        })
        .when(
            !is_loading && error.is_none() && !has_review_items,
            |element| {
                element.child(render_empty_panel_card(
                    "No review comments found for this pull request",
                ))
            },
        )
        .when(has_review_items, |element| {
            element.child(
                list(
                    review_list_state,
                    cx.processor(move |view, index: usize, _window, cx| {
                        let Some(item) = review_items_for_render.get(index) else {
                            return div().into_any_element();
                        };

                        match item {
                            ReviewPanelItem::Section {
                                section,
                                item_count,
                            } => render_review_section_header(*section, *item_count)
                                .into_any_element(),
                            ReviewPanelItem::FileHeader { path, thread_count } => {
                                render_review_file_header(path, *thread_count).into_any_element()
                            }
                            ReviewPanelItem::Thread { thread_id } => {
                                let Some(thread) = view
                                    .review_state
                                    .review_threads()
                                    .iter()
                                    .find(|thread| thread.id == *thread_id)
                                else {
                                    return div().into_any_element();
                                };
                                let review_thread_reply_input = view
                                    .review_state
                                    .review_composer_state
                                    .thread_reply_input
                                    .clone();
                                let reply_body_empty =
                                    review_thread_reply_input.read(cx).value().trim().is_empty();
                                let diff_preview = review_thread_diff_preview(
                                    thread,
                                    view.detail_state.files(),
                                    view.detail_state.diffs(),
                                );

                                render_review_thread_row(ReviewThreadRowRenderState {
                                    index,
                                    thread,
                                    active_review_thread_reply: view
                                        .review_state
                                        .review_composer_state
                                        .active_thread_reply(),
                                    review_thread_reply_input,
                                    reply_body_empty,
                                    is_submitting_reply: view
                                        .review_state
                                        .is_submitting_review_thread_reply(),
                                    reply_error: view.review_state.review_thread_reply_error(),
                                    action_thread_id: view
                                        .review_state
                                        .review_thread_action_thread_id(),
                                    action_error: view.review_state.review_thread_action_error(),
                                    diff_preview,
                                    mono_font_family: cx.theme().mono_font_family.clone(),
                                    view_entity: view_entity.clone(),
                                })
                            }
                            ReviewPanelItem::Review { review_id } => view
                                .review_state
                                .pull_request_reviews()
                                .iter()
                                .find(|review| review.id == *review_id)
                                .map(|review| render_pull_request_review_row(review, index))
                                .unwrap_or_else(|| div().into_any_element()),
                            ReviewPanelItem::Comment { comment_id } => view
                                .review_state
                                .pull_request_comments()
                                .iter()
                                .find(|comment| comment.id == *comment_id)
                                .map(|comment| render_pull_request_comment_row(comment, index))
                                .unwrap_or_else(|| div().into_any_element()),
                        }
                    }),
                )
                .flex_1()
                .min_h_0()
                .min_w_0(),
            )
        })
}

fn render_review_panel_header(review_item_count: usize, is_loading: bool) -> impl IntoElement {
    let item_label = if review_item_count == 1 {
        "1 review item".to_string()
    } else {
        format!("{review_item_count} review items")
    };

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .font_medium()
                .text_color(color::text_primary())
                .child("Review"),
        )
        .child(
            div()
                .flex_none()
                .max_w(px(280.0))
                .min_w_0()
                .flex()
                .items_center()
                .justify_end()
                .gap_1()
                .text_xs()
                .text_color(color::text_muted())
                .when(is_loading, |element| element.child(Spinner::new().small()))
                .child(div().min_w_0().truncate().child(item_label)),
        )
}

fn render_review_section_header(
    section: ReviewPanelSection,
    item_count: usize,
) -> impl IntoElement {
    div()
        .id(section.id())
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .pt_3()
        .pb_1()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .font_medium()
                .text_color(color::text_primary())
                .child(section.title()),
        )
        .child(render_status_pill(
            section.count_label(item_count),
            section.tone(),
        ))
}

fn render_review_file_header(path: &str, thread_count: usize) -> impl IntoElement {
    let thread_label = if thread_count == 1 {
        "1 thread".to_string()
    } else {
        format!("{thread_count} threads")
    };

    div()
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_1()
        .pt_2()
        .pb_1()
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
                        .text_color(color::text_secondary())
                        .child(leading_truncated_path(path, 72)),
                ),
        )
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(color::text_muted())
                .child(thread_label),
        )
}

fn render_pull_request_comment_row(comment: &PullRequestComment, index: usize) -> AnyElement {
    let time_label = pull_request_comment_time_label(comment);
    let time_tooltip = pull_request_comment_time_tooltip(comment);

    div()
        .id(("pull-request-comment-row", index))
        .w_full()
        .min_w_0()
        .flex_initial()
        .py_1()
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .border_1()
                .border_color(color::border())
                .bg(color::app_background())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(color::border_subtle())
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(render_review_avatar(
                                    &comment.author,
                                    comment.author_avatar_url.as_deref(),
                                    24.0,
                                ))
                                .child(
                                    div()
                                        .min_w_0()
                                        .flex_1()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(render_review_author_link(
                                            format!(
                                                "pull-request-comment-author-link-{}",
                                                comment.id
                                            ),
                                            comment.author.clone(),
                                            color::text_primary(),
                                        ))
                                        .child(render_time_metadata(
                                            format!("pull-request-comment-time-{}", comment.id),
                                            time_label,
                                            Some(time_tooltip),
                                            color::text_muted(),
                                        )),
                                ),
                        )
                        .child(render_status_pill("commented", Tone::Info)),
                )
                .child(
                    div()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(color::text_secondary())
                        .child(render_review_markdown_body(
                            format!("pull-request-comment-body-{}", comment.id),
                            &comment.body,
                        )),
                ),
        )
        .into_any_element()
}

fn render_pull_request_review_row(review: &PullRequestReview, index: usize) -> AnyElement {
    let (state_label, state_tone) = pull_request_review_state_tone(review.state);
    let body = review
        .body
        .as_deref()
        .map(comment_body_text)
        .unwrap_or_else(|| format!("{} review", state_label));
    let review_time_label = review_time_label(review);
    let time_label = if review.state == PullRequestReviewState::Commented {
        review_time_label
    } else {
        format!("{} {}", state_label, review_time_label)
    };
    let time_tooltip = review_time_tooltip(review);

    div()
        .id(("review-summary-row", index))
        .w_full()
        .min_w_0()
        .flex_initial()
        .py_1()
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .border_1()
                .border_color(color::border())
                .bg(color::app_background())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(color::border_subtle())
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(render_review_avatar(&review.author, None, 24.0))
                                .child(
                                    div()
                                        .min_w_0()
                                        .flex_1()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(render_review_author_link(
                                            format!(
                                                "pull-request-review-author-link-{}",
                                                review.id
                                            ),
                                            review.author.clone(),
                                            color::text_primary(),
                                        ))
                                        .child(render_time_metadata(
                                            format!("pull-request-review-time-{}", review.id),
                                            time_label,
                                            time_tooltip,
                                            color::text_muted(),
                                        )),
                                ),
                        )
                        .child(render_status_pill(state_label, state_tone)),
                )
                .child(
                    div()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(color::text_secondary())
                        .child(render_review_markdown_body(
                            format!("pull-request-review-body-{}", review.id),
                            &body,
                        )),
                ),
        )
        .into_any_element()
}

fn comment_body_text(body: &str) -> String {
    review_markdown_body(body)
}

pub(super) fn render_review_avatar(
    author: &str,
    avatar_url: Option<&str>,
    size: f32,
) -> AnyElement {
    if let Some(avatar_url) = avatar_url
        .map(str::to_string)
        .or_else(|| github_avatar_url(author))
    {
        return Avatar::new()
            .src(avatar_url)
            .name(author.to_string())
            .with_size(px(size))
            .into_any_element();
    }

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
        .text_size(px((size * 0.52).max(10.0)))
        .line_height(px(size))
        .font_semibold()
        .text_color(color::accent())
        .child(avatar_initial(author))
        .into_any_element()
}

pub(super) fn render_review_author_link(
    id: String,
    author: String,
    text_color: gpui::Rgba,
) -> impl IntoElement {
    let profile_url = profile_url(&author);

    div()
        .id(id)
        .font_medium()
        .text_color(text_color)
        .cursor_pointer()
        .hover(|element| element.text_color(color::accent_hover()))
        .on_click(move |_, _, cx| {
            cx.open_url(&profile_url);
        })
        .child(author)
}

pub(crate) fn review_thread_counts(threads: &[ReviewThread]) -> (usize, usize, usize) {
    let mut unresolved = 0;
    let mut resolved = 0;
    let mut outdated = 0;

    for thread in threads {
        match thread.state {
            ReviewThreadState::Unresolved => unresolved += 1,
            ReviewThreadState::Resolved => resolved += 1,
            ReviewThreadState::Outdated => outdated += 1,
        }
    }

    (unresolved, resolved, outdated)
}

pub(crate) fn review_thread_location(thread: &ReviewThread) -> String {
    thread
        .comments
        .iter()
        .find_map(|comment| comment.position.as_ref())
        .and_then(|position| position.line.or(position.original_line))
        .map_or_else(|| "file".to_string(), |line| format!("line {line}"))
}

fn pull_request_review_state_tone(state: PullRequestReviewState) -> (&'static str, Tone) {
    match state {
        PullRequestReviewState::Pending => ("pending", Tone::Warning),
        PullRequestReviewState::Commented => ("commented", Tone::Info),
        PullRequestReviewState::Approved => ("approved", Tone::Success),
        PullRequestReviewState::ChangesRequested => ("changes requested", Tone::Danger),
        PullRequestReviewState::Dismissed => ("dismissed", Tone::Neutral),
    }
}

pub(crate) fn review_thread_state_label(state: ReviewThreadState) -> (&'static str, gpui::Hsla) {
    let (label, tone) = review_thread_state_tone(state);

    (label, tone_text(tone).into())
}

pub(super) fn review_thread_state_tone(state: ReviewThreadState) -> (&'static str, Tone) {
    match state {
        ReviewThreadState::Unresolved => ("unresolved", Tone::Warning),
        ReviewThreadState::Resolved => ("resolved", Tone::Success),
        ReviewThreadState::Outdated => ("outdated", Tone::Neutral),
    }
}

pub(crate) fn review_time_label(review: &PullRequestReview) -> String {
    review
        .submitted_at
        .map(natural_time_label)
        .unwrap_or_else(|| "not submitted".to_string())
}

fn review_time_tooltip(review: &PullRequestReview) -> Option<String> {
    review.submitted_at.map(full_time_label)
}

pub(super) fn review_comment_time_label(comment: &ReviewComment) -> String {
    natural_time_label_with_edit(comment.created_at, comment.updated_at)
}

pub(super) fn review_comment_time_tooltip(comment: &ReviewComment) -> String {
    full_time_label_with_edit(comment.created_at, comment.updated_at)
}

fn pull_request_comment_time_label(comment: &PullRequestComment) -> String {
    natural_time_label_with_edit(comment.created_at, comment.updated_at)
}

fn pull_request_comment_time_tooltip(comment: &PullRequestComment) -> String {
    full_time_label_with_edit(comment.created_at, comment.updated_at)
}

fn render_time_metadata(
    id: String,
    label: String,
    tooltip: Option<String>,
    text_color: gpui::Rgba,
) -> impl IntoElement {
    div()
        .id(id)
        .text_xs()
        .text_color(text_color)
        .when_some(tooltip, |element, tooltip| {
            element.tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        })
        .child(label)
}

#[cfg(test)]
#[path = "review/tests.rs"]
mod tests;
