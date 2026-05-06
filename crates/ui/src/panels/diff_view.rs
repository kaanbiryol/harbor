#[path = "diff_view/inline_reviews.rs"]
mod inline_reviews;
#[path = "diff_view/layout.rs"]
mod layout;

use std::{collections::HashSet, ops::Range};

use gpui::{
    AnyElement, App, Bounds, Context, Entity, IntoElement, ListHorizontalSizingBehavior,
    MouseButton, Pixels, Point, StyledText, UniformListDecoration, UniformListScrollHandle, Window,
    div, prelude::*, px, rgb, uniform_list,
};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::InputState,
};
use harbor_domain::{DiffFile, ReviewThread, ReviewThreadState};

use crate::diff::{DiffHunk, DiffLine, DiffLineKind, ParsedDiff};
use crate::diff_reviews::{anchored_review_threads, review_threads_for_line};
use crate::workspace::{
    AppView, PendingReviewSession, ReviewCommentUiError, ReviewComposer, ReviewLineSelection,
    ReviewLineTarget, ReviewReactionAction, ReviewThreadUiError,
};

#[cfg(test)]
pub(crate) use inline_reviews::{
    github_avatar_url_for_login, review_comment_action_visibility, review_comment_avatar_url,
    review_reaction_button_label, review_reaction_emoji, visible_review_reaction_contents,
};
use inline_reviews::{
    render_review_composer_inline, render_review_composer_spacer, render_review_marker,
    render_review_thread_inline,
};
pub(crate) use layout::{continuous_diff_file_row_index, continuous_diff_hunk_row_index};
use layout::{
    continuous_diff_row_count, continuous_diff_section_body_row_count,
    continuous_diff_section_for_row, file_is_reviewed, inline_block_render_anchor,
    line_number_width_for_diff, parsed_diff_for_file, review_comment_range_matches_file,
    review_comment_range_matches_line, review_composer_row_count, review_line_target_for_line,
    review_thread_inline_rows_with_controls, row_in_range,
};

const MIN_LINE_NUMBER_WIDTH: f32 = 28.0;
const LINE_NUMBER_PADDING: f32 = 8.0;
const LINE_NUMBER_DIGIT_WIDTH: f32 = 8.0;
const DIFF_ROW_HEIGHT: f32 = 24.0;
const DIFF_FILE_HEADER_ROWS: usize = 2;
const DIFF_FILE_HEADER_HEIGHT: f32 = DIFF_ROW_HEIGHT * 2.0;
const REVIEW_COMPOSER_ROWS: usize = 8;
const REVIEW_COMPOSER_ROWS_WITH_ERROR: usize = 9;
const REVIEW_COMPOSER_MAX_WIDTH: f32 = 820.0;
const REVIEW_THREAD_REPLY_ROWS: usize = 5;
const REVIEW_COMMENT_EDIT_ROWS: usize = 4;
const REVIEW_MARKER_WIDTH: f32 = 24.0;
const PREFIX_WIDTH: f32 = 16.0;

pub(crate) fn render_diff_panel(
    files: &[DiffFile],
    diffs: &[Option<ParsedDiff>],
    visible_file_indices: &[usize],
    reviewed_file_paths: &HashSet<String>,
    review_threads: &[ReviewThread],
    review_composer: Option<&ReviewComposer>,
    review_comment_error: Option<&str>,
    active_review_thread_reply: Option<&str>,
    active_review_comment_edit: Option<&str>,
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
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0xf87171))
                    .child(error.to_string()),
            )
            .into_any_element();
    }

    if visible_file_indices.is_empty() {
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
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .bg(rgb(0x0c0f12))
                    .p_3()
                    .text_color(rgb(0x9aa4b2))
                    .child(if files.is_empty() {
                        "No changed files to preview"
                    } else {
                        "No changed files match the current filters"
                    }),
            )
            .into_any_element();
    }

    let row_count = continuous_diff_row_count(
        files,
        diffs,
        visible_file_indices,
        reviewed_file_paths,
        review_threads,
        review_composer,
        review_comment_error,
        active_review_thread_reply,
        active_review_comment_edit,
    );
    let view_entity = cx.entity().clone();
    let processor_view_entity = view_entity.clone();

    div()
        .image_cache(gpui::retain_all("diff-review-avatar-cache"))
        .id("diff-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .text_color(rgb(0xf1f5f9))
                .child("Unified diff preview")
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x9aa4b2))
                        .child(format!("{} files", visible_file_indices.len())),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .border_1()
                .border_color(rgb(0x242a31))
                .bg(rgb(0x0c0f12))
                .overflow_hidden()
                .child(
                    uniform_list(
                        "diff-lines-list",
                        row_count,
                        cx.processor(move |view, range: std::ops::Range<usize>, _window, _cx| {
                            let visible_file_indices = view.visible_file_indices(_cx);

                            render_continuous_diff_rows(
                                view.diff_files(),
                                view.parsed_diffs(),
                                &visible_file_indices,
                                view.reviewed_file_paths(),
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
                                view.review_comment_edit_comment_id.as_deref(),
                                view.review_comment_edit_input.clone(),
                                view.review_comment_edit_input
                                    .read(_cx)
                                    .value()
                                    .trim()
                                    .is_empty(),
                                view.is_submitting_review_comment_edit,
                                view.review_comment_edit_error.as_ref(),
                                view.review_comment_action_comment_id.as_deref(),
                                view.review_comment_action_error.as_ref(),
                                view.review_reaction_action.as_ref(),
                                view.review_reaction_error.as_ref(),
                                view.active_file_index(),
                                view.active_hunk,
                                processor_view_entity.clone(),
                                range,
                            )
                        }),
                    )
                    .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
                    .with_decoration(DiffStickyHeaderDecoration {
                        view_entity: view_entity.clone(),
                    })
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

struct DiffStickyHeaderDecoration {
    view_entity: Entity<AppView>,
}

impl UniformListDecoration for DiffStickyHeaderDecoration {
    fn compute(
        &self,
        visible_range: Range<usize>,
        bounds: Bounds<Pixels>,
        scroll_offset: Point<Pixels>,
        item_height: Pixels,
        _item_count: usize,
        _window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        if visible_range.is_empty() {
            return div().into_any_element();
        }

        let view = self.view_entity.read(cx);
        let visible_file_indices = view.visible_file_indices(cx);
        let Some(section) = continuous_diff_section_for_row(
            view.diff_files(),
            view.parsed_diffs(),
            &visible_file_indices,
            view.reviewed_file_paths(),
            visible_range.start,
            &view.review_threads,
            view.review_composer.as_ref(),
            view.review_comment_error.as_deref(),
            view.review_thread_reply_thread_id.as_deref(),
            view.review_comment_edit_comment_id.as_deref(),
        ) else {
            return div().into_any_element();
        };

        let scroll_top = -scroll_offset.y;
        let header_top = item_height * section.header_row_index;
        if section.header_row_index == visible_range.start && scroll_top <= header_top {
            return div().into_any_element();
        }

        let Some(file) = view.diff_files().get(section.file_index).cloned() else {
            return div().into_any_element();
        };

        div()
            .relative()
            .w(bounds.size.width)
            .h(bounds.size.height)
            .child(
                div()
                    .absolute()
                    .top(-scroll_offset.y)
                    .left(-scroll_offset.x)
                    .w(bounds.size.width)
                    .child(render_diff_file_section_header(
                        section.file_index,
                        file,
                        section.hunk_count,
                        section.file_index == view.active_file_index(),
                        section.reviewed,
                        true,
                        self.view_entity.clone(),
                    )),
            )
            .into_any_element()
    }
}

fn render_continuous_diff_rows(
    files: &[DiffFile],
    diffs: &[Option<ParsedDiff>],
    visible_file_indices: &[usize],
    reviewed_file_paths: &HashSet<String>,
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
    active_review_comment_edit: Option<&str>,
    review_comment_edit_input: Entity<InputState>,
    review_comment_edit_body_empty: bool,
    is_submitting_review_comment_edit: bool,
    review_comment_edit_error: Option<&ReviewCommentUiError>,
    review_comment_action_comment_id: Option<&str>,
    review_comment_action_error: Option<&ReviewCommentUiError>,
    review_reaction_action: Option<&ReviewReactionAction>,
    review_reaction_error: Option<&ReviewCommentUiError>,
    active_file: usize,
    active_hunk: usize,
    view_entity: Entity<AppView>,
    range: std::ops::Range<usize>,
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
            review_threads,
            review_composer,
            review_comment_error,
            active_review_thread_reply,
            active_review_comment_edit,
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
                            *file_index == active_file,
                            reviewed,
                            false,
                            view_entity.clone(),
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
                review_threads,
                review_composer,
                review_line_selection,
                pending_review,
                review_comment_input.clone(),
                review_comment_body_empty,
                is_submitting_review_comment,
                review_comment_error,
                active_review_thread_reply,
                review_thread_reply_input.clone(),
                review_thread_reply_body_empty,
                is_submitting_review_thread_reply,
                review_thread_reply_error,
                review_thread_action_thread_id,
                review_thread_action_error,
                active_review_comment_edit,
                review_comment_edit_input.clone(),
                review_comment_edit_body_empty,
                is_submitting_review_comment_edit,
                review_comment_edit_error,
                review_comment_action_comment_id,
                review_comment_action_error,
                review_reaction_action,
                review_reaction_error,
                (*file_index == active_file).then_some(active_hunk),
                line_number_width,
                view_entity.clone(),
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

fn render_diff_file_section_header(
    file_index: usize,
    file: DiffFile,
    hunk_count: Option<usize>,
    active: bool,
    reviewed: bool,
    sticky: bool,
    view_entity: Entity<AppView>,
) -> AnyElement {
    let hunk_label = hunk_count.map_or_else(
        || "no parsed hunks".to_string(),
        |count| {
            if count == 1 {
                "1 hunk".to_string()
            } else {
                format!("{count} hunks")
            }
        },
    );
    let header_id = if sticky {
        format!("sticky-diff-file-header-{file_index}")
    } else {
        format!("diff-file-header-{file_index}")
    };
    let review_button = Button::new(format!(
        "{}-diff-file-reviewed-{file_index}",
        if sticky { "sticky" } else { "row" }
    ))
    .icon(if reviewed {
        IconName::Check
    } else {
        IconName::Eye
    })
    .small()
    .compact()
    .tooltip(if reviewed {
        "Mark as unreviewed"
    } else {
        "Mark as reviewed"
    });
    let review_button = if reviewed {
        review_button.primary()
    } else {
        review_button.ghost()
    };
    let review_button = review_button.on_click({
        let view_entity = view_entity.clone();
        move |_, _, cx| {
            view_entity.update(cx, |view, cx| {
                view.toggle_changed_file_reviewed(file_index, cx);
            });
            cx.stop_propagation();
        }
    });
    let path = file.path.clone();
    let select_view_entity = view_entity.clone();

    div()
        .id(header_id)
        .h(px(DIFF_FILE_HEADER_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .px_3()
        .border_1()
        .border_color(if active {
            rgb(0x3b82f6)
        } else if sticky {
            rgb(0x334155)
        } else {
            rgb(0x2f3a4a)
        })
        .bg(if active {
            rgb(0x18243b)
        } else if reviewed {
            rgb(0x111820)
        } else {
            rgb(0x141c2a)
        })
        .font_family(".SystemUIFont")
        .text_color(rgb(0xf1f5f9))
        .whitespace_nowrap()
        .cursor_pointer()
        .when(sticky, |element| element.shadow_lg())
        .hover(|element| element.bg(rgb(0x172033)))
        .on_click(move |_, _, cx| {
            select_view_entity.update(cx, |view, cx| {
                view.select_file(file_index, cx);
            });
        })
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap_3()
                .child(review_button)
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(16.0))
                        .font_medium()
                        .text_color(if reviewed {
                            rgb(0xa7b0bd)
                        } else {
                            rgb(0xf1f5f9)
                        })
                        .child(path),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_3()
                .text_xs()
                .font_medium()
                .text_color(rgb(0xa7b0bd))
                .child(
                    div()
                        .text_color(rgb(0xcbd5e1))
                        .child(format!("{:?}", file.status)),
                )
                .child(
                    div()
                        .text_color(rgb(0x34d399))
                        .child(format!("+{}", file.additions)),
                )
                .child(
                    div()
                        .text_color(rgb(0xf87171))
                        .child(format!("-{}", file.deletions)),
                )
                .child(div().text_color(rgb(0x9aa4b2)).child(hunk_label))
                .when(reviewed, |element| {
                    element.child(
                        div()
                            .rounded_xs()
                            .border_1()
                            .border_color(rgb(0x2f4f3e))
                            .bg(rgb(0x12241b))
                            .px_1()
                            .text_xs()
                            .text_color(rgb(0x86efac))
                            .child("reviewed"),
                    )
                }),
        )
        .into_any_element()
}

fn render_diff_file_section_header_spacer() -> impl IntoElement {
    div().h(px(DIFF_ROW_HEIGHT)).w_full()
}

fn render_diff_unavailable_row(row_index: usize) -> impl IntoElement {
    div()
        .id(format!("diff-unavailable-{row_index}"))
        .h(px(DIFF_ROW_HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .px_2()
        .bg(rgb(0x0c0f12))
        .font_family(".SystemUIFont")
        .text_color(rgb(0xfbbf24))
        .whitespace_nowrap()
        .child("Diff unavailable via GitHub API. Local checkout fallback will be added.")
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
    active_review_comment_edit: Option<&str>,
    review_comment_edit_input: Entity<InputState>,
    review_comment_edit_body_empty: bool,
    is_submitting_review_comment_edit: bool,
    review_comment_edit_error: Option<&ReviewCommentUiError>,
    review_comment_action_comment_id: Option<&str>,
    review_comment_action_error: Option<&ReviewCommentUiError>,
    review_reaction_action: Option<&ReviewReactionAction>,
    review_reaction_error: Option<&ReviewCommentUiError>,
    active_hunk: Option<usize>,
    line_number_width: f32,
    view_entity: Entity<AppView>,
    row_index: &mut usize,
    range: &std::ops::Range<usize>,
    rows: &mut Vec<AnyElement>,
) {
    let anchored_threads = anchored_review_threads(file, review_threads);
    let review_marker_width = REVIEW_MARKER_WIDTH;
    let active_selection_range = review_line_selection.and_then(|selection| {
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
                        view_entity.clone(),
                    )
                    .into_any_element(),
                );
            }
            *row_index += 1;

            let composer_ends_here = review_composer.is_some_and(|composer| {
                review_comment_range_matches_file(file, &composer.range)
                    && composer.anchor.hunk_index == hunk_index
                    && composer.anchor.line_index == line_index
            });

            if composer_ends_here {
                let composer_row_count = review_composer_row_count(review_comment_error);
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
                            if let Some(composer) = review_composer.cloned() {
                                rows.push(render_virtualized_inline_block(
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

            for thread in matching_threads {
                let thread_row_count = review_thread_inline_rows_with_controls(
                    thread,
                    active_review_thread_reply,
                    active_review_comment_edit,
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
                                    active_review_comment_edit,
                                    review_comment_edit_input.clone(),
                                    review_comment_edit_body_empty,
                                    is_submitting_review_comment_edit,
                                    review_comment_edit_error,
                                    review_comment_action_comment_id,
                                    review_comment_action_error,
                                    review_reaction_action,
                                    review_reaction_error,
                                    view_entity.clone(),
                                )
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

fn render_line_number(line: Option<u32>, width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_none()
        .pr_2()
        .text_right()
        .text_color(rgb(0x64748b))
        .child(line.map_or_else(String::new, |line| line.to_string()))
}
