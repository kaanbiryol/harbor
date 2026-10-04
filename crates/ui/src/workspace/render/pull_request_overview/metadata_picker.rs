use super::sidebar::{parse_label_color, render_fallback_avatar};
use crate::gpui_component::{
    Icon, Sizable, avatar::Avatar, button::Button, input::Input, list::ListItem, popover::Popover,
    scroll::ScrollableElement, spinner::Spinner,
};
use crate::{
    actions::PullRequestMetadataField,
    dropdown::dropdown_menu_surface,
    icons::Octicon,
    panels::ImmediateTooltip,
    visual::{Tone, color, tone_colors},
    workspace::AppView,
};
use gpui::{Anchor, AnyElement, Context, IntoElement, div, prelude::*, px};

impl AppView {
    pub(super) fn render_metadata_add_control(
        &self,
        field: PullRequestMetadataField,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let input = self.pull_request_metadata_input(field);
        let input_value = input.read(cx).value();
        let query = input_value.trim();
        let query = match field {
            PullRequestMetadataField::Reviewer | PullRequestMetadataField::Assignee => {
                query.trim_start_matches('@')
            }
            PullRequestMetadataField::Label => query,
        };
        let display_query = query.to_string();
        let query = query.to_lowercase();
        let input_is_empty = query.is_empty();
        let selected_pull_request = self.selected_pull_request();
        let mut choices: Vec<(String, Option<String>, Option<String>)> = match field {
            PullRequestMetadataField::Reviewer => {
                self.pull_request_metadata_options
                    .options
                    .reviewers
                    .iter()
                    .filter(|person| {
                        selected_pull_request.is_none_or(|pull_request| {
                            !person.login.eq_ignore_ascii_case(&pull_request.author)
                                && !pull_request.requested_reviewers.iter().any(|reviewer| {
                                    reviewer.login.eq_ignore_ascii_case(&person.login)
                                })
                        })
                    })
                    .map(|person| (person.login.clone(), person.avatar_url.clone(), None))
                    .collect()
            }
            PullRequestMetadataField::Assignee => self
                .pull_request_metadata_options
                .options
                .assignees
                .iter()
                .filter(|person| {
                    selected_pull_request.is_none_or(|pull_request| {
                        !pull_request
                            .assignees
                            .iter()
                            .any(|assignee| assignee.login.eq_ignore_ascii_case(&person.login))
                    })
                })
                .map(|person| (person.login.clone(), person.avatar_url.clone(), None))
                .collect(),
            PullRequestMetadataField::Label => self
                .pull_request_metadata_options
                .options
                .labels
                .iter()
                .filter(|label| {
                    selected_pull_request.is_none_or(|pull_request| {
                        !pull_request
                            .labels
                            .iter()
                            .any(|existing| existing.name.eq_ignore_ascii_case(&label.name))
                    })
                })
                .map(|label| (label.name.clone(), None, label.color.clone()))
                .collect(),
        };
        if !query.is_empty() {
            choices.retain(|(name, _, _)| name.to_lowercase().contains(&query));
        }
        choices.truncate(20);
        let has_exact_choice = choices
            .iter()
            .any(|(name, _, _)| name.eq_ignore_ascii_case(&query));
        let choices_loading = self.pull_request_metadata_options.loading;
        let choices_error = self.pull_request_metadata_options.error.clone();
        let action_running = self.action_runtime.pull_request_metadata_action_running();
        let action_field = self.action_runtime.pull_request_metadata_field();
        let error = (action_field == Some(field))
            .then(|| {
                self.action_runtime
                    .pull_request_metadata_action_error()
                    .map(str::to_string)
            })
            .flatten();
        let view = cx.entity().clone();
        let field_name = field.name();
        div()
            .debug_selector(move || format!("add-{field_name}-control"))
            .flex_none()
            .child(
                Popover::new(format!("add-{field_name}-popover"))
                    .appearance(false)
                    .anchor(Anchor::TopRight)
                    .on_open_change({
                        let input = input.clone();
                        let view = view.clone();
                        move |open, window, cx| {
                            if *open {
                                input.update(cx, |input, cx| input.focus(window, cx));
                                view.update(cx, |view, cx| {
                                    view.load_pull_request_metadata_options(window, cx);
                                });
                            }
                        }
                    })
                    .trigger(ImmediateTooltip::new(
                        format!("open-add-{field_name}-tooltip"),
                        format!("Add {field_name}"),
                        Button::new(format!("open-add-{field_name}"))
                            .icon(Octicon::Plus)
                            .xsmall()
                            .compact()
                            .outline(),
                    ))
                    .content(move |_, _window, popover_cx| {
                        let popover = popover_cx.entity().clone();
                        let mut content = dropdown_menu_surface(popover_cx, 264.0)
                            .debug_selector(move || format!("add-{field_name}-menu"))
                            .max_h(px(360.0))
                            .overflow_hidden()
                            .p_1()
                            .flex()
                            .flex_col()
                            .child(
                                Input::new(&input)
                                    .small()
                                    .cleanable(true)
                                    .disabled(action_running)
                                    .prefix(
                                        Icon::new(Octicon::Search)
                                            .xsmall()
                                            .text_color(color::text_muted()),
                                    ),
                            )
                            .child(div().my_1().border_t_1().border_color(color::border()));
                        if choices_loading {
                            content = content.child(
                                div()
                                    .px_2()
                                    .py_3()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap_2()
                                    .text_xs()
                                    .text_color(color::text_muted())
                                    .child(Spinner::new().small())
                                    .child("Loading choices…"),
                            );
                        } else if let Some(choices_error) = choices_error.clone() {
                            content = content.child(
                                div()
                                    .px_2()
                                    .py_2()
                                    .text_xs()
                                    .text_color(color::danger())
                                    .child(choices_error),
                            );
                        } else if choices.is_empty() && input_is_empty {
                            let empty_message = match field {
                                PullRequestMetadataField::Reviewer => "No reviewers available",
                                PullRequestMetadataField::Assignee => "No assignees available",
                                PullRequestMetadataField::Label => "No labels available",
                            };
                            content = content.child(
                                div()
                                    .px_2()
                                    .py_3()
                                    .flex()
                                    .justify_center()
                                    .text_xs()
                                    .text_color(color::text_muted())
                                    .child(empty_message),
                            );
                        } else if !choices.is_empty() {
                            content =
                                content.child(
                                    div().max_h(px(240.0)).overflow_y_scrollbar().children(
                                        choices.iter().enumerate().map(
                                            |(index, (name, avatar_url, label_color))| {
                                                let name = name.clone();
                                                let selected_name = name.clone();
                                                let input = input.clone();
                                                let view = view.clone();
                                                let popover = popover.clone();
                                                render_metadata_menu_item(
                                                    format!("metadata-{field_name}-choice-{index}"),
                                                    render_metadata_choice_leading(
                                                        field,
                                                        &name,
                                                        avatar_url.clone(),
                                                        label_color.as_deref(),
                                                    ),
                                                    name,
                                                    action_running,
                                                )
                                                .on_click(move |_, window, cx| {
                                                    input.update(cx, |input, cx| {
                                                        input.set_value(&selected_name, window, cx);
                                                    });
                                                    view.update(cx, |view, cx| {
                                                        view.add_pull_request_metadata(
                                                            field, window, cx,
                                                        );
                                                    });
                                                    popover.update(cx, |popover, cx| {
                                                        popover.dismiss(window, cx);
                                                    });
                                                })
                                            },
                                        ),
                                    ),
                                );
                        }

                        if !choices_loading && !input_is_empty && !has_exact_choice {
                            let add_label = match field {
                                PullRequestMetadataField::Reviewer
                                | PullRequestMetadataField::Assignee => {
                                    format!("Add @{display_query}")
                                }
                                PullRequestMetadataField::Label => {
                                    format!("Add \"{display_query}\"")
                                }
                            };
                            let view = view.clone();
                            let popover = popover.clone();
                            content = content
                                .when(!choices.is_empty(), |element| {
                                    element.child(
                                        div().my_1().border_t_1().border_color(color::border()),
                                    )
                                })
                                .child(
                                    render_metadata_menu_item(
                                        format!("add-pull-request-{field_name}"),
                                        Icon::new(Octicon::Plus)
                                            .xsmall()
                                            .text_color(color::text_muted())
                                            .into_any_element(),
                                        add_label,
                                        action_running,
                                    )
                                    .suffix(|_, _| {
                                        div().text_xs().text_color(color::text_muted()).child("↵")
                                    })
                                    .on_click(
                                        move |_, window, cx| {
                                            view.update(cx, |view, cx| {
                                                view.add_pull_request_metadata(field, window, cx);
                                            });
                                            popover.update(cx, |popover, cx| {
                                                popover.dismiss(window, cx);
                                            });
                                        },
                                    ),
                                );
                        }

                        content.when_some(error.clone(), |element, error| {
                            element.child(
                                div()
                                    .mt_1()
                                    .border_t_1()
                                    .border_color(color::border())
                                    .px_2()
                                    .pt_2()
                                    .pb_1()
                                    .text_xs()
                                    .text_color(color::danger())
                                    .child(error),
                            )
                        })
                    }),
            )
            .into_any_element()
    }
}

fn render_metadata_menu_item(
    id: String,
    leading: AnyElement,
    label: String,
    disabled: bool,
) -> ListItem {
    ListItem::new(id)
        .w_full()
        .h(px(34.0))
        .px_2()
        .py_0()
        .rounded_xs()
        .disabled(disabled)
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .gap_2()
                .child(leading)
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .text_color(color::text_primary())
                        .child(label),
                ),
        )
}

fn render_metadata_choice_leading(
    field: PullRequestMetadataField,
    name: &str,
    avatar_url: Option<String>,
    label_color: Option<&str>,
) -> AnyElement {
    match field {
        PullRequestMetadataField::Reviewer | PullRequestMetadataField::Assignee => {
            if let Some(avatar_url) = avatar_url {
                Avatar::new()
                    .rounded_full()
                    .src(avatar_url)
                    .name(name.to_string())
                    .with_size(px(20.0))
                    .into_any_element()
            } else {
                render_fallback_avatar(name, 20.0).into_any_element()
            }
        }
        PullRequestMetadataField::Label => div()
            .size(px(10.0))
            .flex_none()
            .rounded_full()
            .bg(label_color
                .and_then(parse_label_color)
                .unwrap_or_else(|| tone_colors(Tone::Neutral).text))
            .into_any_element(),
    }
}
