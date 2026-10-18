use crate::gpui_component::{
    StyledExt,
    menu::{ContextMenuExt, PopupMenuItem},
};
use gpui::{Context, IntoElement, ListState, SharedString, div, list, prelude::*};
use harbor_domain::PullRequestCommit;

use crate::{
    date_time::natural_time_label,
    icons::Octicon,
    visual::color,
    workspace::{AppView, async_updates::AppViewAsyncUpdateExt},
};

use super::{
    people::render_avatar, render_empty_state, render_error_panel_card,
    render_loading_panel_skeleton, render_panel_card, render_panel_header,
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
        .image_cache(gpui::retain_all("commit-avatar-cache"))
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
        .when(is_loading && commits.is_empty(), |element| {
            element.child(render_loading_panel_skeleton(
                "commits-loading-skeleton",
                8,
                52.0,
            ))
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(render_error_panel_card(error))
        })
        .when(
            !is_loading && error.is_none() && commits.is_empty(),
            |element| {
                element.child(
                    render_empty_state(
                        Octicon::CodeSquare,
                        "No commits found",
                        "This pull request does not have any commits to display.",
                    )
                    .debug_selector(|| "commits-empty-state".to_string()),
                )
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
                                    .map(|commit| render_commit_row(commit, _cx))
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

fn render_commit_row(commit: &PullRequestCommit, cx: &mut Context<AppView>) -> gpui::AnyElement {
    let subject = commit
        .message
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    let short_sha: String = commit.sha.chars().take(7).collect();
    let time = commit.authored_at.map(natural_time_label);
    let sha = commit.sha.clone();
    let menu_sha = SharedString::from(commit.sha.clone());
    let view = cx.weak_entity();

    div()
        .id(format!("commit-row-{}", commit.sha))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .gap_3()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(color::border())
        .cursor_pointer()
        .hover(|style| style.bg(color::row_hover()))
        .on_click(cx.listener(move |view, _, _, cx| {
            view.select_commit(sha.clone(), cx);
        }))
        .child(render_avatar(
            &commit.author,
            commit.author_avatar_url.as_deref(),
            24.0,
        ))
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
        .context_menu(move |menu, _, _| {
            let context = "failed to run commit context menu item";
            let copy_view = view.clone();
            let open_view = view.clone();
            let copy_sha = menu_sha.clone();
            let open_sha = menu_sha.clone();
            menu.item(PopupMenuItem::new("Copy SHA").on_click(move |_, _, cx| {
                copy_view
                    .update_or_log(cx, context, |view, cx| view.copy_commit_sha(&copy_sha, cx));
            }))
            .item(
                PopupMenuItem::new("Open on GitHub").on_click(move |_, _, cx| {
                    open_view.update_or_log(cx, context, |view, cx| {
                        view.open_commit_on_github(&open_sha, cx)
                    });
                }),
            )
        })
        .into_any_element()
}
