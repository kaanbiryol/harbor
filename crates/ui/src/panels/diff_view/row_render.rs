use std::collections::HashSet;

use gpui::{AnyElement, Entity, IntoElement, MouseButton, StyledText, div, prelude::*, px, rgb};
use harbor_domain::{DiffFile, ReviewThreadState};

use crate::{
    diff::{DiffHunk, DiffLine, DiffLineKind, ParsedDiff},
    diff_reviews::{anchored_review_threads, review_threads_for_line},
    workspace::{AppView, ReviewLineTarget},
};

use super::{
    DIFF_ROW_HEIGHT, PREFIX_WIDTH, REVIEW_MARKER_WIDTH,
    file_section::{render_diff_file_section_header, render_diff_unavailable_row},
    inline_review_layout::{
        review_comment_range_matches_file, review_comment_range_matches_line,
        review_diff_line_anchor_label, review_line_target_for_line,
    },
    inline_reviews::{
        ReviewCommentListRenderState, ReviewComposerRenderState, ReviewThreadRenderState,
        render_review_composer_inline, render_review_marker, render_review_thread_inline,
    },
    layout::{DiffListItem, file_is_reviewed, line_number_width_for_diff, parsed_diff_for_file},
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
        DiffListItem::FileHeader { file_index } => {
            let Some(file) = files.get(*file_index).cloned() else {
                return div().into_any_element();
            };
            let hunk_count = parsed_diff_for_file(diffs, *file_index).map(|diff| diff.hunks.len());
            let reviewed = file_is_reviewed(&file, reviewed_file_paths);

            render_diff_file_section_header(
                *file_index,
                file,
                hunk_count,
                *file_index == row_state.active_file,
                reviewed,
                false,
                row_state.view_entity.clone(),
            )
            .into_any_element()
        }
        DiffListItem::Hunk {
            file_index,
            hunk_index,
        } => {
            let Some(hunk) = parsed_diff_for_file(diffs, *file_index)
                .and_then(|diff| diff.hunks.get(*hunk_index))
            else {
                return div().into_any_element();
            };
            render_diff_hunk_row(
                hunk,
                *hunk_index,
                *file_index == row_state.active_file && *hunk_index == row_state.active_hunk,
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
            hunk_index,
            line_index,
            thread_id,
        } => render_review_thread_item(
            files,
            diffs,
            *file_index,
            *hunk_index,
            *line_index,
            thread_id,
            row_state,
        ),
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
    files: &[DiffFile],
    diffs: &[Option<ParsedDiff>],
    file_index: usize,
    hunk_index: usize,
    line_index: usize,
    thread_id: &str,
    row_state: &DiffRowRenderState<'_>,
) -> AnyElement {
    let Some(file) = files.get(file_index) else {
        return div().into_any_element();
    };
    let Some(diff) = parsed_diff_for_file(diffs, file_index) else {
        return div().into_any_element();
    };
    let anchor_label = diff
        .hunks
        .get(hunk_index)
        .and_then(|hunk| hunk.lines.get(line_index))
        .and_then(|line| review_diff_line_anchor_label(file, line));
    let Some(thread) = row_state
        .review_threads
        .iter()
        .find(|thread| thread.id == thread_id)
    else {
        return div().into_any_element();
    };

    render_review_thread_inline(ReviewThreadRenderState {
        thread,
        anchor_label,
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

fn render_diff_hunk_row(hunk: &DiffHunk, index: usize, active: bool) -> impl IntoElement {
    div()
        .h(px(DIFF_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .gap_2()
        .overflow_hidden()
        .px_2()
        .border_1()
        .border_color(if active { rgb(0x3b82f6) } else { rgb(0x1a2029) })
        .bg(if active { rgb(0x172033) } else { rgb(0x1a2029) })
        .text_color(rgb(0x93c5fd))
        .whitespace_nowrap()
        .child(div().min_w_0().flex_1().truncate().child(format!(
            "hunk {}  {}",
            index + 1,
            hunk.header
        )))
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
    } = input;
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
    let thread_anchor_bg = match line.kind {
        DiffLineKind::Context | DiffLineKind::Metadata => rgb(0x272210),
        DiffLineKind::Added => rgb(0x253119),
        DiffLineKind::Removed => rgb(0x38221b),
    };
    let bg = if dragging_for_comment {
        dragging_bg
    } else if selected_for_comment {
        selected_bg
    } else if has_thread_anchor {
        thread_anchor_bg
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
    } else if has_thread_anchor {
        rgb(0x342b14)
    } else if has_thread_range {
        match line.kind {
            DiffLineKind::Added => rgb(0x193326),
            DiffLineKind::Removed => rgb(0x3a2327),
            DiffLineKind::Context | DiffLineKind::Metadata => rgb(0x1a2531),
        }
    } else {
        rgb(0x18212b)
    };
    let line_id = format!("diff-line-{item_index}");
    let code_text_color = if line.syntax_highlights.is_empty() {
        text_color
    } else {
        rgb(0xd5dde7)
    };

    div()
        .id(line_id)
        .min_h(px(DIFF_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_start()
        .bg(bg)
        .text_color(text_color)
        .font_family("Menlo")
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
                .text_color(text_color)
                .child(prefix),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .text_color(code_text_color)
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
        .whitespace_nowrap()
        .overflow_hidden()
        .text_color(rgb(0x64748b))
        .child(line.map_or_else(String::new, |line| line.to_string()))
}

#[cfg(test)]
mod tests {
    use gpui::{
        Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext, Window, div, px,
    };
    use gpui_component::{Root, Theme, ThemeMode};

    use crate::workspace::AppView;

    use super::*;

    #[gpui::test]
    async fn wraps_long_diff_line_in_narrow_panel(cx: &mut TestAppContext) {
        let cx = init_visual_diff_line_test(cx);

        cx.refresh().expect("test window should refresh");
        cx.run_until_parked();

        let bounds = cx
            .debug_bounds("diff-line-wrap-harness")
            .expect("diff line should render");
        assert!(
            bounds.size.height > px(DIFF_ROW_HEIGHT),
            "wrapped diff line height should exceed one row, got {:?}",
            bounds.size.height
        );
    }

    #[gpui::test]
    async fn keeps_line_numbers_single_line_in_wrapped_rows(cx: &mut TestAppContext) {
        let cx = init_visual_diff_line_test(cx);

        cx.refresh().expect("test window should refresh");
        cx.run_until_parked();

        let bounds = cx
            .debug_bounds("diff-line-number-wrap-harness")
            .expect("diff line should render");
        assert_eq!(
            bounds.size.height,
            px(DIFF_ROW_HEIGHT),
            "line number wrapping should not expand a one-line diff row"
        );
    }

    fn init_visual_diff_line_test(cx: &mut TestAppContext) -> &mut VisualTestContext {
        cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
        });

        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
            let harness = cx.new(|_| DiffLineWrapHarness { view_entity: view });
            Root::new(harness, window, cx)
        });

        cx
    }

    struct DiffLineWrapHarness {
        view_entity: Entity<AppView>,
    }

    impl Render for DiffLineWrapHarness {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let line = DiffLine {
                kind: DiffLineKind::Added,
                old_line: None,
                new_line: Some(12),
                text: "this long diff line should wrap within the narrow panel ".repeat(8),
                syntax_highlights: Vec::new(),
            };

            div().children([
                div()
                    .id("diff-line-wrap-harness")
                    .debug_selector(|| "diff-line-wrap-harness".to_string())
                    .w(px(220.0))
                    .child(render_diff_line(DiffLineRenderInput {
                        item_index: 0,
                        line: &line,
                        thread_count: 0,
                        has_unresolved_thread: false,
                        dragging_for_comment: false,
                        selected_for_comment: false,
                        has_thread_anchor: false,
                        has_thread_range: false,
                        review_line_target: None,
                        line_number_width: 36.0,
                        review_marker_width: REVIEW_MARKER_WIDTH,
                        view_entity: self.view_entity.clone(),
                    })),
                div()
                    .id("diff-line-number-wrap-harness")
                    .debug_selector(|| "diff-line-number-wrap-harness".to_string())
                    .w(px(220.0))
                    .child(render_diff_line(DiffLineRenderInput {
                        item_index: 1,
                        line: &DiffLine {
                            kind: DiffLineKind::Context,
                            old_line: Some(143),
                            new_line: Some(143),
                            text: "short line".to_string(),
                            syntax_highlights: Vec::new(),
                        },
                        thread_count: 0,
                        has_unresolved_thread: false,
                        dragging_for_comment: false,
                        selected_for_comment: false,
                        has_thread_anchor: false,
                        has_thread_range: false,
                        review_line_target: None,
                        line_number_width: line_number_width_for_diff(&ParsedDiff {
                            hunks: vec![DiffHunk {
                                header: "@@ -143,1 +143,1 @@".to_string(),
                                old_start: 143,
                                old_lines: 1,
                                new_start: 143,
                                new_lines: 1,
                                lines: vec![DiffLine {
                                    kind: DiffLineKind::Context,
                                    old_line: Some(143),
                                    new_line: Some(143),
                                    text: "short line".to_string(),
                                    syntax_highlights: Vec::new(),
                                }],
                            }],
                        }),
                        review_marker_width: REVIEW_MARKER_WIDTH,
                        view_entity: self.view_entity.clone(),
                    })),
            ])
        }
    }
}
