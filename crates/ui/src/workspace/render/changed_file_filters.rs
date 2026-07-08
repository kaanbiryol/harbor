use gpui::{Context, IntoElement, div, prelude::*, px, uniform_list};
use gpui_component::{
    Sizable,
    button::{Button, ButtonVariants},
    popover::Popover,
};

use crate::{icons::Octicon, visual::color, workspace::AppView};

use super::{
    changed_file_filter_rows::{file_filter_list_height, render_file_filter_row},
    render_switcher_section_label,
};

impl AppView {
    pub(super) fn render_changed_files_filter_bar(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity().clone();
        let total_count = self.detail_state.files().len();
        let visible_count = self.visible_file_indices(cx).len();
        let type_filters = self.changed_file_type_filters();
        let included_type_count = self.included_file_type_filter_count();
        let owned_file_count = self.owned_file_paths.len();
        let has_owned_filter_data = self.has_owned_file_filter_data();
        let owned_filter_active = self.show_files_owned_by_current_user;
        let has_active_filter = included_type_count < type_filters.len() || owned_filter_active;
        let visible_label = if has_active_filter {
            format!("{visible_count}/{total_count} visible")
        } else {
            format!("{visible_count} visible")
        };
        let visible_label_color = if has_active_filter {
            color::accent()
        } else {
            color::text_muted()
        };

        div()
            .h(px(40.0))
            .w_full()
            .px_2()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                Popover::new("changed-file-filters-popover")
                    .appearance(false)
                    .anchor(gpui::Anchor::TopLeft)
                    .open(self.file_filter_popover_open)
                    .on_open_change({
                        let view = view.clone();
                        move |open, _, cx| {
                            view.update(cx, |view, cx| {
                                view.file_filter_popover_open = *open;
                                if *open {
                                    view.repository_state.repository_switcher_open = false;
                                    view.pull_request_inbox_search_open = false;
                                    view.pull_request_filter_popover_open = false;
                                }
                                cx.notify();
                            });
                        }
                    })
                    .trigger({
                        let button = Button::new("changed-file-filters")
                            .label("Filters")
                            .icon(Octicon::Sliders)
                            .small()
                            .compact()
                            .tooltip("Filter changed files");

                        if has_active_filter {
                            button.outline()
                        } else {
                            button.ghost()
                        }
                    })
                    .content(move |_, window, _popover_cx| {
                        let reset_view = view.clone();
                        let menu_max_height = (window.viewport_size().height - px(16.))
                            .max(px(160.))
                            .min(px(520.));
                        let mut menu = div()
                            .id("changed-file-filters-menu")
                            .w(px(320.))
                            .max_h(menu_max_height)
                            .overflow_y_scroll()
                            .border_1()
                            .border_color(color::border_strong())
                            .bg(color::elevated_background())
                            .p_1()
                            .shadow_lg()
                            .when(has_active_filter, |menu| {
                                menu.child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .flex()
                                        .justify_end()
                                        .text_xs()
                                        .child(
                                            div()
                                                .id("reset-changed-file-filters")
                                                .cursor_pointer()
                                                .text_color(color::accent())
                                                .hover(|element| {
                                                    element.text_color(color::accent_hover())
                                                })
                                                .on_click(move |_, _, cx| {
                                                    reset_view.update(cx, |view, cx| {
                                                        view.reset_changed_file_filters();
                                                        view.ensure_active_file_visible(cx);
                                                        view.sync_diff_list_items(cx);
                                                        let visible_count =
                                                            view.visible_file_indices(cx).len();
                                                        view.status = format!(
                                                            "Reset file filters ({visible_count} visible)"
                                                        );
                                                        cx.notify();
                                                    });
                                                })
                                                .child("Reset"),
                                        ),
                                )
                            })
                            .child(render_switcher_section_label("Ownership"))
                            .child({
                                let view = view.clone();
                                render_file_filter_row(
                                    "all-changed-files-filter-menu",
                                    "All changed files".to_string(),
                                    Some(total_count),
                                    !owned_filter_active,
                                    false,
                                )
                                .on_click(move |_, _, cx| {
                                    view.update(cx, |view, cx| {
                                        view.show_all_changed_files(cx);
                                    });
                                })
                            })
                            .child({
                                let row = render_file_filter_row(
                                    "owned-by-current-user-filter-menu",
                                    "Files owned by me".to_string(),
                                    Some(owned_file_count),
                                    owned_filter_active,
                                    !has_owned_filter_data,
                                );

                                if has_owned_filter_data {
                                    let view = view.clone();
                                    row.on_click(move |_, _, cx| {
                                        view.update(cx, |view, cx| {
                                            view.toggle_files_owned_by_current_user_filter(cx);
                                        });
                                    })
                                } else {
                                    row
                                }
                            })
                            .child(
                                div()
                                    .mt_1()
                                    .border_t_1()
                                    .border_color(color::border())
                                    .pt_1()
                                    .child(render_switcher_section_label("File types")),
                            )
                            .child({
                                let view = view.clone();
                                let all_active = included_type_count == type_filters.len();
                                render_file_filter_row(
                                    "include-all-file-types-menu",
                                    "All file types".to_string(),
                                    Some(total_count),
                                    all_active,
                                    false,
                                )
                                .on_click(move |_, _, cx| {
                                    view.update(cx, |view, cx| {
                                        view.include_all_changed_file_types(cx);
                                    });
                                })
                            });

                        if !type_filters.is_empty() {
                            let row_count = type_filters.len();
                            let list_height = file_filter_list_height(row_count);
                            let type_filters = type_filters.clone();
                            let view = view.clone();

                            menu = menu.child(
                                uniform_list(
                                    "file-type-filter-list",
                                    row_count,
                                    move |range, _window, _cx| {
                                        let mut rows = Vec::with_capacity(range.len());

                                        for row_index in range {
                                            let Some(filter) = type_filters.get(row_index).cloned()
                                            else {
                                                continue;
                                            };
                                            let view = view.clone();
                                            let file_type = filter.key.clone();

                                            rows.push(
                                                render_file_filter_row(
                                                    format!("file-type-filter-{file_type}"),
                                                    filter.label,
                                                    Some(filter.file_count),
                                                    filter.included,
                                                    false,
                                                )
                                                .on_click(move |_, _, cx| {
                                                    view.update(cx, |view, cx| {
                                                        view.toggle_changed_file_type_filter(
                                                            file_type.clone(),
                                                            cx,
                                                        );
                                                    });
                                                }),
                                            );
                                        }

                                        rows
                                    },
                                )
                                .h(px(list_height))
                                .w_full()
                                .min_h_0(),
                            );
                        }

                        menu
                    }),
            )
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_1()
                    .text_xs()
                    .text_color(visible_label_color)
                    .child(div().truncate().child(visible_label)),
            )
    }
}
