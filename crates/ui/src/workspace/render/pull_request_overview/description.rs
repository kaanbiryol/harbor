use std::collections::HashSet;

use super::*;
use crate::icons::Octicon;
use crate::panels::{overview_markdown_blocks, render_review_markdown_state};
use crate::workspace::{
    PullRequestDetailCacheKey, async_updates::AppViewAsyncUpdateExt, state::OverviewMarkdownState,
};

impl AppView {
    pub(super) fn prepare_pull_request_description(
        &mut self,
        pr: &PullRequest,
        detail_key: PullRequestDetailCacheKey,
        cx: &mut Context<Self>,
    ) {
        if self.overview_state.description_ready() {
            return;
        }
        let Some(generation) = self.overview_state.start_description_preparation() else {
            return;
        };

        let Some(body) = pull_request_description_body(pr) else {
            let Some(_) = self
                .overview_state
                .take_description_blocks(&detail_key, generation)
            else {
                return;
            };
            self.overview_state.set_description_blocks(
                &detail_key,
                generation,
                Vec::new(),
                HashSet::new(),
                Vec::new(),
            );
            return;
        };
        let body = body.to_string();
        let split_markdown = cx.background_spawn(async move { overview_markdown_blocks(&body) });

        cx.spawn(async move |this, cx| {
            let blocks = split_markdown.await;
            this.update_or_log(
                cx,
                "failed to prepare pull request description",
                move |view, cx| {
                    if !view
                        .overview_state
                        .is_current_description_preparation(&detail_key, generation)
                    {
                        view.overview_state
                            .cancel_description_preparation(&detail_key, generation);
                        return;
                    }
                    view.install_pull_request_description_blocks(
                        blocks, detail_key, generation, cx,
                    );
                },
            );
        })
        .detach();
    }

    fn install_pull_request_description_blocks(
        &mut self,
        sources: Vec<String>,
        detail_key: PullRequestDetailCacheKey,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(previous_blocks) = self
            .overview_state
            .take_description_blocks(&detail_key, generation)
        else {
            return;
        };
        let mut previous_blocks = previous_blocks.into_iter().map(Some).collect::<Vec<_>>();
        let mut blocks = Vec::with_capacity(sources.len());
        let mut pending_blocks = HashSet::new();

        for (index, source) in sources.into_iter().enumerate() {
            let previous = previous_blocks.get_mut(index).and_then(Option::take);
            let block = match previous {
                Some(block) if block.source == source => block,
                Some(mut block) => {
                    block.source.clone_from(&source);
                    block
                        .state
                        .update(cx, |state, cx| state.set_text(&source, cx));
                    pending_blocks.insert(index);
                    block
                }
                None => {
                    let state =
                        cx.new(|cx| gpui_component::text::TextViewState::markdown(&source, cx));
                    pending_blocks.insert(index);
                    OverviewMarkdownState { source, state }
                }
            };
            blocks.push(block);
        }

        let subscriptions = pending_blocks
            .iter()
            .filter_map(|index| {
                let index = *index;
                let state = blocks.get(index)?.state.clone();
                let detail_key = detail_key.clone();
                Some(cx.observe(&state, move |view, _, cx| {
                    let current = view
                        .overview_state
                        .is_current_description_preparation(&detail_key, generation);
                    let Some(complete) = view.overview_state.mark_description_block_ready(
                        &detail_key,
                        generation,
                        index,
                    ) else {
                        return;
                    };

                    if current {
                        view.overview_state
                            .list_state
                            .remeasure_items(index + 1..index + 2);
                        if complete {
                            let item_count = view.overview_state.description_block_count() + 1;
                            view.overview_state
                                .list_state
                                .remeasure_items(0..item_count);
                        }
                        cx.notify();
                    }
                }))
            })
            .collect();

        if self.overview_state.set_description_blocks(
            &detail_key,
            generation,
            blocks,
            pending_blocks,
            subscriptions,
        ) {
            cx.notify();
        }
    }

    pub(super) fn render_description_header(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let editing = self.pull_request_description_editing;

        div()
            .debug_selector(|| "pull-request-overview-description".to_string())
            .w_full()
            .min_w_0()
            .rounded_tl(px(4.0))
            .rounded_tr(px(4.0))
            .border_t_1()
            .border_l_1()
            .border_r_1()
            .border_color(color::border())
            .bg(color::content_background())
            .px_4()
            .pt_4()
            .pb_3()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_semibold()
                    .text_color(color::text_primary())
                    .child("Description"),
            )
            .when(!editing, |element| {
                element.child(
                    Button::new("edit-pull-request-description")
                        .icon(Octicon::Pencil)
                        .xsmall()
                        .ghost()
                        .tooltip("Edit description if your GitHub permissions allow it")
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.start_pull_request_description_edit(window, cx);
                        })),
                )
            })
            .into_any_element()
    }

    pub(super) fn render_description_block(&self, index: usize, is_last: bool) -> AnyElement {
        let Some(state) = self.overview_state.description_block_state(index) else {
            return div().into_any_element();
        };
        let selector = format!("pull-request-overview-description-block-{index}");
        let content = div()
            .min_w_0()
            .text_sm()
            .text_color(color::text_secondary())
            .child(render_review_markdown_state(&state))
            .into_any_element();

        render_description_body(selector, content, is_last)
    }

    pub(super) fn render_empty_description(&self) -> AnyElement {
        let content = div()
            .text_sm()
            .text_color(color::text_muted())
            .child("No description")
            .into_any_element();
        render_description_body(
            "pull-request-overview-description-empty".to_string(),
            content,
            true,
        )
    }

    pub(super) fn render_description_editor(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let saving = self
            .action_runtime
            .pull_request_description_action_running();
        let error = self
            .action_runtime
            .pull_request_description_action_error()
            .map(str::to_string);
        let description_input = self.pull_request_description_input.clone();
        let content = div()
            .w_full()
            .min_w_0()
            .child(Input::new(&description_input))
            .when_some(error, |element, error| {
                element.child(
                    div()
                        .pt_2()
                        .text_xs()
                        .text_color(color::danger())
                        .child(error),
                )
            })
            .child(
                div()
                    .pt_3()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cancel-pull-request-description")
                            .label("Cancel")
                            .small()
                            .outline()
                            .disabled(saving)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.cancel_pull_request_description_edit(window, cx);
                            })),
                    )
                    .child(
                        Button::new("save-pull-request-description")
                            .label("Save")
                            .small()
                            .loading(saving)
                            .disabled(saving)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.save_pull_request_description(window, cx);
                            })),
                    ),
            )
            .into_any_element();

        render_description_body(
            "pull-request-overview-description-editor".to_string(),
            content,
            true,
        )
    }
}

fn render_description_body(selector: String, content: AnyElement, is_last: bool) -> AnyElement {
    let body = div()
        .debug_selector(move || selector.clone())
        .w_full()
        .min_w_0()
        .border_l_1()
        .border_r_1()
        .border_color(color::border())
        .bg(color::content_background())
        .px_4()
        .when(is_last, |element| {
            element
                .pb_4()
                .border_b_1()
                .rounded_bl(px(4.0))
                .rounded_br(px(4.0))
        })
        .when(!is_last, |element| element.pb_2())
        .child(content);

    div()
        .w_full()
        .when(is_last, |element| element.pb_3())
        .child(body)
        .into_any_element()
}

fn pull_request_description_body(pr: &PullRequest) -> Option<&str> {
    pr.body
        .as_deref()
        .map(str::trim)
        .filter(|body| !body.is_empty())
}
