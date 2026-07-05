use gpui::{Context, IntoElement, div, prelude::*, px, uniform_list};
use gpui_component::{
    IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    popover::Popover,
};

use crate::{visual::color, workspace::AppView};

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
        let filter_label = if has_active_filter {
            format!("Filters {visible_count}/{total_count}")
        } else {
            "Filters".to_string()
        };

        div()
            .pt_2()
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
                                }
                                cx.notify();
                            });
                        }
                    })
                    .trigger({
                        let button = Button::new("changed-file-filters")
                            .label(filter_label)
                            .icon(IconName::Settings2)
                            .small()
                            .compact()
                            .dropdown_caret(true);
                        if has_active_filter {
                            button.primary()
                        } else {
                            button.ghost()
                        }
                    })
                    .content(move |_, _window, _popover_cx| {
                        let mut menu = div()
                            .id("changed-file-filters-menu")
                            .w(px(360.))
                            .max_h(px(520.))
                            .overflow_hidden()
                            .border_1()
                            .border_color(color::border_strong())
                            .bg(color::elevated_background())
                            .p_2()
                            .shadow_lg()
                            .child(
                                div()
                                    .px_1()
                                    .pb_2()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_medium()
                                            .text_color(color::text_primary())
                                            .child("Filter changed files"),
                                    )
                                    .child(
                                        div().text_xs().text_color(color::text_muted()).child(
                                            format!("{visible_count}/{total_count} visible"),
                                        ),
                                    ),
                            )
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
                                let view = view.clone();
                                render_file_filter_row(
                                    "owned-by-current-user-filter-menu",
                                    "Files owned by you".to_string(),
                                    Some(owned_file_count),
                                    owned_filter_active,
                                    !has_owned_filter_data,
                                )
                                .on_click(move |_, _, cx| {
                                    view.update(cx, |view, cx| {
                                        if has_owned_filter_data {
                                            view.toggle_files_owned_by_current_user_filter(cx);
                                        }
                                    });
                                })
                            })
                            .child(
                                div()
                                    .mt_2()
                                    .border_t_1()
                                    .border_color(color::border())
                                    .pt_2()
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
                    .text_xs()
                    .text_color(color::text_muted())
                    .child(if has_active_filter {
                        format!("{visible_count}/{total_count} visible")
                    } else {
                        format!("{visible_count} visible")
                    }),
            )
    }
}
