use gpui::{Context, IntoElement, div, prelude::*, px, uniform_list};
use gpui_component::{
    Sizable,
    button::{Button, ButtonVariants},
    popover::Popover,
};

use crate::{
    dropdown::{
        dropdown_menu_item, dropdown_menu_list_height, dropdown_menu_section,
        dropdown_menu_separator, dropdown_menu_surface,
    },
    icons::Octicon,
    panels::ImmediateTooltip,
    visual::color,
    workspace::AppView,
};

impl AppView {
    pub(super) fn render_changed_files_filter_control(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity().clone();
        let total_count = self.detail_state.files().len();
        let type_filters = self.changed_file_type_filters();
        let included_type_count = self.included_file_type_filter_count();
        let owned_file_count = self.changed_files_state.owned_file_paths.len();
        let has_owned_filter_data = self.has_owned_file_filter_data();
        let owned_filter_active = self.changed_files_state.show_files_owned_by_current_user;
        let has_active_filter = self.has_active_changed_file_filters();

        Popover::new("changed-file-filters-popover")
            .appearance(false)
            .anchor(gpui::Anchor::TopRight)
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
                    .icon(Octicon::Sliders)
                    .small()
                    .compact();

                let button = if has_active_filter {
                    button.outline()
                } else {
                    button.ghost()
                };

                ImmediateTooltip::new(
                    "changed-file-filters-tooltip",
                    "Filter changed files",
                    button,
                )
            })
            .content(move |_, window, popover_cx| {
                let reset_view = view.clone();
                let menu_max_height = (window.viewport_size().height - px(16.))
                    .max(px(160.))
                    .min(px(520.));
                let mut menu = dropdown_menu_surface(popover_cx, 320.0)
                    .id("changed-file-filters-menu")
                    .max_h(menu_max_height)
                    .overflow_y_scroll()
                    .p_1()
                    .when(has_active_filter, |menu| {
                        menu.child(
                            div().px_2().py_1().flex().justify_end().text_xs().child(
                                div()
                                    .id("reset-changed-file-filters")
                                    .cursor_pointer()
                                    .text_color(color::accent())
                                    .hover(|element| element.text_color(color::accent_hover()))
                                    .on_click(move |_, _, cx| {
                                        reset_view.update(cx, |view, cx| {
                                            view.reset_changed_file_filters();
                                            view.ensure_active_file_visible(cx);
                                            view.sync_diff_list_items(cx);
                                            let visible_count = view.visible_file_indices(cx).len();
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
                    .child(dropdown_menu_section("Ownership"))
                    .child({
                        let view = view.clone();
                        dropdown_menu_item(
                            "all-changed-files-filter-menu",
                            "All changed files",
                            Some(total_count),
                            !owned_filter_active,
                            false,
                            false,
                        )
                        .on_click(move |_, _, cx| {
                            view.update(cx, |view, cx| {
                                view.show_all_changed_files(cx);
                            });
                        })
                    })
                    .child({
                        let row = dropdown_menu_item(
                            "owned-by-current-user-filter-menu",
                            "Files owned by me",
                            Some(owned_file_count),
                            owned_filter_active,
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
                            .child(dropdown_menu_separator())
                            .child(dropdown_menu_section("File types")),
                    )
                    .child({
                        let view = view.clone();
                        let all_active = included_type_count == type_filters.len();
                        dropdown_menu_item(
                            "include-all-file-types-menu",
                            "All file types",
                            Some(total_count),
                            all_active,
                            false,
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
                    let list_height = dropdown_menu_list_height(row_count);
                    let type_filters = type_filters.clone();
                    let view = view.clone();

                    menu = menu.child(
                        uniform_list(
                            "file-type-filter-list",
                            row_count,
                            move |range, _window, _cx| {
                                let mut rows = Vec::with_capacity(range.len());

                                for row_index in range {
                                    let Some(filter) = type_filters.get(row_index).cloned() else {
                                        continue;
                                    };
                                    let view = view.clone();
                                    let file_type = filter.key.clone();

                                    rows.push(
                                        dropdown_menu_item(
                                            format!("file-type-filter-{file_type}"),
                                            filter.label,
                                            Some(filter.file_count),
                                            filter.included,
                                            false,
                                            false,
                                        )
                                        .on_click(
                                            move |_, _, cx| {
                                                view.update(cx, |view, cx| {
                                                    view.toggle_changed_file_type_filter(
                                                        file_type.clone(),
                                                        cx,
                                                    );
                                                });
                                            },
                                        ),
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
            })
    }
}
