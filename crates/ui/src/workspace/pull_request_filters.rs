use std::collections::{BTreeMap, HashSet};

use gpui::{Context, ScrollStrategy};
use harbor_domain::PullRequest;

use crate::workspace::AppView;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PullRequestFilterFacet {
    Author,
    Label,
    Assignee,
}

impl PullRequestFilterFacet {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Author => "author",
            Self::Label => "label",
            Self::Assignee => "assignee",
        }
    }

    pub(crate) fn section_label(self) -> &'static str {
        match self {
            Self::Author => "Authors",
            Self::Label => "Labels",
            Self::Assignee => "Assignees",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PullRequestFilterChip {
    pub(crate) facet: PullRequestFilterFacet,
    pub(crate) value: String,
}

impl PullRequestFilterChip {
    pub(crate) fn label(&self) -> String {
        format!("{}: {}", self.facet.key(), self.value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PullRequestFilterOption {
    pub(crate) facet: PullRequestFilterFacet,
    pub(crate) value: String,
    pub(crate) count: usize,
    pub(crate) selected: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PullRequestFilterSections {
    pub(crate) authors: Vec<PullRequestFilterOption>,
    pub(crate) labels: Vec<PullRequestFilterOption>,
    pub(crate) assignees: Vec<PullRequestFilterOption>,
}

impl PullRequestFilterSections {
    pub(crate) fn is_empty(&self) -> bool {
        self.authors.is_empty() && self.labels.is_empty() && self.assignees.is_empty()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PullRequestFilters {
    authors: HashSet<String>,
    labels: HashSet<String>,
    assignees: HashSet<String>,
}

impl PullRequestFilters {
    pub(crate) fn has_active_filter(&self) -> bool {
        !self.authors.is_empty() || !self.labels.is_empty() || !self.assignees.is_empty()
    }

    pub(crate) fn active_count(&self) -> usize {
        self.authors.len() + self.labels.len() + self.assignees.len()
    }

    pub(crate) fn clear(&mut self) {
        self.authors.clear();
        self.labels.clear();
        self.assignees.clear();
    }

    pub(crate) fn active_chips(&self) -> Vec<PullRequestFilterChip> {
        let mut chips = Vec::with_capacity(self.active_count());
        chips.extend(filter_chips(
            PullRequestFilterFacet::Author,
            self.authors.iter(),
        ));
        chips.extend(filter_chips(
            PullRequestFilterFacet::Label,
            self.labels.iter(),
        ));
        chips.extend(filter_chips(
            PullRequestFilterFacet::Assignee,
            self.assignees.iter(),
        ));
        chips
    }

    pub(crate) fn selected(&self, facet: PullRequestFilterFacet, value: &str) -> bool {
        self.values(facet).contains(value)
    }

    pub(crate) fn toggle(&mut self, facet: PullRequestFilterFacet, value: String) {
        let values = self.values_mut(facet);
        if !values.remove(&value) {
            values.insert(value);
        }
    }

    pub(crate) fn remove(&mut self, facet: PullRequestFilterFacet, value: &str) {
        self.values_mut(facet).remove(value);
    }

    pub(crate) fn matches(&self, pull_request: &PullRequest) -> bool {
        self.matches_author(pull_request)
            && self.matches_labels(pull_request)
            && self.matches_assignees(pull_request)
    }

    fn values(&self, facet: PullRequestFilterFacet) -> &HashSet<String> {
        match facet {
            PullRequestFilterFacet::Author => &self.authors,
            PullRequestFilterFacet::Label => &self.labels,
            PullRequestFilterFacet::Assignee => &self.assignees,
        }
    }

    fn values_mut(&mut self, facet: PullRequestFilterFacet) -> &mut HashSet<String> {
        match facet {
            PullRequestFilterFacet::Author => &mut self.authors,
            PullRequestFilterFacet::Label => &mut self.labels,
            PullRequestFilterFacet::Assignee => &mut self.assignees,
        }
    }

    fn matches_author(&self, pull_request: &PullRequest) -> bool {
        self.authors.is_empty() || self.authors.contains(&pull_request.author)
    }

    fn matches_labels(&self, pull_request: &PullRequest) -> bool {
        self.labels.is_empty()
            || pull_request
                .labels
                .iter()
                .any(|label| self.labels.contains(&label.name))
    }

    fn matches_assignees(&self, pull_request: &PullRequest) -> bool {
        self.assignees.is_empty()
            || pull_request
                .assignees
                .iter()
                .any(|assignee| self.assignees.contains(&assignee.login))
    }
}

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
        PullRequestFilterSections {
            authors: filter_options(
                PullRequestFilterFacet::Author,
                self.pull_requests
                    .iter()
                    .map(|pull_request| pull_request.author.as_str()),
                &self.pull_request_filters,
            ),
            labels: filter_options(
                PullRequestFilterFacet::Label,
                self.pull_requests
                    .iter()
                    .flat_map(|pull_request| pull_request.labels.iter())
                    .map(|label| label.name.as_str()),
                &self.pull_request_filters,
            ),
            assignees: filter_options(
                PullRequestFilterFacet::Assignee,
                self.pull_requests
                    .iter()
                    .flat_map(|pull_request| pull_request.assignees.iter())
                    .map(|assignee| assignee.login.as_str()),
                &self.pull_request_filters,
            ),
        }
    }

    pub(crate) fn visible_pull_request_indices(&self) -> Vec<usize> {
        self.pull_requests
            .iter()
            .enumerate()
            .filter(|(_, pull_request)| self.pull_request_filters.matches(pull_request))
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn pull_request_matches_active_filters(&self, pull_request: &PullRequest) -> bool {
        self.pull_request_filters.matches(pull_request)
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
        cx.notify();
    }
}

fn filter_chips<'a>(
    facet: PullRequestFilterFacet,
    values: impl Iterator<Item = &'a String>,
) -> Vec<PullRequestFilterChip> {
    let mut values = values.cloned().collect::<Vec<_>>();
    values.sort_by_cached_key(|value| value.to_lowercase());
    values
        .into_iter()
        .map(|value| PullRequestFilterChip { facet, value })
        .collect()
}

fn filter_options<'a>(
    facet: PullRequestFilterFacet,
    values: impl Iterator<Item = &'a str>,
    filters: &PullRequestFilters,
) -> Vec<PullRequestFilterOption> {
    count_filter_values(values)
        .into_iter()
        .map(|(value, count)| PullRequestFilterOption {
            selected: filters.selected(facet, &value),
            facet,
            value,
            count,
        })
        .collect()
}

fn count_filter_values<'a>(values: impl Iterator<Item = &'a str>) -> Vec<(String, usize)> {
    let mut counts = BTreeMap::new();

    for value in values {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        *counts.entry(value.to_string()).or_insert(0) += 1;
    }

    let mut counts = counts.into_iter().collect::<Vec<_>>();
    counts.sort_by(|(left_value, left_count), (right_value, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_value.to_lowercase().cmp(&right_value.to_lowercase()))
            .then_with(|| left_value.cmp(right_value))
    });
    counts
}

#[cfg(test)]
mod tests {
    use harbor_domain::{Label, PullRequestPerson};

    use super::*;
    use crate::test_fixtures::pull_request;

    #[test]
    fn filters_pull_requests_across_selected_facets() {
        let mut filters = PullRequestFilters::default();
        filters.toggle(PullRequestFilterFacet::Author, "octocat".to_string());
        filters.toggle(PullRequestFilterFacet::Label, "bug".to_string());
        filters.toggle(PullRequestFilterFacet::Assignee, "mona".to_string());

        let mut pull_request = pull_request();
        pull_request.labels = vec![Label {
            name: "bug".to_string(),
            color: None,
        }];
        pull_request.assignees = vec![pull_request_person("mona")];
        assert!(filters.matches(&pull_request));

        pull_request.author = "hubot".to_string();
        assert!(!filters.matches(&pull_request));
    }

    #[test]
    fn labels_and_assignees_match_any_selected_value() {
        let mut filters = PullRequestFilters::default();
        filters.toggle(PullRequestFilterFacet::Label, "bug".to_string());
        filters.toggle(PullRequestFilterFacet::Label, "docs".to_string());
        filters.toggle(PullRequestFilterFacet::Assignee, "mona".to_string());
        filters.toggle(PullRequestFilterFacet::Assignee, "hubot".to_string());

        let mut pull_request = pull_request();
        pull_request.labels = vec![Label {
            name: "docs".to_string(),
            color: None,
        }];
        pull_request.assignees = vec![pull_request_person("hubot")];

        assert!(filters.matches(&pull_request));
    }

    #[test]
    fn filter_options_are_counted_and_sorted() {
        let filters = PullRequestFilters::default();
        let options = filter_options(
            PullRequestFilterFacet::Author,
            ["mona", "octocat", "mona", ""].into_iter(),
            &filters,
        );

        assert_eq!(
            options
                .into_iter()
                .map(|option| (option.value, option.count))
                .collect::<Vec<_>>(),
            vec![("mona".to_string(), 2), ("octocat".to_string(), 1)]
        );
    }

    fn pull_request_person(login: &str) -> PullRequestPerson {
        PullRequestPerson {
            login: login.to_string(),
            avatar_url: None,
        }
    }
}
