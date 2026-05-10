#![expect(
    clippy::too_many_arguments,
    reason = "diff row render helpers pass explicit immutable row state to keep virtualized row rendering local"
)]

use std::{collections::HashSet, ops::Range};

use gpui::{AnyElement, Entity, IntoElement, MouseButton, StyledText, div, prelude::*, px, rgb};
use harbor_domain::{DiffFile, ReviewThreadState};

use crate::{
    diff::{DiffHunk, DiffLine, DiffLineKind, ParsedDiff},
    diff_reviews::{anchored_review_threads, review_threads_for_line},
    workspace::{AppView, ReviewLineTarget},
};

use super::{
    DIFF_FILE_HEADER_ROWS, DIFF_ROW_HEIGHT, PREFIX_WIDTH, REVIEW_MARKER_WIDTH,
    file_section::{
        render_diff_file_section_header, render_diff_file_section_header_spacer,
        render_diff_unavailable_row,
    },
    inline_review_layout::{
        review_comment_range_matches_file, review_comment_range_matches_line,
        review_composer_row_count, review_line_target_for_line,
        review_thread_inline_rows_with_controls,
    },
    inline_reviews::{
        ReviewCommentListRenderState, ReviewComposerRenderState, ReviewThreadRenderState,
        render_review_composer_inline, render_review_composer_spacer, render_review_marker,
        render_review_thread_inline,
    },
    layout::{
        continuous_diff_section_body_row_count, file_is_reviewed, inline_block_render_anchor,
        line_number_width_for_diff, parsed_diff_for_file, row_in_range,
    },
    row_state::DiffRowRenderState,
};

pub(super) fn render_continuous_diff_rows(
    files: &[DiffFile],
    diffs: &[Option<ParsedDiff>],
    visible_file_indices: &[usize],
    reviewed_file_paths: &HashSet<String>,
    row_state: &DiffRowRenderState<'_>,
    range: Range<usize>,
) -> Vec<AnyElement> {
    let mut rows = Vec::with_capacity(range.len());
    let mut row_index = 0;

    for file_index in visible_file_indices {
        if row_index >= range.end {
            break;
        }

        let Some(file) = files.get(*file_index) else {
            continue;
        };
        let parsed_diff = parsed_diff_for_file(diffs, *file_index);
        let hunk_count = parsed_diff.map(|diff| diff.hunks.len());
        let reviewed = file_is_reviewed(file, reviewed_file_paths);
        let body_row_count = continuous_diff_section_body_row_count(
            *file_index,
            file,
            diffs,
            reviewed_file_paths,
            row_state.review_threads,
            row_state.review_composer,
            row_state.review_comment_error,
            row_state.active_review_thread_reply,
            row_state.active_review_comment_edit,
        );
        let section_row_count = DIFF_FILE_HEADER_ROWS + body_row_count;

        if row_index + section_row_count <= range.start {
            row_index += section_row_count;
            continue;
        }

        let header_start_row = row_index;
        let visible_header_row =
            inline_block_render_anchor(header_start_row, DIFF_FILE_HEADER_ROWS, &range);

        for header_row in 0..DIFF_FILE_HEADER_ROWS {
            if row_index >= range.end {
                row_index += DIFF_FILE_HEADER_ROWS - header_row;
                break;
            }

            if row_in_range(row_index, &range) {
                if let Some((render_row, visible_row_offset)) = visible_header_row
                    && render_row == row_index
                {
                    rows.push(render_virtualized_inline_block(
                        render_diff_file_section_header(
                            *file_index,
                            file.clone(),
                            hunk_count,
                            *file_index == row_state.active_file,
                            reviewed,
                            false,
                            row_state.view_entity.clone(),
                        ),
                        visible_row_offset,
                    ));
                } else {
                    rows.push(render_diff_file_section_header_spacer().into_any_element());
                }
            }

            row_index += 1;
        }

        if reviewed {
            continue;
        }

        if let Some(parsed_diff) = parsed_diff {
            let line_number_width = line_number_width_for_diff(parsed_diff);
            render_diff_rows(
                parsed_diff,
                file,
                row_state,
                (*file_index == row_state.active_file).then_some(row_state.active_hunk),
                line_number_width,
                &mut row_index,
                &range,
                &mut rows,
            );
        } else {
            if row_in_range(row_index, &range) {
                rows.push(render_diff_unavailable_row(row_index).into_any_element());
            }
            row_index += 1;
        }
    }

    rows
}

fn render_diff_rows(
    diff: &ParsedDiff,
    file: &DiffFile,
    row_state: &DiffRowRenderState<'_>,
    active_hunk: Option<usize>,
    line_number_width: f32,
    row_index: &mut usize,
    range: &Range<usize>,
    rows: &mut Vec<AnyElement>,
) {
    let anchored_threads = anchored_review_threads(file, row_state.review_threads);
    let review_marker_width = REVIEW_MARKER_WIDTH;
    let active_selection_range = row_state.review_line_selection.and_then(|selection| {
        crate::workspace::review_range_from_targets(&selection.anchor, &selection.current).ok()
    });

    for (hunk_index, hunk) in diff.hunks.iter().enumerate() {
        if *row_index >= range.end {
            break;
        }

        if row_in_range(*row_index, range) {
            rows.push(
                render_diff_hunk_row(hunk, hunk_index, active_hunk == Some(hunk_index))
                    .into_any_element(),
            );
        }
        *row_index += 1;

        for (line_index, line) in hunk.lines.iter().enumerate() {
            if *row_index >= range.end {
                break;
            }

            let matching_threads = review_threads_for_line(&anchored_threads, line);
            let review_line_target =
                review_line_target_for_line(file, hunk_index, line_index, line);
            let selected_for_comment = row_state.review_composer.is_some_and(|composer| {
                review_comment_range_matches_line(file, &composer.range, line)
            });
            let dragging_for_comment = active_selection_range
                .as_ref()
                .is_some_and(|range| review_comment_range_matches_line(file, range, line));
            let has_unresolved_thread = matching_threads
                .iter()
                .any(|thread| thread.state == ReviewThreadState::Unresolved);
            let has_thread_range = row_state
                .review_threads
                .iter()
                .filter_map(|thread| thread.range.as_ref())
                .any(|range| review_comment_range_matches_line(file, range, line));

            if row_in_range(*row_index, range) {
                rows.push(
                    render_diff_line(
                        *row_index,
                        line,
                        matching_threads.len(),
                        has_unresolved_thread,
                        dragging_for_comment,
                        selected_for_comment,
                        has_thread_range,
                        review_line_target.clone(),
                        line_number_width,
                        review_marker_width,
                        row_state.view_entity.clone(),
                    )
                    .into_any_element(),
                );
            }
            *row_index += 1;

            let composer_ends_here = row_state.review_composer.is_some_and(|composer| {
                review_comment_range_matches_file(file, &composer.range)
                    && composer.anchor.hunk_index == hunk_index
                    && composer.anchor.line_index == line_index
            });

            if composer_ends_here {
                let composer_row_count = review_composer_row_count(row_state.review_comment_error);
                let composer_start_row = *row_index;
                let visible_composer_row =
                    inline_block_render_anchor(composer_start_row, composer_row_count, range);

                for composer_row in 0..composer_row_count {
                    if *row_index >= range.end {
                        *row_index += composer_row_count - composer_row;
                        break;
                    }

                    if row_in_range(*row_index, range) {
                        if let Some((render_row, visible_row_offset)) = visible_composer_row
                            && render_row == *row_index
                        {
                            if let Some(composer) = row_state.review_composer.cloned() {
                                rows.push(render_virtualized_inline_block(
                                    render_review_composer_inline(ReviewComposerRenderState {
                                        composer,
                                        has_pending_review: row_state.pending_review.is_some(),
                                        input: row_state.review_comment_input.clone(),
                                        body_empty: row_state.review_comment_body_empty,
                                        is_submitting: row_state.is_submitting_review_comment,
                                        error: row_state
                                            .review_comment_error
                                            .map(ToString::to_string),
                                        row_count: composer_row_count,
                                        line_number_width,
                                        review_marker_width,
                                        view_entity: row_state.view_entity.clone(),
                                    })
                                    .into_any_element(),
                                    visible_row_offset,
                                ));
                            }
                        } else {
                            rows.push(render_review_composer_spacer().into_any_element());
                        }
                    }

                    *row_index += 1;
                }
            }

            let comment_state = ReviewCommentListRenderState {
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
            };

            for thread in matching_threads {
                let thread_row_count = review_thread_inline_rows_with_controls(
                    thread,
                    row_state.active_review_thread_reply,
                    row_state.active_review_comment_edit,
                );
                let thread_start_row = *row_index;
                let visible_thread_row =
                    inline_block_render_anchor(thread_start_row, thread_row_count, range);

                for thread_row in 0..thread_row_count {
                    if *row_index >= range.end {
                        *row_index += thread_row_count - thread_row;
                        break;
                    }

                    if row_in_range(*row_index, range) {
                        if let Some((render_row, visible_row_offset)) = visible_thread_row
                            && render_row == *row_index
                        {
                            rows.push(render_virtualized_inline_block(
                                render_review_thread_inline(ReviewThreadRenderState {
                                    thread,
                                    line_number_width,
                                    active_review_thread_reply: row_state
                                        .active_review_thread_reply,
                                    review_thread_reply_input: row_state
                                        .review_thread_reply_input
                                        .clone(),
                                    reply_body_empty: row_state.review_thread_reply_body_empty,
                                    is_submitting_reply: row_state
                                        .is_submitting_review_thread_reply,
                                    reply_error: row_state.review_thread_reply_error,
                                    action_thread_id: row_state.review_thread_action_thread_id,
                                    action_error: row_state.review_thread_action_error,
                                    comments: comment_state.clone(),
                                    view_entity: row_state.view_entity.clone(),
                                })
                                .into_any_element(),
                                visible_row_offset,
                            ));
                        } else {
                            rows.push(render_review_composer_spacer().into_any_element());
                        }
                    }

                    *row_index += 1;
                }
            }
        }
    }
}

fn render_virtualized_inline_block(content: AnyElement, visible_row_offset: usize) -> AnyElement {
    div()
        .h(px(DIFF_ROW_HEIGHT))
        .w_full()
        .relative()
        .child(
            div()
                .absolute()
                .top(px(-((visible_row_offset as f32) * DIFF_ROW_HEIGHT)))
                .left(px(0.0))
                .w_full()
                .child(content),
        )
        .into_any_element()
}

fn render_diff_hunk_row(hunk: &DiffHunk, index: usize, active: bool) -> impl IntoElement {
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

fn render_diff_line(
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
    let code_text_color = if line.syntax_highlights.is_empty() {
        text_color
    } else {
        rgb(0xd5dde7)
    };

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
        .child(div().flex_none().text_color(code_text_color).child(
            StyledText::new(line.text.clone()).with_highlights(line.syntax_highlights.clone()),
        ))
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

pub(crate) fn render_line_number(line: Option<u32>, width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_none()
        .pr_2()
        .text_right()
        .text_color(rgb(0x64748b))
        .child(line.map_or_else(String::new, |line| line.to_string()))
}
