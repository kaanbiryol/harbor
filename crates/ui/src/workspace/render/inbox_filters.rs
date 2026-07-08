use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px};
use gpui_component::{
    Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    popover::Popover,
};

use crate::{
    icons::Octicon,
    visual::color,
    workspace::{
        AppView, PullRequestFilterFacet, PullRequestFilterOption, PullRequestFilterSections,
    },
};

use super::{changed_file_filter_rows::render_file_filter_row, render_switcher_section_label};

impl AppView {
    pub(super) fn render_pull_request_filters(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let has_current_repository = self.current_repository().is_some();
        let has_pull_requests = !self.pull_requests.is_empty();
        let has_active_filter = self.has_active_pull_request_filters();
        let active_count = self.pull_request_filter_count();
        let sections = self.pull_request_filter_sections();
        let tooltip = if active_count == 0 {
            "Filter pull requests".to_string()
        } else if active_count == 1 {
            "1 active pull request filter".to_string()
        } else {
            format!("{active_count} active pull request filters")
        };

        Popover::new("pull-request-filter-popover")
            .appearance(false)
            .anchor(gpui::Anchor::TopRight)
            .open(self.pull_request_filter_popover_open)
            .on_open_change({
                let view = view.clone();
                move |open, _, cx| {
                    view.update(cx, |view, cx| {
                        view.pull_request_filter_popover_open = *open;
                        if *open {
                            view.repository_state.repository_switcher_open = false;
                            view.pull_request_inbox_search_open = false;
                            view.file_filter_popover_open = false;
                            view.review_action_comment_target = None;
                            view.status = "Pull request filters opened".to_string();
                        }
                        cx.notify();
                    });
                }
            })
            .trigger({
                let button = Button::new("filter-pull-request-inbox")
                    .small()
                    .compact()
                    .icon(Octicon::Sliders)
                    .tooltip(tooltip)
                    .disabled(!has_current_repository || !has_pull_requests);

                if has_active_filter {
                    button.outline()
                } else {
                    button.ghost()
                }
            })
            .content(move |_, window, _popover_cx| {
                let menu_max_height = (window.viewport_size().height - px(16.))
                    .max(px(160.))
                    .min(px(520.));
                let reset_view = view.clone();
                let mut menu = div()
                    .id("pull-request-filter-menu")
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
                            div().px_2().py_1().flex().justify_end().text_xs().child(
                                div()
                                    .id("reset-pull-request-filters")
                                    .cursor_pointer()
                                    .text_color(color::accent())
                                    .hover(|element| element.text_color(color::accent_hover()))
                                    .on_click(move |_, _, cx| {
                                        reset_view.update(cx, |view, cx| {
                                            view.clear_pull_request_filters(cx);
                                        });
                                    })
                                    .child("Reset"),
                            ),
                        )
                    });

                if sections.is_empty() {
                    menu = menu.child(render_pull_request_filter_empty_row(
                        "No filters available for loaded pull requests",
                    ));
                } else {
                    menu = menu.children(render_pull_request_filter_sections(
                        sections.clone(),
                        view.clone(),
                    ));
                }

                menu
            })
    }

    pub(super) fn render_pull_request_filter_chips(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity().clone();

        div()
            .pt_2()
            .flex()
            .items_center()
            .gap_1()
            .flex_wrap()
            .children(
                self.active_pull_request_filter_chips()
                    .into_iter()
                    .map(|chip| {
                        let view = view.clone();
                        let label = chip.label();
                        let chip_id = format!(
                            "pull-request-filter-chip-{}-{}",
                            chip.facet.key(),
                            chip.value
                        );

                        div()
                            .id(chip_id)
                            .h(px(24.))
                            .max_w(px(232.))
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap_1()
                            .rounded_xs()
                            .border_1()
                            .border_color(color::border_strong())
                            .bg(color::elevated_background())
                            .px_2()
                            .text_xs()
                            .text_color(color::text_secondary())
                            .cursor_pointer()
                            .hover(|element| element.bg(color::row_hover()))
                            .on_click(move |_, _, cx| {
                                view.update(cx, |view, cx| {
                                    view.remove_pull_request_filter(chip.facet, &chip.value, cx);
                                });
                            })
                            .child(div().min_w_0().truncate().child(label))
                            .child(
                                Icon::new(Octicon::X)
                                    .xsmall()
                                    .text_color(color::text_muted()),
                            )
                    }),
            )
            .into_any_element()
    }
}

fn render_pull_request_filter_sections(
    sections: PullRequestFilterSections,
    view: gpui::Entity<AppView>,
) -> Vec<AnyElement> {
    let mut elements = Vec::new();

    push_pull_request_filter_section(
        &mut elements,
        PullRequestFilterFacet::Author,
        sections.authors,
        view.clone(),
        false,
    );
    push_pull_request_filter_section(
        &mut elements,
        PullRequestFilterFacet::Label,
        sections.labels,
        view.clone(),
        true,
    );
    push_pull_request_filter_section(
        &mut elements,
        PullRequestFilterFacet::Assignee,
        sections.assignees,
        view,
        true,
    );

    elements
}

fn push_pull_request_filter_section(
    elements: &mut Vec<AnyElement>,
    facet: PullRequestFilterFacet,
    options: Vec<PullRequestFilterOption>,
    view: gpui::Entity<AppView>,
    separated: bool,
) {
    if options.is_empty() {
        return;
    }

    let label = render_switcher_section_label(facet.section_label());
    if separated {
        elements.push(
            div()
                .mt_1()
                .border_t_1()
                .border_color(color::border())
                .pt_1()
                .child(label)
                .into_any_element(),
        );
    } else {
        elements.push(label.into_any_element());
    }

    for option in options {
        let view = view.clone();
        let value = option.value.clone();

        elements.push(
            render_file_filter_row(
                format!("pull-request-filter-{}-{}", facet.key(), option.value),
                option.value,
                Some(option.count),
                option.selected,
                false,
            )
            .on_click(move |_, _, cx| {
                view.update(cx, |view, cx| {
                    view.toggle_pull_request_filter(facet, value.clone(), cx);
                });
            })
            .into_any_element(),
        );
    }
}

fn render_pull_request_filter_empty_row(label: &'static str) -> impl IntoElement {
    div()
        .px_2()
        .py_2()
        .text_sm()
        .text_color(color::text_muted())
        .child(label)
}
