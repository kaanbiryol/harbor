use gpui::{
    Anchor, AnyElement, App, Context, Div, Entity, IntoElement, KeyDownEvent, Stateful, div,
    prelude::*, px, uniform_list,
};
use gpui_component::{
    Disableable, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::Input,
    popover::Popover,
};

use crate::{
    panels::render_pull_request_row,
    visual::color,
    workspace::{AppView, PullRequestInboxCacheKey, PullRequestInboxMode, normalized_search_query},
};

use super::render_switcher_section_label;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PullRequestInboxBodyState {
    LoadingEmpty,
    RefreshingRows,
    ErrorEmpty,
    ErrorRows,
    Empty,
    Rows,
}

fn pull_request_inbox_body_state(
    is_loading: bool,
    has_load_error: bool,
    has_pull_requests: bool,
) -> PullRequestInboxBodyState {
    match (is_loading, has_load_error, has_pull_requests) {
        (true, _, true) => PullRequestInboxBodyState::RefreshingRows,
        (true, _, false) => PullRequestInboxBodyState::LoadingEmpty,
        (false, true, true) => PullRequestInboxBodyState::ErrorRows,
        (false, true, false) => PullRequestInboxBodyState::ErrorEmpty,
        (false, false, true) => PullRequestInboxBodyState::Rows,
        (false, false, false) => PullRequestInboxBodyState::Empty,
    }
}

impl AppView {
    fn pull_request_inbox_mode_count(&self, mode: PullRequestInboxMode) -> Option<usize> {
        let repository = self.repository_state.configured_repo()?;
        let key = PullRequestInboxCacheKey::new(repository.clone(), mode);

        if mode == self.pull_request_inbox.mode() {
            return self
                .pull_request_inbox
                .stored_count(&key)
                .or_else(|| self.pull_request_inbox.total_count())
                .or_else(|| {
                    (!self.pull_request_inbox.has_next_page()).then_some(self.pull_requests.len())
                });
        }

        self.pull_request_inbox.snapshot_count(&key)
    }

    fn render_pull_request_inbox_search(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

    pub(super) fn render_inbox(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_mode = self.pull_request_inbox.mode();
        let load_error = self.pull_request_inbox.load_error().map(str::to_string);
        let body_state = pull_request_inbox_body_state(
            self.pull_request_inbox.is_loading(),
            load_error.is_some(),
            !self.pull_requests.is_empty(),
        );
        let show_list = matches!(
            body_state,
            PullRequestInboxBodyState::RefreshingRows
                | PullRequestInboxBodyState::ErrorRows
                | PullRequestInboxBodyState::Rows
        );
        let empty_message = if self.repository_state.has_configured_repo() {
            current_mode.empty_message()
        } else {
            "Choose a repository from the header"
        };
        let show_page_footer = show_list
            && (self.pull_request_inbox.has_next_page()
                || self.pull_request_inbox.is_loading_more()
                || self.pull_request_inbox.load_more_error().is_some());
        let pull_request_list_item_count = self.pull_requests.len() + usize::from(show_page_footer);

        div()
            .w(px(320.))
            .flex()
            .flex_col()
            .min_h_0()
            .border_1()
            .border_color(color::border())
            .bg(color::panel_background())
            .overflow_hidden()
            .child(
                div()
                    .px_3()
                    .pt_3()
                    .pb_2()
                    .border_b_1()
                    .border_color(color::border())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .truncate()
                                    .text_sm()
                                    .font_medium()
                                    .text_color(color::text_primary())
                                    .child("Pull requests"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(self.render_pull_request_inbox_search(cx))
                                    .child(
                                        Button::new("refresh-pull-request-inbox")
                                            .ghost()
                                            .small()
                                            .compact()
                                            .icon(IconName::Redo)
                                            .tooltip("Refresh pull requests")
                                            .loading(self.pull_request_inbox.is_loading())
                                            .disabled(!self.repository_state.has_configured_repo())
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.reload_pull_request_inbox(cx);
                                            })),
                                    ),
                            ),
                    )
                    .child(div().pt_2().flex().items_center().gap_1().children(
                        PullRequestInboxMode::ALL.into_iter().map(|mode| {
                            let active = mode == current_mode;
                            let count = self.pull_request_inbox_mode_count(mode);

                            render_pull_request_inbox_mode_tab(mode, active, count, cx)
                        }),
                    )),
            )
            .when(
                body_state == PullRequestInboxBodyState::RefreshingRows,
                |element| {
                    element.child(
                        div()
                            .id("pull-request-inbox-refreshing")
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(color::border())
                            .text_xs()
                            .text_color(color::text_muted())
                            .child(format!("Refreshing {}...", current_mode.status_label())),
                    )
                },
            )
            .when(
                body_state == PullRequestInboxBodyState::LoadingEmpty,
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(color::text_muted())
                            .child(format!("Loading {}...", current_mode.status_label())),
                    )
                },
            )
            .when(
                body_state == PullRequestInboxBodyState::ErrorRows,
                |element| {
                    element.child(
                        div()
                            .id("pull-request-inbox-refresh-error")
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(color::border())
                            .text_xs()
                            .text_color(color::danger())
                            .child(format!(
                                "Refresh failed: {}",
                                load_error.clone().unwrap_or_default()
                            )),
                    )
                },
            )
            .when(
                body_state == PullRequestInboxBodyState::ErrorEmpty,
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(color::danger())
                            .child(load_error.clone().unwrap_or_default()),
                    )
                },
            )
            .when(body_state == PullRequestInboxBodyState::Empty, |element| {
                element.child(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(color::text_muted())
                        .child(empty_message),
                )
            })
            .when(show_list, |element| {
                element.child(
                    div()
                        .id("pull-request-inbox-list")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .child(
                            uniform_list(
                                "pull-request-inbox-rows",
                                pull_request_list_item_count,
                                cx.processor(|view, range: std::ops::Range<usize>, _window, cx| {
                                    let mut rows = Vec::with_capacity(range.len());

                                    for index in range {
                                        if index == view.pull_requests.len() {
                                            rows.push(
                                                view.render_pull_request_inbox_page_footer(cx),
                                            );
                                            continue;
                                        }

                                        let Some(pr) = view.pull_requests.get(index) else {
                                            continue;
                                        };
                                        rows.push(render_pull_request_row(
                                            index,
                                            pr,
                                            index == view.selected_pull_request_index(),
                                            cx,
                                        ));
                                    }

                                    rows
                                }),
                            )
                            .track_scroll(&self.pr_list_scroll)
                            .flex_1()
                            .min_h_0()
                            .w_full(),
                        ),
                )
            })
    }

    fn render_pull_request_inbox_page_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let loaded_count = self.pull_requests.len();
        let total_count = self
            .current_pull_request_inbox_key()
            .as_ref()
            .and_then(|key| self.pull_request_inbox.stored_count(key))
            .or_else(|| self.pull_request_inbox.total_count());
        let count_label = match total_count {
            Some(total_count) => format!("Showing {loaded_count} of {total_count}"),
            None => format!("Showing {loaded_count}"),
        };
        let load_more_error = self
            .pull_request_inbox
            .load_more_error()
            .map(str::to_string);
        let can_load_more = self.pull_request_inbox.has_next_page()
            && !self.pull_request_inbox.is_loading()
            && !self.pull_request_inbox.is_loading_more();

        div()
            .id("pull-request-inbox-page-footer")
            .h(px(76.))
            .w_full()
            .border_t_1()
            .border_color(color::border())
            .px_3()
            .py_1()
            .flex()
            .flex_col()
            .justify_center()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(color::text_muted())
                            .child(count_label),
                    )
                    .child(
                        Button::new("load-more-pull-requests")
                            .ghost()
                            .small()
                            .compact()
                            .icon(IconName::ChevronDown)
                            .label("Load more")
                            .tooltip("Load more pull requests")
                            .loading(self.pull_request_inbox.is_loading_more())
                            .disabled(!can_load_more)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.load_more_pull_requests(cx);
                            })),
                    ),
            )
            .when_some(load_more_error, |element, error| {
                element.child(
                    div()
                        .text_xs()
                        .text_color(color::danger())
                        .child(format!("Load more failed: {error}")),
                )
            })
            .into_any_element()
    }
}

fn render_pull_request_inbox_mode_tab(
    mode: PullRequestInboxMode,
    active: bool,
    count: Option<usize>,
    cx: &mut Context<AppView>,
) -> AnyElement {
    div()
        .id(format!("pull-request-inbox-mode-{}", mode.key()))
        .h(px(28.))
        .min_w_0()
        .flex()
        .items_center()
        .gap_1()
        .rounded_xs()
        .px_2()
        .text_xs()
        .font_medium()
        .cursor_pointer()
        .text_color(if active {
            color::text_primary()
        } else {
            color::text_secondary()
        })
        .when(active, |element| {
            element
                .border_1()
                .border_color(color::border_strong())
                .bg(color::row_selected())
        })
        .when(!active, |element| {
            element.hover(|style| style.bg(color::row_hover()))
        })
        .on_click(cx.listener(move |view, _, _, cx| {
            view.select_pull_request_inbox_mode(mode, cx);
        }))
        .child(div().truncate().child(mode.label()))
        .when_some(count, |element, count| {
            element.child(
                div()
                    .min_w(px(16.))
                    .h(px(18.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_xs()
                    .px_1()
                    .text_color(if active {
                        color::text_secondary()
                    } else {
                        color::text_muted()
                    })
                    .bg(if active {
                        color::row_selected_subtle()
                    } else {
                        color::elevated_background()
                    })
                    .child(count.to_string()),
            )
        })
        .into_any_element()
}

fn render_pull_request_inbox_search_empty_row(label: &'static str) -> impl IntoElement {
    div()
        .px_2()
        .py_2()
        .text_sm()
        .text_color(color::text_muted())
        .child(label)
}

fn render_pull_request_inbox_search_row(
    number: u64,
    title: String,
    author: String,
    current: bool,
    highlighted: bool,
) -> Stateful<Div> {
    div()
        .id(("pull-request-inbox-search-row", number))
        .flex()
        .flex_col()
        .gap_1()
        .px_2()
        .py_2()
        .text_sm()
        .cursor_pointer()
        .when(highlighted, |element| element.bg(color::row_selected()))
        .when(current && !highlighted, |element| {
            element.bg(color::row_selected_subtle())
        })
        .hover(|element| element.bg(color::row_hover()))
        .child(
            div()
                .flex()
                .min_w_0()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_none()
                        .font_medium()
                        .text_color(color::text_primary())
                        .child(format!("#{number}")),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(color::text_secondary())
                        .child(title),
                ),
        )
        .child(
            div()
                .flex()
                .min_w_0()
                .items_center()
                .gap_2()
                .text_xs()
                .text_color(color::text_muted())
                .child("by")
                .child(div().min_w_0().truncate().child(author)),
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_existing_pull_request_rows_visible_while_refreshing() {
        assert_eq!(
            pull_request_inbox_body_state(true, false, true),
            PullRequestInboxBodyState::RefreshingRows
        );
        assert_eq!(
            pull_request_inbox_body_state(false, true, true),
            PullRequestInboxBodyState::ErrorRows
        );
        assert_eq!(
            pull_request_inbox_body_state(true, false, false),
            PullRequestInboxBodyState::LoadingEmpty
        );
        assert_eq!(
            pull_request_inbox_body_state(false, true, false),
            PullRequestInboxBodyState::ErrorEmpty
        );
    }
}
