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

use std::collections::HashSet;

use gpui::{Context, IntoElement, ListState, div, list, prelude::*, px, rgb};
use harbor_domain::{DiffFile, ReviewThread};

use crate::diff::ParsedDiff;
use crate::workspace::{AppView, ReviewComposer};

use file_section::render_diff_file_section_header;
use layout::continuous_diff_section_for_item;
pub(crate) use layout::{
    ContinuousDiffLayoutInput, DiffListItem, continuous_diff_file_item_index,
    continuous_diff_hunk_item_index, continuous_diff_items, sync_diff_list_state,
};
use row_render::render_diff_list_item;
pub(super) use row_render::render_line_number;
use row_state::DiffRowRenderState;

const MIN_LINE_NUMBER_WIDTH: f32 = 32.0;
const LINE_NUMBER_PADDING: f32 = 10.0;
const LINE_NUMBER_DIGIT_WIDTH: f32 = 10.0;
const DIFF_ROW_HEIGHT: f32 = 24.0;
const DIFF_FILE_HEADER_HEIGHT: f32 = DIFF_ROW_HEIGHT * 2.0;
const REVIEW_COMPOSER_MAX_WIDTH: f32 = 820.0;
const REVIEW_MARKER_WIDTH: f32 = 24.0;
const PREFIX_WIDTH: f32 = 16.0;

pub(crate) struct DiffPanelRenderInput<'a> {
    pub(crate) files: &'a [DiffFile],
    pub(crate) diffs: &'a [Option<ParsedDiff>],
    pub(crate) visible_file_indices: &'a [usize],
    pub(crate) reviewed_file_paths: &'a HashSet<String>,
    pub(crate) review_threads: &'a [ReviewThread],
    pub(crate) review_composer: Option<&'a ReviewComposer>,
    pub(crate) active_file_index: usize,
    pub(crate) is_loading: bool,
    pub(crate) error: Option<&'a str>,
    pub(crate) list_state: ListState,
    pub(crate) list_items: &'a [DiffListItem],
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

    let logical_scroll_top = input.list_state.logical_scroll_top();
    let sticky_section = continuous_diff_section_for_item(
        input.layout_input(),
        input.list_items,
        logical_scroll_top.item_ix,
    )
    .filter(|section| {
        section.header_item_index != logical_scroll_top.item_ix
            || logical_scroll_top.offset_in_item > px(0.0)
    });
    let view_entity = cx.entity().clone();
    let processor_view_entity = view_entity.clone();
    let list_items_for_render = input.list_items.to_vec();

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
                    div()
                        .relative()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .overflow_hidden()
                        .child(
                            list(
                                input.list_state.clone(),
                                cx.processor(move |view, item_index: usize, _window, cx| {
                                    let row_state = DiffRowRenderState::from_view(
                                        view,
                                        cx,
                                        processor_view_entity.clone(),
                                    );

                                    render_diff_list_item(
                                        list_items_for_render.get(item_index),
                                        view.diff_files(),
                                        view.parsed_diffs(),
                                        view.reviewed_file_paths(),
                                        &row_state,
                                        item_index,
                                    )
                                }),
                            )
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .font_family("Menlo")
                            .text_xs(),
                        )
                        .when_some(sticky_section, {
                            let view_entity = view_entity.clone();
                            move |element, section| {
                                let Some(file) = input.files.get(section.file_index).cloned()
                                else {
                                    return element;
                                };

                                element.child(
                                    div()
                                        .absolute()
                                        .top(px(0.0))
                                        .left(px(0.0))
                                        .right(px(0.0))
                                        .child(render_diff_file_section_header(
                                            section.file_index,
                                            file,
                                            section.hunk_count,
                                            section.file_index == input.active_file_index,
                                            section.reviewed,
                                            true,
                                            view_entity.clone(),
                                        )),
                                )
                            }
                        }),
                ),
        )
        .into_any_element()
}
