use gpui::{
    Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext, Window, div, px,
};
use gpui_component::{Root, Theme, ThemeMode};

use crate::diff::{DiffHunk, DiffLineKind};
use crate::workspace::AppView;

use super::*;

#[gpui::test]
async fn wraps_long_diff_line_in_narrow_panel(cx: &mut TestAppContext) {
    let cx = init_visual_diff_line_test(cx);

    cx.refresh().expect("test window should refresh");
    cx.run_until_parked();

    let bounds = cx
        .debug_bounds("diff-line-wrap-harness")
        .expect("diff line should render");
    assert!(
        bounds.size.height > px(DIFF_ROW_HEIGHT),
        "wrapped diff line height should exceed one row, got {:?}",
        bounds.size.height
    );
}

#[gpui::test]
async fn keeps_line_numbers_single_line_in_wrapped_rows(cx: &mut TestAppContext) {
    let cx = init_visual_diff_line_test(cx);

    cx.refresh().expect("test window should refresh");
    cx.run_until_parked();

    let bounds = cx
        .debug_bounds("diff-line-number-wrap-harness")
        .expect("diff line should render");
    assert_eq!(
        bounds.size.height,
        px(DIFF_ROW_HEIGHT),
        "line number wrapping should not expand a one-line diff row"
    );
}

fn init_visual_diff_line_test(cx: &mut TestAppContext) -> &mut VisualTestContext {
    cx.update(|cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
    });

    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| AppView::new_without_startup_tasks(window, cx));
        let harness = cx.new(|_| DiffLineWrapHarness { view_entity: view });
        Root::new(harness, window, cx)
    });

    cx
}

struct DiffLineWrapHarness {
    view_entity: Entity<AppView>,
}

impl Render for DiffLineWrapHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mono_font_family = gpui::SharedString::from("monospace");
        let line = DiffLine {
            kind: DiffLineKind::Added,
            old_line: None,
            new_line: Some(12),
            text: "this long diff line should wrap within the narrow panel ".repeat(8),
            syntax_highlights: Vec::new(),
        };

        div().children([
            div()
                .id("diff-line-wrap-harness")
                .debug_selector(|| "diff-line-wrap-harness".to_string())
                .w(px(220.0))
                .child(render_diff_line(DiffLineRenderInput {
                    item_index: 0,
                    line: &line,
                    thread_count: 0,
                    has_unresolved_thread: false,
                    dragging_for_comment: false,
                    selected_for_comment: false,
                    has_thread_anchor: false,
                    has_thread_range: false,
                    review_line_target: None,
                    line_number_width: 36.0,
                    review_marker_width: REVIEW_MARKER_WIDTH,
                    view_entity: self.view_entity.clone(),
                    mono_font_family: &mono_font_family,
                })),
            div()
                .id("diff-line-number-wrap-harness")
                .debug_selector(|| "diff-line-number-wrap-harness".to_string())
                .w(px(220.0))
                .child(render_diff_line(DiffLineRenderInput {
                    item_index: 1,
                    line: &DiffLine {
                        kind: DiffLineKind::Context,
                        old_line: Some(143),
                        new_line: Some(143),
                        text: "short line".to_string(),
                        syntax_highlights: Vec::new(),
                    },
                    thread_count: 0,
                    has_unresolved_thread: false,
                    dragging_for_comment: false,
                    selected_for_comment: false,
                    has_thread_anchor: false,
                    has_thread_range: false,
                    review_line_target: None,
                    line_number_width: line_number_width_for_diff(&ParsedDiff {
                        hunks: vec![DiffHunk {
                            header: "@@ -143,1 +143,1 @@".to_string(),
                            old_start: 143,
                            old_lines: 1,
                            new_start: 143,
                            new_lines: 1,
                            lines: vec![DiffLine {
                                kind: DiffLineKind::Context,
                                old_line: Some(143),
                                new_line: Some(143),
                                text: "short line".to_string(),
                                syntax_highlights: Vec::new(),
                            }],
                        }],
                    }),
                    review_marker_width: REVIEW_MARKER_WIDTH,
                    view_entity: self.view_entity.clone(),
                    mono_font_family: &mono_font_family,
                })),
        ])
    }
}
