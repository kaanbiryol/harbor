use crate::gpui_component::StyledExt;
use gpui::{Context, IntoElement, ListState, div, list, prelude::*, px};
use harbor_domain::{Workflow, WorkflowState};

use crate::visual::color;
use crate::workspace::AppView;

use super::super::{render_error_panel_card, render_loading_skeleton_rows};

pub(super) fn render_workflow_sidebar(
    workflows: &[Workflow],
    is_loading: bool,
    error: Option<&str>,
    list_state: ListState,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    div()
        .flex_none()
        .w(px(280.0))
        .pr_2()
        .border_r_1()
        .border_color(color::border_subtle())
        .flex()
        .flex_col()
        .min_h_0()
        .overflow_hidden()
        .child(div().px_3().py_2().font_medium().child("Workflows"))
        .when(is_loading && workflows.is_empty(), |element| {
            element.child(render_loading_skeleton_rows(
                "workflow-sidebar-loading-skeleton",
                10,
                48.0,
            ))
        })
        .when_some(error.map(str::to_string), |element, error| {
            element.child(div().mx_2().my_2().child(render_error_panel_card(error)))
        })
        .when(
            !is_loading && error.is_none() && workflows.is_empty(),
            |element| {
                element.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child("No workflows found"),
                )
            },
        )
        .when(!is_loading || !workflows.is_empty(), |element| {
            element.child(
                list(
                    list_state,
                    cx.processor(|view, index: usize, _window, cx| {
                        let selected_workflow_id =
                            view.repository_actions_state.selected_workflow_id();

                        if index == 0 {
                            return render_workflow_filter_row(
                                None,
                                "All workflows".to_string(),
                                "Repository run history".to_string(),
                                selected_workflow_id.is_none(),
                                cx,
                            )
                            .into_any_element();
                        }

                        let workflow_index = index.saturating_sub(1);
                        let Some(workflow) = view
                            .repository_actions_state
                            .workflows()
                            .get(workflow_index)
                        else {
                            return div().into_any_element();
                        };

                        render_workflow_filter_row(
                            Some(workflow.id),
                            workflow.name.clone(),
                            workflow_sidebar_subtitle(workflow),
                            selected_workflow_id == Some(workflow.id),
                            cx,
                        )
                        .into_any_element()
                    }),
                )
                .flex_1()
                .min_h_0()
                .w_full()
                .min_w_0(),
            )
        })
}

fn render_workflow_filter_row(
    workflow_id: Option<u64>,
    title: String,
    subtitle: String,
    active: bool,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let text_color = if active {
        color::text_primary()
    } else {
        color::text_secondary()
    };

    let row = div()
        .id(("workflow-filter", workflow_id.unwrap_or(0)))
        .w_full()
        .rounded_md()
        .min_w_0()
        .overflow_hidden()
        .px_2()
        .py_2()
        .cursor_pointer()
        .when(active, |element| element.bg(color::row_selected()))
        .hover(move |style| {
            style.bg(if active {
                color::row_selected_active()
            } else {
                color::row_hover()
            })
        })
        .on_click(cx.listener(move |view, _, _, cx| {
            view.select_repository_actions_workflow(workflow_id, cx);
        }))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .text_color(text_color)
                        .child(title),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .text_color(color::text_muted())
                        .child(subtitle),
                ),
        );

    div().w_full().min_w_0().px_1().py_0p5().child(row)
}

fn workflow_sidebar_subtitle(workflow: &Workflow) -> String {
    // Only call out the state when it is unusual; nearly every workflow is active.
    match workflow.state {
        WorkflowState::Active => workflow.path.clone(),
        ref state => format!("{} · {}", workflow_state_label(state), workflow.path),
    }
}

fn workflow_state_label(state: &WorkflowState) -> String {
    match state {
        WorkflowState::Active => "Active".to_string(),
        WorkflowState::DisabledManually => "Disabled".to_string(),
        WorkflowState::DisabledInactivity => "Inactive".to_string(),
        WorkflowState::DisabledFork => "Fork disabled".to_string(),
        WorkflowState::Deleted => "Deleted".to_string(),
        WorkflowState::Unknown(state) => state.clone(),
    }
}
