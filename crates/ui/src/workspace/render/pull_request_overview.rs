use gpui::{AnyElement, Context, Div, IntoElement, Rgba, div, prelude::*, px, rgb};
use gpui_component::{Icon, Sizable, StyledExt, avatar::Avatar, scroll::ScrollableElement};
use harbor_domain::{Label, PullRequest, PullRequestPerson, PullRequestTeam};

use crate::{
    icons::Octicon,
    panels::render_review_markdown_body,
    visual::{Tone, color, tone_colors},
    workspace::AppView,
};

const PULL_REQUEST_OVERVIEW_ROW_HEIGHT: f32 = 44.0;
const PULL_REQUEST_OVERVIEW_MAX_HEIGHT: f32 = 220.0;

impl AppView {
    pub(super) fn render_pull_request_overview(
        &self,
        pr: &PullRequest,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let chevron = if self.pull_request_overview_expanded {
            Octicon::ChevronDown
        } else {
            Octicon::ChevronRight
        };

        div()
            .image_cache(gpui::retain_all("pull-request-overview-avatar-cache"))
            .flex_none()
            .border_b_1()
            .border_color(color::border())
            .bg(color::content_background())
            .child(
                div()
                    .id("pull-request-overview-toggle")
                    .debug_selector(|| "pull-request-overview-toggle".to_string())
                    .h(px(PULL_REQUEST_OVERVIEW_ROW_HEIGHT))
                    .w_full()
                    .min_w_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .hover(|element| element.bg(color::row_hover()))
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.toggle_pull_request_overview(cx);
                    }))
                    .child(Icon::new(chevron).xsmall().text_color(color::text_muted()))
                    .child(
                        div()
                            .flex_none()
                            .text_sm()
                            .font_medium()
                            .text_color(color::text_primary())
                            .child("Description"),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .truncate()
                            .text_xs()
                            .text_color(color::text_muted())
                            .child(pull_request_description_preview(pr)),
                    ),
            )
            .when(self.pull_request_overview_expanded, |element| {
                element.child(
                    div()
                        .debug_selector(|| "pull-request-overview-content".to_string())
                        .max_h(px(PULL_REQUEST_OVERVIEW_MAX_HEIGHT))
                        .overflow_y_scrollbar()
                        .px_3()
                        .pb_3()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(render_pull_request_description(pr))
                        .when(!pr.assignees.is_empty(), |element| {
                            element.child(render_overview_section(
                                "Assignees",
                                render_people_row(&pr.assignees),
                            ))
                        })
                        .when(has_review_requests(pr), |element| {
                            element.child(render_overview_section(
                                "Reviewers",
                                render_review_requests_row(
                                    &pr.requested_reviewers,
                                    &pr.requested_teams,
                                ),
                            ))
                        })
                        .when(!pr.labels.is_empty(), |element| {
                            element.child(render_overview_section(
                                "Labels",
                                render_labels_row(&pr.labels),
                            ))
                        }),
                )
            })
    }
}

fn pull_request_description_preview(pr: &PullRequest) -> String {
    pr.body
        .as_deref()
        .and_then(description_preview)
        .unwrap_or_else(|| "No description".to_string())
}

fn description_preview(body: &str) -> Option<String> {
    body.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line == "---" {
                return None;
            }

            let line = line
                .strip_prefix("- ")
                .or_else(|| line.strip_prefix("* "))
                .or_else(|| line.strip_prefix("+ "))
                .or_else(|| line.strip_prefix("> "))
                .unwrap_or(line)
                .trim();
            let line = line
                .strip_prefix("[ ] ")
                .or_else(|| line.strip_prefix("[x] "))
                .or_else(|| line.strip_prefix("[X] "))
                .unwrap_or(line)
                .trim_matches(&['*', '_', '`'][..]);

            (!line.is_empty()).then(|| line.split_whitespace().collect::<Vec<_>>().join(" "))
        })
        .next()
}

fn render_overview_section(title: &'static str, body: AnyElement) -> impl IntoElement {
    div()
        .w_full()
        .min_w_0()
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
            body,
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
    render_chip()
        .child(render_person_avatar(person))
        .child(div().min_w_0().truncate().child(login))
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
        .child(div().min_w_0().truncate().child(label))
        .into_any_element()
}

fn render_label_chip(label: &Label) -> AnyElement {
    let swatch = label
        .color
        .as_deref()
        .and_then(parse_label_color)
        .unwrap_or_else(|| tone_colors(Tone::Neutral).text);

    render_chip()
        .child(div().size(px(8.0)).flex_none().rounded_full().bg(swatch))
        .child(div().min_w_0().truncate().child(label.name.clone()))
        .into_any_element()
}

fn render_chip() -> Div {
    div()
        .max_w(px(220.0))
        .min_w_0()
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
    use super::{avatar_initial, description_preview, parse_label_color};

    #[test]
    fn description_preview_prefers_content_over_headings() {
        assert_eq!(
            description_preview(
                "## summary\n\n- adds a large review inbox fixture\n- includes review threads"
            )
            .as_deref(),
            Some("adds a large review inbox fixture")
        );
    }

    #[test]
    fn description_preview_normalizes_task_list_markdown() {
        assert_eq!(
            description_preview("# validation\n\n- [x]   cargo test --workspace").as_deref(),
            Some("cargo test --workspace")
        );
    }

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
