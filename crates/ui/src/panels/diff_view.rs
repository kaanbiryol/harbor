#[path = "diff_view/file_section.rs"]
mod file_section;
#[path = "diff_view/inline_review_layout.rs"]
mod inline_review_layout;
#[path = "diff_view/inline_reviews.rs"]
mod inline_reviews;
#[path = "diff_view/layout.rs"]
mod layout;
#[path = "diff_view/line_style.rs"]
mod line_style;
#[path = "diff_view/row_render.rs"]
mod row_render;
#[path = "diff_view/row_state.rs"]
mod row_state;

pub(crate) use inline_reviews::{
    ReviewCommentActionsMenuState, render_review_comment_actions_menu,
    render_review_comment_edit_composer, render_review_reactions, review_comment_ui_state,
};

use std::{collections::HashSet, sync::Arc};

use gpui::{Context, IntoElement, ListState, div, list, prelude::*, px};
use gpui_component::{
    Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
};
use harbor_domain::{DiffFile, PullRequest, ReviewThread};

use crate::diff::ParsedDiff;
use crate::diff_reviews::ReviewThreadIndex;
use crate::icons::Octicon;
use crate::visual::color;
use crate::workspace::{AppView, ReviewComposer};

use file_section::render_diff_file_section_header;
use layout::continuous_diff_section_for_item;
pub(crate) use layout::{
    ContinuousDiffLayoutInput, DiffListItem, continuous_diff_items, diff_file_item_index,
    diff_hunk_item_index, sync_diff_list_state,
};
use row_render::render_diff_list_item;
pub(super) use row_render::render_line_number;
use row_state::DiffRowRenderState;

const MIN_LINE_NUMBER_WIDTH: f32 = 32.0;
const LINE_NUMBER_PADDING: f32 = 10.0;
const LINE_NUMBER_DIGIT_WIDTH: f32 = 9.5;
const DIFF_CODE_FONT_SIZE: f32 = 12.5;
const DIFF_ROW_HEIGHT: f32 = 20.0;
const INLINE_REVIEW_FONT_SIZE: f32 = 13.0;
const DIFF_FILE_HEADER_HEIGHT: f32 = 44.0;
const REVIEW_COMPOSER_MAX_WIDTH: f32 = 820.0;
const REVIEW_MARKER_WIDTH: f32 = 24.0;
const PREFIX_WIDTH: f32 = 16.0;

pub(crate) struct DiffPanelRenderInput<'a> {
    pub(crate) pull_request: Option<&'a PullRequest>,
    pub(crate) files: &'a [DiffFile],
    pub(crate) diffs: &'a [Option<ParsedDiff>],
    pub(crate) visible_file_indices: &'a [usize],
    pub(crate) reviewed_file_paths: &'a HashSet<String>,
    pub(crate) expanded_diff_file_paths: &'a HashSet<String>,
    pub(crate) collapsed_diff_file_paths: &'a HashSet<String>,
    pub(crate) review_threads: &'a [ReviewThread],
    pub(crate) review_composer: Option<&'a ReviewComposer>,
    pub(crate) active_file_index: usize,
    pub(crate) commit_sha: Option<&'a str>,
    pub(crate) is_loading: bool,
    pub(crate) error: Option<&'a str>,
    pub(crate) list_state: ListState,
    pub(crate) list_items: Arc<[DiffListItem]>,
}

impl<'a> DiffPanelRenderInput<'a> {
    fn layout_input(&self) -> ContinuousDiffLayoutInput<'a> {
        ContinuousDiffLayoutInput {
            files: self.files,
            diffs: self.diffs,
            visible_file_indices: self.visible_file_indices,
            reviewed_file_paths: self.reviewed_file_paths,
            expanded_diff_file_paths: self.expanded_diff_file_paths,
            collapsed_diff_file_paths: self.collapsed_diff_file_paths,
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
            .child(
                div()
                    .border_1()
                    .border_color(color::border())
                    .bg(color::content_background())
                    .p_3()
                    .text_color(color::text_muted())
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
            .child(
                div()
                    .border_1()
                    .border_color(color::border())
                    .bg(color::content_background())
                    .p_3()
                    .text_color(color::danger())
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
            .child(
                div()
                    .border_1()
                    .border_color(color::border())
                    .bg(color::content_background())
                    .p_3()
                    .text_color(color::text_muted())
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
        &input.list_items,
        logical_scroll_top.item_ix,
    )
    .filter(|section| {
        section.header_item_index != logical_scroll_top.item_ix
            || logical_scroll_top.offset_in_item > px(0.0)
    });
    let view_entity = cx.entity().clone();
    let processor_view_entity = view_entity.clone();
    let list_items_for_render = input.list_items.clone();
    let review_thread_index = Arc::new(ReviewThreadIndex::new(input.review_threads));
    let commit_scope = input.commit_sha.map(|sha| {
        let short_sha: String = sha.chars().take(7).collect();
        let pull_request = input.pull_request;
        div()
            .flex()
            .flex_none()
            .w_full()
            .h(px(56.0))
            .items_center()
            .justify_between()
            .gap_3()
            .px_3()
            .py_2()
            .border_1()
            .border_color(color::border())
            .bg(color::content_background())
            .text_sm()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_none()
                            .size_7()
                            .rounded_md()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(color::row_hover())
                            .child(Icon::new(Octicon::GitPullRequest).text_color(color::accent())),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap_0p5()
                            .min_w_0()
                            .child(
                                div()
                                    .font_medium()
                                    .text_sm()
                                    .text_color(color::text_primary())
                                    .child("This diff is part of a pull request"),
                            )
                            .when_some(pull_request, |element, pull_request| {
                                element.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .min_w_0()
                                        .text_xs()
                                        .text_color(color::text_secondary())
                                        .child(format!(
                                            "{}/{}",
                                            pull_request.repo.owner, pull_request.repo.name
                                        ))
                                        .child(
                                            div()
                                                .flex_none()
                                                .rounded_sm()
                                                .px_1()
                                                .bg(color::row_hover())
                                                .text_color(color::accent())
                                                .child(format!("#{}", pull_request.number)),
                                        )
                                        .child(div().truncate().child(pull_request.title.clone())),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_none()
                    .child(
                        div()
                            .rounded_full()
                            .px_1p5()
                            .py_0p5()
                            .font_family("Lilex")
                            .text_xs()
                            .bg(color::row_hover())
                            .text_color(color::accent())
                            .child(short_sha),
                    )
                    .child(
                        Button::new("show-full-pull-request-diff")
                            .label("View full PR diff")
                            .xsmall()
                            .compact()
                            .ghost()
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.show_full_pull_request_diff(cx);
                            })),
                    ),
            )
    });

    div()
        .image_cache(gpui::retain_all("diff-review-avatar-cache"))
        .id("diff-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .when_some(commit_scope, |element, scope| element.gap_2().child(scope))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .border_1()
                .border_color(color::border())
                .bg(color::content_background())
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
                                        review_thread_index.clone(),
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
                                            section.file_index == input.active_file_index,
                                            section.reviewed,
                                            section.expanded,
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
