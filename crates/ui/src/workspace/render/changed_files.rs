use gpui::{
    AnyElement, Context, IntoElement, Rgba, SharedString, div, prelude::*, px, uniform_list,
};
use gpui_component::{Icon, Sizable, StyledExt};

use crate::{
    icons::Octicon,
    panels::{render_changed_file_row, render_changed_folder_row, render_status_pill},
    visual::{Tone, color, layout},
    workspace::{AppView, ChangedFileTreeRow},
};

fn render_changed_files_message(message: impl Into<SharedString>, text_color: Rgba) -> AnyElement {
    div()
        .flex_1()
        .px_3()
        .py_3()
        .text_sm()
        .text_color(text_color)
        .child(message.into())
        .into_any_element()
}

impl AppView {
    pub(super) fn render_changed_files_body(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.detail_state.files_loading() {
            return render_changed_files_message("Loading changed files...", color::text_muted());
        }

        if let Some(error) = self.detail_state.files_error() {
            return render_changed_files_message(error.to_string(), color::danger());
        }

        if self.detail_state.files().is_empty() {
            return render_changed_files_message("No changed files", color::text_muted());
        }

        if self.changed_file_tree_rows(cx).is_empty() {
            return render_changed_files_message("No files match filter", color::text_muted());
        }

        self.render_changed_files_list(cx).into_any_element()
    }

    pub(super) fn render_changed_files_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let row_count = self.changed_file_tree_rows(cx).len();

        uniform_list(
            "changed-files-list",
            row_count,
            cx.processor(|view, range: std::ops::Range<usize>, _window, cx| {
                let tree_rows = view.changed_file_tree_rows(cx);
                let mut rows = Vec::with_capacity(range.len());

                for row_index in range {
                    let Some(row) = tree_rows.get(row_index) else {
                        continue;
                    };
                    match row {
                        ChangedFileTreeRow::Folder(folder_row) => {
                            rows.push(render_changed_folder_row(folder_row, cx));
                        }
                        ChangedFileTreeRow::File(file_row) => {
                            let Some(file) = view.detail_state.files().get(file_row.file_index)
                            else {
                                continue;
                            };
                            rows.push(render_changed_file_row(
                                file_row,
                                file,
                                file_row.file_index == view.active_file_index(),
                                view.reviewed_file_paths.contains(&file.path),
                                cx,
                            ));
                        }
                    }
                }

                rows
            }),
        )
        .track_scroll(&self.file_list_scroll)
        .flex_1()
        .min_h_0()
        .w_full()
    }

    pub(super) fn render_changed_files_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let reviewed_count = self.reviewed_file_count();
        let total_count = self.detail_state.files().len();
        let reviewed_tone = if total_count > 0 && reviewed_count == total_count {
            Tone::Success
        } else if reviewed_count > 0 {
            Tone::Info
        } else {
            Tone::Neutral
        };
        let visible_count = self.visible_file_indices(cx).len();
        let has_active_filter = self.has_active_changed_file_filters();
        let visible_count_label = if has_active_filter {
            format!("{visible_count}/{total_count}")
        } else {
            total_count.to_string()
        };

        div()
            .border_b_1()
            .border_color(color::border())
            .bg(color::content_background())
            .child(
                div()
                    .h(px(layout::CHANGED_FILE_TOOLBAR_HEIGHT))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
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
                                    .truncate()
                                    .text_sm()
                                    .font_medium()
                                    .text_color(color::text_primary())
                                    .child("Changed files"),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(if has_active_filter {
                                        color::accent()
                                    } else {
                                        color::text_muted()
                                    })
                                    .child(visible_count_label),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(render_status_pill(
                                format!("{reviewed_count}/{total_count} reviewed"),
                                reviewed_tone,
                            ))
                            .child(self.render_changed_files_filter_control(cx)),
                    ),
            )
    }
}
