use gpui::{
    Anchor, App, Context, Entity, IntoElement, KeyDownEvent, div, prelude::*, px, uniform_list,
};
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
    popover::Popover,
};

use crate::{
    icons::Octicon,
    visual::color,
    workspace::{AppView, normalized_search_query},
};

use super::{
    inbox_search_rows::{
        pull_request_inbox_search_list_height, render_pull_request_inbox_search_empty_row,
        render_pull_request_inbox_search_row,
    },
    render_switcher_section_label,
};

impl AppView {
    pub(super) fn render_pull_request_inbox_search(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity().clone();
        let pull_request_query =
            normalized_search_query(&self.pull_request_search_input.read(cx).value());
        let pull_requests = self.filtered_switcher_pull_requests(cx);
        let selected_pull_request = self.selected_pull_request_index();
        let pull_request_selection = self
            .pull_request_switcher_selection
            .min(pull_requests.len().saturating_sub(1));
        let pull_request_search_input = self.pull_request_search_input.clone();
        let has_pull_request_query = !pull_request_query.is_empty();
        let has_active_filters = self.has_active_pull_request_filters();
        let has_current_repository = self.current_repository().is_some();

        Popover::new("pull-request-inbox-search-popover")
            .appearance(false)
            .anchor(Anchor::TopRight)
            .open(self.pull_request_inbox_search_open)
            .on_open_change({
                let view = view.clone();
                move |open, window, cx| {
                    view.update(cx, |view, cx| {
                        view.pull_request_inbox_search_open = *open;
                        if *open {
                            view.repository_state.repository_switcher_open = false;
                            view.pull_request_filter_popover_open = false;
                            view.file_filter_popover_open = false;
                            view.status = "Pull request search opened".to_string();
                            view.pull_request_search_input.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                                input.focus(window, cx);
                            });
                            view.reset_pull_request_switcher_selection(cx);
                        }
                        cx.notify();
                    });
                }
            })
            .trigger(
                Button::new("search-pull-request-inbox")
                    .ghost()
                    .small()
                    .compact()
                    .icon(Octicon::Search)
                    .tooltip("Search pull requests")
                    .disabled(!has_current_repository),
            )
            .content(move |_, _window, popover_cx| {
                let view = view.clone();
                let popover = popover_cx.entity().clone();
                let mut results = div()
                    .id("pull-request-inbox-search-results")
                    .max_h(px(408.))
                    .overflow_hidden()
                    .p_2()
                    .child(render_switcher_section_label("results"));

                if !has_current_repository {
                    results = results.child(render_pull_request_inbox_search_empty_row(
                        "select a repository before searching pull requests",
                    ));
                } else if pull_requests.is_empty() {
                    let label = if has_pull_request_query {
                        "no pull requests match search"
                    } else if has_active_filters {
                        "no pull requests match filters"
                    } else {
                        "no pull requests in this list"
                    };
                    results = results.child(render_pull_request_inbox_search_empty_row(label));
                } else {
                    let row_count = pull_requests.len();
                    let list_height = pull_request_inbox_search_list_height(row_count);
                    let pull_requests = pull_requests.clone();
                    let view = view.clone();
                    let popover = popover.clone();

                    results = results.child(
                        uniform_list(
                            "pull-request-inbox-search-list",
                            row_count,
                            move |range, _window, _cx| {
                                let mut rows = Vec::with_capacity(range.len());

                                for row_index in range {
                                    let Some((index, pull_request)) =
                                        pull_requests.get(row_index).cloned()
                                    else {
                                        continue;
                                    };
                                    let current = index == selected_pull_request;
                                    let highlighted = row_index == pull_request_selection;
                                    let number = pull_request.number;
                                    let title = pull_request.title;
                                    let author = pull_request.author;
                                    let view = view.clone();
                                    let popover = popover.clone();

                                    rows.push(
                                        render_pull_request_inbox_search_row(
                                            number,
                                            title,
                                            author,
                                            current,
                                            highlighted,
                                        )
                                        .on_click(
                                            move |_, window, cx| {
                                                view.update(cx, |view, cx| {
                                                    view.select_pull_request(index, cx);
                                                    view.pull_request_inbox_search_open = false;
                                                    cx.notify();
                                                });
                                                popover.update(cx, |popover, cx| {
                                                    popover.dismiss(window, cx);
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

                div()
                    .id("pull-request-inbox-search-menu")
                    .on_key_down({
                        let view = view.clone();
                        move |event, _, cx| {
                            handle_pull_request_inbox_search_key(event, &view, cx);
                        }
                    })
                    .w(px(360.))
                    .max_h(px(480.))
                    .overflow_hidden()
                    .border_1()
                    .border_color(color::border_strong())
                    .bg(color::elevated_background())
                    .shadow_lg()
                    .child(
                        div()
                            .p_2()
                            .border_b_1()
                            .border_color(color::border())
                            .child(
                                Input::new(&pull_request_search_input)
                                    .small()
                                    .cleanable(true),
                            ),
                    )
                    .child(results)
            })
    }
}

fn handle_pull_request_inbox_search_key(
    event: &KeyDownEvent,
    view: &Entity<AppView>,
    cx: &mut App,
) {
    if event.keystroke.modifiers.modified() {
        return;
    }

    match event.keystroke.key.as_str() {
        "up" => {
            cx.stop_propagation();
            view.update(cx, |view, cx| {
                view.move_pull_request_switcher_selection(-1, cx);
            });
        }
        "down" => {
            cx.stop_propagation();
            view.update(cx, |view, cx| {
                view.move_pull_request_switcher_selection(1, cx);
            });
        }
        _ => {}
    }
}
