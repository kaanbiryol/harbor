use gpui::{Anchor, Context, div, prelude::*, px, uniform_list};
use gpui_component::{
    Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::Input,
    popover::Popover,
};

use crate::{
    visual::color,
    workspace::{AppView, RepositorySwitcherChoice, normalized_search_query},
};

use super::{
    super::render_switcher_section_label,
    repository_switcher_rows::{
        handle_repository_switcher_key, render_switcher_empty_row, render_switcher_error_row,
        render_switcher_loading_row, render_switcher_notice_row, render_switcher_repository_row,
        render_switcher_typed_repository_row, repository_switcher_list_height,
    },
};

impl AppView {
    pub(super) fn render_repository_switcher(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let repository_label = self.header_repository_label();
        let repository_search_value = self
            .repository_state
            .repository_search_input
            .read(cx)
            .value();
        let repository_query = normalized_search_query(&repository_search_value);
        let choices = self.repository_switcher_choices(cx);
        let current_repository = self.current_repository().cloned();
        let repository_error = self.repository_state.error().map(str::to_string);
        let repository_notice = self.repository_state.notice().map(str::to_string);
        let is_loading_repositories = self.repository_state.is_loading();
        let repository_selection = self
            .repository_state
            .repository_switcher_selection
            .min(choices.len().saturating_sub(1));
        let repository_search_input = self.repository_state.repository_search_input.clone();
        let has_repository_query = !repository_query.is_empty();
        let repository_switcher_disabled = self.github_auth_gate_visible();

        Popover::new("repository-switcher-popover")
            .appearance(false)
            .anchor(Anchor::TopLeft)
            .open(!repository_switcher_disabled && self.repository_state.repository_switcher_open)
            .on_open_change({
                let view = view.clone();
                move |open, window, cx| {
                    view.update(cx, |view, cx| {
                        if view.github_auth_gate_visible() {
                            view.repository_state.repository_switcher_open = false;
                            cx.notify();
                            return;
                        }

                        view.repository_state.repository_switcher_open = *open;
                        if *open {
                            view.pull_request_inbox_search_open = false;
                            view.file_filter_popover_open = false;
                            view.status = "Repository search opened".to_string();
                            view.repository_state.repository_search_input.update(
                                cx,
                                |input, cx| {
                                    input.set_value("", window, cx);
                                    input.focus(window, cx);
                                },
                            );
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
                    .dropdown_caret(!repository_switcher_disabled)
                    .disabled(repository_switcher_disabled)
                    .max_w(px(260.))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_medium()
                            .text_color(color::text_primary())
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
                    .border_color(color::border_strong())
                    .bg(color::elevated_background())
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

                if let Some(notice) = repository_notice.clone() {
                    menu = menu.child(render_switcher_notice_row(notice));
                }

                if is_loading_repositories {
                    menu = menu.child(render_switcher_loading_row(
                        "Fetching repositories from GitHub...",
                    ));
                }

                if choices.is_empty() {
                    let label = if has_repository_query || is_loading_repositories {
                        "Type owner/repo to open a repository"
                    } else {
                        "No repositories found. Type owner/repo to open a repository"
                    };
                    menu = menu.child(render_switcher_empty_row(label));
                } else {
                    let choice_count = choices.len();
                    let list_height = repository_switcher_list_height(choice_count);
                    let choices = choices.clone();
                    let current_repository = current_repository.clone();
                    let view = view.clone();
                    let popover = popover.clone();

                    menu = menu.child(
                        uniform_list(
                            "repository-switcher-list",
                            choice_count,
                            move |range, _window, _cx| {
                                let mut rows = Vec::with_capacity(range.len());

                                for row_index in range {
                                    let Some(choice) = choices.get(row_index).cloned() else {
                                        continue;
                                    };
                                    let current =
                                        current_repository.as_ref() == Some(choice.repository());
                                    let highlighted = row_index == repository_selection;
                                    let view = view.clone();
                                    let popover = popover.clone();

                                    let row = match &choice {
                                        RepositorySwitcherChoice::Cached(repository) => {
                                            render_switcher_repository_row(
                                                repository,
                                                current,
                                                highlighted,
                                            )
                                        }
                                        RepositorySwitcherChoice::Typed(repository) => {
                                            render_switcher_typed_repository_row(
                                                repository,
                                                highlighted,
                                            )
                                        }
                                    };

                                    rows.push(row.on_click(move |_, window, cx| {
                                        view.update(cx, |view, cx| {
                                            view.select_repository_choice_from_switcher(
                                                choice.clone(),
                                                cx,
                                            );
                                            view.repository_state.repository_switcher_open = false;
                                            view.pull_request_inbox_search_open = false;
                                            cx.notify();
                                        });
                                        popover.update(cx, |popover, cx| {
                                            popover.dismiss(window, cx);
                                        });
                                    }));
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
}
