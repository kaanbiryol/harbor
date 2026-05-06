use gpui::{
    Anchor, App, Context, Div, Entity, IntoElement, KeyDownEvent, Stateful, div, prelude::*, px,
    rgb, uniform_list,
};
use gpui_component::{
    Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::Input,
    popover::Popover,
};
use harbor_domain::RepoId;

use crate::workspace::{AppView, normalized_search_query, parse_repo_id};

use super::super::render_switcher_section_label;

const REPOSITORY_SWITCHER_ROW_HEIGHT: f32 = 34.;
const REPOSITORY_SWITCHER_MAX_VISIBLE_ROWS: usize = 10;

fn render_switcher_empty_row(label: impl Into<String>) -> impl IntoElement {
    div()
        .px_2()
        .py_2()
        .text_sm()
        .text_color(rgb(0x7d8794))
        .child(label.into())
}

fn render_switcher_loading_row(label: &'static str) -> impl IntoElement {
    div()
        .px_2()
        .py_2()
        .text_sm()
        .text_color(rgb(0x93c5fd))
        .child(label)
}

fn render_switcher_error_row(error: String) -> impl IntoElement {
    div()
        .mx_1()
        .mb_1()
        .border_1()
        .border_color(rgb(0x4b2a2f))
        .bg(rgb(0x211417))
        .px_2()
        .py_2()
        .text_xs()
        .text_color(rgb(0xf87171))
        .child(error)
}

fn render_switcher_repository_row(
    repository: &RepoId,
    current: bool,
    highlighted: bool,
) -> Stateful<Div> {
    div()
        .id(format!("switcher-repository-{}", repository.full_name()))
        .h(px(REPOSITORY_SWITCHER_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_2()
        .py_1()
        .text_sm()
        .cursor_pointer()
        .when(highlighted, |element| element.bg(rgb(0x263241)))
        .when(current && !highlighted, |element| element.bg(rgb(0x202936)))
        .hover(|element| element.bg(rgb(0x222a34)))
        .child(
            div()
                .min_w_0()
                .truncate()
                .font_medium()
                .text_color(rgb(0xe6e8eb))
                .child(repository.full_name()),
        )
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(rgb(0x7d8794))
                .child(if current { "current" } else { "repo" }),
        )
}

fn repository_switcher_list_height(repository_count: usize) -> f32 {
    REPOSITORY_SWITCHER_ROW_HEIGHT
        * repository_count.min(REPOSITORY_SWITCHER_MAX_VISIBLE_ROWS) as f32
}

fn render_switcher_typed_repository_row(repository: &RepoId, highlighted: bool) -> Stateful<Div> {
    div()
        .id(format!(
            "switcher-typed-repository-{}",
            repository.full_name()
        ))
        .h(px(REPOSITORY_SWITCHER_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_2()
        .py_1()
        .text_sm()
        .cursor_pointer()
        .when(highlighted, |element| element.bg(rgb(0x263241)))
        .hover(|element| element.bg(rgb(0x222a34)))
        .child(
            div()
                .min_w_0()
                .truncate()
                .font_medium()
                .text_color(rgb(0xe6e8eb))
                .child(format!("Open {}", repository.full_name())),
        )
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(rgb(0x7d8794))
                .child("typed"),
        )
}

fn render_switcher_pull_request_row(
    number: u64,
    title: String,
    author: String,
    current: bool,
    highlighted: bool,
) -> Stateful<Div> {
    div()
        .id(("switcher-pull-request", number))
        .flex()
        .flex_col()
        .gap_1()
        .px_2()
        .py_2()
        .text_sm()
        .cursor_pointer()
        .when(highlighted, |element| element.bg(rgb(0x263241)))
        .when(current && !highlighted, |element| element.bg(rgb(0x202936)))
        .hover(|element| element.bg(rgb(0x222a34)))
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
                        .text_color(rgb(0xe6e8eb))
                        .child(format!("#{number}")),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(rgb(0xcbd5e1))
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
                .text_color(rgb(0x7d8794))
                .child("by")
                .child(div().min_w_0().truncate().child(author)),
        )
}

fn handle_repository_switcher_key(event: &KeyDownEvent, view: &Entity<AppView>, cx: &mut App) {
    if event.keystroke.modifiers.modified() {
        return;
    }

    match event.keystroke.key.as_str() {
        "up" => {
            cx.stop_propagation();
            view.update(cx, |view, cx| {
                view.move_repository_switcher_selection(-1, cx);
            });
        }
        "down" => {
            cx.stop_propagation();
            view.update(cx, |view, cx| {
                view.move_repository_switcher_selection(1, cx);
            });
        }
        _ => {}
    }
}

fn handle_pull_request_switcher_key(event: &KeyDownEvent, view: &Entity<AppView>, cx: &mut App) {
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

impl AppView {
    pub(super) fn render_repository_switcher(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let repository_label = self.header_repository_label();
        let repository_search_value = self.repository_search_input.read(cx).value();
        let repository_query = normalized_search_query(&repository_search_value);
        let repositories = self.filtered_switcher_repositories(cx);
        let typed_repository = if repositories.is_empty() {
            parse_repo_id(&repository_search_value)
        } else {
            None
        };
        let current_repository = self.current_repository().cloned();
        let repository_error = self.repository_error.clone();
        let is_loading_repositories = self.is_loading_repositories;
        let repository_selection = self
            .repository_switcher_selection
            .min(repositories.len().saturating_sub(1));
        let repository_search_input = self.repository_search_input.clone();
        let has_repository_query = !repository_query.is_empty();

        Popover::new("repository-switcher-popover")
            .appearance(false)
            .anchor(Anchor::TopLeft)
            .open(self.repository_switcher_open)
            .on_open_change({
                let view = view.clone();
                move |open, window, cx| {
                    view.update(cx, |view, cx| {
                        view.repository_switcher_open = *open;
                        if *open {
                            view.pull_request_switcher_open = false;
                            view.file_filter_popover_open = false;
                            view.status = "Repository search opened".to_string();
                            view.repository_search_input.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                                input.focus(window, cx);
                            });
                            view.reset_repository_switcher_selection(cx);
                        }
                        cx.notify();
                    });
                }
            })
            .trigger(
                Button::new("repository-switcher")
                    .ghost()
                    .small()
                    .compact()
                    .dropdown_caret(true)
                    .max_w(px(260.))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_medium()
                            .text_color(rgb(0xf1f5f9))
                            .child(repository_label),
                    ),
            )
            .content(move |_, _window, popover_cx| {
                let view = view.clone();
                let popover = popover_cx.entity().clone();
                let mut menu = div()
                    .id("repository-switcher-menu")
                    .on_key_down({
                        let view = view.clone();
                        move |event, _, cx| {
                            handle_repository_switcher_key(event, &view, cx);
                        }
                    })
                    .w(px(460.))
                    .max_h(px(520.))
                    .overflow_y_scroll()
                    .border_1()
                    .border_color(rgb(0x343b44))
                    .bg(rgb(0x171b20))
                    .p_2()
                    .shadow_lg()
                    .child(render_switcher_section_label("search repositories"))
                    .child(
                        div()
                            .px_1()
                            .pb_2()
                            .child(Input::new(&repository_search_input).small().cleanable(true)),
                    )
                    .child(render_switcher_section_label("repositories"));

                if let Some(error) = repository_error.clone() {
                    menu = menu.child(render_switcher_error_row(error));
                }

                if is_loading_repositories {
                    menu = menu.child(render_switcher_loading_row(
                        "Fetching repositories from GitHub...",
                    ));
                }

                if repositories.is_empty() {
                    if let Some(repository) = typed_repository.clone() {
                        let selected_repository = repository.clone();
                        let view = view.clone();
                        let popover = popover.clone();

                        menu = menu.child(
                            render_switcher_typed_repository_row(&repository, true).on_click(
                                move |_, window, cx| {
                                    view.update(cx, |view, cx| {
                                        view.select_repository_from_switcher(
                                            selected_repository.clone(),
                                            cx,
                                        );
                                        view.repository_switcher_open = false;
                                        cx.notify();
                                    });
                                    popover.update(cx, |popover, cx| {
                                        popover.dismiss(window, cx);
                                    });
                                },
                            ),
                        );
                    } else {
                        let label = if has_repository_query || is_loading_repositories {
                            "Type owner/repo to open a repository"
                        } else {
                            "No repositories found. Type owner/repo to open a repository"
                        };
                        menu = menu.child(render_switcher_empty_row(label));
                    }
                } else {
                    let repository_count = repositories.len();
                    let list_height = repository_switcher_list_height(repository_count);
                    let repositories = repositories.clone();
                    let current_repository = current_repository.clone();
                    let view = view.clone();
                    let popover = popover.clone();

                    menu = menu.child(
                        uniform_list(
                            "repository-switcher-list",
                            repository_count,
                            move |range, _window, _cx| {
                                let mut rows = Vec::with_capacity(range.len());

                                for row_index in range {
                                    let Some(repository) = repositories.get(row_index).cloned()
                                    else {
                                        continue;
                                    };
                                    let current = current_repository.as_ref() == Some(&repository);
                                    let highlighted = row_index == repository_selection;
                                    let view = view.clone();
                                    let popover = popover.clone();

                                    rows.push(
                                        render_switcher_repository_row(
                                            &repository,
                                            current,
                                            highlighted,
                                        )
                                        .on_click(
                                            move |_, window, cx| {
                                                view.update(cx, |view, cx| {
                                                    view.select_repository_from_switcher(
                                                        repository.clone(),
                                                        cx,
                                                    );
                                                    view.repository_switcher_open = false;
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

                menu
            })
    }

    pub(super) fn render_pull_request_switcher(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let pull_request_label = self.header_pull_request_label();
        let pull_request_query =
            normalized_search_query(&self.pull_request_search_input.read(cx).value());
        let pull_requests = self.filtered_switcher_pull_requests(cx);
        let inbox_mode = self.pull_request_inbox_mode;
        let selected_pull_request = self.selected_pr;
        let pull_request_selection = self
            .pull_request_switcher_selection
            .min(pull_requests.len().saturating_sub(1));
        let pull_request_search_input = self.pull_request_search_input.clone();
        let has_pull_request_query = !pull_request_query.is_empty();
        let has_current_repository = self.current_repository().is_some();

        Popover::new("pull-request-switcher-popover")
            .appearance(false)
            .anchor(Anchor::TopLeft)
            .open(self.pull_request_switcher_open)
            .on_open_change({
                let view = view.clone();
                move |open, window, cx| {
                    view.update(cx, |view, cx| {
                        view.pull_request_switcher_open = *open;
                        if *open {
                            view.repository_switcher_open = false;
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
                Button::new("pull-request-switcher")
                    .ghost()
                    .small()
                    .compact()
                    .dropdown_caret(true)
                    .disabled(!has_current_repository)
                    .max_w(px(560.))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(0xcbd5e1))
                            .child(pull_request_label),
                    ),
            )
            .content(move |_, _window, popover_cx| {
                let view = view.clone();
                let popover = popover_cx.entity().clone();
                let mut menu = div()
                    .id("pull-request-switcher-menu")
                    .on_key_down({
                        let view = view.clone();
                        move |event, _, cx| {
                            handle_pull_request_switcher_key(event, &view, cx);
                        }
                    })
                    .w(px(520.))
                    .max_h(px(520.))
                    .overflow_y_scroll()
                    .border_1()
                    .border_color(rgb(0x343b44))
                    .bg(rgb(0x171b20))
                    .p_2()
                    .shadow_lg()
                    .child(render_switcher_section_label("search pull requests"))
                    .child(
                        div().px_1().pb_2().child(
                            Input::new(&pull_request_search_input)
                                .small()
                                .cleanable(true),
                        ),
                    )
                    .child(render_switcher_section_label("pull requests"));

                if !has_current_repository {
                    menu = menu.child(render_switcher_empty_row(
                        "select a repository before searching pull requests",
                    ));
                } else if pull_requests.is_empty() {
                    let label = if has_pull_request_query {
                        "no pull requests match search"
                    } else {
                        inbox_mode.empty_message()
                    };
                    menu = menu.child(render_switcher_empty_row(label));
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

                        menu = menu.child(
                            render_switcher_pull_request_row(
                                number,
                                title,
                                author,
                                current,
                                highlighted,
                            )
                            .on_click(move |_, window, cx| {
                                view.update(cx, |view, cx| {
                                    view.select_pull_request(index, cx);
                                    view.pull_request_switcher_open = false;
                                    cx.notify();
                                });
                                popover.update(cx, |popover, cx| {
                                    popover.dismiss(window, cx);
                                });
                            }),
                        );
                    }
                }

                menu
            })
    }
}
