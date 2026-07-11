use gpui::{Anchor, AnyElement, Context, Div, IntoElement, Rgba, div, prelude::*, px, rgb};
use gpui_component::{
    Disableable, Icon, Sizable, StyledExt,
    avatar::Avatar,
    button::{Button, ButtonVariants},
    input::Input,
    list::ListItem,
    popover::Popover,
    scroll::ScrollableElement,
};
use harbor_domain::{
    Label, MergeState, PullRequest, PullRequestPerson, PullRequestTeam, ReviewDecision,
};

use crate::{
    actions::{PanelTab, PullRequestMetadataField},
    icons::Octicon,
    panels::{overview_markdown_body, render_review_markdown_body},
    visual::{Tone, color, tone_colors},
    workspace::AppView,
};

const OVERVIEW_SIDEBAR_WIDTH: f32 = 280.0;

impl AppView {
    pub(super) fn render_pull_request_overview_panel(
        &self,
        pr: Option<&PullRequest>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(pr) = pr else {
            return div()
                .text_sm()
                .text_color(color::text_muted())
                .child("Select a pull request to see its overview")
                .into_any_element();
        };

        div()
            .debug_selector(|| "pull-request-overview-panel".to_string())
            .image_cache(gpui::retain_all("pull-request-overview-avatar-cache"))
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scrollbar()
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_3()
                    .w_full()
                    .min_w_0()
                    .child(self.render_description_card(pr, cx))
                    .child(
                        div()
                            .debug_selector(|| "pull-request-overview-sidebar".to_string())
                            .w(px(OVERVIEW_SIDEBAR_WIDTH))
                            .flex_none()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(self.render_merge_readiness_card(pr, cx))
                            .child(self.render_people_card(pr, cx))
                            .child(self.render_labels_card(pr, cx)),
                    ),
            )
            .into_any_element()
    }
}

impl AppView {
    fn render_merge_readiness_card(&self, pr: &PullRequest, cx: &mut Context<Self>) -> AnyElement {
        let (review_label, review_tone) = review_readiness(pr.review_decision);
        let (merge_label, merge_tone) = merge_readiness(pr.merge_state);
        let unresolved_tone = if pr.unresolved_threads == 0 {
            Tone::Success
        } else {
            Tone::Warning
        };

        render_overview_card("Merge readiness")
            .debug_selector(|| "pull-request-merge-readiness".to_string())
            .gap_0()
            .child(render_readiness_row(
                "pull-request-review-readiness-row",
                "Review",
                review_label,
                Octicon::Eye,
                review_tone,
                false,
                false,
            ))
            .child(render_readiness_row(
                "pull-request-merge-readiness-row",
                "Merge",
                merge_label,
                Octicon::CheckCircle,
                merge_tone,
                true,
                false,
            ))
            .child(
                div()
                    .debug_selector(|| "pull-request-unresolved-conversations".to_string())
                    .child(
                        render_readiness_row(
                            "pull-request-unresolved-conversations-row",
                            "Conversations",
                            format!("{} unresolved", pr.unresolved_threads),
                            Octicon::CommentDiscussion,
                            unresolved_tone,
                            true,
                            true,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.select_panel_tab(PanelTab::Review, cx);
                        })),
                    ),
            )
            .into_any_element()
    }

    fn render_description_card(&self, pr: &PullRequest, cx: &mut Context<Self>) -> AnyElement {
        let editing = self.pull_request_description_editing;
        let saving = self
            .action_runtime
            .pull_request_description_action_running();
        let error = self
            .action_runtime
            .pull_request_description_action_error()
            .map(str::to_string);
        let description_input = self.pull_request_description_input.clone();

        div()
            .debug_selector(|| "pull-request-overview-description".to_string())
            .flex_1()
            .min_w_0()
            .rounded_sm()
            .border_1()
            .border_color(color::border())
            .bg(color::content_background())
            .p_4()
            .child(
                div()
                    .pb_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .text_color(color::text_primary())
                            .child("Description"),
                    )
                    .when(!editing, |element| {
                        element.child(
                            Button::new("edit-pull-request-description")
                                .icon(Octicon::Pencil)
                                .xsmall()
                                .secondary()
                                .tooltip("Edit description if your GitHub permissions allow it")
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.start_pull_request_description_edit(window, cx);
                                })),
                        )
                    }),
            )
            .when(!editing, |element| {
                element.child(render_pull_request_description(pr))
            })
            .when(editing, |element| {
                element
                    .child(Input::new(&description_input))
                    .when_some(error, |element, error| {
                        element.child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(color::danger())
                                .child(error),
                        )
                    })
                    .child(
                        div()
                            .pt_3()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("cancel-pull-request-description")
                                    .label("Cancel")
                                    .small()
                                    .outline()
                                    .disabled(saving)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.cancel_pull_request_description_edit(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("save-pull-request-description")
                                    .label("Save")
                                    .small()
                                    .loading(saving)
                                    .disabled(saving)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.save_pull_request_description(window, cx);
                                    })),
                            ),
                    )
            })
            .into_any_element()
    }

    fn render_people_card(&self, pr: &PullRequest, cx: &mut Context<Self>) -> AnyElement {
        let author = PullRequestPerson {
            login: pr.author.clone(),
            avatar_url: None,
        };

        render_overview_card("People")
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

    fn render_labels_card(&self, pr: &PullRequest, cx: &mut Context<Self>) -> AnyElement {
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
        let input_is_empty = input.read(cx).value().trim().is_empty();
        let action_running = self.action_runtime.pull_request_metadata_action_running();
        let action_field = self.action_runtime.pull_request_metadata_field();
        let field_running = action_running && action_field == Some(field);
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
                        move |open, window, cx| {
                            if *open {
                                input.update(cx, |input, cx| input.focus(window, cx));
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
                    .content(move |_, _window, _popover_cx| {
                        div()
                            .w(px(280.0))
                            .border_1()
                            .border_color(color::border_strong())
                            .bg(color::elevated_background())
                            .shadow_lg()
                            .p_2()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(Input::new(&input).small().cleanable(true))
                            .when_some(error.clone(), |element, error| {
                                element
                                    .child(div().text_xs().text_color(color::danger()).child(error))
                            })
                            .child(
                                div().flex().justify_end().child(
                                    Button::new(format!("add-pull-request-{field_name}"))
                                        .icon(Octicon::Plus)
                                        .label("Add")
                                        .small()
                                        .loading(field_running)
                                        .disabled(action_running || input_is_empty)
                                        .on_click({
                                            let view = view.clone();
                                            move |_, window, cx| {
                                                view.update(cx, |view, cx| {
                                                    view.add_pull_request_metadata(
                                                        field, window, cx,
                                                    );
                                                });
                                            }
                                        }),
                                ),
                            )
                    }),
            )
            .into_any_element()
    }
}

fn render_overview_card(title: &'static str) -> Div {
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

fn render_readiness_row(
    id: &'static str,
    label: &'static str,
    value: impl Into<String>,
    icon: Octicon,
    tone: Tone,
    divided: bool,
    navigable: bool,
) -> ListItem {
    let colors = tone_colors(tone);
    let value = value.into();

    ListItem::new(id)
        .w_full()
        .h(px(40.0))
        .px_0()
        .py_0()
        .rounded_none()
        .disabled(!navigable)
        .when(divided, |element| {
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
                        .text_xs()
                        .text_color(color::text_secondary())
                        .child(label),
                ),
        )
        .suffix(move |_, _| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .font_medium()
                .text_color(colors.text)
                .child(value.clone())
                .when(navigable, |element| {
                    element.child(
                        Icon::new(Octicon::ChevronRight)
                            .xsmall()
                            .text_color(color::text_muted()),
                    )
                })
        })
}

fn review_readiness(decision: Option<ReviewDecision>) -> (&'static str, Tone) {
    match decision {
        Some(ReviewDecision::Approved) => ("Approved", Tone::Success),
        Some(ReviewDecision::ChangesRequested) => ("Changes requested", Tone::Danger),
        Some(ReviewDecision::ReviewRequired) => ("Review required", Tone::Warning),
        None => ("Not reviewed", Tone::Info),
    }
}

fn merge_readiness(state: Option<MergeState>) -> (&'static str, Tone) {
    match state {
        Some(MergeState::Clean) => ("Ready", Tone::Success),
        Some(MergeState::Dirty) => ("Conflicts", Tone::Danger),
        Some(MergeState::Blocked) => ("Blocked", Tone::Danger),
        Some(MergeState::Behind) => ("Behind", Tone::Warning),
        Some(MergeState::Unknown) | None => ("Unknown", Tone::Neutral),
    }
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

fn render_pull_request_description(pr: &PullRequest) -> AnyElement {
    let Some(body) = pr
        .body
        .as_deref()
        .map(str::trim)
        .filter(|body| !body.is_empty())
    else {
        return div()
            .text_sm()
            .text_color(color::text_muted())
            .child("No description")
            .into_any_element();
    };

    div()
        .min_w_0()
        .pr_1()
        .text_sm()
        .text_color(color::text_secondary())
        .child(render_review_markdown_body(
            format!("pull-request-description-{}", pr.number),
            &overview_markdown_body(body),
        ))
        .into_any_element()
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
    let avatar_url = person
        .avatar_url
        .clone()
        .or_else(|| github_avatar_url_for_login(&person.login));

    if let Some(avatar_url) = avatar_url {
        return Avatar::new()
            .src(avatar_url)
            .name(person.login.clone())
            .with_size(px(16.0))
            .into_any_element();
    }

    render_fallback_avatar(&person.login, 16.0).into_any_element()
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

fn avatar_initial(label: &str) -> String {
    label
        .trim()
        .chars()
        .find(|character| character.is_alphanumeric())
        .map(|character| character.to_uppercase().collect())
        .unwrap_or_else(|| "?".to_string())
}

fn github_avatar_url_for_login(login: &str) -> Option<String> {
    let login = login.trim();

    if login.is_empty()
        || login.eq_ignore_ascii_case("ghost")
        || login.eq_ignore_ascii_case("you")
        || login.chars().any(char::is_whitespace)
    {
        None
    } else {
        Some(format!("https://github.com/{login}.png?size=48"))
    }
}

fn has_review_requests(pr: &PullRequest) -> bool {
    !pr.requested_reviewers.is_empty() || !pr.requested_teams.is_empty()
}

fn parse_label_color(color: &str) -> Option<Rgba> {
    let color = color.trim().trim_start_matches('#');
    if color.len() != 6 || !color.chars().all(|character| character.is_ascii_hexdigit()) {
        return None;
    }

    u32::from_str_radix(color, 16).ok().map(rgb)
}

#[cfg(test)]
mod tests {
    use super::{avatar_initial, parse_label_color};

    #[test]
    fn parses_github_label_colors() {
        assert!(parse_label_color("34d399").is_some());
        assert!(parse_label_color("#34d399").is_some());
        assert!(parse_label_color("bad").is_none());
        assert!(parse_label_color("zzzzzz").is_none());
    }

    #[test]
    fn derives_avatar_initials() {
        assert_eq!(avatar_initial("octocat"), "O");
        assert_eq!(avatar_initial(" team-reviewers"), "T");
        assert_eq!(avatar_initial(""), "?");
    }
}
