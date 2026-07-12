use gpui::{Anchor, AnyElement, Context, Div, IntoElement, Rgba, div, prelude::*, px, rgb};
use gpui_component::{
    Icon, Sizable, StyledExt, avatar::Avatar, button::Button, input::Input, list::ListItem,
    popover::Popover, scroll::ScrollableElement, spinner::Spinner,
};
use harbor_domain::{Label, PullRequest, PullRequestPerson, PullRequestTeam, ReviewDecision};

use crate::{
    actions::PullRequestMetadataField,
    github::{avatar_initial, avatar_url},
    icons::Octicon,
    panels::{
        MergeReadiness, PullRequestReadiness, ReviewReadiness,
        merge_readiness as classify_merge_readiness,
        pull_request_readiness as classify_pull_request_readiness,
        review_readiness as classify_review_readiness,
    },
    visual::{Tone, color, tone_colors},
    workspace::AppView,
};

impl AppView {
    pub(super) fn render_people_card(
        &self,
        pr: &PullRequest,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let author = PullRequestPerson {
            login: pr.author.clone(),
            avatar_url: None,
        };

        render_overview_card("People")
            .debug_selector(|| "pull-request-people-card".to_string())
            .child(render_overview_section(
                "Author",
                div()
                    .debug_selector(|| "pull-request-author".to_string())
                    .child(render_people_row(std::slice::from_ref(&author)))
                    .into_any_element(),
            ))
            .child(render_overview_section(
                "Reviewers",
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .min_h(px(28.0))
                    .child(if has_review_requests(pr) {
                        render_review_requests_row(&pr.requested_reviewers, &pr.requested_teams)
                    } else {
                        render_empty_value("No reviewers requested")
                    })
                    .child(self.render_metadata_add_control(PullRequestMetadataField::Reviewer, cx))
                    .into_any_element(),
            ))
            .child(render_overview_section(
                "Assignees",
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .min_h(px(28.0))
                    .child(if pr.assignees.is_empty() {
                        render_empty_value("No assignees")
                    } else {
                        render_people_row(&pr.assignees)
                    })
                    .child(self.render_metadata_add_control(PullRequestMetadataField::Assignee, cx))
                    .into_any_element(),
            ))
            .into_any_element()
    }

    pub(super) fn render_labels_card(
        &self,
        pr: &PullRequest,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        render_overview_card("Labels")
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .min_h(px(28.0))
                    .child(if pr.labels.is_empty() {
                        render_empty_value("No labels")
                    } else {
                        render_labels_row(&pr.labels)
                    })
                    .child(self.render_metadata_add_control(PullRequestMetadataField::Label, cx)),
            )
            .into_any_element()
    }

    fn render_metadata_add_control(
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
                    .trigger(
                        Button::new(format!("open-add-{field_name}"))
                            .icon(Octicon::Plus)
                            .small()
                            .compact()
                            .outline()
                            .tooltip(format!("Add {field_name}")),
                    )
                    .content(move |_, _window, popover_cx| {
                        let popover = popover_cx.entity().clone();
                        let mut content = div()
                            .debug_selector(move || format!("add-{field_name}-menu"))
                            .w(px(264.0))
                            .max_h(px(360.0))
                            .overflow_hidden()
                            .border_1()
                            .border_color(color::border_strong())
                            .bg(color::elevated_background())
                            .shadow_lg()
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
                                    .child("Loading..."),
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

pub(super) fn render_overview_card(title: &'static str) -> Div {
    div()
        .rounded_sm()
        .border_1()
        .border_color(color::border())
        .bg(color::content_background())
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_semibold()
                .text_color(color::text_primary())
                .child(title),
        )
}

pub(super) fn render_readiness_row(
    id: &'static str,
    label: &'static str,
    description: &'static str,
    value: impl Into<String>,
    icon: Octicon,
    tone: Tone,
    navigable: bool,
) -> ListItem {
    let colors = tone_colors(tone);
    let value = value.into();

    ListItem::new(id)
        .w_full()
        .h(px(52.0))
        .px_0()
        .py_0()
        .rounded_none()
        .disabled(!navigable)
        .when(label != "Review", |element| {
            element.border_t_1().border_color(color::border_subtle())
        })
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .child(Icon::new(icon).xsmall().text_color(colors.text))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .text_color(color::text_primary())
                                .child(label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(color::text_muted())
                                .child(description),
                        ),
                ),
        )
        .suffix(move |_, _| {
            div()
                .flex()
                .items_center()
                .text_xs()
                .font_medium()
                .text_color(colors.text)
                .child(value.clone())
        })
}

pub(super) fn render_readiness_status(
    label: &'static str,
    description: &'static str,
    tone: Tone,
) -> Div {
    let colors = tone_colors(tone);

    div()
        .py_3()
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .size(px(44.0))
                .flex_none()
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(colors.background)
                .child(
                    Icon::new(Octicon::CodeSquare)
                        .size(px(16.0))
                        .text_color(colors.text),
                ),
        )
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_medium()
                        .text_color(colors.text)
                        .child(label),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child(description),
                ),
        )
}

pub(super) fn render_readiness_section_title(title: &'static str) -> Div {
    div()
        .mt_2()
        .pt_3()
        .pb_1()
        .border_t_1()
        .border_color(color::border_subtle())
        .text_sm()
        .font_semibold()
        .text_color(color::text_primary())
        .child(title)
}

pub(super) fn render_summary_row(
    id: &'static str,
    label: &'static str,
    value: impl Into<String>,
    tone: Tone,
) -> impl IntoElement {
    let colors = tone_colors(tone);

    div()
        .id(id)
        .h(px(36.0))
        .flex()
        .items_center()
        .gap_2()
        .child(
            Icon::new(Octicon::CheckCircle)
                .xsmall()
                .text_color(colors.text),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_xs()
                .text_color(color::text_secondary())
                .child(label),
        )
        .child(
            div()
                .text_xs()
                .text_color(color::text_muted())
                .child(value.into()),
        )
}

pub(super) fn pull_request_readiness(pr: &PullRequest) -> (&'static str, &'static str, Tone) {
    let readiness = classify_pull_request_readiness(pr);
    let tone = match readiness {
        PullRequestReadiness::Conflicts
        | PullRequestReadiness::ChecksFailed
        | PullRequestReadiness::ChangesRequested => Tone::Danger,
        PullRequestReadiness::ChecksPending
        | PullRequestReadiness::ReviewRequired
        | PullRequestReadiness::ConversationsOpen => Tone::Warning,
        PullRequestReadiness::Draft => Tone::Neutral,
        PullRequestReadiness::Ready => Tone::Success,
    };

    (readiness.label(), readiness.description(), tone)
}

pub(super) fn review_readiness_description(decision: Option<ReviewDecision>) -> &'static str {
    classify_review_readiness(decision).description()
}

pub(super) fn review_readiness(decision: Option<ReviewDecision>) -> (&'static str, Tone) {
    let readiness = classify_review_readiness(decision);
    let tone = match readiness {
        ReviewReadiness::Approved => Tone::Success,
        ReviewReadiness::ChangesRequested => Tone::Danger,
        ReviewReadiness::Pending => Tone::Warning,
    };

    (readiness.label(), tone)
}

pub(super) fn merge_readiness(pr: &PullRequest) -> (&'static str, &'static str, Tone) {
    let readiness = classify_merge_readiness(pr);
    let tone = match readiness {
        MergeReadiness::Conflicts | MergeReadiness::Blocked => Tone::Danger,
        MergeReadiness::Behind
        | MergeReadiness::WaitingForApproval
        | MergeReadiness::ConversationsOpen
        | MergeReadiness::ChecksPending => Tone::Warning,
        MergeReadiness::Unknown => Tone::Neutral,
        MergeReadiness::Ready => Tone::Success,
    };

    (readiness.label(), readiness.description(), tone)
}

fn render_empty_value(label: &'static str) -> AnyElement {
    div()
        .text_xs()
        .text_color(color::text_muted())
        .child(label)
        .into_any_element()
}

fn render_overview_section(title: &'static str, body: AnyElement) -> impl IntoElement {
    div()
        .w_full()
        .min_w_0()
        .pt_3()
        .border_t_1()
        .border_color(color::border_subtle())
        .child(
            div()
                .pb_1()
                .text_xs()
                .font_medium()
                .text_color(color::text_muted())
                .child(title),
        )
        .child(body)
}

fn render_people_row(people: &[PullRequestPerson]) -> AnyElement {
    render_wrapping_row(people.iter().map(render_person_chip).collect())
}

fn render_review_requests_row(
    reviewers: &[PullRequestPerson],
    teams: &[PullRequestTeam],
) -> AnyElement {
    let mut chips = Vec::with_capacity(reviewers.len() + teams.len());
    chips.extend(reviewers.iter().map(render_person_chip));
    chips.extend(teams.iter().map(render_team_chip));

    render_wrapping_row(chips)
}

fn render_labels_row(labels: &[Label]) -> AnyElement {
    render_wrapping_row(labels.iter().map(render_label_chip).collect())
}

fn render_wrapping_row(children: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_1()
        .min_w_0()
        .children(children)
        .into_any_element()
}

fn render_person_chip(person: &PullRequestPerson) -> AnyElement {
    let login = person.login.clone();
    let selector = format!("pull-request-person-{login}");
    render_chip()
        .debug_selector(move || selector.clone())
        .child(render_person_avatar(person))
        .child(render_chip_label(login))
        .into_any_element()
}

fn render_team_chip(team: &PullRequestTeam) -> AnyElement {
    let label = if team.name.trim().is_empty() {
        team.slug.clone()
    } else {
        team.name.clone()
    };

    render_chip()
        .child(render_team_avatar(&label))
        .child(render_chip_label(label))
        .into_any_element()
}

fn render_label_chip(label: &Label) -> AnyElement {
    let selector = format!("pull-request-label-{}", label.name);
    let swatch = label
        .color
        .as_deref()
        .and_then(parse_label_color)
        .unwrap_or_else(|| tone_colors(Tone::Neutral).text);

    render_chip()
        .debug_selector(move || selector.clone())
        .child(div().size(px(8.0)).flex_none().rounded_full().bg(swatch))
        .child(render_chip_label(label.name.clone()))
        .into_any_element()
}

fn render_chip_label(label: String) -> impl IntoElement {
    div().flex_none().max_w(px(188.0)).truncate().child(label)
}

fn render_chip() -> Div {
    div()
        .flex_none()
        .max_w(px(220.0))
        .flex()
        .items_center()
        .gap_1()
        .rounded_xs()
        .border_1()
        .border_color(color::border())
        .bg(color::panel_background())
        .px_1()
        .py_0p5()
        .text_xs()
        .text_color(color::text_secondary())
}

fn render_person_avatar(person: &PullRequestPerson) -> AnyElement {
    render_person_avatar_with_size(person, 16.0)
}

pub(super) fn render_person_avatar_with_size(person: &PullRequestPerson, size: f32) -> AnyElement {
    let avatar_url = person
        .avatar_url
        .clone()
        .or_else(|| avatar_url(&person.login));

    if let Some(avatar_url) = avatar_url {
        return Avatar::new()
            .src(avatar_url)
            .name(person.login.clone())
            .with_size(px(size))
            .into_any_element();
    }

    render_fallback_avatar(&person.login, size).into_any_element()
}

fn render_team_avatar(label: &str) -> AnyElement {
    render_fallback_avatar(label, 16.0).into_any_element()
}

fn render_fallback_avatar(label: &str, size: f32) -> impl IntoElement {
    div()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .rounded_full()
        .border_1()
        .border_color(color::border_strong())
        .bg(color::row_selected_subtle())
        .text_size(px((size * 0.52).max(9.0)))
        .line_height(px(size))
        .font_semibold()
        .text_color(color::accent())
        .child(avatar_initial(label))
}

fn has_review_requests(pr: &PullRequest) -> bool {
    !pr.requested_reviewers.is_empty() || !pr.requested_teams.is_empty()
}

pub(super) fn parse_label_color(color: &str) -> Option<Rgba> {
    let color = color.trim().trim_start_matches('#');
    if color.len() != 6 || !color.chars().all(|character| character.is_ascii_hexdigit()) {
        return None;
    }

    u32::from_str_radix(color, 16).ok().map(rgb)
}
