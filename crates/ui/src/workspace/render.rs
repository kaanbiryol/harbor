use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{
    App, Context, FocusHandle, Focusable, IntoElement, Render, Window, div, prelude::*, px,
};
use gpui_component::{
    IconName, Sizable,
    button::{Button, ButtonVariants},
};
use harbor_domain::PullRequest;
use harbor_github::GitHubRateLimitStatus;

use crate::actions::*;
use crate::panels::{
    DiffPanelRenderInput, render_actions_panel, render_checks_panel, render_diff_panel,
    render_logs_panel, render_review_panel,
};
use crate::visual::{color, font};
use crate::workspace::AppView;

const SHOW_STATUS_BAR_RATE_LIMITS: bool = true;
const SHOW_STATUS_BAR_CACHED_MESSAGES: bool = true;

#[path = "render/changed_files.rs"]
mod changed_files;
#[path = "render/details.rs"]
mod details;
#[path = "render/header.rs"]
pub(crate) mod header;
#[path = "render/inbox.rs"]
mod inbox;
#[path = "render/pending_review.rs"]
mod pending_review;

impl Focusable for AppView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

pub(super) fn render_switcher_section_label(label: &'static str) -> impl IntoElement {
    div()
        .px_2()
        .py_1()
        .text_xs()
        .text_color(color::text_muted())
        .child(label)
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.sync_runtime.did_focus() {
            if self.repository_state.repository_switcher_open {
                self.repository_state
                    .repository_search_input
                    .update(cx, |input, cx| input.focus(window, cx));
            } else {
                window.focus(&self.focus_handle, cx);
            }
            self.sync_runtime.mark_focused_once();
        }

        if self.active_tab == PanelTab::Diff {
            self.sync_diff_list_items(cx);
        }

        let selected_pr = self.selected_pull_request().cloned();

        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle(cx))
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::open_selected))
            .on_action(cx.listener(Self::cycle_panel_tab))
            .on_action(cx.listener(Self::toggle_pull_request_inbox))
            .on_action(cx.listener(Self::toggle_repository_switcher))
            .on_action(cx.listener(Self::close_panel))
            .on_action(cx.listener(Self::refresh_selected))
            .on_action(cx.listener(Self::checkout_pr))
            .on_action(cx.listener(Self::open_in_browser))
            .on_action(cx.listener(Self::approve_pr))
            .on_action(cx.listener(Self::request_changes))
            .on_action(cx.listener(Self::merge_pr))
            .on_action(cx.listener(Self::open_logs))
            .on_action(cx.listener(Self::trigger_build))
            .on_action(cx.listener(Self::rerun_failed))
            .on_action(cx.listener(Self::filter_current_list))
            .on_action(cx.listener(Self::select_next_file))
            .on_action(cx.listener(Self::select_previous_file))
            .on_action(cx.listener(Self::select_next_hunk))
            .on_action(cx.listener(Self::select_previous_hunk))
            .on_action(cx.listener(Self::copy_active_file_path))
            .on_action(cx.listener(Self::open_active_file_on_github))
            .on_action(cx.listener(Self::choose_local_checkout))
            .on_action(cx.listener(Self::open_with_vs_code))
            .on_action(cx.listener(Self::open_with_cursor))
            .on_action(cx.listener(Self::open_with_zed))
            .on_action(cx.listener(Self::open_with_finder))
            .on_action(cx.listener(Self::open_with_terminal))
            .on_action(cx.listener(Self::open_with_ghostty))
            .on_action(cx.listener(Self::open_with_warp))
            .on_action(cx.listener(Self::open_with_xcode))
            .size_full()
            .flex()
            .flex_col()
            .bg(color::app_background())
            .text_color(color::text_primary())
            .font_family(font::UI)
            .child(self.render_title_bar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .gap_2()
                    .p_2()
                    .when(self.pull_request_inbox.is_visible(), |element| {
                        element.child(self.render_inbox(cx))
                    })
                    .child(self.render_details(selected_pr.as_ref(), cx))
                    .child(self.render_panel(selected_pr.as_ref(), cx)),
            )
            .child(self.render_status_bar(cx))
    }
}

impl AppView {
    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let inbox_toggle_icon = if self.pull_request_inbox.is_visible() {
            IconName::PanelLeft
        } else {
            IconName::PanelLeftOpen
        };
        let inbox_toggle_tooltip = if self.pull_request_inbox.is_visible() {
            "Hide pull request inbox"
        } else {
            "Show pull request inbox"
        };
        let status_label = status_bar_status_label(&self.status).to_string();
        let (rate_limit_label, rate_limit_color) = if SHOW_STATUS_BAR_RATE_LIMITS {
            let rate_limits = self.github_api.latest_rate_limits();
            let rate_limit = self.github_api.latest_rate_limit();
            (
                github_rate_limits_label(&rate_limits)
                    .or_else(|| rate_limit.as_ref().and_then(github_rate_limit_label)),
                rate_limit
                    .as_ref()
                    .map(github_rate_limit_color)
                    .unwrap_or_else(color::text_muted),
            )
        } else {
            (None, color::text_muted())
        };

        div()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .text_xs()
            .text_color(color::text_muted())
            .border_1()
            .border_color(color::border())
            .child(
                Button::new("toggle-pull-request-inbox")
                    .ghost()
                    .small()
                    .compact()
                    .icon(inbox_toggle_icon)
                    .tooltip(inbox_toggle_tooltip)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.toggle_pull_request_inbox(&TogglePullRequestInbox, window, cx);
                    })),
            )
            .child(div().min_w_0().flex_1().truncate().child(status_label))
            .when_some(rate_limit_label, |element, label| {
                element.child(
                    div()
                        .flex_none()
                        .max_w(px(260.))
                        .truncate()
                        .text_color(rate_limit_color)
                        .child(label),
                )
            })
    }

    fn render_panel(&self, pr: Option<&PullRequest>, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .min_h_0()
            .min_w_0()
            .border_1()
            .border_color(color::border())
            .bg(color::panel_background())
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .p_2()
                    .border_1()
                    .border_color(color::border())
                    .children(
                        PanelTab::ALL
                            .iter()
                            .copied()
                            .enumerate()
                            .map(|(index, tab)| {
                                let active = tab == self.active_tab;
                                let view = view.clone();

                                div()
                                    .id(("panel-tab", index))
                                    .px_3()
                                    .py_1()
                                    .rounded_xs()
                                    .text_sm()
                                    .text_color(if active {
                                        color::text_primary()
                                    } else {
                                        color::text_secondary()
                                    })
                                    .cursor_pointer()
                                    .when(active, |element| {
                                        element
                                            .border_1()
                                            .border_color(color::border_strong())
                                            .bg(color::row_selected())
                                    })
                                    .hover(move |element| {
                                        if active {
                                            element
                                        } else {
                                            element.bg(color::row_hover())
                                        }
                                    })
                                    .on_click(move |_, _, cx| {
                                        view.update(cx, |view, cx| {
                                            view.select_panel_tab(tab, cx);
                                        });
                                    })
                                    .child(tab.label())
                            }),
                    ),
            )
            .child(
                div()
                    .id("panel-content-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .min_w_0()
                    .p_3()
                    .text_sm()
                    .child(match self.active_tab {
                        PanelTab::Diff => {
                            let visible_file_indices = self.visible_file_indices(cx);
                            render_diff_panel(
                                DiffPanelRenderInput {
                                    files: &self.detail_state.files,
                                    diffs: &self.detail_state.diffs,
                                    visible_file_indices: &visible_file_indices,
                                    reviewed_file_paths: &self.reviewed_file_paths,
                                    review_threads: &self.review_state.review_threads,
                                    review_composer: self
                                        .review_state
                                        .review_composer_state
                                        .inline_composer(),
                                    active_file_index: self.active_file_index(),
                                    is_loading: self.detail_state.files_loading(),
                                    error: self.detail_state.files_error(),
                                    list_state: self.diff_list_state.clone(),
                                    list_items: &self.diff_list_items,
                                },
                                cx,
                            )
                            .into_any_element()
                        }
                        PanelTab::Review => render_review_panel(
                            &self.review_state.pull_request_reviews,
                            &self.review_state.review_threads,
                            self.review_state.reviews_loading(),
                            self.review_state.reviews_error(),
                            self.review_list_scroll.clone(),
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Checks => render_checks_panel(
                            pr.map(|pr| pr.checks_summary).unwrap_or_default(),
                            &self.detail_state.check_runs,
                            self.detail_state.checks_loading(),
                            self.detail_state.checks_error(),
                        )
                        .into_any_element(),
                        PanelTab::Actions => render_actions_panel(
                            pr,
                            &self.detail_state.workflow_runs,
                            self.detail_state.workflows_loading(),
                            self.detail_state.workflows_error(),
                            self.action_runtime.workflow_action_error(),
                            self.action_runtime.workflow_action_running(),
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Logs => render_logs_panel(
                            self.selected_workflow_run_for_logs(),
                            &self.detail_state.workflow_jobs,
                            self.detail_state.log_state.chunk(),
                            self.detail_state.log_state.is_loading(),
                            self.detail_state.log_state.error(),
                            self.detail_state.log_state.list_scroll.clone(),
                            cx,
                        )
                        .into_any_element(),
                    }),
            )
    }
}

fn status_bar_status_label(status: &str) -> &str {
    if !SHOW_STATUS_BAR_CACHED_MESSAGES && status_bar_message_mentions_cached_data(status) {
        ""
    } else {
        status
    }
}

fn status_bar_message_mentions_cached_data(status: &str) -> bool {
    let status = status.to_ascii_lowercase();
    (status.starts_with("showing ") && status.contains("cached"))
        || status.contains("; showing cached data")
}

fn github_rate_limit_label(rate_limit: &GitHubRateLimitStatus) -> Option<String> {
    let resource = rate_limit.resource.as_deref().unwrap_or("api");
    let budget = match (rate_limit.remaining, rate_limit.limit) {
        (Some(remaining), Some(limit)) => format!("{remaining}/{limit}"),
        (Some(remaining), None) => format!("{remaining} left"),
        (None, Some(limit)) => format!("limit {limit}"),
        (None, None) => return None,
    };

    if github_rate_limit_is_low(rate_limit) {
        if let Some(retry_after_seconds) = rate_limit.retry_after_seconds {
            return Some(format!(
                "github {resource}: {budget} retry {}",
                duration_label(retry_after_seconds)
            ));
        }

        if let Some(reset_label) = rate_limit.reset_epoch_seconds.and_then(reset_epoch_label) {
            return Some(format!("github {resource}: {budget} resets {reset_label}"));
        }
    }

    Some(format!("github {resource}: {budget}"))
}

fn github_rate_limits_label(rate_limits: &[GitHubRateLimitStatus]) -> Option<String> {
    if rate_limits.len() <= 1 {
        return rate_limits.first().and_then(github_rate_limit_label);
    }

    let labels = rate_limits
        .iter()
        .filter_map(|rate_limit| {
            let resource = rate_limit.resource.as_deref().unwrap_or("api");
            match (rate_limit.remaining, rate_limit.limit) {
                (Some(remaining), Some(limit)) => Some(format!("{resource} {remaining}/{limit}")),
                (Some(remaining), None) => Some(format!("{resource} {remaining} left")),
                _ => None,
            }
        })
        .collect::<Vec<_>>();

    (!labels.is_empty()).then(|| format!("github {}", labels.join(" ")))
}

fn github_rate_limit_color(rate_limit: &GitHubRateLimitStatus) -> gpui::Rgba {
    if rate_limit.remaining == Some(0) {
        color::danger()
    } else if github_rate_limit_is_low(rate_limit) {
        color::warning()
    } else {
        color::text_muted()
    }
}

fn github_rate_limit_is_low(rate_limit: &GitHubRateLimitStatus) -> bool {
    match (rate_limit.remaining, rate_limit.limit) {
        (Some(remaining), Some(limit)) if limit > 0 => remaining.saturating_mul(5) <= limit,
        (Some(remaining), _) => remaining <= 100,
        _ => false,
    }
}

fn reset_epoch_label(epoch_seconds: u64) -> Option<String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();

    if epoch_seconds <= now {
        return Some("now".to_string());
    }

    Some(duration_label(epoch_seconds - now))
}

fn duration_label(seconds: u64) -> String {
    if seconds < 60 {
        format!("in {seconds}s")
    } else if seconds < 3600 {
        format!("in {}m", seconds.div_ceil(60))
    } else {
        format!("in {}h", seconds.div_ceil(3600))
    }
}
