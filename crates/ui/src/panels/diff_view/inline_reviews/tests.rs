use gpui::{
    Context, Entity, IntoElement, Modifiers, Render, TestAppContext, VisualTestContext, Window,
};
use gpui_component::{Root, Theme, ThemeMode, input::InputState};

use crate::test_fixtures::review_thread as test_review_thread;
use crate::workspace::{AppView, ReviewCommentUiError, ReviewReactionAction, ReviewThreadUiError};

use super::*;

#[test]
fn compacts_large_inline_review_threads() {
    assert_eq!(hidden_inline_review_comment_count(21), 0);
    assert_eq!(visible_inline_review_reply_start_index(21), 1);
    assert_eq!(hidden_inline_review_comment_count(125), 104);
    assert_eq!(visible_inline_review_reply_start_index(125), 105);
}

#[gpui::test]
async fn renders_comment_actions_for_every_comment(cx: &mut TestAppContext) {
    let (_, _, cx) = init_visual_review_test(cx);

    render_inline_review_harness(cx);
    assert!(
        cx.debug_bounds("inline-review-comment-actions-comment-1")
            .is_some()
    );
}

#[gpui::test]
async fn reply_button_opens_thread_reply_mode(cx: &mut TestAppContext) {
    let (view_entity, _, cx) = init_visual_review_test(cx);

    render_inline_review_harness(cx);
    let reply_bounds = cx
        .debug_bounds("inline-review-reply-thread-1")
        .expect("reply button should render");
    cx.simulate_click(reply_bounds.center(), Modifiers::none());

    assert_eq!(
        view_entity.read_with(cx, |view, _| view
            .review_state
            .review_composer_state
            .active_thread_reply()
            .map(str::to_string)),
        Some("thread-1".to_string())
    );
}

#[gpui::test]
async fn comment_edit_cancel_exits_edit_mode(cx: &mut TestAppContext) {
    let (view_entity, harness_entity, cx) = init_visual_review_test(cx);
    harness_entity.update(cx, |harness, cx| {
        harness.thread.comments[0].viewer_can_update = true;
        cx.notify();
    });

    cx.update(|window, app| {
        view_entity.update(app, |view, cx| {
            view.open_review_comment_edit(
                "comment-1".to_string(),
                "Please check this line.".to_string(),
                window,
                cx,
            );
        });
    });
    render_inline_review_harness(cx);
    let cancel_bounds = cx
        .debug_bounds("inline-review-comment-edit-cancel-comment-1")
        .expect("edit cancel button should render");
    cx.simulate_click(cancel_bounds.center(), Modifiers::none());

    assert!(view_entity.read_with(cx, |view, _| {
        view.review_state
            .review_composer_state
            .active_comment_edit()
            .is_none()
    }));
}

fn init_visual_review_test(
    cx: &mut TestAppContext,
) -> (
    Entity<AppView>,
    Entity<InlineReviewThreadHarness>,
    &mut VisualTestContext,
) {
    cx.update(|cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let mut view_entity = None;
    let mut harness_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
        let harness = cx.new(|_| InlineReviewThreadHarness {
            view_entity: view.clone(),
            thread: review_thread(),
        });
        view_entity = Some(view.clone());
        harness_entity = Some(harness.clone());
        Root::new(harness, window, cx)
    });

    (
        view_entity.expect("test AppView should be created"),
        harness_entity.expect("test inline review harness should be created"),
        cx,
    )
}

fn render_inline_review_harness(cx: &mut VisualTestContext) {
    cx.refresh().expect("test window should refresh");
    cx.run_until_parked();
}

struct InlineReviewThreadHarness {
    view_entity: Entity<AppView>,
    thread: ReviewThread,
}

impl Render for InlineReviewThreadHarness {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let render_state = self
            .view_entity
            .read_with(cx, |view, app| ReviewThreadTestState {
                active_reply_thread_id: view
                    .review_state
                    .review_composer_state
                    .active_thread_reply()
                    .map(str::to_string),
                reply_input: view
                    .review_state
                    .review_composer_state
                    .thread_reply_input
                    .clone(),
                reply_body_empty: view
                    .review_state
                    .review_composer_state
                    .thread_reply_input
                    .read(app)
                    .value()
                    .trim()
                    .is_empty(),
                is_submitting_reply: view.review_state.is_submitting_review_thread_reply(),
                review_thread_reply_error: view.review_state.review_thread_reply_error().cloned(),
                action_thread_id: view
                    .review_state
                    .review_thread_action_thread_id()
                    .map(str::to_string),
                action_error: view.review_state.review_thread_action_error().cloned(),
                active_comment_edit_id: view
                    .review_state
                    .review_composer_state
                    .active_comment_edit()
                    .map(str::to_string),
                comment_edit_input: view
                    .review_state
                    .review_composer_state
                    .comment_edit_input
                    .clone(),
                edit_body_empty: view
                    .review_state
                    .review_composer_state
                    .comment_edit_input
                    .read(app)
                    .value()
                    .trim()
                    .is_empty(),
                is_submitting_edit: view.review_state.is_submitting_review_comment_edit(),
                review_comment_edit_error: view.review_state.review_comment_edit_error().cloned(),
                action_comment_id: view
                    .review_state
                    .review_comment_action_comment_id()
                    .map(str::to_string),
                comment_action_error: view.review_state.review_comment_action_error().cloned(),
                reaction_action: view.review_state.review_reaction_action().cloned(),
                reaction_error: view.review_state.review_reaction_error().cloned(),
            });
        let active_reply_thread_id = render_state.active_reply_thread_id.as_deref();
        let action_thread_id = render_state.action_thread_id.as_deref();
        let active_comment_edit_id = render_state.active_comment_edit_id.as_deref();
        let action_comment_id = render_state.action_comment_id.as_deref();
        let comments = ReviewCommentListRenderState {
            active_review_comment_edit: active_comment_edit_id,
            review_comment_edit_input: render_state.comment_edit_input.clone(),
            edit_body_empty: render_state.edit_body_empty,
            is_submitting_edit: render_state.is_submitting_edit,
            edit_error: render_state.review_comment_edit_error.as_ref(),
            action_comment_id,
            comment_action_error: render_state.comment_action_error.as_ref(),
            reaction_action: render_state.reaction_action.as_ref(),
            reaction_error: render_state.reaction_error.as_ref(),
            view_entity: self.view_entity.clone(),
        };

        render_review_thread_inline(ReviewThreadRenderState {
            thread: &self.thread,
            line_number_width: 44.0,
            active_review_thread_reply: active_reply_thread_id,
            review_thread_reply_input: render_state.reply_input.clone(),
            reply_body_empty: render_state.reply_body_empty,
            is_submitting_reply: render_state.is_submitting_reply,
            reply_error: render_state.review_thread_reply_error.as_ref(),
            action_thread_id,
            action_error: render_state.action_error.as_ref(),
            comments: comments.clone(),
            view_entity: self.view_entity.clone(),
        })
        .into_element()
    }
}

struct ReviewThreadTestState {
    active_reply_thread_id: Option<String>,
    reply_input: Entity<InputState>,
    reply_body_empty: bool,
    is_submitting_reply: bool,
    review_thread_reply_error: Option<ReviewThreadUiError>,
    action_thread_id: Option<String>,
    action_error: Option<ReviewThreadUiError>,
    active_comment_edit_id: Option<String>,
    comment_edit_input: Entity<InputState>,
    edit_body_empty: bool,
    is_submitting_edit: bool,
    review_comment_edit_error: Option<ReviewCommentUiError>,
    action_comment_id: Option<String>,
    comment_action_error: Option<ReviewCommentUiError>,
    reaction_action: Option<ReviewReactionAction>,
    reaction_error: Option<ReviewCommentUiError>,
}

fn review_thread() -> ReviewThread {
    let mut thread = test_review_thread(ReviewThreadState::Unresolved);
    thread.comments[0].viewer_can_update = false;
    thread
}
