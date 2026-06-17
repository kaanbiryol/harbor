use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use gpui::{
    AnyElement, Context, IntoElement, UniformListScrollHandle, div, prelude::*, px, uniform_list,
};
use gpui_component::StyledExt;
use harbor_domain::{
    DiffFile, PullRequestComment, PullRequestReview, PullRequestReviewState, ReviewComment,
    ReviewSide, ReviewThread, ReviewThreadState,
};

use crate::{
    diff::{DiffLineKind, ParsedDiff},
    visual::{Tone, color, tone_colors, tone_text},
    workspace::AppView,
};

use super::review_thread_rows::{
    ReviewThreadRowRenderState, render_review_thread_row, review_timeline_row_height,
};
use super::{
    render_empty_panel_card, render_error_panel_card, render_metric_pill, render_panel_header,
    render_status_pill,
};

pub(crate) struct ReviewPanelRenderInput<'a> {
    pub(crate) reviews: &'a [PullRequestReview],
    pub(crate) comments: &'a [PullRequestComment],
    pub(crate) threads: &'a [ReviewThread],
    pub(crate) active_review_thread_reply: Option<&'a str>,
    pub(crate) is_loading: bool,
    pub(crate) error: Option<&'a str>,
    pub(crate) scroll_handle: UniformListScrollHandle,
}

pub(crate) fn render_review_panel(
    input: ReviewPanelRenderInput<'_>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let ReviewPanelRenderInput {
        reviews,
        comments,
        threads,
        active_review_thread_reply,
        is_loading,
        error,
        scroll_handle,
    } = input;
    let (unresolved, resolved, outdated) = review_thread_counts(threads);
    let view_entity = cx.entity().clone();
    let timeline_items = review_timeline_items(reviews, threads, comments);
    let has_timeline_items = !timeline_items.is_empty();
    let active_review_item_index = active_review_thread_reply.and_then(|thread_id| {
        timeline_items.iter().position(|item| {
            matches!(
                &item.kind,
                ReviewTimelineItemKind::Thread { thread_id: item_thread_id }
                    if item_thread_id == thread_id
            )
        })
    });
    let timeline_items_for_render = timeline_items.clone();

    div()
        .id("review-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_2()
        .child(render_panel_header(
            "Review",
            Some(format!("{} timeline items", timeline_items.len())),
        ))
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
        .when(is_loading, |element| {
            element.child(render_empty_panel_card("Loading review comments..."))
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(render_error_panel_card(error))
        })
        .when(
            !is_loading && error.is_none() && !has_timeline_items,
            |element| {
                element.child(render_empty_panel_card(
                    "No review comments found for this pull request",
                ))
            },
        )
        .when(has_timeline_items, |element| {
            element.child(
                div().flex().flex_col().flex_1().min_h_0().min_w_0().child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .overflow_hidden()
                        .child(
                            uniform_list(
                                "review-timeline-list",
                                timeline_items_for_render.len(),
                                cx.processor(
                                    move |view, range: std::ops::Range<usize>, _window, _cx| {
                                        let mut rows = Vec::with_capacity(range.len());
                                        let active_review_thread_reply = view
                                            .review_state
                                            .review_composer_state
                                            .active_thread_reply();
                                        let use_expanded_rows = active_review_thread_reply
                                            .is_some_and(|thread_id| {
                                                view.review_state
                                                    .review_threads
                                                    .iter()
                                                    .any(|thread| thread.id == thread_id)
                                            });

                                        for index in range {
                                            let Some(item) = timeline_items_for_render.get(index)
                                            else {
                                                continue;
                                            };

                                            match &item.kind {
                                                ReviewTimelineItemKind::Thread { thread_id } => {
                                                    let Some(thread) = view
                                                        .review_state
                                                        .review_threads
                                                        .iter()
                                                        .find(|thread| thread.id == *thread_id)
                                                    else {
                                                        continue;
                                                    };
                                                    let diff_preview = review_thread_diff_preview(
                                                        thread,
                                                        view.detail_state.files(),
                                                        view.detail_state.diffs(),
                                                    );
                                                    rows.push(render_review_thread_row(
                                                        ReviewThreadRowRenderState {
                                                            index,
                                                            thread,
                                                            active_review_thread_reply,
                                                            review_thread_reply_input: view
                                                                .review_state
                                                                .review_composer_state
                                                                .thread_reply_input
                                                                .clone(),
                                                            reply_body_empty: view
                                                                .review_state
                                                                .review_composer_state
                                                                .thread_reply_input
                                                                .read(_cx)
                                                                .value()
                                                                .trim()
                                                                .is_empty(),
                                                            is_submitting_reply: view
                                                                .review_state
                                                                .is_submitting_review_thread_reply(
                                                                ),
                                                            reply_error: view
                                                                .review_state
                                                                .review_thread_reply_error(),
                                                            action_thread_id: view
                                                                .review_state
                                                                .review_thread_action_thread_id(),
                                                            action_error: view
                                                                .review_state
                                                                .review_thread_action_error(),
                                                            use_expanded_rows,
                                                            diff_preview,
                                                            view_entity: view_entity.clone(),
                                                        },
                                                    ));
                                                }
                                                ReviewTimelineItemKind::Review { review_id } => {
                                                    let Some(review) = view
                                                        .review_state
                                                        .pull_request_reviews
                                                        .iter()
                                                        .find(|review| review.id == *review_id)
                                                    else {
                                                        continue;
                                                    };
                                                    rows.push(render_pull_request_review_row(
                                                        review,
                                                        index,
                                                        use_expanded_rows,
                                                    ));
                                                }
                                                ReviewTimelineItemKind::Comment { comment_id } => {
                                                    let Some(comment) = view
                                                        .review_state
                                                        .pull_request_comments
                                                        .iter()
                                                        .find(|comment| comment.id == *comment_id)
                                                    else {
                                                        continue;
                                                    };
                                                    rows.push(render_pull_request_comment_row(
                                                        comment,
                                                        index,
                                                        use_expanded_rows,
                                                    ));
                                                }
                                            }
                                        }

                                        rows
                                    },
                                ),
                            )
                            .with_width_from_item(active_review_item_index)
                            .track_scroll(&scroll_handle)
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .min_w_0(),
                        ),
                ),
            )
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReviewTimelineItem {
    kind: ReviewTimelineItemKind,
    sort_time: Option<DateTime<Utc>>,
    sequence: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ReviewTimelineItemKind {
    Thread { thread_id: String },
    Review { review_id: String },
    Comment { comment_id: String },
}

fn review_timeline_items(
    reviews: &[PullRequestReview],
    threads: &[ReviewThread],
    comments: &[PullRequestComment],
) -> Vec<ReviewTimelineItem> {
    let mut items = Vec::new();
    let mut sequence = 0;

    for review in reviews {
        let has_summary = review
            .body
            .as_deref()
            .and_then(review_body_summary)
            .is_some();
        if !has_summary && review_has_inline_comment(review, threads) {
            continue;
        }

        items.push(ReviewTimelineItem {
            kind: ReviewTimelineItemKind::Review {
                review_id: review.id.clone(),
            },
            sort_time: review.submitted_at,
            sequence,
        });
        sequence += 1;
    }

    for thread in threads {
        items.push(ReviewTimelineItem {
            kind: ReviewTimelineItemKind::Thread {
                thread_id: thread.id.clone(),
            },
            sort_time: review_thread_sort_time(thread),
            sequence,
        });
        sequence += 1;
    }

    for comment in comments {
        items.push(ReviewTimelineItem {
            kind: ReviewTimelineItemKind::Comment {
                comment_id: comment.id.clone(),
            },
            sort_time: Some(comment.created_at),
            sequence,
        });
        sequence += 1;
    }

    items.sort_by(compare_review_timeline_items);

    items
}

fn review_thread_sort_time(thread: &ReviewThread) -> Option<DateTime<Utc>> {
    thread.comments.last().map(|comment| comment.created_at)
}

fn compare_review_timeline_items(
    left: &ReviewTimelineItem,
    right: &ReviewTimelineItem,
) -> Ordering {
    match (left.sort_time.as_ref(), right.sort_time.as_ref()) {
        (Some(left_time), Some(right_time)) => left_time.cmp(right_time),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
    .then_with(|| left.sequence.cmp(&right.sequence))
}

fn render_pull_request_comment_row(
    comment: &PullRequestComment,
    index: usize,
    use_expanded_rows: bool,
) -> AnyElement {
    let row_height = review_timeline_row_height(use_expanded_rows);

    div()
        .id(("pull-request-comment-row", index))
        .h(px(row_height))
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_2()
        .px_3()
        .py_2()
        .border_1()
        .border_color(color::border())
        .bg(color::app_background())
        .overflow_hidden()
        .child(
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
                        .text_color(color::text_primary())
                        .child(format!("{} commented", comment.author)),
                )
                .child(render_status_pill("commented", Tone::Info)),
        )
        .child(
            div()
                .text_xs()
                .text_color(color::text_muted())
                .child(comment_time_label(comment.created_at)),
        )
        .child(
            div()
                .min_h_0()
                .overflow_hidden()
                .border_1()
                .border_color(color::border_subtle())
                .bg(color::content_background())
                .px_2()
                .py_2()
                .text_xs()
                .text_color(color::text_secondary())
                .whitespace_normal()
                .child(comment_body_text(&comment.body)),
        )
        .into_any_element()
}

fn render_pull_request_review_row(
    review: &PullRequestReview,
    index: usize,
    use_expanded_rows: bool,
) -> AnyElement {
    let (state_label, state_tone) = pull_request_review_state_tone(review.state);
    let row_height = review_timeline_row_height(use_expanded_rows);
    let body = review
        .body
        .as_deref()
        .and_then(review_body_summary)
        .unwrap_or_else(|| format!("{} review", state_label));

    div()
        .id(("review-summary-row", index))
        .h(px(row_height))
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_2()
        .px_3()
        .py_2()
        .border_1()
        .border_color(color::border())
        .bg(color::app_background())
        .overflow_hidden()
        .child(
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
                        .text_color(color::text_primary())
                        .child(format!("{} {}", review.author, state_label)),
                )
                .child(render_status_pill(state_label, state_tone)),
        )
        .child(
            div()
                .text_xs()
                .text_color(color::text_muted())
                .child(review_time_label(review)),
        )
        .child(
            div()
                .border_1()
                .border_color(color::border_subtle())
                .bg(color::content_background())
                .px_2()
                .py_2()
                .text_xs()
                .text_color(color::text_secondary())
                .truncate()
                .child(body),
        )
        .into_any_element()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReviewDiffPreview {
    path: String,
    line: Option<u32>,
    marker: &'static str,
    text: String,
    tone: Tone,
}

pub(super) fn render_review_diff_preview(preview: ReviewDiffPreview) -> impl IntoElement {
    let line_label = preview
        .line
        .map(|line| line.to_string())
        .unwrap_or_else(|| "-".to_string());

    div()
        .mt_2()
        .min_w_0()
        .overflow_hidden()
        .border_1()
        .border_color(color::border_subtle())
        .child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .font_medium()
                .text_color(color::text_secondary())
                .bg(color::content_background())
                .truncate()
                .child(preview.path),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .text_xs()
                .bg(tone_colors(preview.tone).background)
                .text_color(color::text_primary())
                .child(
                    div()
                        .w_8()
                        .text_right()
                        .font_family("monospace")
                        .child(line_label),
                )
                .child(div().w_3().font_family("monospace").child(preview.marker))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .font_family("monospace")
                        .child(preview.text),
                ),
        )
}

fn review_body_summary(body: &str) -> Option<String> {
    body.lines()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
}

fn comment_body_text(body: &str) -> String {
    let body = body.trim();

    if body.is_empty() {
        "empty comment".to_string()
    } else {
        body.to_string()
    }
}

fn review_has_inline_comment(review: &PullRequestReview, threads: &[ReviewThread]) -> bool {
    threads
        .iter()
        .flat_map(|thread| thread.comments.iter())
        .any(|comment| review_matches_comment(review, comment))
}

fn review_matches_comment(review: &PullRequestReview, comment: &ReviewComment) -> bool {
    comment
        .pull_request_review_id
        .as_deref()
        .is_some_and(|review_id| review_id == review.id)
        || review
            .node_id
            .as_deref()
            .is_some_and(|node_id| comment.pull_request_review_node_id.as_deref() == Some(node_id))
}

pub(super) fn review_thread_diff_preview(
    thread: &ReviewThread,
    files: &[DiffFile],
    diffs: &[Option<ParsedDiff>],
) -> Option<ReviewDiffPreview> {
    let comment = thread.comments.first()?;

    review_comment_diff_preview(comment, thread, files, diffs)
}

fn review_comment_diff_preview(
    comment: &ReviewComment,
    thread: &ReviewThread,
    files: &[DiffFile],
    diffs: &[Option<ParsedDiff>],
) -> Option<ReviewDiffPreview> {
    let (path, side, line) = review_comment_diff_target(comment, thread)?;
    let fallback = || ReviewDiffPreview {
        path: path.clone(),
        line: Some(line),
        marker: "",
        text: "diff context unavailable".to_string(),
        tone: Tone::Neutral,
    };
    let Some((_, diff)) = files.iter().zip(diffs.iter()).find(|(file, _)| {
        file.path == path || file.previous_path.as_deref() == Some(path.as_str())
    }) else {
        return Some(fallback());
    };
    let Some(diff) = diff.as_ref() else {
        return Some(fallback());
    };
    let Some(diff_line) = diff
        .hunks
        .iter()
        .flat_map(|hunk| hunk.lines.iter())
        .find(|diff_line| match side {
            ReviewSide::Left => diff_line.old_line == Some(line),
            ReviewSide::Right => diff_line.new_line == Some(line),
        })
    else {
        return Some(fallback());
    };
    let (marker, tone) = match diff_line.kind {
        DiffLineKind::Added => ("+", Tone::Success),
        DiffLineKind::Removed => ("-", Tone::Danger),
        DiffLineKind::Context => (" ", Tone::Neutral),
        DiffLineKind::Metadata => ("", Tone::Neutral),
    };
    let preview_line = match side {
        ReviewSide::Left => diff_line.old_line,
        ReviewSide::Right => diff_line.new_line,
    };

    Some(ReviewDiffPreview {
        path,
        line: preview_line,
        marker,
        text: diff_line.text.clone(),
        tone,
    })
}

fn review_comment_diff_target(
    comment: &ReviewComment,
    thread: &ReviewThread,
) -> Option<(String, ReviewSide, u32)> {
    if let Some(position) = comment.position.as_ref() {
        let line = match position.side {
            ReviewSide::Left => position.original_line.or(position.line),
            ReviewSide::Right => position.line.or(position.original_line),
        }?;
        return Some((position.path.clone(), position.side, line));
    }

    let range = thread.range.as_ref()?;

    Some((range.path.clone(), range.side, range.line))
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

pub(crate) fn single_line(value: &str) -> String {
    review_body_summary(value).unwrap_or_else(|| "empty comment".to_string())
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
        .map(comment_time_label)
        .unwrap_or_else(|| "not submitted".to_string())
}

pub(super) fn review_comment_time_label(comment: &ReviewComment) -> String {
    comment_time_label(comment.created_at)
}

fn comment_time_label(created_at: DateTime<Utc>) -> String {
    created_at.format("%Y-%m-%d %H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use gpui::{
        Context, Entity, IntoElement, Modifiers, Render, TestAppContext, VisualTestContext, Window,
    };
    use gpui_component::{Root, Theme, ThemeMode, input::InputState};
    use harbor_domain::FileStatus;

    use super::*;
    use crate::{
        test_fixtures::{review_thread as test_review_thread, test_time},
        workspace::ReviewThreadUiError,
    };

    #[test]
    fn counts_review_threads_by_state() {
        let threads = vec![
            review_thread_with_state(ReviewThreadState::Unresolved),
            review_thread_with_state(ReviewThreadState::Resolved),
            review_thread_with_state(ReviewThreadState::Outdated),
            review_thread_with_state(ReviewThreadState::Unresolved),
        ];

        assert_eq!(review_thread_counts(&threads), (2, 1, 1));
    }

    #[test]
    fn builds_diff_preview_for_inline_review_comments() {
        let mut thread = review_thread();
        let comment = thread
            .comments
            .first_mut()
            .expect("test thread should have a comment");
        comment.author = "alex".to_string();
        comment.body = "Please tighten this branch.".to_string();
        let position = comment
            .position
            .as_mut()
            .expect("test comment should have a position");
        position.line = Some(11);
        position.original_line = None;
        let (files, diffs) = review_diff_fixture();

        assert_eq!(
            review_thread_diff_preview(&thread, &files, &diffs),
            Some(ReviewDiffPreview {
                path: "src/lib.rs".to_string(),
                line: Some(11),
                marker: "+",
                text: "Please tighten this branch.".to_string(),
                tone: Tone::Success,
            })
        );
    }

    #[test]
    fn timeline_includes_review_threads() {
        let thread = review_thread();

        let items = review_timeline_items(&[], &[thread], &[]);

        assert_eq!(items.len(), 1);
        assert!(matches!(
            &items[0].kind,
            ReviewTimelineItemKind::Thread { thread_id } if thread_id == "thread-1"
        ));
    }

    #[test]
    fn timeline_includes_review_summaries() {
        let mut review = pull_request_review("401", None, Some("Overall direction looks right."));
        review.submitted_at = Some(test_time());

        let items = review_timeline_items(&[review], &[], &[]);

        assert_eq!(items.len(), 1);
        assert!(matches!(
            &items[0].kind,
            ReviewTimelineItemKind::Review { review_id } if review_id == "401"
        ));
    }

    #[test]
    fn timeline_includes_review_state_without_summary_when_not_carried_by_inline_comments() {
        let mut review = pull_request_review("401", Some("review-node-401"), None);
        review.state = PullRequestReviewState::Approved;
        review.submitted_at = Some(test_time());

        let items = review_timeline_items(&[review], &[], &[]);

        assert_eq!(items.len(), 1);
        assert!(matches!(
            &items[0].kind,
            ReviewTimelineItemKind::Review { review_id } if review_id == "401"
        ));
    }

    #[test]
    fn timeline_skips_empty_review_when_inline_thread_represents_it() {
        let mut review = pull_request_review("401", Some("review-node-401"), None);
        review.state = PullRequestReviewState::ChangesRequested;
        review.submitted_at = Some(test_time());
        let mut thread = review_thread();
        thread.comments[0].pull_request_review_id = Some("401".to_string());
        thread.comments[0].pull_request_review_node_id = Some("review-node-401".to_string());

        let items = review_timeline_items(&[review], &[thread], &[]);

        assert_eq!(items.len(), 1);
        assert!(matches!(
            &items[0].kind,
            ReviewTimelineItemKind::Thread { thread_id } if thread_id == "thread-1"
        ));
    }

    #[test]
    fn timeline_includes_pull_request_comments() {
        let comment = pull_request_comment("comment-1", "Can we do this?");

        let items = review_timeline_items(&[], &[], &[comment]);

        assert_eq!(items.len(), 1);
        assert!(matches!(
            &items[0].kind,
            ReviewTimelineItemKind::Comment { comment_id } if comment_id == "comment-1"
        ));
    }

    #[test]
    fn timeline_orders_older_summaries_before_recent_threads() {
        let mut review = pull_request_review("401", None, Some("Older summary."));
        review.submitted_at = Some(test_time());
        let mut thread = review_thread();
        thread.comments[0].created_at = test_time() + chrono::Duration::minutes(5);

        let items = review_timeline_items(&[review], &[thread], &[]);

        assert_eq!(items.len(), 2);
        assert!(matches!(
            &items[0].kind,
            ReviewTimelineItemKind::Review { review_id } if review_id == "401"
        ));
    }

    #[gpui::test]
    async fn review_panel_reply_button_opens_and_cancel_clears_reply_mode(cx: &mut TestAppContext) {
        let (view_entity, cx) = init_visual_review_panel_test(cx);

        render_review_panel_row_harness(cx);
        let reply_bounds = cx
            .debug_bounds("review-panel-reply-thread-thread-1")
            .expect("review panel reply button should render");
        cx.simulate_click(reply_bounds.center(), Modifiers::none());

        assert_eq!(
            view_entity.read_with(cx, |view, _| view
                .review_state
                .review_composer_state
                .active_thread_reply()
                .map(str::to_string)),
            Some("thread-1".to_string())
        );

        render_review_panel_row_harness(cx);
        let cancel_bounds = cx
            .debug_bounds("review-panel-cancel-thread-reply-thread-1")
            .expect("review panel reply cancel button should render");
        cx.simulate_click(cancel_bounds.center(), Modifiers::none());

        assert!(view_entity.read_with(cx, |view, _| {
            view.review_state
                .review_composer_state
                .active_thread_reply()
                .is_none()
        }));
    }

    #[gpui::test]
    async fn review_panel_toggle_reports_missing_selected_pull_request(cx: &mut TestAppContext) {
        let (view_entity, cx) = init_visual_review_panel_test(cx);

        render_review_panel_row_harness(cx);
        let toggle_bounds = cx
            .debug_bounds("review-panel-toggle-thread-thread-1")
            .expect("review panel toggle button should render");
        cx.simulate_click(toggle_bounds.center(), Modifiers::none());

        assert_eq!(
            view_entity.read_with(cx, |view, _| {
                view.review_state
                    .review_thread_action_error()
                    .map(|error| (error.thread_id.clone(), error.message.clone()))
            }),
            Some((
                "thread-1".to_string(),
                "Select a pull request before updating a thread".to_string()
            ))
        );
    }

    fn init_visual_review_panel_test(
        cx: &mut TestAppContext,
    ) -> (Entity<AppView>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
        });

        let mut view_entity = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
            let harness = cx.new(|_| ReviewPanelRowHarness {
                view_entity: view.clone(),
                thread: review_thread(),
            });
            view_entity = Some(view);
            Root::new(harness, window, cx)
        });

        (view_entity.expect("test AppView should be created"), cx)
    }

    fn render_review_panel_row_harness(cx: &mut VisualTestContext) {
        cx.refresh().expect("test window should refresh");
        cx.run_until_parked();
    }

    struct ReviewPanelRowHarness {
        view_entity: Entity<AppView>,
        thread: ReviewThread,
    }

    impl Render for ReviewPanelRowHarness {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let render_state =
                self.view_entity
                    .read_with(cx, |view, app| ReviewPanelRowTestState {
                        active_reply_thread_id: view
                            .review_state
                            .review_composer_state
                            .active_thread_reply()
                            .map(str::to_string),
                        reply_input: view
                            .review_state
                            .review_composer_state
                            .thread_reply_input
                            .clone(),
                        reply_body_empty: view
                            .review_state
                            .review_composer_state
                            .thread_reply_input
                            .read(app)
                            .value()
                            .trim()
                            .is_empty(),
                        is_submitting_reply: view.review_state.is_submitting_review_thread_reply(),
                        reply_error: view.review_state.review_thread_reply_error().cloned(),
                        action_thread_id: view
                            .review_state
                            .review_thread_action_thread_id()
                            .map(str::to_string),
                        action_error: view.review_state.review_thread_action_error().cloned(),
                    });

            render_review_thread_row(ReviewThreadRowRenderState {
                index: 0,
                thread: &self.thread,
                active_review_thread_reply: render_state.active_reply_thread_id.as_deref(),
                review_thread_reply_input: render_state.reply_input.clone(),
                reply_body_empty: render_state.reply_body_empty,
                is_submitting_reply: render_state.is_submitting_reply,
                reply_error: render_state.reply_error.as_ref(),
                action_thread_id: render_state.action_thread_id.as_deref(),
                action_error: render_state.action_error.as_ref(),
                use_expanded_rows: render_state.active_reply_thread_id.is_some(),
                diff_preview: None,
                view_entity: self.view_entity.clone(),
            })
        }
    }

    struct ReviewPanelRowTestState {
        active_reply_thread_id: Option<String>,
        reply_input: Entity<InputState>,
        reply_body_empty: bool,
        is_submitting_reply: bool,
        reply_error: Option<ReviewThreadUiError>,
        action_thread_id: Option<String>,
        action_error: Option<ReviewThreadUiError>,
    }

    fn review_thread() -> ReviewThread {
        review_thread_with_state(ReviewThreadState::Unresolved)
    }

    fn review_thread_with_state(state: ReviewThreadState) -> ReviewThread {
        test_review_thread(state)
    }

    fn review_diff_fixture() -> (Vec<DiffFile>, Vec<Option<ParsedDiff>>) {
        let file = DiffFile {
            path: "src/lib.rs".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 0,
            changes: 1,
            patch: None,
        };
        let diff = crate::diff::parse_unified_diff(
            "@@ -10,2 +10,3 @@\n context\n+Please tighten this branch.\n unchanged\n",
        );

        (vec![file], vec![Some(diff)])
    }

    fn pull_request_review(
        id: &str,
        node_id: Option<&str>,
        body: Option<&str>,
    ) -> PullRequestReview {
        PullRequestReview {
            id: id.to_string(),
            node_id: node_id.map(str::to_string),
            author: "alex".to_string(),
            state: PullRequestReviewState::Commented,
            body: body.map(str::to_string),
            submitted_at: None,
        }
    }

    fn pull_request_comment(id: &str, body: &str) -> PullRequestComment {
        PullRequestComment {
            id: id.to_string(),
            author: "alex".to_string(),
            author_avatar_url: None,
            body: body.to_string(),
            created_at: test_time(),
            updated_at: None,
        }
    }
}
