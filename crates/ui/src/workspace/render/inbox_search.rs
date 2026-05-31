use gpui::{Anchor, App, Context, Entity, IntoElement, KeyDownEvent, div, prelude::*, px};
use gpui_component::{
    Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
    popover::Popover,
};

use crate::{
    visual::color,
    workspace::{AppView, normalized_search_query},
};

use super::{
    inbox_search_rows::{
        render_pull_request_inbox_search_empty_row, render_pull_request_inbox_search_row,
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
                    .icon(IconName::Search)
                    .tooltip("Search pull requests")
                    .disabled(!has_current_repository),
            )
            .content(move |_, _window, popover_cx| {
                let view = view.clone();
                let popover = popover_cx.entity().clone();
                let mut results = div()
                    .id("pull-request-inbox-search-results")
                    .max_h(px(408.))
                    .overflow_y_scroll()
                    .p_2()
                    .child(render_switcher_section_label("results"));

                if !has_current_repository {
                    results = results.child(render_pull_request_inbox_search_empty_row(
                        "select a repository before searching pull requests",
                    ));
                } else if pull_requests.is_empty() {
                    let label = if has_pull_request_query {
                        "no pull requests match search"
                    } else {
                        "no pull requests in this list"
                    };
                    results = results.child(render_pull_request_inbox_search_empty_row(label));
                } else {
                    for (row_index, (index, pull_request)) in pull_requests.iter().enumerate() {
                        let current = *index == selected_pull_request;
                        let highlighted = row_index == pull_request_selection;
                        let number = pull_request.number;
                        let title = pull_request.title.clone();
                        let author = pull_request.author.clone();
                        let view = view.clone();
                        let popover = popover.clone();
                        let index = *index;

                        results = results.child(
                            render_pull_request_inbox_search_row(
                                number,
                                title,
                                author,
                                current,
                                highlighted,
                            )
                            .on_click(move |_, window, cx| {
                                view.update(cx, |view, cx| {
                                    view.select_pull_request(index, cx);
                                    view.pull_request_inbox_search_open = false;
                                    cx.notify();
                                });
                                popover.update(cx, |popover, cx| {
                                    popover.dismiss(window, cx);
                                });
                            }),
                        );
                    }
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
