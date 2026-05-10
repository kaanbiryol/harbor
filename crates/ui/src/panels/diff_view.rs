#[path = "diff_view/file_section.rs"]
mod file_section;
#[path = "diff_view/inline_review_layout.rs"]
mod inline_review_layout;
#[path = "diff_view/inline_reviews.rs"]
mod inline_reviews;
#[path = "diff_view/layout.rs"]
mod layout;
#[path = "diff_view/row_render.rs"]
mod row_render;
#[path = "diff_view/row_state.rs"]
mod row_state;

use std::{collections::HashSet, ops::Range};

use gpui::{
    AnyElement, App, Bounds, Context, Entity, IntoElement, ListHorizontalSizingBehavior, Pixels,
    Point, UniformListDecoration, UniformListScrollHandle, Window, div, prelude::*, rgb,
    uniform_list,
};
use harbor_domain::{DiffFile, ReviewThread};

use crate::diff::ParsedDiff;
use crate::workspace::{AppView, ReviewComposer};

use file_section::render_diff_file_section_header;
pub(crate) use layout::{
    ContinuousDiffLayoutInput, continuous_diff_file_row_index, continuous_diff_hunk_row_index,
};
use layout::{continuous_diff_row_count, continuous_diff_section_for_row};
use row_render::render_continuous_diff_rows;
pub(super) use row_render::render_line_number;
use row_state::DiffRowRenderState;

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

pub(crate) struct DiffPanelRenderInput<'a> {
    pub(crate) files: &'a [DiffFile],
    pub(crate) diffs: &'a [Option<ParsedDiff>],
    pub(crate) visible_file_indices: &'a [usize],
    pub(crate) reviewed_file_paths: &'a HashSet<String>,
    pub(crate) review_threads: &'a [ReviewThread],
    pub(crate) review_composer: Option<&'a ReviewComposer>,
    pub(crate) review_comment_error: Option<&'a str>,
    pub(crate) active_review_thread_reply: Option<&'a str>,
    pub(crate) active_review_comment_edit: Option<&'a str>,
    pub(crate) is_loading: bool,
    pub(crate) error: Option<&'a str>,
    pub(crate) scroll_handle: UniformListScrollHandle,
}

impl<'a> DiffPanelRenderInput<'a> {
    fn layout_input(&self) -> ContinuousDiffLayoutInput<'a> {
        ContinuousDiffLayoutInput {
            files: self.files,
            diffs: self.diffs,
            visible_file_indices: self.visible_file_indices,
            reviewed_file_paths: self.reviewed_file_paths,
            review_threads: self.review_threads,
            review_composer: self.review_composer,
            review_comment_error: self.review_comment_error,
            active_review_thread_reply: self.active_review_thread_reply,
            active_review_comment_edit: self.active_review_comment_edit,
        }
    }
}

pub(crate) fn render_diff_panel(
    input: DiffPanelRenderInput<'_>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    if input.is_loading {
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

    if let Some(error) = input.error {
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

    if input.visible_file_indices.is_empty() {
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
                    .child(if input.files.is_empty() {
                        "No changed files to preview"
                    } else {
                        "No changed files match the current filters"
                    }),
            )
            .into_any_element();
    }

    let row_count = continuous_diff_row_count(input.layout_input());
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
                        .child(format!("{} files", input.visible_file_indices.len())),
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

                            let row_state = DiffRowRenderState::from_view(
                                view,
                                _cx,
                                processor_view_entity.clone(),
                            );

                            render_continuous_diff_rows(
                                view.diff_files(),
                                view.parsed_diffs(),
                                &visible_file_indices,
                                view.reviewed_file_paths(),
                                &row_state,
                                range,
                            )
                        }),
                    )
                    .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
                    .with_decoration(DiffStickyHeaderDecoration {
                        view_entity: view_entity.clone(),
                    })
                    .track_scroll(&input.scroll_handle)
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
            layout_input_from_view(view, &visible_file_indices),
            visible_range.start,
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

fn layout_input_from_view<'a>(
    view: &'a AppView,
    visible_file_indices: &'a [usize],
) -> ContinuousDiffLayoutInput<'a> {
    ContinuousDiffLayoutInput {
        files: view.diff_files(),
        diffs: view.parsed_diffs(),
        visible_file_indices,
        reviewed_file_paths: view.reviewed_file_paths(),
        review_threads: &view.review_threads,
        review_composer: view.review_composer_state.composer.as_ref(),
        review_comment_error: view.review_comment_error.as_deref(),
        active_review_thread_reply: view.review_composer_state.thread_reply_thread_id.as_deref(),
        active_review_comment_edit: view
            .review_composer_state
            .comment_edit_comment_id
            .as_deref(),
    }
}
