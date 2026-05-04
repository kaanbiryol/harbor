use gpui::{
    Anchor, App, Context, Div, Entity, FocusHandle, Focusable, IntoElement, KeyDownEvent, Render,
    Stateful, Window, div, prelude::*, px, rgb, uniform_list,
};
use gpui_component::{
    Disableable, Icon, IconName, Sizable, StyledExt, TitleBar,
    button::{Button, ButtonVariants, DropdownButton},
    input::Input,
    popover::Popover,
};
use harbor_domain::{PullRequest, RepoId};
use harbor_git::ExternalApp;
use harbor_github::SubmitPullRequestReviewEvent;

use crate::actions::*;
use crate::panels::{
    merge_blocker, render_actions_panel, render_changed_file_row, render_changed_folder_row,
    render_checks_panel, render_diff_panel, render_logs_panel, render_merge_state,
    render_pull_request_row, render_review_decision, render_review_panel, review_action_blocker,
};
use crate::workspace::{
    AppView, ChangedFileTreeRow, PendingReviewSession, PullRequestInboxMode,
    normalized_search_query, parse_repo_id,
};

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

fn pending_review_comment_count_label(comment_count: usize) -> String {
    match comment_count {
        0 => "pending comments".to_string(),
        1 => "1 pending comment".to_string(),
        count => format!("{count} pending comments"),
    }
}

fn render_switcher_repository_row(
    repository: &RepoId,
    current: bool,
    highlighted: bool,
) -> Stateful<Div> {
    div()
        .id(format!("switcher-repository-{}", repository.full_name()))
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
        .child(div().text_xs().text_color(rgb(0x7d8794)).child(if current {
            "current"
        } else {
            "repo"
        }))
}

fn render_switcher_typed_repository_row(repository: &RepoId, highlighted: bool) -> Stateful<Div> {
    div()
        .id(format!(
            "switcher-typed-repository-{}",
            repository.full_name()
        ))
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
        .child(div().text_xs().text_color(rgb(0x7d8794)).child("typed"))
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

fn open_with_action(app: ExternalApp) -> Box<dyn gpui::Action> {
    match app {
        ExternalApp::VsCode => Box::new(OpenWithVsCode),
        ExternalApp::Cursor => Box::new(OpenWithCursor),
        ExternalApp::Zed => Box::new(OpenWithZed),
        ExternalApp::Finder => Box::new(OpenWithFinder),
        ExternalApp::Terminal => Box::new(OpenWithTerminal),
        ExternalApp::Ghostty => Box::new(OpenWithGhostty),
        ExternalApp::Warp => Box::new(OpenWithWarp),
        ExternalApp::Xcode => Box::new(OpenWithXcode),
    }
}

fn open_with_icon(app: ExternalApp) -> IconName {
    match app {
        ExternalApp::Finder => IconName::FolderOpen,
        ExternalApp::Terminal | ExternalApp::Ghostty | ExternalApp::Warp => {
            IconName::SquareTerminal
        }
        ExternalApp::VsCode | ExternalApp::Cursor | ExternalApp::Zed | ExternalApp::Xcode => {
            IconName::Frame
        }
    }
}

pub(crate) fn open_with_app_disabled(
    has_local_path: bool,
    local_action_running: bool,
    app: ExternalApp,
) -> bool {
    !has_local_path || local_action_running || !app.is_available()
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.did_focus {
            if self.repository_switcher_open {
                self.repository_search_input
                    .update(cx, |input, cx| input.focus(window, cx));
            } else {
                window.focus(&self.focus_handle, cx);
            }
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
            .on_action(cx.listener(Self::choose_local_checkout))
            .on_action(cx.listener(Self::open_with_vs_code))
            .on_action(cx.listener(Self::open_with_cursor))
            .on_action(cx.listener(Self::open_with_zed))
            .on_action(cx.listener(Self::open_with_finder))
            .on_action(cx.listener(Self::open_with_terminal))
            .on_action(cx.listener(Self::open_with_ghostty))
            .on_action(cx.listener(Self::open_with_warp))
            .on_action(cx.listener(Self::open_with_xcode))
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
        let repository_search_value = self.repository_search_input.read(cx).value();
        let repository_query = normalized_search_query(&repository_search_value);
        let pull_request_query =
            normalized_search_query(&self.pull_request_search_input.read(cx).value());
        let repositories = self.filtered_switcher_repositories(cx);
        let typed_repository = if repositories.is_empty() {
            parse_repo_id(&repository_search_value)
        } else {
            None
        };
        let current_repository = self.current_repository().cloned();
        let repository_error = self.repository_error.clone();
        let is_loading_repositories = self.is_loading_repositories;
        let pull_requests = self.filtered_switcher_pull_requests(cx);
        let inbox_mode = self.pull_request_inbox_mode;
        let selected_pull_request = self.selected_pr;
        let repository_selection = self
            .repository_switcher_selection
            .min(repositories.len().saturating_sub(1));
        let pull_request_selection = self
            .pull_request_switcher_selection
            .min(pull_requests.len().saturating_sub(1));
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
                                                    view.file_filter_popover_open = false;
                                                    view.status =
                                                        "Repository search opened".to_string();
                                                    view.repository_search_input
                                                        .update(cx, |input, cx| {
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
                                        let view = repository_view.clone();
                                        let popover = popover_cx.entity().clone();
                                        let mut menu = div()
                                            .id("repository-switcher-menu")
                                            .on_key_down({
                                                let view = view.clone();
                                                move |event, _, cx| {
                                                    handle_repository_switcher_key(
                                                        event, &view, cx,
                                                    );
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
                                                    render_switcher_typed_repository_row(
                                                        &repository,
                                                        true,
                                                    )
                                                    .on_click(move |_, window, cx| {
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
                                                    }),
                                                );
                                            } else {
                                                let label = if has_repository_query
                                                    || is_loading_repositories
                                                {
                                                    "Type owner/repo to open a repository"
                                                } else {
                                                    "No repositories found. Type owner/repo to open a repository"
                                                };
                                                menu = menu.child(render_switcher_empty_row(label));
                                            }
                                        } else {
                                            for (row_index, repository) in
                                                repositories.iter().enumerate()
                                            {
                                                let repository = repository.clone();
                                                let current =
                                                    current_repository.as_ref() == Some(&repository);
                                                let highlighted =
                                                    row_index == repository_selection;
                                                let view = view.clone();
                                                let popover = popover.clone();

                                                menu = menu.child(
                                                    render_switcher_repository_row(
                                                        &repository,
                                                        current,
                                                        highlighted,
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
                                                    view.file_filter_popover_open = false;
                                                    view.status =
                                                        "Pull request search opened".to_string();
                                                    view.pull_request_search_input
                                                        .update(cx, |input, cx| {
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
                                        let view = pull_request_view.clone();
                                        let popover = popover_cx.entity().clone();
                                        let mut menu = div()
                                            .id("pull-request-switcher-menu")
                                            .on_key_down({
                                                let view = view.clone();
                                                move |event, _, cx| {
                                                    handle_pull_request_switcher_key(
                                                        event, &view, cx,
                                                    );
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
                                                inbox_mode.empty_message()
                                            };
                                            menu = menu.child(render_switcher_empty_row(label));
                                        } else {
                                            for (row_index, (index, pull_request)) in
                                                pull_requests.iter().enumerate()
                                            {
                                                let current = *index == selected_pull_request;
                                                let highlighted =
                                                    row_index == pull_request_selection;
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
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(self.render_open_with_dropdown())
                            .child(
                                Button::new("command-palette")
                                    .ghost()
                                    .small()
                                    .compact()
                                    .label("cmd+k"),
                            ),
                    ),
            )
    }

    fn render_open_with_dropdown(&self) -> impl IntoElement {
        let has_repository = self.current_repository().is_some();
        let local_path = self.current_repository_local_path().cloned();
        let has_local_path = local_path.is_some();
        let local_action_running = self.local_task.is_some();

        DropdownButton::new("open-with")
            .button(
                Button::new("open-with-primary")
                    .icon(IconName::ExternalLink)
                    .label("Open With")
                    .small()
                    .compact(),
            )
            .small()
            .compact()
            .outline()
            .disabled(!has_repository || local_action_running)
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                let mut menu = menu.max_w(px(320.)).menu_with_disabled(
                    "Choose Local Checkout...",
                    Box::new(ChooseLocalCheckout),
                    !has_repository || local_action_running,
                );

                if let Some(local_path) = local_path.clone() {
                    menu = menu.label(format!("Local: {}", local_path.display()));
                } else {
                    menu = menu.label("No local checkout selected");
                }

                menu = menu.separator();

                for app in ExternalApp::ALL {
                    let disabled =
                        open_with_app_disabled(has_local_path, local_action_running, app);
                    menu = menu.menu_with_icon_and_disabled(
                        app.label(),
                        open_with_icon(app),
                        open_with_action(app),
                        disabled,
                    );
                }

                menu
            })
    }

    fn render_command_palette(&self) -> impl IntoElement {
        div()
            .mx_2()
            .mt_2()
            .p_3()
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
        let current_mode = self.pull_request_inbox_mode;
        let repository_label = self
            .configured_repo
            .as_ref()
            .map(RepoId::full_name)
            .unwrap_or_else(|| "choose a repository from the header".to_string());
        let empty_message = if self.configured_repo.is_some() {
            current_mode.empty_message()
        } else {
            "Choose a repository from the header"
        };

        div()
            .w(px(320.))
            .flex()
            .flex_col()
            .min_h_0()
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
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(rgb(0x9aa4b2))
                            .child(repository_label),
                    )
                    .child(div().pt_2().flex().items_center().gap_1().children(
                        PullRequestInboxMode::ALL.into_iter().map(|mode| {
                            let active = mode == current_mode;
                            let button =
                                Button::new(format!("pull-request-inbox-mode-{}", mode.key()))
                                    .label(mode.label())
                                    .small()
                                    .compact();
                            let button = if active {
                                button.primary()
                            } else {
                                button.ghost()
                            };

                            button.on_click(cx.listener(move |view, _, _, cx| {
                                view.select_pull_request_inbox_mode(mode, cx);
                            }))
                        }),
                    )),
            )
            .when(self.is_loading_prs, |element| {
                element.child(
                    div()
                        .flex_1()
                        .px_3()
                        .py_3()
                        .text_sm()
                        .text_color(rgb(0x9aa4b2))
                        .child(format!("Loading {}...", current_mode.status_label())),
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
                            .child(empty_message),
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

        let review_action_disabled =
            self.is_running_pr_action || review_action_blocker(pr).is_some();
        let merge_action_disabled = self.is_running_pr_action || merge_blocker(pr).is_some();
        let pull_request_url = pr.url.clone();
        let pull_request_number = pr.number;

        div()
            .w(px(360.))
            .flex()
            .flex_col()
            .min_h_0()
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
                            .id(("pull-request-title-link", pr.number))
                            .text_sm()
                            .text_color(rgb(0x93c5fd))
                            .cursor_pointer()
                            .hover(|element| element.text_color(rgb(0xbfdbfe)))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                cx.open_url(&pull_request_url);
                                view.status =
                                    format!("Opened PR #{pull_request_number} in browser");
                                cx.notify();
                            }))
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
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.run_pull_request_action(
                                            PullRequestAction::Approve,
                                            window,
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
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.run_pull_request_action(
                                            PullRequestAction::RequestChanges,
                                            window,
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
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.run_pull_request_action(
                                            PullRequestAction::Merge,
                                            window,
                                            cx,
                                        );
                                    })),
                            ),
                    )
                    .when_some(self.pending_review.clone(), |element, pending_review| {
                        element.child(self.render_pending_review_bar(pending_review, cx))
                    })
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
            .child(self.render_changed_files_header(cx))
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
                !self.is_loading_files
                    && self.files_error.is_none()
                    && !self.files.is_empty()
                    && self.changed_file_tree_rows(cx).is_empty(),
                |element| {
                    element.child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_3()
                            .text_sm()
                            .text_color(rgb(0x9aa4b2))
                            .child("No files match filter"),
                    )
                },
            )
            .when(
                !self.is_loading_files
                    && self.files_error.is_none()
                    && !self.files.is_empty()
                    && !self.changed_file_tree_rows(cx).is_empty(),
                |element| {
                    let row_count = self.changed_file_tree_rows(cx).len();

                    element.child(
                        uniform_list(
                            "changed-files-list",
                            row_count,
                            cx.processor(|view, range: std::ops::Range<usize>, _window, cx| {
                                let tree_rows = view.changed_file_tree_rows(cx);
                                let mut rows = Vec::with_capacity(range.len());

                                for row_index in range {
                                    let Some(row) = tree_rows.get(row_index) else {
                                        continue;
                                    };
                                    match row {
                                        ChangedFileTreeRow::Folder(folder_row) => {
                                            rows.push(render_changed_folder_row(folder_row, cx));
                                        }
                                        ChangedFileTreeRow::File(file_row) => {
                                            let Some(file) = view.files.get(file_row.file_index)
                                            else {
                                                continue;
                                            };
                                            rows.push(render_changed_file_row(
                                                file_row,
                                                file,
                                                file_row.file_index == view.active_file,
                                                view.reviewed_file_paths.contains(&file.path),
                                                cx,
                                            ));
                                        }
                                    }
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

    fn render_changed_files_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let reviewed_count = self.reviewed_file_count();
        let total_count = self.files.len();
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
                            .anchor(Anchor::TopLeft)
                            .open(self.file_filter_popover_open)
                            .on_open_change({
                                let view = view.clone();
                                move |open, _, cx| {
                                    view.update(cx, |view, cx| {
                                        view.file_filter_popover_open = *open;
                                        if *open {
                                            view.command_palette_open = false;
                                            view.repository_switcher_open = false;
                                            view.pull_request_switcher_open = false;
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

    fn render_pending_review_bar(
        &self,
        pending_review: PendingReviewSession,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let body_input = self.pending_review_body_input.clone();
        let submitting = self.is_submitting_pending_review;
        let body_empty = self
            .pending_review_body_input
            .read(cx)
            .value()
            .trim()
            .is_empty();
        let comment_submit_disabled =
            submitting || (pending_review.comment_count == 0 && body_empty);

        div().pt_3().child(
            div()
                .border_1()
                .border_color(rgb(0x355071))
                .bg(rgb(0x172033))
                .px_3()
                .py_2()
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
                                .text_color(rgb(0xe6e8eb))
                                .child("pending review"),
                        )
                        .child(div().text_color(rgb(0x93c5fd)).child(
                            pending_review_comment_count_label(pending_review.comment_count),
                        )),
                )
                .child(
                    div().pt_2().child(
                        Input::new(&body_input)
                            .small()
                            .w_full()
                            .appearance(false)
                            .bordered(true)
                            .focus_bordered(true),
                    ),
                )
                .child(
                    div()
                        .pt_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("submit-pending-approve")
                                .label("Approve")
                                .small()
                                .outline()
                                .loading(submitting)
                                .disabled(submitting)
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.submit_pending_pull_request_review(
                                        SubmitPullRequestReviewEvent::Approve,
                                        window,
                                        cx,
                                    );
                                })),
                        )
                        .child(
                            Button::new("submit-pending-comment")
                                .label("Comment")
                                .small()
                                .outline()
                                .loading(submitting)
                                .disabled(comment_submit_disabled)
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.submit_pending_pull_request_review(
                                        SubmitPullRequestReviewEvent::Comment,
                                        window,
                                        cx,
                                    );
                                })),
                        )
                        .child(
                            Button::new("submit-pending-request-changes")
                                .label("Request changes")
                                .small()
                                .outline()
                                .loading(submitting)
                                .disabled(submitting)
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.submit_pending_pull_request_review(
                                        SubmitPullRequestReviewEvent::RequestChanges,
                                        window,
                                        cx,
                                    );
                                })),
                        ),
                )
                .when_some(self.pending_review_error.clone(), |element, error| {
                    element.child(
                        div()
                            .pt_2()
                            .text_xs()
                            .text_color(rgb(0xf87171))
                            .child(error),
                    )
                }),
        )
    }

    fn render_panel(&self, pr: Option<&PullRequest>, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h_0()
            .min_w_0()
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
                            self.review_composer.as_ref(),
                            self.review_comment_error.as_deref(),
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
