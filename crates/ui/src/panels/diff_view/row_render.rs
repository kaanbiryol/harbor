use std::collections::HashSet;

use gpui::{
    AnyElement, Entity, IntoElement, MouseButton, SharedString, StyledText, div, prelude::*, px,
};
use harbor_domain::{DiffFile, ReviewThreadState};

use crate::{
    diff::{DiffLine, ParsedDiff},
    diff_reviews::{anchored_review_threads, review_threads_for_line},
    visual::color,
    workspace::{AppView, ReviewLineTarget},
};

use super::{
    DIFF_CODE_FONT_SIZE, DIFF_ROW_HEIGHT, PREFIX_WIDTH, REVIEW_MARKER_WIDTH,
    file_section::{render_diff_file_section_header, render_diff_unavailable_row},
    inline_review_layout::{
        review_comment_range_matches_file, review_comment_range_matches_line,
        review_line_target_for_line,
    },
    inline_reviews::{
        ReviewCommentListRenderState, ReviewComposerRenderState, ReviewThreadRenderState,
        render_review_composer_inline, render_review_marker, render_review_thread_inline,
    },
    layout::{DiffListItem, file_is_reviewed, line_number_width_for_diff, parsed_diff_for_file},
    line_style::{DiffLineStyleInput, diff_line_style},
    row_state::DiffRowRenderState,
};

pub(super) fn render_diff_list_item(
    item: Option<&DiffListItem>,
    files: &[DiffFile],
    diffs: &[Option<ParsedDiff>],
    reviewed_file_paths: &HashSet<String>,
    row_state: &DiffRowRenderState<'_>,
    item_index: usize,
) -> AnyElement {
    let Some(item) = item else {
        return div().into_any_element();
    };

    match item {
        DiffListItem::FileHeader {
            file_index,
            expanded,
        } => {
            let Some(file) = files.get(*file_index).cloned() else {
                return div().into_any_element();
            };
            let reviewed = file_is_reviewed(&file, reviewed_file_paths);

            render_diff_file_section_header(
                *file_index,
                file,
                *file_index == row_state.active_file,
                reviewed,
                *expanded,
                false,
                row_state.view_entity.clone(),
            )
            .into_any_element()
        }
        DiffListItem::Line {
            file_index,
            hunk_index,
            line_index,
        } => render_diff_line_item(
            files,
            diffs,
            *file_index,
            *hunk_index,
            *line_index,
            row_state,
            item_index,
        ),
        DiffListItem::ReviewComposer {
            file_index,
            hunk_index,
            line_index,
        } => render_review_composer_item(
            files,
            diffs,
            *file_index,
            *hunk_index,
            *line_index,
            row_state,
        ),
        DiffListItem::ReviewThread {
            file_index,
            thread_id,
            ..
        } => render_review_thread_item(diffs, *file_index, thread_id, row_state),
        DiffListItem::DiffUnavailable { .. } => {
            render_diff_unavailable_row(item_index).into_any_element()
        }
    }
}

fn render_diff_line_item(
    files: &[DiffFile],
    diffs: &[Option<ParsedDiff>],
    file_index: usize,
    hunk_index: usize,
    line_index: usize,
    row_state: &DiffRowRenderState<'_>,
    item_index: usize,
) -> AnyElement {
    let Some(file) = files.get(file_index) else {
        return div().into_any_element();
    };
    let Some(diff) = parsed_diff_for_file(diffs, file_index) else {
        return div().into_any_element();
    };
    let Some(line) = diff
        .hunks
        .get(hunk_index)
        .and_then(|hunk| hunk.lines.get(line_index))
    else {
        return div().into_any_element();
    };

    let anchored_threads = anchored_review_threads(file, row_state.review_threads);
    let matching_threads = review_threads_for_line(&anchored_threads, line);
    let active_selection_range = row_state.review_line_selection.and_then(|selection| {
        crate::workspace::review_range_from_targets(&selection.anchor, &selection.current).ok()
    });
    let review_line_target = review_line_target_for_line(file, hunk_index, line_index, line);
    let selected_for_comment = row_state
        .review_composer
        .is_some_and(|composer| review_comment_range_matches_line(file, &composer.range, line));
    let dragging_for_comment = active_selection_range
        .as_ref()
        .is_some_and(|range| review_comment_range_matches_line(file, range, line));
    let has_unresolved_thread = matching_threads
        .iter()
        .any(|thread| thread.state == ReviewThreadState::Unresolved);
    let has_thread_anchor = !matching_threads.is_empty();
    let has_thread_range = row_state
        .review_threads
        .iter()
        .filter_map(|thread| thread.range.as_ref())
        .any(|range| review_comment_range_matches_line(file, range, line));

    render_diff_line(DiffLineRenderInput {
        item_index,
        line,
        thread_count: matching_threads.len(),
        has_unresolved_thread,
        dragging_for_comment,
        selected_for_comment,
        has_thread_anchor,
        has_thread_range,
        review_line_target,
        line_number_width: line_number_width_for_diff(diff),
        review_marker_width: REVIEW_MARKER_WIDTH,
        view_entity: row_state.view_entity.clone(),
        mono_font_family: &row_state.mono_font_family,
    })
    .into_any_element()
}

fn render_review_composer_item(
    files: &[DiffFile],
    diffs: &[Option<ParsedDiff>],
    file_index: usize,
    hunk_index: usize,
    line_index: usize,
    row_state: &DiffRowRenderState<'_>,
) -> AnyElement {
    let Some(file) = files.get(file_index) else {
        return div().into_any_element();
    };
    let Some(diff) = parsed_diff_for_file(diffs, file_index) else {
        return div().into_any_element();
    };
    let Some(composer) = row_state.review_composer.cloned() else {
        return div().into_any_element();
    };
    if !review_comment_range_matches_file(file, &composer.range)
        || composer.anchor.hunk_index != hunk_index
        || composer.anchor.line_index != line_index
    {
        return div().into_any_element();
    }

    render_review_composer_inline(ReviewComposerRenderState {
        composer,
        has_pending_review: row_state.pending_review.is_some(),
        input: row_state.review_comment_input.clone(),
        body_empty: row_state.review_comment_body_empty,
        is_submitting: row_state.is_submitting_review_comment,
        error: row_state.review_comment_error.map(ToString::to_string),
        line_number_width: line_number_width_for_diff(diff),
        review_marker_width: REVIEW_MARKER_WIDTH,
        view_entity: row_state.view_entity.clone(),
    })
    .into_any_element()
}

fn render_review_thread_item(
    diffs: &[Option<ParsedDiff>],
    file_index: usize,
    thread_id: &str,
    row_state: &DiffRowRenderState<'_>,
) -> AnyElement {
    let Some(diff) = parsed_diff_for_file(diffs, file_index) else {
        return div().into_any_element();
    };
    let Some(thread) = row_state
        .review_threads
        .iter()
        .find(|thread| thread.id == thread_id)
    else {
        return div().into_any_element();
    };

    render_review_thread_inline(ReviewThreadRenderState {
        thread,
        line_number_width: line_number_width_for_diff(diff),
        active_review_thread_reply: row_state.active_review_thread_reply,
        review_thread_reply_input: row_state.review_thread_reply_input.clone(),
        reply_body_empty: row_state.review_thread_reply_body_empty,
        is_submitting_reply: row_state.is_submitting_review_thread_reply,
        reply_error: row_state.review_thread_reply_error,
        action_thread_id: row_state.review_thread_action_thread_id,
        action_error: row_state.review_thread_action_error,
        comments: review_comment_list_state(row_state),
        view_entity: row_state.view_entity.clone(),
    })
    .into_any_element()
}

fn review_comment_list_state<'a>(
    row_state: &DiffRowRenderState<'a>,
) -> ReviewCommentListRenderState<'a> {
    ReviewCommentListRenderState {
        active_review_comment_edit: row_state.active_review_comment_edit,
        review_comment_edit_input: row_state.review_comment_edit_input.clone(),
        edit_body_empty: row_state.review_comment_edit_body_empty,
        is_submitting_edit: row_state.is_submitting_review_comment_edit,
        edit_error: row_state.review_comment_edit_error,
        action_comment_id: row_state.review_comment_action_comment_id,
        comment_action_error: row_state.review_comment_action_error,
        reaction_action: row_state.review_reaction_action,
        reaction_error: row_state.review_reaction_error,
        view_entity: row_state.view_entity.clone(),
    }
}

struct DiffLineRenderInput<'a> {
    item_index: usize,
    line: &'a DiffLine,
    thread_count: usize,
    has_unresolved_thread: bool,
    dragging_for_comment: bool,
    selected_for_comment: bool,
    has_thread_anchor: bool,
    has_thread_range: bool,
    review_line_target: Option<ReviewLineTarget>,
    line_number_width: f32,
    review_marker_width: f32,
    view_entity: Entity<AppView>,
    mono_font_family: &'a SharedString,
}

fn render_diff_line(input: DiffLineRenderInput<'_>) -> impl IntoElement {
    let DiffLineRenderInput {
        item_index,
        line,
        thread_count,
        has_unresolved_thread,
        dragging_for_comment,
        selected_for_comment,
        has_thread_anchor,
        has_thread_range,
        review_line_target,
        line_number_width,
        review_marker_width,
        view_entity,
        mono_font_family,
    } = input;
    let line_id = format!("diff-line-{item_index}");
    let style = diff_line_style(DiffLineStyleInput {
        kind: line.kind,
        dragging_for_comment,
        selected_for_comment,
        has_thread_anchor,
        has_thread_range,
        has_syntax_highlights: !line.syntax_highlights.is_empty(),
    });

    div()
        .id(line_id)
        .min_h(px(DIFF_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_start()
        .bg(style.background)
        .text_color(style.text_color)
        .font_family(mono_font_family.clone())
        .text_size(px(DIFF_CODE_FONT_SIZE))
        .line_height(px(DIFF_ROW_HEIGHT))
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
                .text_color(style.text_color)
                .child(style.prefix),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .text_color(style.code_text_color)
                .whitespace_normal()
                .child(
                    StyledText::new(line.text.clone())
                        .with_highlights(line.syntax_highlights.clone()),
                ),
        )
        .when_some(review_line_target, move |element, target| {
            let view_entity = view_entity.clone();
            let move_view_entity = view_entity.clone();
            let up_view_entity = view_entity.clone();
            let down_target = target.clone();
            let move_target = target.clone();

            element
                .cursor_pointer()
                .hover(move |element| element.bg(style.hover_background))
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

pub(crate) fn render_line_number(line: Option<u32>, width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_none()
        .pr_2()
        .text_right()
        .whitespace_nowrap()
        .overflow_hidden()
        .text_color(color::text_muted())
        .child(line.map_or_else(String::new, |line| line.to_string()))
}

#[cfg(test)]
#[path = "row_render_tests.rs"]
mod tests;
