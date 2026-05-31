use gpui::{App, Div, Entity, IntoElement, KeyDownEvent, Stateful, div, prelude::*, px};
use gpui_component::StyledExt;
use harbor_domain::RepoId;

use crate::{visual::color, workspace::AppView};

const REPOSITORY_SWITCHER_ROW_HEIGHT: f32 = 34.;
const REPOSITORY_SWITCHER_MAX_VISIBLE_ROWS: usize = 10;

pub(super) fn render_switcher_empty_row(label: impl Into<String>) -> impl IntoElement {
    div()
        .px_2()
        .py_2()
        .text_sm()
        .text_color(color::text_muted())
        .child(label.into())
}

pub(super) fn render_switcher_loading_row(label: &'static str) -> impl IntoElement {
    div()
        .px_2()
        .py_2()
        .text_sm()
        .text_color(color::accent())
        .child(label)
}

pub(super) fn render_switcher_error_row(error: String) -> impl IntoElement {
    div()
        .mx_1()
        .mb_1()
        .border_1()
        .border_color(color::danger_background())
        .bg(color::danger_background())
        .px_2()
        .py_2()
        .text_xs()
        .text_color(color::danger())
        .child(error)
}

pub(super) fn render_switcher_notice_row(notice: String) -> impl IntoElement {
    div()
        .mx_1()
        .mb_1()
        .border_1()
        .border_color(color::border())
        .bg(color::elevated_background())
        .px_2()
        .py_2()
        .text_xs()
        .text_color(color::text_muted())
        .child(notice)
}

pub(super) fn render_switcher_repository_row(
    repository: &RepoId,
    current: bool,
    highlighted: bool,
) -> Stateful<Div> {
    div()
        .id(format!("switcher-repository-{}", repository.full_name()))
        .h(px(REPOSITORY_SWITCHER_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_2()
        .py_1()
        .text_sm()
        .cursor_pointer()
        .when(highlighted, |element| element.bg(color::row_selected()))
        .when(current && !highlighted, |element| {
            element.bg(color::row_selected_subtle())
        })
        .hover(|element| element.bg(color::row_hover()))
        .child(
            div()
                .min_w_0()
                .truncate()
                .font_medium()
                .text_color(color::text_primary())
                .child(repository.full_name()),
        )
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(color::text_muted())
                .child(if current { "current" } else { "repo" }),
        )
}

pub(super) fn repository_switcher_list_height(repository_count: usize) -> f32 {
    REPOSITORY_SWITCHER_ROW_HEIGHT
        * repository_count.min(REPOSITORY_SWITCHER_MAX_VISIBLE_ROWS) as f32
}

pub(super) fn render_switcher_typed_repository_row(
    repository: &RepoId,
    highlighted: bool,
) -> Stateful<Div> {
    div()
        .id(format!(
            "switcher-typed-repository-{}",
            repository.full_name()
        ))
        .h(px(REPOSITORY_SWITCHER_ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_2()
        .py_1()
        .text_sm()
        .cursor_pointer()
        .when(highlighted, |element| element.bg(color::row_selected()))
        .hover(|element| element.bg(color::row_hover()))
        .child(
            div()
                .min_w_0()
                .truncate()
                .font_medium()
                .text_color(color::text_primary())
                .child(format!("Open {}", repository.full_name())),
        )
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(color::text_muted())
                .child("typed"),
        )
}

pub(super) fn handle_repository_switcher_key(
    event: &KeyDownEvent,
    view: &Entity<AppView>,
    cx: &mut App,
) {
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
