use gpui::{Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};

use crate::{
    actions::PanelTab,
    workspace::{
        AppView, ReviewLineTarget,
        reviews::{review_comment_range_label, review_composer_from_selection},
    },
};

impl AppView {
    pub(crate) fn start_review_line_selection(
        &mut self,
        target: ReviewLineTarget,
        cx: &mut Context<Self>,
    ) {
        self.review_state
            .review_composer_state
            .start_line_selection(target);
        self.review_state.review_comment_error = None;
        self.active_tab = PanelTab::Diff;
        self.status = "Started review line selection".to_string();
        cx.notify();
    }

    pub(crate) fn extend_review_line_selection(
        &mut self,
        target: ReviewLineTarget,
        cx: &mut Context<Self>,
    ) {
        self.review_state
            .review_composer_state
            .extend_line_selection(target);
        cx.notify();
    }

    pub(crate) fn finish_review_line_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(selection) = self
            .review_state
            .review_composer_state
            .take_line_selection()
        else {
            return;
        };

        match review_composer_from_selection(&selection.anchor, &selection.current) {
            Ok(composer) => {
                let range = composer.range.clone();
                let label = review_comment_range_label(&range);
                self.review_state
                    .review_composer_state
                    .comment_input
                    .update(cx, |input, cx| {
                        input.set_value("", window, cx);
                        input.focus(window, cx);
                    });
                self.review_state
                    .review_composer_state
                    .open_inline(composer, selection);
                self.review_state.review_comment_error = None;
                self.status = format!("Opened review composer for {label}");
            }
            Err(message) => {
                self.review_state.review_composer_state.clear();
                self.review_state.review_comment_error = Some(message.clone());
                self.status = message;
            }
        }

        cx.notify();
    }

    pub(crate) fn cancel_review_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clear_review_composer_state();
        self.review_state
            .review_composer_state
            .comment_input
            .update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
        self.status = "Cancelled review comment".to_string();
        cx.notify();
    }

    pub(crate) fn on_review_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            cx.notify();
        }
    }
}
