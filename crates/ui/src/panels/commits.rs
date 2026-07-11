use gpui::{Context, IntoElement, ListState, div, list, prelude::*};
use gpui_component::StyledExt;
use harbor_domain::PullRequestCommit;

use crate::{date_time::natural_time_label, visual::color, workspace::AppView};

use super::{
    render_empty_panel_card, render_error_panel_card, render_panel_card, render_panel_header,
    sync_virtual_list_item_count,
};

pub(crate) struct CommitsPanelRenderInput<'a> {
    pub(crate) commits: &'a [PullRequestCommit],
    pub(crate) is_loading: bool,
    pub(crate) error: Option<&'a str>,
    pub(crate) list_state: ListState,
}

pub(crate) fn render_commits_panel(
    input: CommitsPanelRenderInput<'_>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let CommitsPanelRenderInput {
        commits,
        is_loading,
        error,
        list_state,
    } = input;
    sync_virtual_list_item_count(&list_state, commits.len());

    div()
        .id("commits-panel")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_2()
        .child(render_panel_header(
            "Commits",
            Some(format!("{} commits", commits.len())),
        ))
        .when(is_loading, |element| {
            element.child(render_empty_panel_card("Loading commits..."))
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(render_error_panel_card(error))
        })
        .when(
            !is_loading && error.is_none() && commits.is_empty(),
            |element| {
                element.child(render_empty_panel_card(
                    "No commits found for this pull request",
                ))
            },
        )
        .when(!commits.is_empty(), |element| {
            element.child(
                render_panel_card()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(
                        list(
                            list_state,
                            cx.processor(|view, index, _window, _cx| {
                                view.detail_state
                                    .commits()
                                    .get(index)
                                    .map(render_commit_row)
                                    .unwrap_or_else(|| div().into_any_element())
                            }),
                        )
                        .flex_1()
                        .min_h_0()
                        .w_full(),
                    ),
            )
        })
}

fn render_commit_row(commit: &PullRequestCommit) -> gpui::AnyElement {
    let subject = commit
        .message
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    let short_sha: String = commit.sha.chars().take(7).collect();
    let time = commit.authored_at.map(natural_time_label);
    let initial = commit
        .author
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();

    div()
        .id(format!("commit-row-{}", commit.sha))
        .flex()
        .items_center()
        .gap_3()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(color::border())
        .child(
            div()
                .flex_none()
                .size_6()
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(color::row_hover())
                .text_xs()
                .font_medium()
                .text_color(color::text_secondary())
                .child(initial),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .truncate()
                        .font_medium()
                        .text_color(color::text_primary())
                        .child(subject),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child(commit.author.clone())
                        .when_some(time, |element, time| element.child("·").child(time)),
                ),
        )
        .child(
            div()
                .flex_none()
                .font_family("Lilex")
                .text_xs()
                .text_color(color::text_secondary())
                .child(short_sha),
        )
        .into_any_element()
}
