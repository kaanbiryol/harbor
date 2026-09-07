use crate::gpui_component::{
    Disableable, Sizable, StyledExt,
    button::Button,
    menu::{DropdownMenu, PopupMenuItem},
    switch::Switch,
};
use gpui::{AnyElement, App, Entity, IntoElement, div, prelude::*};
use harbor_domain::preferences::{
    AppPreferences, PreferredEditor, PreferredTerminal, RefreshInterval,
};

use crate::{
    visual::color,
    workspace::{
        AppView, SettingsSection,
        settings::{preferred_editor_app, preferred_terminal_app},
    },
};

pub(super) fn render_preferences(
    view: Entity<AppView>,
    section: SettingsSection,
    cx: &App,
) -> AnyElement {
    let state = view.read(cx);
    let preferences = &state.preferences.value;
    let disabled = state.preferences.loading;
    let content = div().flex().flex_col().gap_4().min_w_0();
    match section {
        SettingsSection::General => content
            .child(row(
                "Refresh while active",
                "Refresh pull requests, reviews, and checks while Harbor is focused.",
                choice(
                    "settings-focused-refresh",
                    preferences.focused_refresh,
                    RefreshInterval::ALL
                        .map(|interval| (interval, interval.label().to_string()))
                        .to_vec(),
                    disabled,
                    view.clone(),
                    |preferences, value| preferences.focused_refresh = value,
                ),
            ))
            .child(row(
                "Refresh in the background",
                "Controls refresh and notification timing while Harbor is in the background.",
                choice(
                    "settings-background-refresh",
                    preferences.background_refresh,
                    RefreshInterval::ALL
                        .map(|interval| (interval, interval.label().to_string()))
                        .to_vec(),
                    disabled,
                    view.clone(),
                    |preferences, value| preferences.background_refresh = value,
                ),
            ))
            .child(row(
                "Code font size",
                "Text size in diffs and workflow logs.",
                choice(
                    "settings-code-font-size",
                    preferences.code_font_size,
                    [10.0, 11.0, 12.0, 12.5, 13.0, 14.0, 16.0, 18.0, 20.0, 24.0]
                        .map(|size| (size, format!("{size} px")))
                        .to_vec(),
                    disabled,
                    view.clone(),
                    |preferences, value| preferences.code_font_size = value,
                ),
            ))
            .child(row(
                "Wrap long diff lines",
                "Fit long lines within the diff panel.",
                toggle(
                    "settings-wrap-diff-lines",
                    "Wrap long diff lines",
                    preferences.wrap_diff_lines,
                    disabled,
                    view,
                    |preferences, value| preferences.wrap_diff_lines = value,
                ),
            ))
            .into_any_element(),
        SettingsSection::Notifications => {
            let notifications = &preferences.notifications;
            let events_disabled = disabled || !notifications.enabled;
            content
                .child(row(
                    "Desktop notifications",
                    "Notify you about changes while Harbor is in the background.",
                    toggle(
                        "settings-notifications",
                        "Desktop notifications",
                        notifications.enabled,
                        disabled,
                        view.clone(),
                        |preferences, value| preferences.notifications.enabled = value,
                    ),
                ))
                .child(row(
                    "Failed checks",
                    "A check starts failing.",
                    toggle(
                        "settings-notify-failed",
                        "Failed checks",
                        notifications.checks_failed,
                        events_disabled,
                        view.clone(),
                        |preferences, value| preferences.notifications.checks_failed = value,
                    ),
                ))
                .child(row(
                    "Passed checks",
                    "Pending checks finish successfully.",
                    toggle(
                        "settings-notify-passed",
                        "Passed checks",
                        notifications.checks_passed,
                        events_disabled,
                        view.clone(),
                        |preferences, value| preferences.notifications.checks_passed = value,
                    ),
                ))
                .child(row(
                    "Approvals",
                    "A pull request becomes approved.",
                    toggle(
                        "settings-notify-approved",
                        "Approvals",
                        notifications.approvals,
                        events_disabled,
                        view.clone(),
                        |preferences, value| preferences.notifications.approvals = value,
                    ),
                ))
                .child(row(
                    "Changes requested",
                    "A review requests changes.",
                    toggle(
                        "settings-notify-changes",
                        "Changes requested",
                        notifications.changes_requested,
                        events_disabled,
                        view.clone(),
                        |preferences, value| preferences.notifications.changes_requested = value,
                    ),
                ))
                .child(row(
                    "Other review activity",
                    "Other review decisions and activity on existing pull requests.",
                    toggle(
                        "settings-notify-review",
                        "Other review activity",
                        notifications.review_activity,
                        events_disabled,
                        view.clone(),
                        |preferences, value| preferences.notifications.review_activity = value,
                    ),
                ))
                .child(row(
                    "Pull request updates",
                    "New PRs, commits, merges, closures, and merge readiness changes.",
                    toggle(
                        "settings-notify-updates",
                        "Pull request updates",
                        notifications.pull_request_updates,
                        events_disabled,
                        view,
                        |preferences, value| preferences.notifications.pull_request_updates = value,
                    ),
                ))
                .into_any_element()
        }
        SettingsSection::LocalDevelopment => {
            let editor = preferred_editor_app(preferences.editor);
            let terminal = preferred_terminal_app(preferences.terminal);
            let reset_view = view.clone();
            let choose_view = view.clone();
            content
                .child(row(
                    "Default editor",
                    "Open the selected PR file in this editor.",
                    choice(
                        "settings-editor",
                        preferences.editor,
                        [
                            PreferredEditor::VsCode,
                            PreferredEditor::Cursor,
                            PreferredEditor::Zed,
                            PreferredEditor::Xcode,
                        ]
                        .map(|editor| (editor, preferred_editor_app(editor).label().to_string()))
                        .to_vec(),
                        disabled,
                        view.clone(),
                        |preferences, value| preferences.editor = value,
                    ),
                ))
                .when(
                    !state.is_loading_external_app_availability()
                        && !state.external_app_is_available(editor),
                    |element| element.child(unavailable(editor.label())),
                )
                .child(row(
                    "Default terminal",
                    "Open a terminal in the PR worktree.",
                    choice(
                        "settings-terminal",
                        preferences.terminal,
                        [
                            PreferredTerminal::Terminal,
                            PreferredTerminal::Ghostty,
                            PreferredTerminal::Warp,
                        ]
                        .map(|terminal| {
                            (
                                terminal,
                                preferred_terminal_app(terminal).label().to_string(),
                            )
                        })
                        .to_vec(),
                        disabled,
                        view,
                        |preferences, value| preferences.terminal = value,
                    ),
                ))
                .when(
                    !state.is_loading_external_app_availability()
                        && !state.external_app_is_available(terminal),
                    |element| element.child(unavailable(terminal.label())),
                )
                .child(row(
                    "Worktree folder",
                    "Used for future checkouts. Existing worktrees stay where they are.",
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Button::new("settings-worktree-choose")
                                .small()
                                .label("Choose…")
                                .disabled(disabled)
                                .on_click(move |_, _, cx| {
                                    choose_view
                                        .update(cx, |view, cx| view.choose_worktree_directory(cx));
                                }),
                        )
                        .child(
                            Button::new("settings-worktree-reset")
                                .small()
                                .label("Reset")
                                .disabled(disabled || preferences.worktree_directory.is_none())
                                .on_click(move |_, _, cx| {
                                    reset_view.update(cx, |view, cx| {
                                        view.update_preferences(
                                            |preferences| preferences.worktree_directory = None,
                                            cx,
                                        )
                                    });
                                }),
                        ),
                ))
                .child(
                    div().text_sm().text_color(color::text_muted()).child(
                        preferences
                            .worktree_directory
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| {
                                "Default: .harbor-worktrees next to each local repository"
                                    .to_string()
                            }),
                    ),
                )
                .into_any_element()
        }
        SettingsSection::GitHub => content.into_any_element(),
    }
}

fn unavailable(app: &'static str) -> impl IntoElement {
    div().text_xs().text_color(color::warning()).child(format!(
        "{app} is not installed. Install it or choose another application."
    ))
}

fn row(
    title: &'static str,
    description: &'static str,
    control: impl IntoElement,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .min_w_0()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_medium()
                        .text_color(color::text_primary())
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child(description),
                ),
        )
        .child(div().flex_none().child(control))
}

fn toggle(
    id: &'static str,
    label: &'static str,
    checked: bool,
    disabled: bool,
    view: Entity<AppView>,
    update: impl Fn(&mut AppPreferences, bool) + 'static,
) -> impl IntoElement {
    Switch::new(id)
        .small()
        .accessibility_label(label)
        .checked(checked)
        .disabled(disabled)
        .on_click(move |checked, _, cx| {
            view.update(cx, |view, cx| {
                view.update_preferences(|preferences| update(preferences, *checked), cx)
            });
        })
}

fn choice<T: Copy + PartialEq + 'static>(
    id: &'static str,
    current: T,
    choices: Vec<(T, String)>,
    disabled: bool,
    view: Entity<AppView>,
    update: impl Fn(&mut AppPreferences, T) + Copy + 'static,
) -> impl IntoElement {
    let label = choices
        .iter()
        .find(|(value, _)| *value == current)
        .map(|(_, label)| label.clone())
        .unwrap_or_else(|| "Custom".to_string());
    Button::new(id)
        .small()
        .label(label)
        .dropdown_caret(true)
        .disabled(disabled)
        .dropdown_menu(move |mut menu, _, _| {
            for (value, label) in &choices {
                let value = *value;
                let view = view.clone();
                menu = menu.item(
                    PopupMenuItem::new(label.clone())
                        .checked(value == current)
                        .on_click(move |_, _, cx| {
                            view.update(cx, |view, cx| {
                                view.update_preferences(
                                    |preferences| update(preferences, value),
                                    cx,
                                )
                            });
                        }),
                );
            }
            menu
        })
}
