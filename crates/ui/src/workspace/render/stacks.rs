use crate::gpui_component::{
    Disableable, Icon, Sizable, StyledExt, ThemeStyled,
    button::{Button, ButtonVariants},
    list::ListItem,
    popover::Popover,
};
use gpui::{AnyElement, Context, IntoElement, div, prelude::*, px, uniform_list};
use harbor_domain::{
    MergeState, PullRequest, PullRequestState, pull_request_signals::merge_blocker,
};

use crate::{icons::Octicon, visual::color, workspace::AppView};

impl AppView {
    pub(super) fn render_stack_badge(
        &self,
        pull_request: &PullRequest,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(membership) = pull_request.stack.as_ref() else {
            return div().into_any_element();
        };
        let view = cx.entity().downgrade();
        Popover::new("pull-request-stack-popover")
            .appearance(false)
            .anchor(gpui::Anchor::TopLeft)
            .open(self.detail_state.stack_expanded)
            .on_open_change({
                let view = view.clone();
                move |open, _, cx| {
                    crate::workspace::async_updates::AppViewAsyncUpdateExt::update_or_log(
                        &view,
                        cx,
                        "failed to open pull request stack",
                        |view, cx| {
                            view.detail_state.stack_expanded = *open;
                            if *open && !view.detail_state.stack_load.is_loading() {
                                view.load_selected_pull_request_stack(cx);
                            }
                            cx.notify();
                        },
                    );
                }
            })
            .trigger(
                Button::new("pull-request-stack-badge")
                    .icon(Octicon::Stack)
                    .label(format!("{}/{}", membership.position, membership.size))
                    .xsmall()
                    .outline()
                    .rounded_full()
                    .tooltip(format!("Stack #{} · view pull requests", membership.number)),
            )
            .content(move |_, window, cx| {
                let viewport_size = window.viewport_size();
                use crate::workspace::async_updates::AppViewAsyncUpdateExt;
                view.update_or_log(cx, "failed to render pull request stack", |view, cx| {
                    view.render_stack_content(viewport_size, cx)
                })
                .unwrap_or_else(|| div().into_any_element())
            })
            .into_any_element()
    }

    fn render_stack_content(
        &self,
        viewport_size: gpui::Size<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let width = (viewport_size.width - px(24.)).clamp(px(0.), px(600.));
        let max_height = (viewport_size.height - px(220.)).clamp(px(100.), px(420.));
        let selected_number = self
            .selected_pull_request()
            .map(|pull_request| pull_request.number);
        let stack_number = self
            .selected_pull_request()
            .and_then(|pull_request| pull_request.stack.as_ref())
            .map(|membership| membership.number);
        let mut content = div()
            .debug_selector(|| "pull-request-stack-content".to_string())
            .w(width)
            .min_w_0()
            .flex()
            .flex_col()
            .popover_style(cx)
            .text_sm()
            .text_color(color::text_primary())
            .bg(color::content_background())
            .border_1()
            .border_color(color::border_strong())
            .rounded_lg()
            .overflow_hidden()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(color::border())
                    .child(
                        div()
                            .font_medium()
                            .child(format!("Stack #{}", stack_number.unwrap_or_default())),
                    )
                    .child(
                        Button::new("close-pull-request-stack")
                            .icon(Octicon::X)
                            .xsmall()
                            .ghost()
                            .tooltip("Close stack")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.detail_state.stack_expanded = false;
                                cx.notify();
                            })),
                    ),
            );
        if let Some(error) = self.detail_state.stack_load.error() {
            return content
                .child(
                    div()
                        .p_3()
                        .text_sm()
                        .text_color(color::danger())
                        .child(error.to_string()),
                )
                .child(
                    Button::new("retry-stack")
                        .label("Retry")
                        .small()
                        .ghost()
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.load_selected_pull_request_stack(cx);
                            cx.notify();
                        })),
                )
                .into_any_element();
        }
        let Some(stack) = self.detail_state.stack.as_ref() else {
            return content
                .child(
                    div()
                        .p_3()
                        .text_sm()
                        .text_color(color::text_muted())
                        .child("Loading stack…"),
                )
                .into_any_element();
        };
        let blocker = stack.merge_blocker();
        if stack
            .pull_requests
            .iter()
            .any(|pull_request| pull_request.merge_state == Some(MergeState::Dirty))
        {
            content = content.child(div().px_3().py_2().text_sm().text_color(color::warning()).child("This stack has conflicts. Resolve them on the command line before continuing."));
        } else if let Some(blocker) = blocker.as_ref() {
            content = content.child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(color::text_muted())
                    .child(blocker.clone()),
            );
        }
        let rows_stack = stack.clone();
        let row_count = stack.pull_requests.len();
        let view = cx.entity().downgrade();
        content = content.child(
            uniform_list("stack-pull-requests", row_count, move |range, _, _| {
                range
                    .filter_map(|index| {
                        row_count
                            .checked_sub(index + 1)
                            .and_then(|index| rows_stack.pull_requests.get(index))
                    })
                    .map(|pull_request| {
                        let selected = Some(pull_request.number) == selected_number;
                        let (status, tone) = match pull_request.state {
                            PullRequestState::Merged => ("Merged".to_string(), color::accent()),
                            PullRequestState::Closed => ("Closed".to_string(), color::danger()),
                            PullRequestState::Open
                                if pull_request.merge_capabilities.queue_state
                                    == harbor_domain::MergeQueueState::Queued =>
                            {
                                ("Queued".to_string(), color::warning())
                            }
                            PullRequestState::Open => match merge_blocker(pull_request) {
                                Some(_) => (
                                    if pull_request.is_draft {
                                        "Draft"
                                    } else {
                                        "Not ready"
                                    }
                                    .to_string(),
                                    color::warning(),
                                ),
                                None => ("Ready".to_string(), color::success()),
                            },
                        };
                        let member_number = pull_request.number;
                        let stack = rows_stack.clone();
                        let view = view.clone();
                        ListItem::new(("stack-pr", pull_request.number))
                            .debug_selector(|| format!("stack-pr-row-{member_number}"))
                            .selected(selected)
                            .w_full()
                            .min_w_0()
                            .overflow_hidden()
                            .h(px(64.))
                            .px_3()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .w_full()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .w(px(16.))
                                            .flex_none()
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .gap_1()
                                            .child(
                                                Icon::new(Octicon::GitPullRequest)
                                                    .small()
                                                    .text_color(tone),
                                            )
                                            .child(
                                                div()
                                                    .w(px(2.))
                                                    .h(px(24.))
                                                    .bg(color::border_strong()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .debug_selector(|| {
                                                        format!("stack-pr-title-{member_number}")
                                                    })
                                                    .w_full()
                                                    .text_sm()
                                                    .font_medium()
                                                    .truncate()
                                                    .child(pull_request.title.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(color::text_muted())
                                                    .truncate()
                                                    .child(format!(
                                                        "#{} · {}",
                                                        pull_request.number, pull_request.head_ref
                                                    )),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .debug_selector(|| {
                                                format!("stack-pr-status-{member_number}")
                                            })
                                            .flex_none()
                                            .w(px(70.))
                                            .text_align(gpui::TextAlign::Right)
                                            .text_xs()
                                            .text_color(tone)
                                            .child(status),
                                    ),
                            )
                            .on_click(move |_, _, cx| {
                                use crate::workspace::async_updates::AppViewAsyncUpdateExt;
                                view.update_or_log(
                                    cx,
                                    "failed to select stack pull request",
                                    |view, cx| {
                                        if let Some(member) = stack
                                            .pull_requests
                                            .iter()
                                            .find(|member| member.number == member_number)
                                        {
                                            view.select_stack_pull_request(member.clone(), cx);
                                        }
                                    },
                                );
                            })
                            .into_any_element()
                    })
                    .collect::<Vec<_>>()
            })
            .h(px(row_count as f32 * 64.).min(max_height))
            .w_full(),
        );
        let running = self.action_runtime.pull_request_action_running();
        let label = if stack.uses_merge_queue() {
            "Enqueue stack"
        } else {
            "Squash and merge stack"
        };
        content
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(color::accent())
                    .child(format!("○  {}", stack.base_ref)),
            )
            .child(
                div()
                    .p_3()
                    .border_t_1()
                    .border_color(color::border())
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("merge-pull-request-stack")
                            .label(format!("{label} ({})", stack.open_count()))
                            .small()
                            .success()
                            .when(
                                running
                                    || self.detail_state.stack_load.is_loading()
                                    || blocker.is_some(),
                                |button| button.outline(),
                            )
                            .disabled(
                                running
                                    || self.detail_state.stack_load.is_loading()
                                    || blocker.is_some(),
                            )
                            .loading(running)
                            .tooltip(
                                blocker.unwrap_or_else(|| format!("{label} from the bottom up")),
                            )
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.run_stack_merge(cx);
                            })),
                    )
                    .when(self.detail_state.stack_load.is_loading(), |element| {
                        element.child(
                            div()
                                .text_xs()
                                .text_color(color::text_muted())
                                .child("Refreshing…"),
                        )
                    }),
            )
            .child(
                div()
                    .px_3()
                    .pb_2()
                    .text_xs()
                    .text_color(color::text_muted())
                    .child("Use ⇧ J / ⇧ K to navigate down / up the stack"),
            )
            .when_some(
                self.action_runtime.pull_request_action_error(),
                |element, error| {
                    element.child(
                        div()
                            .px_3()
                            .pb_2()
                            .text_xs()
                            .text_color(color::danger())
                            .child(error.to_string()),
                    )
                },
            )
            .into_any_element()
    }
}
