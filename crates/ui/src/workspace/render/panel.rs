use gpui::{Context, IntoElement, div, prelude::*};
use gpui_component::{Icon, Sizable, StyledExt};
use harbor_domain::PullRequest;

use crate::{
    actions::PanelTab,
    panels::{
        ActionsPanelRenderInput, CheckPanelRenderInput, DiffPanelRenderInput,
        ReviewPanelRenderInput, render_actions_panel, render_checks_panel, render_diff_panel,
        render_logs_panel, render_review_panel,
    },
    visual::color,
    workspace::AppView,
};

impl AppView {
    pub(super) fn render_panel(
        &self,
        pr: Option<&PullRequest>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity().clone();
        let dense_content = matches!(self.active_tab, PanelTab::Diff | PanelTab::Logs);

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
                    .items_center()
                    .gap_1()
                    .px_2()
                    .pt_2()
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
                                    .pb_2()
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
                    .when(dense_content, |element| element.p_2())
                    .when(!dense_content, |element| element.p_3())
                    .text_sm()
                    .child(match self.active_tab {
                        PanelTab::Diff => {
                            let visible_file_indices = self.visible_file_indices(cx);
                            render_diff_panel(
                                DiffPanelRenderInput {
                                    files: self.detail_state.files(),
                                    diffs: self.detail_state.diffs(),
                                    visible_file_indices: &visible_file_indices,
                                    reviewed_file_paths: &self.reviewed_file_paths,
                                    expanded_diff_file_paths: &self.expanded_diff_file_paths,
                                    collapsed_diff_file_paths: &self.collapsed_diff_file_paths,
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
                            ReviewPanelRenderInput {
                                reviews: &self.review_state.pull_request_reviews,
                                comments: &self.review_state.pull_request_comments,
                                threads: &self.review_state.review_threads,
                                is_loading: self.review_state.reviews_loading(),
                                error: self.review_state.reviews_error(),
                                review_list_state: self.review_list_state.clone(),
                            },
                            cx,
                        )
                        .into_any_element(),
                        PanelTab::Checks => render_checks_panel(
                            CheckPanelRenderInput {
                                summary: pr.map(|pr| pr.checks_summary).unwrap_or_default(),
                                check_runs: self.detail_state.check_runs(),
                                collapsed_groups: self.collapsed_check_groups(),
                                active_filter: self.checks_filter(),
                                is_loading: self.detail_state.checks_loading(),
                                error: self.detail_state.checks_error(),
                                list_state: self.checks_list_state.clone(),
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
                                repository_workflows_loading: self
                                    .repository_actions_state
                                    .workflows_loading(),
                                repository_runs_loading: self
                                    .repository_actions_state
                                    .runs_loading(),
                                repository_workflows_error: self
                                    .repository_actions_state
                                    .workflows_error(),
                                repository_runs_error: self.repository_actions_state.runs_error(),
                                selected_pr_workflow_runs: self.detail_state.workflow_runs(),
                                selected_pr_workflows_loading: self
                                    .detail_state
                                    .workflows_loading(),
                                selected_pr_workflows_error: self.detail_state.workflows_error(),
                                action_error: self.action_runtime.workflow_action_error(),
                                is_running_action: self.action_runtime.workflow_action_running(),
                                workflow_list_state: self.actions_workflow_list_state.clone(),
                                run_list_state: self.actions_list_state.clone(),
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
