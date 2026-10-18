use crate::gpui_component::{
    Disableable, Root, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
    input::Textarea,
};
use gpui::{Context, Window, prelude::*, px};

use crate::workspace::AppView;

impl AppView {
    pub(crate) fn open_review_action_comment_dialog_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.root::<Root>().flatten().is_none() {
            return;
        }

        let view = cx.entity().clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let state = view.read(cx);
            let Some(target) = state.review_action_comment_target else {
                return dialog;
            };
            let input = state.review_action_comment_input.clone();
            let body_empty = input.read(cx).value().trim().is_empty();
            let action_running = state.action_runtime.pull_request_action_kind()
                == Some(crate::actions::PullRequestActionKind::Review);
            let submit_disabled = state.action_runtime.pull_request_action_running()
                || (target.requires_body() && body_empty);

            dialog
                .title(target.title())
                .w(px(440.))
                .on_close({
                    let view = view.clone();
                    move |_, _, cx| {
                        view.update(cx, |view, cx| view.close_review_action_comment_dialog(cx));
                    }
                })
                .child(
                    Textarea::new(&input)
                        .w_full()
                        .appearance(false)
                        .bordered(true),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("cancel-review-action-comment")
                                .label("Cancel")
                                .small()
                                .outline()
                                .on_click({
                                    let view = view.clone();
                                    move |_, window, cx| {
                                        window.close_dialog(cx);
                                        view.update(cx, |view, cx| {
                                            view.close_review_action_comment_dialog(cx)
                                        });
                                    }
                                }),
                        )
                        .child(
                            Button::new("submit-review-action-comment")
                                .label(target.submit_label())
                                .small()
                                .primary()
                                .loading(action_running)
                                .disabled(submit_disabled)
                                .on_click({
                                    let view = view.clone();
                                    move |_, window, cx| {
                                        view.update(cx, |view, cx| {
                                            view.submit_review_action_comment_dialog(window, cx)
                                        });
                                    }
                                }),
                        ),
                )
        });
    }
}
