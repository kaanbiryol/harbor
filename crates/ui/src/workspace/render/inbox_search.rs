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
    panels::ImmediateTooltip,
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
        let pull_requests = self.pull_request_switcher_results(cx);
        let selected_pull_request = self
            .selected_pull_request()
            .map(|pull_request| (pull_request.repo.clone(), pull_request.number));
        let pull_request_selection = self
            .pull_request_switcher_selection
            .min(pull_requests.len().saturating_sub(1));
        let pull_request_search_input = self.pull_request_search_input.clone();
        let has_pull_request_query = !pull_request_query.is_empty();
        let has_current_repository = self.current_repository().is_some();
        let search_loading = self.pull_request_search_state.is_loading();
        let search_error = self.pull_request_search_state.error().map(str::to_string);
        let search_loading_more = self.pull_request_search_state.is_loading_more();
        let search_load_more_error = self
            .pull_request_search_state
            .load_more_error()
            .map(str::to_string);
        let search_has_more = self.pull_request_search_state.next_cursor().is_some();
        let search_total_count = self.pull_request_search_state.total_count();

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
                        } else {
                            view.clear_pull_request_search();
                        }
                        cx.notify();
                    });
                }
            })
            .trigger(ImmediateTooltip::new(
                "search-pull-request-inbox-tooltip",
                "Search pull requests",
                Button::new("search-pull-request-inbox")
                    .ghost()
                    .small()
                    .compact()
                    .icon(Octicon::Search)
                    .disabled(!has_current_repository),
            ))
            .content(move |_, _window, popover_cx| {
                let view = view.clone();
                let popover = popover_cx.entity().clone();
                let mut results = div()
                    .id("pull-request-inbox-search-results")
                    .flex()
                    .flex_col()
                    .p_2();

                if !has_current_repository {
                    results = results.child(render_pull_request_inbox_search_empty_row(
                        "select a repository before searching pull requests",
                    ));
                } else if !has_pull_request_query {
                    results = results.child(render_pull_request_inbox_search_empty_row(
                        "type to search pull requests on GitHub",
                    ));
                } else if search_loading {
                    results = results.child(render_pull_request_inbox_search_empty_row(
                        "searching GitHub…",
                    ));
                } else if let Some(error) = search_error.as_ref() {
                    let view = view.clone();
                    results = results.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .px_2()
                            .py_2()
                            .text_sm()
                            .text_color(color::text_muted())
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .truncate()
                                    .child(format!("search failed: {error}")),
                            )
                            .child(
                                Button::new("retry-pull-request-search")
                                    .ghost()
                                    .small()
                                    .compact()
                                    .label("Retry")
                                    .on_click(move |_, _, cx| {
                                        view.update(cx, |view, cx| {
                                            view.retry_pull_request_search(cx);
                                        });
                                    }),
                            ),
                    );
                } else if pull_requests.is_empty() {
                    results = results.child(render_pull_request_inbox_search_empty_row(
                        "no pull requests match search",
                    ));
                } else {
                    let row_count = pull_requests.len();
                    let list_height = pull_request_inbox_search_list_height(row_count);
                    let pull_requests = pull_requests.clone();
                    let result_list_view = view.clone();
                    let popover = popover.clone();
                    let selected_pull_request = selected_pull_request.clone();

                    results = results
                        .child(render_switcher_section_label("results"))
                        .child(
                            uniform_list(
                                "pull-request-inbox-search-list",
                                row_count,
                                move |range, _window, _cx| {
                                    let mut rows = Vec::with_capacity(range.len());

                                    for row_index in range {
                                        let Some(result) = pull_requests.get(row_index).cloned()
                                        else {
                                            continue;
                                        };
                                        let pull_request = &result.pull_request;
                                        let current = selected_pull_request.as_ref().is_some_and(
                                            |(repository, number)| {
                                                repository == &pull_request.repo
                                                    && *number == pull_request.number
                                            },
                                        );
                                        let highlighted = row_index == pull_request_selection;
                                        let number = pull_request.number;
                                        let title = pull_request.title.clone();
                                        let author = pull_request.author.clone();
                                        let view = result_list_view.clone();
                                        let popover = popover.clone();
                                        let result = result.clone();

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
                                                    let result = result.clone();
                                                    view.update(cx, move |view, cx| {
                                                        view.select_pull_request_switcher_result(
                                                            result, cx,
                                                        );
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

                    let result_summary = if let Some(error) = search_load_more_error.as_ref() {
                        format!("could not load more: {error}")
                    } else if let Some(total_count) = search_total_count {
                        if search_has_more {
                            format!("{row_count} of {total_count} results")
                        } else {
                            format!("{total_count} results")
                        }
                    } else {
                        format!("{row_count} results")
                    };
                    let view = view.clone();
                    results = results.child(
                        div()
                            .px_2()
                            .py_1()
                            .border_t_1()
                            .border_color(color::border())
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .text_xs()
                            .text_color(color::text_muted())
                            .child(result_summary)
                            .when(search_has_more, |element| {
                                element.child(
                                    Button::new("load-more-pull-request-search-results")
                                        .ghost()
                                        .small()
                                        .compact()
                                        .label("More")
                                        .loading(search_loading_more)
                                        .disabled(search_loading_more)
                                        .on_click({
                                            let view = view.clone();
                                            move |_, _, cx| {
                                                view.update(cx, |view, cx| {
                                                    view.load_more_pull_request_search_results(cx);
                                                });
                                            }
                                        }),
                                )
                            }),
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
                    .flex()
                    .flex_col()
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
