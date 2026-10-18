use gpui::{Context, ScrollStrategy};
use harbor_domain::pull_request_filters::{
    PullRequestFilterChip, PullRequestFilterFacet, PullRequestFilterSections,
};

use crate::workspace::AppView;

impl AppView {
    pub(crate) fn has_active_pull_request_filters(&self) -> bool {
        self.pull_request_filters.has_active_filter()
    }

    pub(crate) fn pull_request_filter_count(&self) -> usize {
        self.pull_request_filters.active_count()
    }

    pub(crate) fn active_pull_request_filter_chips(&self) -> Vec<PullRequestFilterChip> {
        self.pull_request_filters.active_chips()
    }

    pub(crate) fn pull_request_filter_sections(&self) -> PullRequestFilterSections {
        self.pull_request_filters.sections(&self.pull_requests)
    }

    pub(crate) fn visible_pull_request_indices(&self) -> Vec<usize> {
        self.pull_requests
            .iter()
            .enumerate()
            .filter(|(_, pull_request)| self.pull_request_filters.matches(pull_request))
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn selected_pull_request_list_position(&self) -> usize {
        if !self.pull_request_filters.has_active_filter() {
            return self.selected_pull_request_index();
        }

        self.visible_pull_request_indices()
            .iter()
            .position(|index| *index == self.selected_pull_request_index())
            .unwrap_or(0)
    }

    pub(crate) fn scroll_selected_pull_request_into_view(&self, strategy: ScrollStrategy) {
        self.pr_list_scroll
            .scroll_to_item(self.selected_pull_request_list_position(), strategy);
    }

    pub(crate) fn toggle_pull_request_filter(
        &mut self,
        facet: PullRequestFilterFacet,
        value: String,
        cx: &mut Context<Self>,
    ) {
        self.pull_request_filters.toggle(facet, value);
        self.after_pull_request_filter_change(cx);
    }

    pub(crate) fn remove_pull_request_filter(
        &mut self,
        facet: PullRequestFilterFacet,
        value: &str,
        cx: &mut Context<Self>,
    ) {
        self.pull_request_filters.remove(facet, value);
        self.after_pull_request_filter_change(cx);
    }

    pub(crate) fn clear_pull_request_filters(&mut self, cx: &mut Context<Self>) {
        self.pull_request_filters.clear();
        self.after_pull_request_filter_change(cx);
    }

    pub(crate) fn reset_pull_request_filters(&mut self) {
        self.pull_request_filters.clear();
        self.pull_request_filter_popover_open = false;
    }

    fn after_pull_request_filter_change(&mut self, cx: &mut Context<Self>) {
        let visible_indices = self.visible_pull_request_indices();
        let has_active_filter = self.pull_request_filters.has_active_filter();

        if let Some(first_visible_index) = visible_indices.first().copied()
            && !visible_indices.contains(&self.selected_pull_request_index())
        {
            self.select_pull_request(first_visible_index, cx);
        }

        let visible_count = visible_indices.len();
        self.pr_list_scroll.scroll_to_item(
            if visible_count == 0 {
                0
            } else {
                self.selected_pull_request_list_position()
            },
            ScrollStrategy::Top,
        );
        self.status = if has_active_filter {
            format!("Filtered pull requests to {visible_count} visible")
        } else {
            "Cleared pull request filters".to_string()
        };
        if !self.current_pull_request_search_query(cx).is_empty() {
            self.schedule_pull_request_search(cx);
        }
        cx.notify();
    }
}
