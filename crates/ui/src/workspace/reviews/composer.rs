use crate::gpui_component::input::{InputEvent, TextareaState};
use gpui::{Context, Entity, Window};
use harbor_domain::{ReviewCommentRange, ReviewSide};

use crate::{actions::PanelTab, workspace::AppView};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewLineTarget {
    pub(crate) hunk_index: usize,
    pub(crate) line_index: usize,
    pub(crate) range: ReviewCommentRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewComposer {
    pub(crate) anchor: ReviewLineTarget,
    pub(crate) range: ReviewCommentRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewLineSelection {
    pub(crate) anchor: ReviewLineTarget,
    pub(crate) current: ReviewLineTarget,
}

fn review_composer_from_selection(
    anchor: &ReviewLineTarget,
    current: &ReviewLineTarget,
) -> std::result::Result<ReviewComposer, String> {
    let range = review_range_from_targets(anchor, current)?;
    let anchor = if anchor.line_index >= current.line_index {
        anchor.clone()
    } else {
        current.clone()
    };

    Ok(ReviewComposer { anchor, range })
}

pub(crate) fn review_range_from_targets(
    anchor: &ReviewLineTarget,
    current: &ReviewLineTarget,
) -> std::result::Result<ReviewCommentRange, String> {
    if anchor.hunk_index != current.hunk_index {
        return Err("Review comments can only span one contiguous diff section".to_string());
    }

    if anchor.range.path != current.range.path {
        return Err("Review comments can only span one file".to_string());
    }

    if anchor.range.side != current.range.side {
        return Err("Review comments can only span one diff side".to_string());
    }

    let (start, end) = if anchor.line_index <= current.line_index {
        (anchor, current)
    } else {
        (current, anchor)
    };
    let mut range = end.range.clone();

    if start.line_index != end.line_index {
        range.start_line = Some(start.range.line);
        range.start_side = Some(start.range.side);
    } else {
        range.start_line = None;
        range.start_side = None;
    }

    Ok(range)
}

fn review_comment_range_label(range: &ReviewCommentRange) -> String {
    let side = match range.side {
        ReviewSide::Left => "left",
        ReviewSide::Right => "right",
    };

    if let Some(start_line) = range.start_line {
        format!("{side} lines {start_line}-{}", range.line)
    } else {
        format!("{side} line {}", range.line)
    }
}

pub(crate) struct ReviewComposerState {
    mode: ReviewComposerMode,
    pub(crate) comment_input: Entity<TextareaState>,
    pub(crate) thread_reply_input: Entity<TextareaState>,
    pub(crate) comment_edit_input: Entity<TextareaState>,
    pub(crate) pending_review_body_input: Entity<TextareaState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ReviewComposerMode {
    Idle,
    Selecting {
        line_selection: ReviewLineSelection,
    },
    Inline {
        composer: ReviewComposer,
        line_selection: ReviewLineSelection,
    },
    ThreadReply {
        thread_id: String,
    },
    CommentEdit {
        comment_id: String,
    },
}

impl ReviewComposerState {
    pub(crate) fn new(
        comment_input: Entity<TextareaState>,
        thread_reply_input: Entity<TextareaState>,
        comment_edit_input: Entity<TextareaState>,
        pending_review_body_input: Entity<TextareaState>,
    ) -> Self {
        Self {
            mode: ReviewComposerMode::Idle,
            comment_input,
            thread_reply_input,
            comment_edit_input,
            pending_review_body_input,
        }
    }

    pub(crate) fn start_line_selection(&mut self, target: ReviewLineTarget) {
        self.mode = ReviewComposerMode::Selecting {
            line_selection: ReviewLineSelection {
                anchor: target.clone(),
                current: target,
            },
        };
    }

    pub(crate) fn extend_line_selection(&mut self, target: ReviewLineTarget) {
        if let ReviewComposerMode::Selecting { line_selection } = &mut self.mode {
            line_selection.current = target;
        }
    }

    pub(crate) fn take_line_selection(&mut self) -> Option<ReviewLineSelection> {
        let ReviewComposerMode::Selecting { line_selection } =
            std::mem::replace(&mut self.mode, ReviewComposerMode::Idle)
        else {
            return None;
        };

        Some(line_selection)
    }

    pub(crate) fn open_inline(
        &mut self,
        composer: ReviewComposer,
        line_selection: ReviewLineSelection,
    ) {
        self.mode = ReviewComposerMode::Inline {
            composer,
            line_selection,
        };
    }

    pub(crate) fn open_thread_reply(&mut self, thread_id: String) {
        self.mode = ReviewComposerMode::ThreadReply { thread_id };
    }

    pub(crate) fn open_comment_edit(&mut self, comment_id: String) {
        self.mode = ReviewComposerMode::CommentEdit { comment_id };
    }

    pub(crate) fn clear(&mut self) {
        self.mode = ReviewComposerMode::Idle;
    }

    pub(crate) fn inline_composer(&self) -> Option<&ReviewComposer> {
        match &self.mode {
            ReviewComposerMode::Inline { composer, .. } => Some(composer),
            ReviewComposerMode::Idle
            | ReviewComposerMode::Selecting { .. }
            | ReviewComposerMode::ThreadReply { .. }
            | ReviewComposerMode::CommentEdit { .. } => None,
        }
    }

    pub(crate) fn line_selection(&self) -> Option<&ReviewLineSelection> {
        match &self.mode {
            ReviewComposerMode::Selecting { line_selection }
            | ReviewComposerMode::Inline { line_selection, .. } => Some(line_selection),
            ReviewComposerMode::Idle
            | ReviewComposerMode::ThreadReply { .. }
            | ReviewComposerMode::CommentEdit { .. } => None,
        }
    }

    pub(crate) fn active_thread_reply(&self) -> Option<&str> {
        match &self.mode {
            ReviewComposerMode::ThreadReply { thread_id } => Some(thread_id.as_str()),
            ReviewComposerMode::Idle
            | ReviewComposerMode::Selecting { .. }
            | ReviewComposerMode::Inline { .. }
            | ReviewComposerMode::CommentEdit { .. } => None,
        }
    }

    pub(crate) fn active_comment_edit(&self) -> Option<&str> {
        match &self.mode {
            ReviewComposerMode::CommentEdit { comment_id } => Some(comment_id.as_str()),
            ReviewComposerMode::Idle
            | ReviewComposerMode::Selecting { .. }
            | ReviewComposerMode::Inline { .. }
            | ReviewComposerMode::ThreadReply { .. } => None,
        }
    }

    pub(crate) fn take_active_comment_edit_if(
        &mut self,
        predicate: impl FnOnce(&str) -> bool,
    ) -> Option<String> {
        let ReviewComposerMode::CommentEdit { comment_id } = &self.mode else {
            return None;
        };
        if !predicate(comment_id) {
            return None;
        }
        let ReviewComposerMode::CommentEdit { comment_id } =
            std::mem::replace(&mut self.mode, ReviewComposerMode::Idle)
        else {
            return None;
        };
        Some(comment_id)
    }
}

impl AppView {
    pub(crate) fn start_review_line_selection(
        &mut self,
        target: ReviewLineTarget,
        cx: &mut Context<Self>,
    ) {
        self.review_state
            .review_composer_state
            .start_line_selection(target);
        self.review_state.clear_review_comment_error();
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
                self.review_state.clear_review_comment_error();
                self.status = format!("Opened review composer for {label}");
            }
            Err(message) => {
                self.review_state.review_composer_state.clear();
                self.review_state.set_review_comment_error(message.clone());
                self.status = message;
            }
        }

        self.sync_diff_list_items(cx);
        cx.notify();
    }

    pub(crate) fn cancel_review_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.review_state.clear_composer_and_action_state();
        self.review_state
            .review_composer_state
            .comment_input
            .update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
        self.sync_diff_list_items(cx);
        self.status = "Cancelled review comment".to_string();
        cx.notify();
    }

    pub(crate) fn on_review_input_event(
        &mut self,
        _: &Entity<TextareaState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_composer_modes_are_mutually_exclusive() {
        let target = ReviewLineTarget {
            hunk_index: 0,
            line_index: 1,
            range: ReviewCommentRange {
                path: "src/lib.rs".to_string(),
                line: 10,
                side: ReviewSide::Right,
                start_line: None,
                start_side: None,
            },
        };
        let selection = ReviewLineSelection {
            anchor: target.clone(),
            current: target.clone(),
        };
        let composer = ReviewComposer {
            anchor: target,
            range: selection.current.range.clone(),
        };

        let modes = [
            ReviewComposerMode::Idle,
            ReviewComposerMode::Selecting {
                line_selection: selection.clone(),
            },
            ReviewComposerMode::Inline {
                composer,
                line_selection: selection,
            },
            ReviewComposerMode::ThreadReply {
                thread_id: "thread-1".to_string(),
            },
            ReviewComposerMode::CommentEdit {
                comment_id: "comment-1".to_string(),
            },
        ];

        for mode in modes {
            let active_count = [
                matches!(mode, ReviewComposerMode::Selecting { .. }),
                matches!(mode, ReviewComposerMode::Inline { .. }),
                matches!(mode, ReviewComposerMode::ThreadReply { .. }),
                matches!(mode, ReviewComposerMode::CommentEdit { .. }),
            ]
            .into_iter()
            .filter(|active| *active)
            .count();
            assert!(active_count <= 1);
        }
    }
}
