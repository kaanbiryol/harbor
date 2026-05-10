use gpui::{Context, Div, IntoElement, Stateful, div, prelude::*, px, rgb};
use gpui_component::{
    Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    popover::Popover,
};

use crate::workspace::AppView;

use super::render_switcher_section_label;

fn render_file_filter_row(
    id: impl Into<gpui::ElementId>,
    label: String,
    count: Option<usize>,
    checked: bool,
    disabled: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(34.))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .rounded_xs()
        .px_2()
        .mb_1()
        .text_sm()
        .cursor_pointer()
        .when(checked && !disabled, |element| element.bg(rgb(0x243244)))
        .when(disabled, |element| element.cursor_default().opacity(0.45))
        .hover(move |element| {
            if disabled {
                element
            } else {
                element.bg(rgb(0x202a35))
            }
        })
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(checked, |element| {
                            element.child(
                                Icon::new(IconName::Check)
                                    .xsmall()
                                    .text_color(rgb(0x93c5fd)),
                            )
                        }),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(if disabled {
                            rgb(0x7d8794)
                        } else {
                            rgb(0xe6e8eb)
                        })
                        .child(label),
                ),
        )
        .when_some(count, |element, count| {
            element.child(
                div()
                    .flex_none()
                    .min_w(px(24.))
                    .px_1()
                    .text_align(gpui::TextAlign::Right)
                    .text_xs()
                    .text_color(rgb(0x9aa4b2))
                    .child(count.to_string()),
            )
        })
}

impl AppView {
    pub(super) fn render_changed_files_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let reviewed_count = self.reviewed_file_count();
        let total_count = self.detail_state.files.len();
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
            .px_3()
            .py_2()
            .border_1()
            .border_color(rgb(0x242a31))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .text_xs()
                    .child(
                        div()
                            .font_medium()
                            .text_color(rgb(0xd5dde7))
                            .child("Changed files"),
                    )
                    .child(
                        div()
                            .text_color(rgb(0x9aa4b2))
                            .child(format!("{reviewed_count}/{total_count} reviewed")),
                    ),
            )
            .child(
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
                                    .overflow_y_scroll()
                                    .border_1()
                                    .border_color(rgb(0x343b44))
                                    .bg(rgb(0x171b20))
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
                                                    .text_color(rgb(0xe6e8eb))
                                                    .child("Filter changed files"),
                                            )
                                            .child(
                                                div().text_xs().text_color(rgb(0x7d8794)).child(
                                                    format!(
                                                        "{visible_count}/{total_count} visible"
                                                    ),
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
                                        .on_click(
                                            move |_, _, cx| {
                                                view.update(cx, |view, cx| {
                                                    view.show_all_changed_files(cx);
                                                });
                                            },
                                        )
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
                                        .on_click(
                                            move |_, _, cx| {
                                                view.update(cx, |view, cx| {
                                                if has_owned_filter_data {
                                                    view.toggle_files_owned_by_current_user_filter(
                                                        cx,
                                                    );
                                                }
                                            });
                                            },
                                        )
                                    })
                                    .child(
                                        div()
                                            .mt_2()
                                            .border_t_1()
                                            .border_color(rgb(0x242a31))
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
                                        .on_click(
                                            move |_, _, cx| {
                                                view.update(cx, |view, cx| {
                                                    view.include_all_changed_file_types(cx);
                                                });
                                            },
                                        )
                                    });

                                for filter in type_filters.clone() {
                                    let view = view.clone();
                                    let file_type = filter.key.clone();
                                    menu = menu.child(
                                        render_file_filter_row(
                                            format!("file-type-filter-{file_type}"),
                                            filter.label,
                                            Some(filter.file_count),
                                            filter.included,
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

                                menu
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x7d8794))
                            .child(if has_active_filter {
                                format!("{visible_count}/{total_count} visible")
                            } else {
                                format!("{visible_count} visible")
                            }),
                    ),
            )
    }
}
