use gpui::{
    AnyElement, Context, Entity, IntoElement, ListHorizontalSizingBehavior, MouseButton,
    UniformListScrollHandle, div, prelude::*, px, rgb, uniform_list,
};
use gpui_component::{
    Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
};
use harbor_domain::{DiffFile, ReviewCommentRange, ReviewSide, ReviewThread, ReviewThreadState};

use crate::diff::{DiffHunk, DiffLine, DiffLineKind, ParsedDiff};
use crate::diff_reviews::{
    REVIEW_THREAD_INLINE_ROWS, anchored_review_threads, diff_row_count_with_reviews,
    review_threads_for_line,
};
use crate::workspace::{
    AppView, PendingReviewSession, ReviewCommentSubmission, ReviewComposer, ReviewLineSelection,
    ReviewLineTarget, ReviewThreadUiError,
};

use super::review::{review_thread_state_label, single_line};

const MIN_LINE_NUMBER_WIDTH: f32 = 28.0;
const LINE_NUMBER_PADDING: f32 = 8.0;
const LINE_NUMBER_DIGIT_WIDTH: f32 = 8.0;
const DIFF_ROW_HEIGHT: f32 = 24.0;
const REVIEW_COMPOSER_ROWS: usize = 8;
const REVIEW_COMPOSER_ROWS_WITH_ERROR: usize = 9;
const REVIEW_COMPOSER_MAX_WIDTH: f32 = 820.0;
const REVIEW_MARKER_WIDTH: f32 = 24.0;
const PREFIX_WIDTH: f32 = 16.0;

pub(crate) fn render_diff_panel(
    file: Option<&DiffFile>,
    parsed_diff: Option<&ParsedDiff>,
    review_threads: &[ReviewThread],
    review_composer: Option<&ReviewComposer>,
    review_comment_error: Option<&str>,
    is_loading: bool,
    error: Option<&str>,
    scroll_handle: UniformListScrollHandle,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    if is_loading {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(
                div()
                    .text_color(rgb(0xf1f5f9))
                    .child("Unified diff preview"),
            )
            .child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("Loading diff..."),
            )
            .into_any_element();
    }

    if let Some(error) = error {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(
                div()
                    .text_color(rgb(0xf1f5f9))
                    .child("Unified diff preview"),
            )
            .child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0xf87171))
                    .child(error.to_string()),
            )
            .into_any_element();
    }

    let Some(file) = file else {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(
                div()
                    .text_color(rgb(0xf1f5f9))
                    .child("Unified diff preview"),
            )
            .child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child("Select a changed file to preview its diff"),
            )
            .into_any_element();
    };

    let Some(parsed_diff) = parsed_diff else {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(render_diff_file_header(file, None))
            .child(
                div()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0xfbbf24))
                    .child(
                        "Diff unavailable via GitHub API. Local checkout fallback will be added.",
                    ),
            )
            .into_any_element();
    };

    let row_count = diff_row_count_with_review_controls(
        parsed_diff,
        file,
        review_threads,
        review_composer,
        review_comment_error,
    );
    let view_entity = cx.entity().clone();

    div()
        .id("diff-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .gap_2()
        .child(render_diff_file_header(file, Some(parsed_diff.hunks.len())))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .rounded_sm()
                .border_1()
                .border_color(rgb(0x242a31))
                .bg(rgb(0x0c0f12))
                .overflow_hidden()
                .child(
                    uniform_list(
                        "diff-lines-list",
                        row_count,
                        cx.processor(move |view, range: std::ops::Range<usize>, _window, _cx| {
                            let Some(file) = view.active_file() else {
                                return Vec::new();
                            };
                            let Some(parsed_diff) = view.active_diff() else {
                                return Vec::new();
                            };
                            let line_number_width = line_number_width_for_diff(parsed_diff);

                            render_diff_rows(
                                parsed_diff,
                                file,
                                &view.review_threads,
                                view.review_composer.as_ref(),
                                view.review_line_selection.as_ref(),
                                view.pending_review.as_ref(),
                                view.review_comment_input.clone(),
                                view.review_comment_input
                                    .read(_cx)
                                    .value()
                                    .trim()
                                    .is_empty(),
                                view.is_submitting_review_comment,
                                view.review_comment_error.as_deref(),
                                view.review_thread_reply_thread_id.as_deref(),
                                view.review_thread_reply_input.clone(),
                                view.review_thread_reply_input
                                    .read(_cx)
                                    .value()
                                    .trim()
                                    .is_empty(),
                                view.is_submitting_review_thread_reply,
                                view.review_thread_reply_error.as_ref(),
                                view.review_thread_action_thread_id.as_deref(),
                                view.review_thread_action_error.as_ref(),
                                view.active_hunk,
                                line_number_width,
                                view_entity.clone(),
                                range,
                            )
                        }),
                    )
                    .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
                    .track_scroll(&scroll_handle)
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .font_family("Menlo")
                    .text_xs(),
                ),
        )
        .into_any_element()
}

pub(crate) fn render_diff_file_header(
    file: &DiffFile,
    hunk_count: Option<usize>,
) -> impl IntoElement {
    let hunk_label = hunk_count.map_or_else(
        || "no parsed hunks".to_string(),
        |count| format!("{count} hunks"),
    );

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .text_color(rgb(0xf1f5f9))
        .child(file.path.clone())
        .child(div().text_xs().text_color(rgb(0x9aa4b2)).child(format!(
            "{:?}  +{} -{}  {}",
            file.status, file.additions, file.deletions, hunk_label
        )))
}

fn render_diff_rows(
    diff: &ParsedDiff,
    file: &DiffFile,
    review_threads: &[ReviewThread],
    review_composer: Option<&ReviewComposer>,
    review_line_selection: Option<&ReviewLineSelection>,
    pending_review: Option<&PendingReviewSession>,
    review_comment_input: Entity<InputState>,
    review_comment_body_empty: bool,
    is_submitting_review_comment: bool,
    review_comment_error: Option<&str>,
    active_review_thread_reply: Option<&str>,
    review_thread_reply_input: Entity<InputState>,
    review_thread_reply_body_empty: bool,
    is_submitting_review_thread_reply: bool,
    review_thread_reply_error: Option<&ReviewThreadUiError>,
    review_thread_action_thread_id: Option<&str>,
    review_thread_action_error: Option<&ReviewThreadUiError>,
    active_hunk: usize,
    line_number_width: f32,
    view_entity: Entity<AppView>,
    range: std::ops::Range<usize>,
) -> Vec<AnyElement> {
    let anchored_threads = anchored_review_threads(file, review_threads);
    let review_marker_width = REVIEW_MARKER_WIDTH;
    let active_selection_range = review_line_selection.and_then(|selection| {
        crate::workspace::review_range_from_targets(&selection.anchor, &selection.current).ok()
    });
    let mut rows = Vec::with_capacity(range.len());
    let mut row_index = 0;

    for (hunk_index, hunk) in diff.hunks.iter().enumerate() {
        if row_index >= range.end {
            break;
        }

        if row_in_range(row_index, &range) {
            rows.push(
                render_diff_hunk_row(hunk, hunk_index, hunk_index == active_hunk)
                    .into_any_element(),
            );
        }
        row_index += 1;

        for (line_index, line) in hunk.lines.iter().enumerate() {
            if row_index >= range.end {
                break;
            }

            let matching_threads = review_threads_for_line(&anchored_threads, line);
            let review_line_target =
                review_line_target_for_line(file, hunk_index, line_index, line);
            let selected_for_comment = review_composer.is_some_and(|composer| {
                review_comment_range_matches_line(file, &composer.range, line)
            });
            let dragging_for_comment = active_selection_range
                .as_ref()
                .is_some_and(|range| review_comment_range_matches_line(file, range, line));
            let has_unresolved_thread = matching_threads
                .iter()
                .any(|thread| thread.state == ReviewThreadState::Unresolved);
            let has_thread_range = review_threads
                .iter()
                .filter_map(|thread| thread.range.as_ref())
                .any(|range| review_comment_range_matches_line(file, range, line));

            if row_in_range(row_index, &range) {
                rows.push(
                    render_diff_line(
                        row_index,
                        line,
                        matching_threads.len(),
                        has_unresolved_thread,
                        dragging_for_comment,
                        selected_for_comment,
                        has_thread_range,
                        review_line_target.clone(),
                        line_number_width,
                        review_marker_width,
                        view_entity.clone(),
                    )
                    .into_any_element(),
                );
            }
            row_index += 1;

            let composer_ends_here = review_composer.is_some_and(|composer| {
                composer.anchor.hunk_index == hunk_index && composer.anchor.line_index == line_index
            });

            if composer_ends_here {
                let composer_row_count = review_composer_row_count(review_comment_error);

                for composer_row in 0..composer_row_count {
                    if row_index >= range.end {
                        row_index += composer_row_count - composer_row;
                        break;
                    }

                    if row_in_range(row_index, &range) {
                        if composer_row == 0 {
                            if let Some(composer) = review_composer.cloned() {
                                rows.push(
                                    render_review_composer_inline(
                                        composer,
                                        pending_review.cloned(),
                                        review_comment_input.clone(),
                                        review_comment_body_empty,
                                        is_submitting_review_comment,
                                        review_comment_error,
                                        composer_row_count,
                                        line_number_width,
                                        review_marker_width,
                                        view_entity.clone(),
                                    )
                                    .into_any_element(),
                                );
                            }
                        } else {
                            rows.push(render_review_composer_spacer().into_any_element());
                        }
                    }

                    row_index += 1;
                }
            }

            for thread in matching_threads {
                for thread_row in 0..REVIEW_THREAD_INLINE_ROWS {
                    if row_index >= range.end {
                        row_index += REVIEW_THREAD_INLINE_ROWS - thread_row;
                        break;
                    }

                    if row_in_range(row_index, &range) {
                        if thread_row == 0 {
                            rows.push(
                                render_review_thread_inline(
                                    thread,
                                    line_number_width,
                                    active_review_thread_reply,
                                    review_thread_reply_input.clone(),
                                    review_thread_reply_body_empty,
                                    is_submitting_review_thread_reply,
                                    review_thread_reply_error,
                                    review_thread_action_thread_id,
                                    review_thread_action_error,
                                    view_entity.clone(),
                                )
                                .into_any_element(),
                            );
                        } else {
                            rows.push(render_review_composer_spacer().into_any_element());
                        }
                    }

                    row_index += 1;
                }
            }
        }
    }

    rows
}

fn row_in_range(row_index: usize, range: &std::ops::Range<usize>) -> bool {
    row_index >= range.start && row_index < range.end
}

pub(crate) fn render_diff_hunk_row(
    hunk: &DiffHunk,
    index: usize,
    active: bool,
) -> impl IntoElement {
    div()
        .h(px(DIFF_ROW_HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .border_1()
        .border_color(if active { rgb(0x3b82f6) } else { rgb(0x1a2029) })
        .bg(if active { rgb(0x172033) } else { rgb(0x1a2029) })
        .text_color(rgb(0x93c5fd))
        .whitespace_nowrap()
        .child(format!("hunk {}  {}", index + 1, hunk.header))
}

pub(crate) fn render_diff_line(
    row_index: usize,
    line: &DiffLine,
    thread_count: usize,
    has_unresolved_thread: bool,
    dragging_for_comment: bool,
    selected_for_comment: bool,
    has_thread_range: bool,
    review_line_target: Option<ReviewLineTarget>,
    line_number_width: f32,
    review_marker_width: f32,
    view_entity: Entity<AppView>,
) -> impl IntoElement {
    let (prefix, bg, text_color) = match line.kind {
        DiffLineKind::Context => (" ", rgb(0x0c0f12), rgb(0xcbd5e1)),
        DiffLineKind::Added => ("+", rgb(0x10231a), rgb(0xa7f3d0)),
        DiffLineKind::Removed => ("-", rgb(0x291516), rgb(0xfca5a5)),
        DiffLineKind::Metadata => ("\\", rgb(0x111827), rgb(0x9aa4b2)),
    };
    let selected_bg = match line.kind {
        DiffLineKind::Context => rgb(0x20324a),
        DiffLineKind::Added => rgb(0x174832),
        DiffLineKind::Removed => rgb(0x4d2b32),
        DiffLineKind::Metadata => rgb(0x20324a),
    };
    let dragging_bg = match line.kind {
        DiffLineKind::Context => rgb(0x263d5b),
        DiffLineKind::Added => rgb(0x1b5a3f),
        DiffLineKind::Removed => rgb(0x61363e),
        DiffLineKind::Metadata => rgb(0x263d5b),
    };
    let thread_range_bg = match line.kind {
        DiffLineKind::Context => rgb(0x141b24),
        DiffLineKind::Added => rgb(0x14291f),
        DiffLineKind::Removed => rgb(0x301d20),
        DiffLineKind::Metadata => rgb(0x141b24),
    };
    let bg = if dragging_for_comment {
        dragging_bg
    } else if selected_for_comment {
        selected_bg
    } else if has_thread_range {
        thread_range_bg
    } else {
        bg
    };
    let hover_bg = if dragging_for_comment {
        match line.kind {
            DiffLineKind::Added => rgb(0x20694a),
            DiffLineKind::Removed => rgb(0x704049),
            DiffLineKind::Context | DiffLineKind::Metadata => rgb(0x2c486a),
        }
    } else if selected_for_comment {
        match line.kind {
            DiffLineKind::Added => rgb(0x1b553d),
            DiffLineKind::Removed => rgb(0x5a3239),
            DiffLineKind::Context | DiffLineKind::Metadata => rgb(0x243a55),
        }
    } else if has_thread_range {
        match line.kind {
            DiffLineKind::Added => rgb(0x193326),
            DiffLineKind::Removed => rgb(0x3a2327),
            DiffLineKind::Context | DiffLineKind::Metadata => rgb(0x1a2531),
        }
    } else {
        rgb(0x18212b)
    };
    let line_id = format!("diff-line-{row_index}");

    div()
        .id(line_id)
        .h(px(DIFF_ROW_HEIGHT))
        .w_full()
        .flex()
        .items_start()
        .bg(bg)
        .text_color(text_color)
        .whitespace_nowrap()
        .child(render_line_number(line.old_line, line_number_width))
        .child(render_line_number(line.new_line, line_number_width))
        .child(render_review_marker(
            thread_count,
            has_unresolved_thread,
            review_marker_width,
        ))
        .child(
            div()
                .w(px(PREFIX_WIDTH))
                .flex_none()
                .text_color(text_color)
                .child(prefix),
        )
        .child(div().flex_none().child(line.text.clone()))
        .when_some(review_line_target, move |element, target| {
            let view_entity = view_entity.clone();
            let move_view_entity = view_entity.clone();
            let up_view_entity = view_entity.clone();
            let down_target = target.clone();
            let move_target = target.clone();

            element
                .cursor_pointer()
                .hover(move |element| element.bg(hover_bg))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let target = down_target.clone();
                    view_entity.update(cx, move |view, cx| {
                        view.start_review_line_selection(target, cx);
                    });
                    cx.stop_propagation();
                })
                .on_mouse_move(move |_, _, cx| {
                    let target = move_target.clone();
                    move_view_entity.update(cx, move |view, cx| {
                        view.extend_review_line_selection(target, cx);
                    });
                })
                .on_mouse_up(MouseButton::Left, move |_, window, cx| {
                    up_view_entity.update(cx, move |view, cx| {
                        view.finish_review_line_selection(window, cx);
                    });
                    cx.stop_propagation();
                })
        })
}

fn render_review_composer_inline(
    composer: ReviewComposer,
    pending_review: Option<PendingReviewSession>,
    review_comment_input: Entity<InputState>,
    body_empty: bool,
    is_submitting: bool,
    error: Option<&str>,
    row_count: usize,
    line_number_width: f32,
    review_marker_width: f32,
    view_entity: Entity<AppView>,
) -> impl IntoElement {
    let target_label = review_comment_range_label(&composer.range);
    let submit_disabled = body_empty || is_submitting;
    let has_pending_review = pending_review.is_some();
    let height = row_count as f32 * DIFF_ROW_HEIGHT;

    div()
        .h(px(height))
        .w_full()
        .flex()
        .items_start()
        .bg(rgb(0x0c0f12))
        .text_color(rgb(0xcbd5e1))
        .font_family(".SystemUIFont")
        .child(render_line_number(None, line_number_width))
        .child(render_line_number(None, line_number_width))
        .child(render_review_menu_marker(review_marker_width))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_2()
                .py_1()
                .pr_3()
                .child(
                    div()
                        .w_full()
                        .max_w(px(REVIEW_COMPOSER_MAX_WIDTH))
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0x2c3745))
                        .bg(rgb(0x121923))
                        .px_3()
                        .py_2()
                        .child(
                            div()
                                .pb_2()
                                .text_xs()
                                .font_medium()
                                .text_color(rgb(0x9fc7ff))
                                .child(format!("Comment on {target_label}")),
                        )
                        .child(
                            div()
                                .w_full()
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(0x354252))
                                .bg(rgb(0x0b1118))
                                .px_2()
                                .py_1()
                                .child(
                                    Input::new(&review_comment_input)
                                        .w_full()
                                        .small()
                                        .h(px(DIFF_ROW_HEIGHT * 3.0))
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false),
                                ),
                        )
                        .when_some(error.map(ToString::to_string), |element, error| {
                            element.child(
                                div()
                                    .pt_2()
                                    .text_xs()
                                    .text_color(rgb(0xf87171))
                                    .child(error),
                            )
                        })
                        .child(
                            div()
                                .pt_2()
                                .flex()
                                .items_center()
                                .justify_end()
                                .gap_2()
                                .child(
                                    Button::new("cancel-review-comment")
                                        .label("Cancel")
                                        .xsmall()
                                        .ghost()
                                        .disabled(is_submitting)
                                        .on_click({
                                            let view_entity = view_entity.clone();
                                            move |_, window, cx| {
                                                view_entity.update(cx, |view, cx| {
                                                    view.cancel_review_composer(window, cx);
                                                });
                                            }
                                        }),
                                )
                                .when_some(pending_review, {
                                    let view_entity = view_entity.clone();
                                    move |element, _pending_review| {
                                        element.child(
                                            Button::new("add-review-comment")
                                                .label("Add review comment")
                                                .xsmall()
                                                .primary()
                                                .loading(is_submitting)
                                                .disabled(submit_disabled)
                                                .on_click(move |_, _, cx| {
                                                    view_entity.update(cx, |view, cx| {
                                                        view.submit_review_comment(
                                                            ReviewCommentSubmission::AddToReview,
                                                            cx,
                                                        );
                                                    });
                                                }),
                                        )
                                    }
                                })
                                .when(!has_pending_review, {
                                    let view_entity = view_entity.clone();
                                    move |element| {
                                        element
                                            .child(
                                                Button::new("add-single-comment")
                                                    .label("Add single comment")
                                                    .xsmall()
                                                    .outline()
                                                    .loading(is_submitting)
                                                    .disabled(submit_disabled)
                                                    .on_click({
                                                        let view_entity = view_entity.clone();
                                                        move |_, _, cx| {
                                                            view_entity.update(cx, |view, cx| {
                                                                view.submit_review_comment(
                                                                    ReviewCommentSubmission::SingleComment,
                                                                    cx,
                                                                );
                                                            });
                                                        }
                                                    }),
                                            )
                                            .child(
                                                Button::new("start-review-comment")
                                                    .label("Start review")
                                                    .xsmall()
                                                    .primary()
                                                    .loading(is_submitting)
                                                    .disabled(submit_disabled)
                                                    .on_click(move |_, _, cx| {
                                                        view_entity.update(cx, |view, cx| {
                                                            view.submit_review_comment(
                                                                ReviewCommentSubmission::StartReview,
                                                                cx,
                                                            );
                                                        });
                                                    }),
                                            )
                                    }
                                }),
                        ),
                ),
        )
}

fn render_review_composer_spacer() -> impl IntoElement {
    div().h(px(DIFF_ROW_HEIGHT)).w_full()
}

fn render_review_thread_inline(
    thread: &ReviewThread,
    line_number_width: f32,
    active_review_thread_reply: Option<&str>,
    review_thread_reply_input: Entity<InputState>,
    reply_body_empty: bool,
    is_submitting_reply: bool,
    reply_error: Option<&ReviewThreadUiError>,
    action_thread_id: Option<&str>,
    action_error: Option<&ReviewThreadUiError>,
    view_entity: Entity<AppView>,
) -> impl IntoElement {
    let (label, color) = review_thread_state_label(thread.state);
    let latest_comment = thread.comments.last();
    let preview = latest_comment
        .map(|comment| single_line(&comment.body))
        .unwrap_or_else(|| "No comments in this thread".to_string());
    let author = latest_comment
        .map(|comment| comment.author.as_str())
        .unwrap_or("review");
    let height = REVIEW_THREAD_INLINE_ROWS as f32 * DIFF_ROW_HEIGHT;
    let active_reply = active_review_thread_reply == Some(thread.id.as_str());
    let thread_action_running = action_thread_id == Some(thread.id.as_str());
    let thread_reply_submitting = active_reply && is_submitting_reply;
    let reply_disabled = reply_body_empty || thread_reply_submitting;
    let is_resolved = thread.state == ReviewThreadState::Resolved;
    let can_toggle_resolution = thread.state != ReviewThreadState::Outdated;
    let reply_error = reply_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let action_error = action_error
        .filter(|error| error.thread_id == thread.id)
        .map(|error| error.message.clone());
    let thread_id = thread.id.clone();
    let toggle_label = if is_resolved { "Reopen" } else { "Resolve" };

    div()
        .h(px(height))
        .w_full()
        .flex()
        .items_start()
        .bg(rgb(0x0c0f12))
        .text_color(rgb(0xcbd5e1))
        .font_family(".SystemUIFont")
        .whitespace_nowrap()
        .child(render_line_number(None, line_number_width))
        .child(render_line_number(None, line_number_width))
        .child(render_review_marker(
            1,
            thread.state == ReviewThreadState::Unresolved,
            REVIEW_MARKER_WIDTH,
        ))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_2()
                .py_1()
                .pr_3()
                .child(
                    div()
                        .w_full()
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0x2c3745))
                        .bg(rgb(0x121923))
                        .px_3()
                        .py_2()
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
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_medium()
                                                .text_color(color)
                                                .child(label),
                                        )
                                        .child(div().text_xs().text_color(rgb(0x64748b)).child(
                                            review_comment_count_label(thread.comments.len()),
                                        )),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Button::new(format!("reply-thread-{thread_id}"))
                                                .label(if active_reply {
                                                    "Replying"
                                                } else {
                                                    "Reply"
                                                })
                                                .xsmall()
                                                .outline()
                                                .disabled(is_submitting_reply)
                                                .on_click({
                                                    let view_entity = view_entity.clone();
                                                    let thread_id = thread_id.clone();
                                                    move |_, window, cx| {
                                                        view_entity.update(cx, |view, cx| {
                                                            view.open_review_thread_reply(
                                                                thread_id.clone(),
                                                                window,
                                                                cx,
                                                            );
                                                        });
                                                    }
                                                }),
                                        )
                                        .child(
                                            Button::new(format!("toggle-thread-{thread_id}"))
                                                .label(toggle_label)
                                                .xsmall()
                                                .ghost()
                                                .loading(thread_action_running)
                                                .disabled(
                                                    !can_toggle_resolution || thread_action_running,
                                                )
                                                .on_click({
                                                    let view_entity = view_entity.clone();
                                                    let thread_id = thread_id.clone();
                                                    move |_, _, cx| {
                                                        view_entity.update(cx, |view, cx| {
                                                            view.set_review_thread_resolved(
                                                                thread_id.clone(),
                                                                !is_resolved,
                                                                cx,
                                                            );
                                                        });
                                                    }
                                                }),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .pt_1()
                                .text_xs()
                                .text_color(rgb(0x9aa4b2))
                                .truncate()
                                .child(format!("{author}: {preview}")),
                        )
                        .when(active_reply, {
                            let view_entity = view_entity.clone();
                            let thread_id = thread_id.clone();
                            move |element| {
                                element
                                    .child(
                                        div()
                                            .mt_1()
                                            .w_full()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(rgb(0x354252))
                                            .bg(rgb(0x0b1118))
                                            .px_2()
                                            .py_1()
                                            .child(
                                                Input::new(&review_thread_reply_input)
                                                    .w_full()
                                                    .small()
                                                    .h(px(DIFF_ROW_HEIGHT * 2.0))
                                                    .appearance(false)
                                                    .bordered(false)
                                                    .focus_bordered(false),
                                            ),
                                    )
                                    .when_some(reply_error.clone(), |element, error| {
                                        element.child(
                                            div()
                                                .pt_1()
                                                .text_xs()
                                                .text_color(rgb(0xf87171))
                                                .child(error),
                                        )
                                    })
                                    .child(
                                        div()
                                            .pt_1()
                                            .flex()
                                            .items_center()
                                            .justify_end()
                                            .gap_2()
                                            .child(
                                                Button::new(format!(
                                                    "cancel-thread-reply-{thread_id}"
                                                ))
                                                .label("Cancel")
                                                .xsmall()
                                                .ghost()
                                                .disabled(thread_reply_submitting)
                                                .on_click({
                                                    let view_entity = view_entity.clone();
                                                    move |_, window, cx| {
                                                        view_entity.update(cx, |view, cx| {
                                                            view.cancel_review_thread_reply(
                                                                window, cx,
                                                            );
                                                        });
                                                    }
                                                }),
                                            )
                                            .child(
                                                Button::new(format!(
                                                    "submit-thread-reply-{thread_id}"
                                                ))
                                                .label("Send reply")
                                                .xsmall()
                                                .primary()
                                                .loading(thread_reply_submitting)
                                                .disabled(reply_disabled)
                                                .on_click({
                                                    let view_entity = view_entity.clone();
                                                    let thread_id = thread_id.clone();
                                                    move |_, _, cx| {
                                                        view_entity.update(cx, |view, cx| {
                                                            view.submit_review_thread_reply(
                                                                thread_id.clone(),
                                                                cx,
                                                            );
                                                        });
                                                    }
                                                }),
                                            ),
                                    )
                            }
                        })
                        .when_some(action_error, |element, error| {
                            element.child(
                                div()
                                    .pt_1()
                                    .text_xs()
                                    .text_color(rgb(0xf87171))
                                    .child(error),
                            )
                        }),
                ),
        )
}

fn render_review_marker(
    thread_count: usize,
    has_unresolved_thread: bool,
    width: f32,
) -> impl IntoElement {
    let marker = match thread_count {
        0 => String::new(),
        1 => "R".to_string(),
        count => format!("R{count}"),
    };
    let color = if has_unresolved_thread {
        rgb(0xfbbf24)
    } else {
        rgb(0x64748b)
    };

    div()
        .w(px(width))
        .flex_none()
        .text_center()
        .text_color(color)
        .child(marker)
}

fn render_review_menu_marker(width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_none()
        .text_center()
        .text_color(rgb(0x93c5fd))
        .child("")
}

fn review_comment_count_label(comment_count: usize) -> String {
    if comment_count == 1 {
        "1 comment".to_string()
    } else {
        format!("{comment_count} comments")
    }
}

fn diff_row_count_with_review_controls(
    diff: &ParsedDiff,
    file: &DiffFile,
    review_threads: &[ReviewThread],
    review_composer: Option<&ReviewComposer>,
    review_comment_error: Option<&str>,
) -> usize {
    let mut row_count = diff_row_count_with_reviews(diff, file, review_threads);

    if review_composer
        .is_some_and(|composer| review_comment_range_matches_file(file, &composer.range))
    {
        row_count += review_composer_row_count(review_comment_error);
    }

    row_count
}

fn review_composer_row_count(error: Option<&str>) -> usize {
    if error.is_some() {
        REVIEW_COMPOSER_ROWS_WITH_ERROR
    } else {
        REVIEW_COMPOSER_ROWS
    }
}

fn review_line_target_for_line(
    file: &DiffFile,
    hunk_index: usize,
    line_index: usize,
    line: &DiffLine,
) -> Option<ReviewLineTarget> {
    match line.kind {
        DiffLineKind::Metadata => None,
        DiffLineKind::Removed => {
            let line_number = line.old_line?;
            Some(ReviewLineTarget {
                hunk_index,
                line_index,
                range: ReviewCommentRange {
                    path: file.path.clone(),
                    line: line_number,
                    side: ReviewSide::Left,
                    start_line: None,
                    start_side: None,
                },
            })
        }
        DiffLineKind::Added | DiffLineKind::Context => {
            line.new_line.map(|line_number| ReviewLineTarget {
                hunk_index,
                line_index,
                range: ReviewCommentRange {
                    path: file.path.clone(),
                    line: line_number,
                    side: ReviewSide::Right,
                    start_line: None,
                    start_side: None,
                },
            })
        }
    }
}

fn review_comment_range_matches_line(
    file: &DiffFile,
    range: &ReviewCommentRange,
    line: &DiffLine,
) -> bool {
    if !review_comment_range_matches_file(file, range) {
        return false;
    }

    match range.side {
        ReviewSide::Left => line.old_line.is_some_and(|line_number| {
            line_number >= range.start_line.unwrap_or(range.line) && line_number <= range.line
        }),
        ReviewSide::Right => line.new_line.is_some_and(|line_number| {
            line_number >= range.start_line.unwrap_or(range.line) && line_number <= range.line
        }),
    }
}

fn review_comment_range_matches_file(file: &DiffFile, range: &ReviewCommentRange) -> bool {
    path_matches_file(file, &range.path)
}

fn review_comment_range_label(range: &ReviewCommentRange) -> String {
    let side = match range.side {
        ReviewSide::Left => "left",
        ReviewSide::Right => "right",
    };

    if let Some(start_line) = range.start_line {
        format!("{side} lines {start_line}-{}", range.line)
    } else {
        format!("{side} line {}", range.line)
    }
}

fn path_matches_file(file: &DiffFile, path: &str) -> bool {
    path == file.path || file.previous_path.as_deref() == Some(path)
}

fn render_line_number(line: Option<u32>, width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_none()
        .pr_2()
        .text_right()
        .text_color(rgb(0x64748b))
        .child(line.map_or_else(String::new, |line| line.to_string()))
}

fn line_number_width_for_diff(diff: &ParsedDiff) -> f32 {
    let max_line = diff
        .hunks
        .iter()
        .flat_map(|hunk| hunk.lines.iter())
        .flat_map(|line| [line.old_line, line.new_line])
        .flatten()
        .max()
        .unwrap_or(1);
    let digits = max_line.to_string().len() as f32;

    (digits * LINE_NUMBER_DIGIT_WIDTH + LINE_NUMBER_PADDING).max(MIN_LINE_NUMBER_WIDTH)
}

#[cfg(test)]
mod tests {
    use harbor_domain::FileStatus;

    use crate::diff::parse_unified_diff;

    use super::*;

    #[test]
    fn keeps_small_diff_gutters_compact() {
        let diff = parse_unified_diff("@@ -8,2 +8,2 @@\n one\n two\n");

        assert_eq!(line_number_width_for_diff(&diff), MIN_LINE_NUMBER_WIDTH);
    }

    #[test]
    fn expands_gutter_for_large_line_numbers() {
        let diff = parse_unified_diff("@@ -99999,2 +100000,2 @@\n context\n-removed\n+added\n");

        assert_eq!(
            line_number_width_for_diff(&diff),
            6.0 * LINE_NUMBER_DIGIT_WIDTH + LINE_NUMBER_PADDING
        );
    }

    #[test]
    fn selects_right_side_target_for_added_line() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -1 +1,2 @@\n context\n+added\n");
        let target = review_line_target_for_line(&file, 0, 1, &diff.hunks[0].lines[1])
            .expect("added line should be commentable");

        assert_eq!(target.range.path, "src/lib.rs");
        assert_eq!(target.range.side, ReviewSide::Right);
        assert_eq!(target.range.line, 2);
        assert_eq!(target.range.start_line, None);
    }

    #[test]
    fn selects_left_side_target_for_removed_line() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -10,2 +10 @@\n-removed\n context\n");
        let target = review_line_target_for_line(&file, 0, 0, &diff.hunks[0].lines[0])
            .expect("removed line should be commentable");

        assert_eq!(target.range.path, "src/lib.rs");
        assert_eq!(target.range.side, ReviewSide::Left);
        assert_eq!(target.range.line, 10);
        assert_eq!(target.range.start_line, None);
    }

    #[test]
    fn counts_inline_composer_row() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -1 +1,2 @@\n context\n+added\n");
        let target = review_line_target_for_line(&file, 0, 1, &diff.hunks[0].lines[1])
            .expect("added line should be commentable");
        let composer = ReviewComposer {
            anchor: target.clone(),
            range: target.range,
        };

        assert_eq!(
            diff_row_count_with_review_controls(&diff, &file, &[], Some(&composer), None),
            3 + REVIEW_COMPOSER_ROWS
        );
    }

    #[test]
    fn builds_multiline_right_side_review_range() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -1 +1,3 @@\n context\n+added\n+again\n");
        let start = review_line_target_for_line(&file, 0, 1, &diff.hunks[0].lines[1])
            .expect("added line should be commentable");
        let end = review_line_target_for_line(&file, 0, 2, &diff.hunks[0].lines[2])
            .expect("added line should be commentable");

        let range = crate::workspace::review_range_from_targets(&start, &end).unwrap();

        assert_eq!(range.path, "src/lib.rs");
        assert_eq!(range.side, ReviewSide::Right);
        assert_eq!(range.start_line, Some(2));
        assert_eq!(range.start_side, Some(ReviewSide::Right));
        assert_eq!(range.line, 3);
    }

    #[test]
    fn rejects_mixed_side_review_range() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -1 +1 @@\n-old\n+new\n");
        let left = review_line_target_for_line(&file, 0, 0, &diff.hunks[0].lines[0])
            .expect("removed line should be commentable");
        let right = review_line_target_for_line(&file, 0, 1, &diff.hunks[0].lines[1])
            .expect("added line should be commentable");

        let error = crate::workspace::review_range_from_targets(&left, &right)
            .expect_err("mixed side selection should fail");

        assert_eq!(error, "Review comments can only span one diff side");
    }

    fn test_file(path: &str) -> DiffFile {
        DiffFile {
            path: path.to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 1,
            changes: 2,
            patch: None,
        }
    }
}
