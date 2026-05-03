use gpui::{
    Anchor, App, Context, Div, FocusHandle, Focusable, IntoElement, Render, Stateful, Window, div,
    prelude::*, px, rgb, uniform_list,
};
use gpui_component::{
    Disableable, Sizable, StyledExt, TitleBar,
    button::{Button, ButtonVariants},
    input::Input,
    popover::Popover,
};
use harbor_domain::{PullRequest, RepoId};

use crate::actions::*;
use crate::panels::{
    merge_blocker, render_actions_panel, render_changed_file_row, render_checks_panel,
    render_diff_panel, render_logs_panel, render_merge_state, render_pull_request_row,
    render_review_decision, render_review_panel, review_action_blocker,
};
use crate::workspace::AppView;

impl Focusable for AppView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn render_switcher_section_label(label: &'static str) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .text_xs()
        .text_color(rgb(0x7d8794))
        .child(label)
}

fn render_switcher_empty_row(label: &'static str) -> impl IntoElement {
    div()
        .px_2()
        .py_2()
        .text_sm()
        .text_color(rgb(0x7d8794))
        .child(label)
}

fn render_switcher_error_row(error: String) -> impl IntoElement {
    div()
        .mx_1()
        .mb_1()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0x4b2a2f))
        .bg(rgb(0x211417))
        .px_2()
        .py_2()
        .text_xs()
        .text_color(rgb(0xf87171))
        .child(error)
}

fn normalized_search_query(query: &str) -> String {
    query.trim().to_lowercase()
}

fn repository_matches_query(repository: &RepoId, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    repository.full_name().to_lowercase().contains(query)
}

fn pull_request_matches_query(pull_request: &PullRequest, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    pull_request.title.to_lowercase().contains(query)
        || pull_request.number.to_string().contains(query)
        || pull_request.author.to_lowercase().contains(query)
}

fn render_switcher_repository_row(repository: &RepoId, selected: bool) -> Stateful<Div> {
    div()
        .id(format!("switcher-repository-{}", repository.full_name()))
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .rounded_sm()
        .px_2()
        .py_1()
        .text_sm()
        .cursor_pointer()
        .when(selected, |element| element.bg(rgb(0x263241)))
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
                .text_xs()
                .text_color(rgb(0x7d8794))
                .child(if selected { "current" } else { "repo" }),
        )
}

fn render_switcher_pull_request_row(
    number: u64,
    title: String,
    author: String,
    selected: bool,
) -> Stateful<Div> {
    div()
        .id(("switcher-pull-request", number))
        .flex()
        .flex_col()
        .gap_1()
        .rounded_sm()
        .px_2()
        .py_2()
        .text_sm()
        .cursor_pointer()
        .when(selected, |element| element.bg(rgb(0x263241)))
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

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.did_focus {
            window.focus(&self.focus_handle, cx);
            self.did_focus = true;
        }

        let selected_pr = self.selected_pull_request().cloned();

        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle(cx))
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::open_selected))
            .on_action(cx.listener(Self::cycle_panel_tab))
            .on_action(cx.listener(Self::toggle_command_palette))
            .on_action(cx.listener(Self::toggle_repository_switcher))
            .on_action(cx.listener(Self::close_panel))
            .on_action(cx.listener(Self::refresh_selected))
            .on_action(cx.listener(Self::checkout_pr))
            .on_action(cx.listener(Self::open_in_browser))
            .on_action(cx.listener(Self::approve_pr))
            .on_action(cx.listener(Self::request_changes))
            .on_action(cx.listener(Self::merge_pr))
            .on_action(cx.listener(Self::open_logs))
            .on_action(cx.listener(Self::trigger_build))
            .on_action(cx.listener(Self::rerun_failed))
            .on_action(cx.listener(Self::filter_current_list))
            .on_action(cx.listener(Self::select_next_file))
            .on_action(cx.listener(Self::select_previous_file))
            .on_action(cx.listener(Self::select_next_hunk))
            .on_action(cx.listener(Self::select_previous_hunk))
            .on_action(cx.listener(Self::copy_active_file_path))
            .on_action(cx.listener(Self::open_active_file_on_github))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x101214))
            .text_color(rgb(0xe6e8eb))
            .child(self.render_title_bar(cx))
            .when(self.command_palette_open, |element| {
                element.child(self.render_command_palette())
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .gap_2()
                    .p_2()
                    .child(self.render_inbox(cx))
                    .child(self.render_details(selected_pr.as_ref(), cx))
                    .child(self.render_panel(selected_pr.as_ref(), cx)),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(rgb(0x9aa4b2))
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .child(self.status.clone()),
            )
    }
}

impl AppView {
    fn current_repository(&self) -> Option<&RepoId> {
        self.selected_pull_request()
            .map(|pull_request| &pull_request.repo)
            .or(self.configured_repo.as_ref())
    }

    fn switcher_repositories(&self) -> Vec<RepoId> {
        let mut repositories = self.repositories.clone();

        if let Some(repository) = self.configured_repo.clone() {
            if !repositories.iter().any(|existing| existing == &repository) {
                repositories.push(repository);
            }
        }

        for pull_request in &self.pull_requests {
            if !repositories
                .iter()
                .any(|repository| repository == &pull_request.repo)
            {
                repositories.push(pull_request.repo.clone());
            }
        }

        repositories
    }

    fn header_repository_label(&self) -> String {
        self.current_repository()
            .map(|repository| repository.name.clone())
            .unwrap_or_else(|| "repository".to_string())
    }

    fn header_pull_request_label(&self) -> String {
        if let Some(pull_request) = self.selected_pull_request() {
            return format!("#{} {}", pull_request.number, pull_request.title);
        }

        if self.is_loading_prs {
            "loading pull requests".to_string()
        } else if self.load_error.is_some() {
            "pull requests unavailable".to_string()
        } else {
            "no pull request selected".to_string()
        }
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let repository_label = self.header_repository_label();
        let pull_request_label = self.header_pull_request_label();
        let repository_query =
            normalized_search_query(&self.repository_search_input.read(cx).value());
        let pull_request_query =
            normalized_search_query(&self.pull_request_search_input.read(cx).value());
        let repositories = self
            .switcher_repositories()
            .into_iter()
            .filter(|repository| repository_matches_query(repository, &repository_query))
            .collect::<Vec<_>>();
        let current_repository = self.current_repository().cloned();
        let repository_error = self.repository_error.clone();
        let pull_requests = current_repository
            .as_ref()
            .map(|repository| {
                self.pull_requests
                    .iter()
                    .enumerate()
                    .filter(|(_, pull_request)| &pull_request.repo == repository)
                    .filter(|(_, pull_request)| {
                        pull_request_matches_query(pull_request, &pull_request_query)
                    })
                    .map(|(index, pull_request)| (index, pull_request.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let selected_pull_request = self.selected_pr;
        let repository_search_input = self.repository_search_input.clone();
        let pull_request_search_input = self.pull_request_search_input.clone();
        let has_repository_query = !repository_query.is_empty();
        let has_pull_request_query = !pull_request_query.is_empty();
        let has_current_repository = current_repository.is_some();
        let repository_view = view.clone();
        let pull_request_view = view.clone();

        TitleBar::new()
            .bg(rgb(0x101214))
            .border_color(rgb(0x242a31))
            .child(
                div()
                    .flex()
                    .h_full()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .pr_2()
                    .child(
                        div()
                            .flex()
                            .h_full()
                            .min_w_0()
                            .items_center()
                            .gap_1()
                            .child(
                                Popover::new("repository-switcher-popover")
                                    .appearance(false)
                                    .anchor(Anchor::TopLeft)
                                    .open(self.repository_switcher_open)
                                    .on_open_change({
                                        let view = repository_view.clone();
                                        move |open, window, cx| {
                                            view.update(cx, |view, cx| {
                                                view.repository_switcher_open = *open;
                                                if *open {
                                                    view.command_palette_open = false;
                                                    view.pull_request_switcher_open = false;
                                                    view.status =
                                                        "Repository search opened".to_string();
                                                    view.repository_search_input
                                                        .update(cx, |input, cx| {
                                                            input.set_value("", window, cx);
                                                            input.focus(window, cx);
                                                        });
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
                                        let view = repository_view.clone();
                                        let popover = popover_cx.entity().clone();
                                        let mut menu = div()
                                            .id("repository-switcher-menu")
                                            .w(px(460.))
                                            .max_h(px(520.))
                                            .overflow_y_scroll()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(rgb(0x343b44))
                                            .bg(rgb(0x171b20))
                                            .p_2()
                                            .shadow_lg()
                                            .child(render_switcher_section_label(
                                                "search repositories",
                                            ))
                                            .child(div().px_1().pb_2().child(
                                                Input::new(&repository_search_input)
                                                    .small()
                                                    .cleanable(true),
                                            ))
                                            .child(render_switcher_section_label("repositories"));

                                        if let Some(error) = repository_error.clone() {
                                            menu = menu.child(render_switcher_error_row(error));
                                        }

                                        if repositories.is_empty() {
                                            let label = if has_repository_query {
                                                "no repositories match search"
                                            } else {
                                                "no repositories found"
                                            };
                                            menu = menu.child(render_switcher_empty_row(label));
                                        } else {
                                            for repository in &repositories {
                                                let repository = repository.clone();
                                                let selected =
                                                    current_repository.as_ref() == Some(&repository);
                                                let view = view.clone();
                                                let popover = popover.clone();

                                                menu = menu.child(
                                                    render_switcher_repository_row(
                                                        &repository,
                                                        selected,
                                                    )
                                                    .on_click(move |_, window, cx| {
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
                                                    }),
                                                );
                                            }
                                        }

                                        menu
                                    }),
                            )
                            .child(div().px_1().text_color(rgb(0x6f7782)).child("/"))
                            .child(
                                Popover::new("pull-request-switcher-popover")
                                    .appearance(false)
                                    .anchor(Anchor::TopLeft)
                                    .open(self.pull_request_switcher_open)
                                    .on_open_change({
                                        let view = pull_request_view.clone();
                                        move |open, window, cx| {
                                            view.update(cx, |view, cx| {
                                                view.pull_request_switcher_open = *open;
                                                if *open {
                                                    view.command_palette_open = false;
                                                    view.repository_switcher_open = false;
                                                    view.status =
                                                        "Pull request search opened".to_string();
                                                    view.pull_request_search_input
                                                        .update(cx, |input, cx| {
                                                            input.set_value("", window, cx);
                                                            input.focus(window, cx);
                                                        });
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
                                        let view = pull_request_view.clone();
                                        let popover = popover_cx.entity().clone();
                                        let mut menu = div()
                                            .id("pull-request-switcher-menu")
                                            .w(px(520.))
                                            .max_h(px(520.))
                                            .overflow_y_scroll()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(rgb(0x343b44))
                                            .bg(rgb(0x171b20))
                                            .p_2()
                                            .shadow_lg()
                                            .child(render_switcher_section_label(
                                                "search pull requests",
                                            ))
                                            .child(div().px_1().pb_2().child(
                                                Input::new(&pull_request_search_input)
                                                    .small()
                                                    .cleanable(true),
                                            ))
                                            .child(render_switcher_section_label("pull requests"));

                                        if !has_current_repository {
                                            menu = menu.child(render_switcher_empty_row(
                                                "select a repository before searching pull requests",
                                            ));
                                        } else if pull_requests.is_empty() {
                                            let label = if has_pull_request_query {
                                                "no pull requests match search"
                                            } else {
                                                "no open pull requests for selected repository"
                                            };
                                            menu = menu.child(render_switcher_empty_row(label));
                                        } else {
                                            for (index, pull_request) in &pull_requests {
                                                let selected = *index == selected_pull_request;
                                                let number = pull_request.number;
                                                let title = pull_request.title.clone();
                                                let author = pull_request.author.clone();
                                                let view = view.clone();
                                                let popover = popover.clone();
                                                let index = *index;

                                                menu = menu.child(
                                                    render_switcher_pull_request_row(
                                                        number, title, author, selected,
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
                                    }),
                            ),
                    )
                    .child(
                        Button::new("command-palette")
                            .ghost()
                            .small()
                            .compact()
                            .label("cmd+k"),
                    ),
            )
    }

    fn render_command_palette(&self) -> impl IntoElement {
        div()
            .mx_2()
            .mt_2()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x3a424c))
            .bg(rgb(0x171b20))
            .child(
                div()
                    .pb_2()
                    .text_sm()
                    .text_color(rgb(0xf1f5f9))
                    .child("Command palette placeholder"),
            )
            .children(COMMANDS.iter().map(|command| {
                div()
                    .flex()
                    .justify_between()
                    .py_1()
                    .text_sm()
                    .child(command.title)
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x9aa4b2))
                            .child(command.shortcut),
                    )
            }))
    }

    fn render_inbox(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let show_list =
            !self.is_loading_prs && self.load_error.is_none() && !self.pull_requests.is_empty();

        div()
            .w(px(320.))
            .flex()
            .flex_col()
            .min_h_0()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x242a31))
            .bg(rgb(0x15191e))
            .overflow_hidden()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(rgb(0xf1f5f9))
                    .child("Pull request inbox")
                    .child(
                        div().pt_1().text_xs().text_color(rgb(0x9aa4b2)).child(
                            self.configured_repo
                                .as_ref()
                                .map(RepoId::full_name)
                                .unwrap_or_else(|| "no repository selected".to_string()),
                        ),
                    ),
            )
            .when(self.is_loading_prs, |element| {
                element.child(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(rgb(0x9aa4b2))
                        .child("Loading open pull requests..."),
                )
            })
            .when(
                !self.is_loading_prs && self.load_error.is_some(),
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(rgb(0xf87171))
                            .child(self.load_error.clone().unwrap_or_default()),
                    )
                },
            )
            .when(
                !self.is_loading_prs && self.load_error.is_none() && self.pull_requests.is_empty(),
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(rgb(0x9aa4b2))
                            .child("No open pull requests"),
                    )
                },
            )
            .when(show_list, |element| {
                element.child(
                    uniform_list(
                        "pull-request-inbox-list",
                        self.pull_requests.len(),
                        cx.processor(|view, range: std::ops::Range<usize>, _window, cx| {
                            let mut rows = Vec::with_capacity(range.len());

                            for index in range {
                                let Some(pr) = view.pull_requests.get(index) else {
                                    continue;
                                };
                                rows.push(render_pull_request_row(
                                    index,
                                    pr,
                                    index == view.selected_pr,
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
                )
            })
    }

    fn render_details(&self, pr: Option<&PullRequest>, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(pr) = pr else {
            return div()
                .w(px(360.))
                .flex()
                .flex_col()
                .min_h_0()
                .rounded_md()
                .border_1()
                .border_color(rgb(0x242a31))
                .bg(rgb(0x15191e))
                .overflow_hidden()
                .p_3()
                .text_sm()
                .text_color(rgb(0x9aa4b2))
                .child("Select a pull request to see details")
                .into_any_element();
        };

        let review_action_disabled = self.configured_repo.is_none()
            || self.is_running_pr_action
            || review_action_blocker(pr).is_some();
        let merge_action_disabled = self.configured_repo.is_none()
            || self.is_running_pr_action
            || merge_blocker(pr).is_some();

        div()
            .w(px(360.))
            .flex()
            .flex_col()
            .min_h_0()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x242a31))
            .bg(rgb(0x15191e))
            .overflow_hidden()
            .child(
                div()
                    .p_3()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .child(
                        div()
                            .text_sm()
                            .child(format!("#{} {}", pr.number, pr.title)),
                    )
                    .child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(rgb(0x9aa4b2))
                            .child(format!("{} / {}", pr.repo.full_name(), pr.head_sha)),
                    )
                    .when(self.is_loading_details, |element| {
                        element.child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(rgb(0x9aa4b2))
                                .child("Loading latest PR details..."),
                        )
                    })
                    .when_some(self.details_error.clone(), |element, error| {
                        element.child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(rgb(0xf87171))
                                .child(error),
                        )
                    })
                    .child(
                        div()
                            .pt_2()
                            .flex()
                            .gap_2()
                            .child(render_review_decision(pr.review_decision))
                            .child(render_merge_state(pr.merge_state))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0xfbbf24))
                                    .child(format!("{} unresolved", pr.unresolved_threads)),
                            ),
                    )
                    .child(
                        div()
                            .pt_3()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("approve-pr")
                                    .label("approve")
                                    .small()
                                    .outline()
                                    .loading(self.is_running_pr_action)
                                    .disabled(review_action_disabled)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.run_pull_request_action(
                                            PullRequestAction::Approve,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("request-pr-changes")
                                    .label("changes")
                                    .small()
                                    .outline()
                                    .loading(self.is_running_pr_action)
                                    .disabled(review_action_disabled)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.run_pull_request_action(
                                            PullRequestAction::RequestChanges,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("merge-pr")
                                    .label("merge")
                                    .small()
                                    .outline()
                                    .loading(self.is_running_pr_action)
                                    .disabled(merge_action_disabled)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.run_pull_request_action(PullRequestAction::Merge, cx);
                                    })),
                            ),
                    )
                    .when_some(self.pr_action_error.clone(), |element, error| {
                        element.child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(rgb(0xf87171))
                                .child(error),
                        )
                    }),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(rgb(0x9aa4b2))
                    .child("Changed files"),
            )
            .when(self.is_loading_files, |element| {
                element.child(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(rgb(0x9aa4b2))
                        .child("Loading changed files..."),
                )
            })
            .when_some(self.files_error.clone(), |element, error| {
                element.child(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(rgb(0xf87171))
                        .child(error),
                )
            })
            .when(
                !self.is_loading_files && self.files_error.is_none() && self.files.is_empty(),
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(rgb(0x9aa4b2))
                            .child("No changed files"),
                    )
                },
            )
            .when(
                !self.is_loading_files && self.files_error.is_none() && !self.files.is_empty(),
                |element| {
                    element.child(
                        uniform_list(
                            "changed-files-list",
                            self.files.len(),
                            cx.processor(|view, range: std::ops::Range<usize>, _window, cx| {
                                let mut rows = Vec::with_capacity(range.len());

                                for index in range {
                                    let Some(file) = view.files.get(index) else {
                                        continue;
                                    };
                                    rows.push(render_changed_file_row(
                                        index,
                                        file,
                                        index == view.active_file,
                                        cx,
                                    ));
                                }

                                rows
                            }),
                        )
                        .track_scroll(&self.file_list_scroll)
                        .flex_1()
                        .min_h_0()
                        .w_full(),
                    )
                },
            )
            .into_any_element()
    }

    fn render_panel(&self, pr: Option<&PullRequest>, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h_0()
            .min_w_0()
            .rounded_md()
            .border_1()
            .border_color(rgb(0x242a31))
            .bg(rgb(0x15191e))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .p_2()
                    .border_1()
                    .border_color(rgb(0x242a31))
                    .children(
                        PanelTab::ALL
                            .iter()
                            .copied()
                            .enumerate()
                            .map(|(index, tab)| {
                                let active = tab == self.active_tab;
                                let view = view.clone();

                                div()
                                    .id(("panel-tab", index))
                                    .px_3()
                                    .py_1()
                                    .rounded_sm()
                                    .text_sm()
                                    .cursor_pointer()
                                    .when(active, |element| element.bg(rgb(0x243244)))
                                    .hover(move |element| {
                                        if active {
                                            element
                                        } else {
                                            element.bg(rgb(0x1d2530))
                                        }
                                    })
                                    .on_click(move |_, _, cx| {
                                        view.update(cx, |view, cx| {
                                            view.select_panel_tab(tab, cx);
                                        });
                                    })
                                    .child(tab.label())
                            }),
                    ),
            )
            .child(
                div()
                    .id("panel-content-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .min_w_0()
                    .p_3()
                    .text_sm()
                    .child(match self.active_tab {
                        PanelTab::Diff => render_diff_panel(
                            self.active_file(),
                            self.active_diff(),
                            &self.review_threads,
                            self.is_loading_files,
                            self.files_error.as_deref(),
                            self.diff_list_scroll.clone(),
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Review => render_review_panel(
                            &self.pull_request_reviews,
                            &self.review_threads,
                            self.is_loading_reviews,
                            self.reviews_error.as_deref(),
                            self.review_list_scroll.clone(),
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Checks => render_checks_panel(
                            pr.map(|pr| pr.checks_summary).unwrap_or_default(),
                            &self.check_runs,
                            self.is_loading_checks,
                            self.checks_error.as_deref(),
                        )
                        .into_any_element(),
                        PanelTab::Actions => render_actions_panel(
                            pr,
                            &self.workflow_runs,
                            self.is_loading_workflows,
                            self.workflows_error.as_deref(),
                            self.action_error.as_deref(),
                            self.is_running_action,
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Logs => render_logs_panel(
                            self.selected_workflow_run_for_logs(),
                            &self.workflow_jobs,
                            self.log_chunk.as_ref(),
                            self.is_loading_logs,
                            self.logs_error.as_deref(),
                            self.log_list_scroll.clone(),
                            cx,
                        )
                        .into_any_element(),
                    }),
            )
    }
}
