use std::sync::Arc;

use gpui::{AppContext, Context, ScrollHandle, Task};

use crate::{diff::ParsedDiff, workspace::AppView};

#[derive(Default)]
pub(crate) struct DiffDisplayState {
    source: Option<Arc<Vec<Option<ParsedDiff>>>>,
    width_task: Option<Task<()>>,
    pub(crate) columns: usize,
    pub(crate) horizontal_scroll: ScrollHandle,
}

impl AppView {
    pub(super) fn prepare_diff_display(&mut self, cx: &mut Context<Self>) {
        if self.preferences.value.wrap_diff_lines {
            self.diff_display.width_task = None;
            self.diff_display.source = None;
            self.diff_display.columns = 0;
            self.diff_display
                .horizontal_scroll
                .set_offset(Default::default());
            return;
        }
        let diffs = self.detail_state.shared_diffs();
        if self
            .diff_display
            .source
            .as_ref()
            .is_some_and(|source| Arc::ptr_eq(source, &diffs))
        {
            return;
        }
        self.diff_display.source = Some(diffs.clone());
        self.diff_display.columns = 0;
        let task = cx.background_spawn(async move {
            diffs
                .iter()
                .flatten()
                .flat_map(|diff| &diff.hunks)
                .flat_map(|hunk| &hunk.lines)
                .map(|line| unwrapped_line_columns(&line.text))
                .max()
                .unwrap_or(0)
        });
        self.diff_display.width_task = Some(cx.spawn(async move |this, cx| {
            let columns = task.await;
            if let Err(error) = this.update(cx, |view, cx| {
                view.diff_display.columns = columns;
                view.diff_display.width_task = None;
                cx.notify();
            }) {
                tracing::warn!(%error, "failed to update horizontal diff extent");
            }
        }));
    }
}

fn unwrapped_line_columns(text: &str) -> usize {
    // UTF-8 byte counts conservatively cover wide fallback glyphs; tabs need extra space.
    text.bytes()
        .map(|byte| if byte == b'\t' { 8 } else { 1 })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    async fn long_diffs_scroll_horizontally_and_return_to_wrapping(cx: &mut gpui::TestAppContext) {
        use crate::{
            actions::PanelTab,
            gpui_component::{Root, Theme, ThemeMode},
            test_fixtures::{diff_file, pull_request},
            workspace::test_support::FakeGitHubApi,
        };
        use harbor_domain::FileStatus;
        cx.update(|cx| {
            crate::gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
        });
        let mut entity = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| {
                AppView::new_with_github_api(Arc::new(FakeGitHubApi::default()), window, cx)
            });
            entity = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = entity.expect("view");
        let patch = format!(
            "@@ -0,0 +1,10000 @@\n{}",
            format!("+{}\n", "long line ".repeat(40)).repeat(10_000)
        );
        let parsed = crate::diff::parse_unified_diff(&patch);
        let started = std::time::Instant::now();
        let width_task = view.update(cx, |view, cx| {
            view.pull_requests = vec![pull_request()];
            view.active_tab = PanelTab::Diff;
            view.detail_state.replace_diff_files(
                vec![diff_file("src/lib.rs", FileStatus::Added)],
                vec![Some(parsed)],
            );
            view.preferences.value.wrap_diff_lines = false;
            view.sync_diff_list_items(cx);
            cx.notify();
            view.diff_display
                .width_task
                .take()
                .expect("background width task")
        });
        width_task.await;
        eprintln!(
            "10k-line horizontal diff preparation: {:?}",
            started.elapsed()
        );
        cx.refresh().expect("render horizontal diff");
        let viewport = cx
            .debug_bounds("diff-horizontal-viewport")
            .expect("diff viewport");
        view.read_with(cx, |view, _| {
            assert!(view.diff_display.horizontal_scroll.max_offset().x > gpui::px(0.0))
        });
        cx.simulate_event(gpui::ScrollWheelEvent {
            position: viewport.center(),
            delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(-200.0), gpui::px(0.0))),
            ..Default::default()
        });
        cx.refresh().expect("scroll horizontal diff");
        view.read_with(cx, |view, _| {
            assert!(view.diff_display.horizontal_scroll.offset().x < gpui::px(0.0))
        });
        view.update(cx, |view, cx| {
            view.diff_list_state.scroll_to(gpui::ListOffset {
                item_ix: 500,
                offset_in_item: gpui::px(0.0),
            });
            cx.notify();
        });
        cx.refresh().expect("scroll to review position");
        view.update(cx, |view, cx| {
            view.update_preferences(|preferences| preferences.code_font_size = 18.0, cx)
        });
        cx.refresh().expect("resize code font");
        view.read_with(cx, |view, _| {
            assert_eq!(view.diff_list_state.logical_scroll_top().item_ix, 500)
        });
        view.update(cx, |view, cx| {
            view.update_preferences(|preferences| preferences.wrap_diff_lines = true, cx)
        });
        cx.refresh().expect("render wrapped diff");
        view.read_with(cx, |view, _| {
            assert_eq!(
                view.diff_display.horizontal_scroll.max_offset().x,
                gpui::px(0.0)
            )
        });
    }

    #[test]
    fn horizontal_extent_accounts_for_tabs_and_wide_characters() {
        assert_eq!(unwrapped_line_columns("plain"), 5);
        assert_eq!(unwrapped_line_columns("\tx"), 9);
        assert!(unwrapped_line_columns("界") >= 2);
    }
}
