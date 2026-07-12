use gpui::{Context, IntoElement, ListState, div, list, prelude::*, px};
use gpui_component::{Icon, Sizable, StyledExt};
use harbor_domain::{Workflow, WorkflowState};

use crate::icons::Octicon;
use crate::visual::color;
use crate::workspace::AppView;

use super::super::{render_error_panel_card, render_panel_card};

pub(super) fn render_workflow_sidebar(
    workflows: &[Workflow],
    is_loading: bool,
    error: Option<&str>,
    list_state: ListState,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    render_panel_card()
        .flex_none()
        .w(px(236.0))
        .flex()
        .flex_col()
        .min_h_0()
        .overflow_hidden()
        .child(
            div()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(color::border())
                .font_medium()
                .child("Workflows"),
        )
        .when(is_loading, |element| {
            element.child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(color::text_muted())
                    .child("Loading workflows..."),
            )
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
        .child(
            list(
                list_state,
                cx.processor(|view, index: usize, _window, cx| {
                    let selected_workflow_id = view.repository_actions_state.selected_workflow_id();

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

    div()
        .id(("workflow-filter", workflow_id.unwrap_or(0)))
        .mx_2()
        .my_1()
        .rounded_xs()
        .px_2()
        .py_2()
        .cursor_pointer()
        .bg(if active {
            color::row_selected()
        } else {
            color::content_background()
        })
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
        .flex()
        .items_start()
        .gap_2()
        .child(Icon::new(Octicon::Gear).xsmall().text_color(if active {
            color::accent()
        } else {
            color::text_muted()
        }))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_medium()
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
        )
}

fn workflow_sidebar_subtitle(workflow: &Workflow) -> String {
    format!(
        "{}  {}",
        workflow_state_label(&workflow.state),
        workflow.path
    )
}

fn workflow_state_label(state: &WorkflowState) -> String {
    match state {
        WorkflowState::Active => "active".to_string(),
        WorkflowState::DisabledManually => "disabled".to_string(),
        WorkflowState::DisabledInactivity => "inactive".to_string(),
        WorkflowState::DisabledFork => "fork disabled".to_string(),
        WorkflowState::Deleted => "deleted".to_string(),
        WorkflowState::Unknown(state) => state.clone(),
    }
}
