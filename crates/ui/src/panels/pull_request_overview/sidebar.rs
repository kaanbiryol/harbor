use crate::gpui_component::{Icon, Sizable, StyledExt};
use crate::{
    actions::PullRequestMetadataField,
    icons::Octicon,
    panels::{
        ImmediateTooltip,
        people::{render_avatar, render_initial_avatar},
    },
    visual::{Tone, color, tone_colors},
    workspace::AppView,
};
use gpui::{AnyElement, Context, Div, IntoElement, Rgba, div, prelude::*, px, rgb};
use harbor_domain::{
    Label, PullRequest, PullRequestPerson, PullRequestTeam,
    reviews::{PullRequestReviewer, ReviewerStatus, pull_request_reviewers},
};

impl AppView {
    pub(super) fn render_people_card(
        &self,
        pr: &PullRequest,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let author = PullRequestPerson {
            login: pr.author.clone(),
            avatar_url: pr.author_avatar_url.clone(),
        };
        let reviewers = pull_request_reviewers(
            &pr.requested_reviewers,
            self.review_state.pull_request_reviews(),
        );
        let reviewer_add_control =
            self.render_metadata_add_control(PullRequestMetadataField::Reviewer, cx);
        let assignee_add_control =
            self.render_metadata_add_control(PullRequestMetadataField::Assignee, cx);

        render_overview_card("People")
            .debug_selector(|| "pull-request-people-card".to_string())
            .child(render_overview_section(
                "Author",
                div()
                    .debug_selector(|| "pull-request-author".to_string())
                    .child(render_people_row(std::slice::from_ref(&author)))
                    .into_any_element(),
            ))
            .child(render_overview_section_with_action(
                "Reviewers",
                reviewer_add_control,
                div()
                    .min_h(px(24.0))
                    .child(if !reviewers.is_empty() || !pr.requested_teams.is_empty() {
                        render_reviewers_row(&reviewers, &pr.requested_teams)
                    } else {
                        render_empty_value("No reviewers")
                    })
                    .into_any_element(),
            ))
            .child(render_overview_section_with_action(
                "Assignees",
                assignee_add_control,
                div()
                    .min_h(px(24.0))
                    .child(if pr.assignees.is_empty() {
                        render_empty_value("No assignees")
                    } else {
                        render_people_row(&pr.assignees)
                    })
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

fn render_overview_section_with_action(
    title: &'static str,
    action: AnyElement,
    body: AnyElement,
) -> impl IntoElement {
    div()
        .w_full()
        .min_w_0()
        .pt_3()
        .border_t_1()
        .border_color(color::border_subtle())
        .child(
            div()
                .pb_1()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_medium()
                        .text_color(color::text_muted())
                        .child(title),
                )
                .child(action),
        )
        .child(body)
}

fn render_people_row(people: &[PullRequestPerson]) -> AnyElement {
    render_wrapping_row(people.iter().map(render_person_chip).collect())
}

fn reviewer_status_presentation(status: ReviewerStatus) -> (&'static str, Octicon, Tone) {
    match status {
        ReviewerStatus::Requested => ("Review requested", Octicon::Clock, Tone::Warning),
        ReviewerStatus::Commented => ("Commented", Octicon::Eye, Tone::Info),
        ReviewerStatus::Approved => ("Approved", Octicon::Check, Tone::Success),
        ReviewerStatus::ChangesRequested => ("Changes requested", Octicon::X, Tone::Danger),
    }
}

fn render_reviewers_row(
    reviewers: &[PullRequestReviewer],
    teams: &[PullRequestTeam],
) -> AnyElement {
    let mut chips = Vec::with_capacity(reviewers.len() + teams.len());
    chips.extend(reviewers.iter().map(render_reviewer_chip));
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
        .child(render_avatar(
            &person.login,
            person.avatar_url.as_deref(),
            16.0,
        ))
        .child(render_chip_label(login))
        .into_any_element()
}

fn render_reviewer_chip(reviewer: &PullRequestReviewer) -> AnyElement {
    let login = reviewer.person.login.clone();
    let selector = format!("pull-request-person-{login}");
    let status_selector = format!("pull-request-review-status-{login}");
    let (status_label, status_icon, status_tone) = reviewer_status_presentation(reviewer.status);
    let status_colors = tone_colors(status_tone);

    render_chip()
        .debug_selector(move || selector.clone())
        .child(render_avatar(
            &reviewer.person.login,
            reviewer.person.avatar_url.as_deref(),
            16.0,
        ))
        .child(render_chip_label(login.clone()))
        .child(ImmediateTooltip::new(
            format!("pull-request-review-status-tooltip-{login}"),
            status_label,
            div()
                .debug_selector(move || status_selector.clone())
                .flex_none()
                .child(
                    Icon::new(status_icon)
                        .xsmall()
                        .text_color(status_colors.text),
                ),
        ))
        .into_any_element()
}

fn render_team_chip(team: &PullRequestTeam) -> AnyElement {
    let label = if team.name.trim().is_empty() {
        team.slug.clone()
    } else {
        team.name.clone()
    };

    render_chip()
        .child(render_initial_avatar(&label, 16.0))
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

pub(super) fn parse_label_color(color: &str) -> Option<Rgba> {
    let color = color.trim().trim_start_matches('#');
    if color.len() != 6 || !color.chars().all(|character| character.is_ascii_hexdigit()) {
        return None;
    }

    u32::from_str_radix(color, 16).ok().map(rgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_label_colors() {
        assert!(parse_label_color("34d399").is_some());
        assert!(parse_label_color("#34d399").is_some());
        assert!(parse_label_color("bad").is_none());
        assert!(parse_label_color("zzzzzz").is_none());
    }
}
