use crate::gpui_component::{Root, Theme, ThemeMode, input::TextareaState};
use gpui::{
    Context, Entity, IntoElement, Modifiers, Render, TestAppContext, VisualTestContext, Window,
};
use harbor_domain::{
    DiffFile, FileStatus, FileViewedState, ReviewCommentRange, ReviewSide, ReviewThreadState,
};

use super::*;
use crate::{
    diff::ParsedDiff,
    panels::review_markdown::{ReviewSuggestionContext, ReviewSuggestionOriginalLine},
    test_fixtures::review_thread as test_review_thread,
    workspace::ReviewThreadUiError,
};

#[test]
fn builds_diff_preview_for_inline_review_comments() {
    let mut thread = review_thread();
    let comment = thread
        .comments
        .first_mut()
        .expect("test thread should have a comment");
    comment.author = "alex".to_string();
    comment.body = "Please tighten this branch.\n\n```suggestion\nreplacement\n```".to_string();
    let position = comment
        .position
        .as_mut()
        .expect("test comment should have a position");
    position.line = Some(11);
    position.original_line = None;
    let (files, diffs) = review_diff_fixture();

    assert_eq!(
        review_thread_diff_preview(&thread, &files, &diffs),
        Some(ReviewDiffPreview {
            lines: vec![ReviewDiffPreviewLine {
                line: Some(11),
                marker: "+",
                text: "Please tighten this branch.".to_string(),
                tone: Tone::Success,
            }],
            suggestion_context: Some(ReviewSuggestionContext {
                original_lines: vec![ReviewSuggestionOriginalLine {
                    line_number: 11,
                    text: "Please tighten this branch.".to_string(),
                }],
                replacement_start_line: 11,
            }),
        })
    );
}

#[test]
fn builds_diff_preview_for_selected_review_ranges() {
    let mut thread = review_thread();
    thread
        .comments
        .first_mut()
        .expect("test thread should have a comment")
        .body = "```suggestion\nreplacement\n```".to_string();
    thread.range = Some(ReviewCommentRange {
        path: "src/lib.rs".to_string(),
        line: 12,
        side: ReviewSide::Right,
        start_line: Some(11),
        start_side: Some(ReviewSide::Right),
    });
    let (files, diffs) = review_diff_fixture();

    assert_eq!(
        review_thread_diff_preview(&thread, &files, &diffs),
        Some(ReviewDiffPreview {
            lines: vec![
                ReviewDiffPreviewLine {
                    line: Some(11),
                    marker: "+",
                    text: "Please tighten this branch.".to_string(),
                    tone: Tone::Success,
                },
                ReviewDiffPreviewLine {
                    line: Some(12),
                    marker: "+",
                    text: "Also cover this selected line.".to_string(),
                    tone: Tone::Success,
                },
            ],
            suggestion_context: Some(ReviewSuggestionContext {
                original_lines: vec![
                    ReviewSuggestionOriginalLine {
                        line_number: 11,
                        text: "Please tighten this branch.".to_string(),
                    },
                    ReviewSuggestionOriginalLine {
                        line_number: 12,
                        text: "Also cover this selected line.".to_string(),
                    },
                ],
                replacement_start_line: 11,
            }),
        })
    );
}

#[test]
fn preserves_review_panel_comment_markdown_body() {
    assert_eq!(
        comment_body_text("**bold**\n\n- list item\n\n```suggestion\nlet value = 1;\n```"),
        "**bold**\n\n- list item\n\n```text\nlet value = 1;\n```"
    );
    assert_eq!(comment_body_text(" \n\t "), "empty comment");
}

#[gpui::test]
async fn review_panel_reply_button_opens_and_cancel_clears_reply_mode(cx: &mut TestAppContext) {
    let (view_entity, cx) = init_visual_review_panel_test(cx);

    render_review_panel_row_harness(cx);
    let reply_bounds = cx
        .debug_bounds("review-panel-reply-thread-thread-1")
        .expect("review panel reply button should render");
    cx.simulate_click(reply_bounds.center(), Modifiers::none());

    assert_eq!(
        view_entity.read_with(cx, |view, _| view
            .review_state
            .review_composer_state
            .active_thread_reply()
            .map(str::to_string)),
        Some("thread-1".to_string())
    );

    render_review_panel_row_harness(cx);
    let cancel_bounds = cx
        .debug_bounds("review-panel-cancel-thread-reply-thread-1")
        .expect("review panel reply cancel button should render");
    cx.simulate_click(cancel_bounds.center(), Modifiers::none());

    assert!(view_entity.read_with(cx, |view, _| {
        view.review_state
            .review_composer_state
            .active_thread_reply()
            .is_none()
    }));
}

#[gpui::test]
async fn review_panel_toggle_reports_missing_selected_pull_request(cx: &mut TestAppContext) {
    let (view_entity, cx) = init_visual_review_panel_test(cx);

    render_review_panel_row_harness(cx);
    let toggle_bounds = cx
        .debug_bounds("review-panel-toggle-thread-thread-1")
        .expect("review panel toggle button should render");
    cx.simulate_click(toggle_bounds.center(), Modifiers::none());

    assert_eq!(
        view_entity.read_with(cx, |view, _| {
            view.review_state
                .review_thread_action_error()
                .map(|error| (error.thread_id.clone(), error.message.clone()))
        }),
        Some((
            "thread-1".to_string(),
            "Select a pull request before updating a thread".to_string()
        ))
    );
}

fn init_visual_review_panel_test(
    cx: &mut TestAppContext,
) -> (Entity<AppView>, &mut VisualTestContext) {
    cx.update(|cx| {
        crate::gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let mut view_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
        let harness = cx.new(|_| ReviewPanelRowHarness {
            view_entity: view.clone(),
            thread: review_thread(),
        });
        view_entity = Some(view);
        Root::new(harness, window, cx)
    });

    (view_entity.expect("test AppView should be created"), cx)
}

fn render_review_panel_row_harness(cx: &mut VisualTestContext) {
    cx.refresh().expect("test window should refresh");
    cx.run_until_parked();
}

struct ReviewPanelRowHarness {
    view_entity: Entity<AppView>,
    thread: ReviewThread,
}

impl Render for ReviewPanelRowHarness {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let render_state = self
            .view_entity
            .read_with(cx, |view, app| ReviewPanelRowTestState {
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
                reply_error: view.review_state.review_thread_reply_error().cloned(),
                action_thread_id: view
                    .review_state
                    .review_thread_action_thread_id()
                    .map(str::to_string),
                action_error: view.review_state.review_thread_action_error().cloned(),
            });

        render_review_thread_row(ReviewThreadRowRenderState {
            index: 0,
            thread: &self.thread,
            active_review_thread_reply: render_state.active_reply_thread_id.as_deref(),
            review_thread_reply_input: render_state.reply_input.clone(),
            reply_body_empty: render_state.reply_body_empty,
            is_submitting_reply: render_state.is_submitting_reply,
            reply_error: render_state.reply_error.as_ref(),
            action_thread_id: render_state.action_thread_id.as_deref(),
            action_error: render_state.action_error.as_ref(),
            diff_preview: None,
            mono_font_family: cx.theme().mono_font_family.clone(),
            view_entity: self.view_entity.clone(),
        })
    }
}

struct ReviewPanelRowTestState {
    active_reply_thread_id: Option<String>,
    reply_input: Entity<TextareaState>,
    reply_body_empty: bool,
    is_submitting_reply: bool,
    reply_error: Option<ReviewThreadUiError>,
    action_thread_id: Option<String>,
    action_error: Option<ReviewThreadUiError>,
}

fn review_thread() -> ReviewThread {
    test_review_thread(ReviewThreadState::Unresolved)
}

fn review_diff_fixture() -> (Vec<DiffFile>, Vec<Option<ParsedDiff>>) {
    let file = DiffFile {
        path: "src/lib.rs".to_string(),
        previous_path: None,
        status: FileStatus::Modified,
        additions: 1,
        deletions: 0,
        changes: 1,
        patch: None,
        viewed_state: FileViewedState::Unviewed,
    };
    let diff = crate::diff::parse_unified_diff(
        "@@ -10,2 +10,4 @@\n context\n+Please tighten this branch.\n+Also cover this selected line.\n unchanged\n",
    );

    (vec![file], vec![Some(diff)])
}
