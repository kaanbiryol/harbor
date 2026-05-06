use gpui::{App, Div, Entity, IntoElement, KeyDownEvent, Stateful, div, prelude::*, px, rgb};
use gpui_component::{IconName, StyledExt};
use harbor_domain::RepoId;
use harbor_git::ExternalApp;

use crate::actions::*;
use crate::workspace::AppView;

const REPOSITORY_SWITCHER_ROW_HEIGHT: f32 = 34.;
const REPOSITORY_SWITCHER_MAX_VISIBLE_ROWS: usize = 10;

pub(super) fn render_switcher_empty_row(label: impl Into<String>) -> impl IntoElement {
    div()
        .px_2()
        .py_2()
        .text_sm()
        .text_color(rgb(0x7d8794))
        .child(label.into())
}

pub(super) fn render_switcher_loading_row(label: &'static str) -> impl IntoElement {
    div()
        .px_2()
        .py_2()
        .text_sm()
        .text_color(rgb(0x93c5fd))
        .child(label)
}

pub(super) fn render_switcher_error_row(error: String) -> impl IntoElement {
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
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(rgb(0x7d8794))
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
        .child(
            div()
                .flex_none()
                .text_xs()
                .text_color(rgb(0x7d8794))
                .child("typed"),
        )
}

pub(super) fn render_switcher_pull_request_row(
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

pub(super) fn handle_pull_request_switcher_key(
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

pub(super) fn open_with_action(app: ExternalApp) -> Box<dyn gpui::Action> {
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

pub(super) fn open_with_icon(app: ExternalApp) -> IconName {
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
