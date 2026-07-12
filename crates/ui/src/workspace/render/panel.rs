use gpui::{Context, IntoElement, div, prelude::*};
use gpui_component::{Icon, Sizable, StyledExt};
use harbor_domain::PullRequest;

use crate::{
    actions::PanelTab,
    panels::{
        ActionsPanelRenderInput, CheckPanelRenderInput, CommitsPanelRenderInput,
        DiffPanelRenderInput, ReviewPanelRenderInput, render_actions_panel, render_checks_panel,
        render_commits_panel, render_diff_panel, render_logs_panel, render_review_panel,
        render_status_pill,
    },
    visual::{Tone, color},
    workspace::AppView,
};

impl AppView {
    pub(super) fn render_panel_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let unresolved_threads = self
            .selected_pull_request()
            .map(|pull_request| pull_request.unresolved_threads)
            .unwrap_or_default();
        let commit_count = self
            .detail_state
            .commits_loaded()
            .then(|| self.detail_state.commits().len());

        div()
            .debug_selector(|| "pull-request-panel-tabs".to_string())
            .flex_none()
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .pt_1()
            .border_b_1()
            .border_color(color::border())
            .children(
                PanelTab::ALL
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, tab)| {
                        let active = tab == self.active_tab;
                        let tab_color = if active {
                            color::accent()
                        } else {
                            color::text_secondary()
                        };
                        let view = view.clone();

                        div()
                            .id(("panel-tab", index))
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_3()
                            .pb_1()
                            .pt_1()
                            .text_sm()
                            .text_color(tab_color)
                            .cursor_pointer()
                            .when(active, |element| {
                                element
                                    .border_b_1()
                                    .border_color(color::accent())
                                    .font_medium()
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
                            .child(Icon::new(tab.icon()).xsmall().text_color(tab_color))
                            .child(tab.label())
                            .when(
                                tab == PanelTab::Review && unresolved_threads > 0,
                                |element| {
                                    element.child(
                                        div()
                                            .debug_selector(|| {
                                                "review-tab-unresolved-count".to_string()
                                            })
                                            .child(render_status_pill(
                                                unresolved_threads.to_string(),
                                                Tone::Warning,
                                            )),
                                    )
                                },
                            )
                            .when_some(
                                (tab == PanelTab::Commits).then_some(commit_count).flatten(),
                                |element, commit_count| {
                                    element.child(
                                        div()
                                            .debug_selector(|| "commits-tab-count".to_string())
                                            .child(render_status_pill(
                                                commit_count.to_string(),
                                                Tone::Neutral,
                                            )),
                                    )
                                },
                            )
                    }),
            )
    }

    pub(super) fn render_panel(
        &mut self,
        pr: Option<&PullRequest>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let dense_content = matches!(self.active_tab, PanelTab::Diff | PanelTab::Logs);

        div()
            .debug_selector(|| "pull-request-panel".to_string())
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
                    .id("panel-content-scroll")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .min_w_0()
                    .when(dense_content, |element| element.p_2())
                    .when(!dense_content, |element| element.p_3())
                    .text_sm()
                    .child(match self.active_tab {
                        PanelTab::Overview => self
                            .render_pull_request_overview_panel(pr, cx)
                            .into_any_element(),
                        PanelTab::Diff => {
                            let visible_file_indices = self.visible_file_indices(cx);
                            render_diff_panel(
                                DiffPanelRenderInput {
                                    pull_request: pr,
                                    files: self.detail_state.files(),
                                    diffs: self.detail_state.diffs(),
                                    visible_file_indices: &visible_file_indices,
                                    reviewed_file_paths: &self
                                        .changed_files_state
                                        .reviewed_file_paths,
                                    expanded_diff_file_paths: &self
                                        .changed_files_state
                                        .expanded_diff_file_paths,
                                    collapsed_diff_file_paths: &self
                                        .changed_files_state
                                        .collapsed_diff_file_paths,
                                    review_threads: self.review_state.review_threads(),
                                    review_composer: self
                                        .review_state
                                        .review_composer_state
                                        .inline_composer(),
                                    active_file_index: self.active_file_index(),
                                    commit_sha: self.active_commit_sha.as_deref(),
                                    is_loading: self.detail_state.files_loading(),
                                    error: self.detail_state.files_error(),
                                    list_state: self.diff_list_state.clone(),
                                    list_items: self.diff_list_items.clone(),
                                },
                                cx,
                            )
                            .into_any_element()
                        }
                        PanelTab::Review => render_review_panel(
                            ReviewPanelRenderInput {
                                reviews: self.review_state.pull_request_reviews(),
                                comments: self.review_state.pull_request_comments(),
                                threads: self.review_state.review_threads(),
                                is_loading: self.review_state.reviews_loading(),
                                error: self.review_state.reviews_error(),
                                review_list_state: self.panel_list_state.review.clone(),
                            },
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Checks => render_checks_panel(
                            CheckPanelRenderInput {
                                summary: pr.map(|pr| pr.checks_summary).unwrap_or_default(),
                                check_runs: self.detail_state.check_runs(),
                                expanded_groups: self.expanded_check_groups(),
                                active_filter: self.checks_filter(),
                                is_loading: self.detail_state.checks_loading(),
                                error: self.detail_state.checks_error(),
                                list_state: self.panel_list_state.checks.clone(),
                            },
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Commits => render_commits_panel(
                            CommitsPanelRenderInput {
                                commits: self.detail_state.commits(),
                                is_loading: self.detail_state.commits_loading(),
                                error: self.detail_state.commits_error(),
                                list_state: self.panel_list_state.commits.clone(),
                            },
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Actions => render_actions_panel(
                            ActionsPanelRenderInput {
                                repository: self.repository_state.configured_repo(),
                                pr,
                                repository_workflows: self.repository_actions_state.workflows(),
                                selected_repository_workflow_id: self
                                    .repository_actions_state
                                    .selected_workflow_id(),
                                repository_workflow_runs: self
                                    .repository_actions_state
                                    .workflow_runs(),
                                repository_workflow_run_total_count: self
                                    .repository_actions_state
                                    .workflow_run_total_count(),
                                repository_workflow_runs_has_next_page: self
                                    .repository_actions_state
                                    .next_workflow_run_page()
                                    .is_some(),
                                repository_workflows_loading: self
                                    .repository_actions_state
                                    .workflows_loading(),
                                repository_runs_loading: self
                                    .repository_actions_state
                                    .runs_loading(),
                                repository_runs_loading_more: self
                                    .repository_actions_state
                                    .is_loading_more_runs(),
                                repository_workflows_error: self
                                    .repository_actions_state
                                    .workflows_error(),
                                repository_runs_error: self.repository_actions_state.runs_error(),
                                repository_runs_load_more_error: self
                                    .repository_actions_state
                                    .load_more_runs_error(),
                                selected_pr_workflow_runs: self.detail_state.workflow_runs(),
                                selected_pr_workflows_loading: self
                                    .detail_state
                                    .workflows_loading(),
                                selected_pr_workflows_error: self.detail_state.workflows_error(),
                                action_error: self.action_runtime.workflow_action_error(),
                                is_running_action: self.action_runtime.workflow_action_running(),
                                workflow_list_state: self.panel_list_state.action_workflows.clone(),
                                run_list_state: self.panel_list_state.action_runs.clone(),
                            },
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Logs => render_logs_panel(
                            self.selected_workflow_run_for_logs(),
                            self.detail_state.workflow_jobs(),
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
