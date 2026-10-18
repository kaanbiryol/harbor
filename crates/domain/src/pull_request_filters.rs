use std::collections::{BTreeMap, HashSet};

use crate::PullRequest;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PullRequestFilterFacet {
    Author,
    Label,
    Assignee,
}

impl PullRequestFilterFacet {
    pub fn key(self) -> &'static str {
        match self {
            Self::Author => "author",
            Self::Label => "label",
            Self::Assignee => "assignee",
        }
    }

    pub fn section_label(self) -> &'static str {
        match self {
            Self::Author => "Authors",
            Self::Label => "Labels",
            Self::Assignee => "Assignees",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullRequestFilterChip {
    pub facet: PullRequestFilterFacet,
    pub value: String,
}

impl PullRequestFilterChip {
    pub fn label(&self) -> String {
        format!("{}: {}", self.facet.key(), self.value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PullRequestFilterOption {
    pub facet: PullRequestFilterFacet,
    pub value: String,
    pub count: usize,
    pub selected: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PullRequestFilterSections {
    pub authors: Vec<PullRequestFilterOption>,
    pub labels: Vec<PullRequestFilterOption>,
    pub assignees: Vec<PullRequestFilterOption>,
}

impl PullRequestFilterSections {
    pub fn is_empty(&self) -> bool {
        self.authors.is_empty() && self.labels.is_empty() && self.assignees.is_empty()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PullRequestFilters {
    authors: HashSet<String>,
    labels: HashSet<String>,
    assignees: HashSet<String>,
}

impl PullRequestFilters {
    pub fn has_active_filter(&self) -> bool {
        !self.authors.is_empty() || !self.labels.is_empty() || !self.assignees.is_empty()
    }

    pub fn active_count(&self) -> usize {
        self.authors.len() + self.labels.len() + self.assignees.len()
    }

    pub fn clear(&mut self) {
        self.authors.clear();
        self.labels.clear();
        self.assignees.clear();
    }

    pub fn active_chips(&self) -> Vec<PullRequestFilterChip> {
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

    pub fn selected(&self, facet: PullRequestFilterFacet, value: &str) -> bool {
        self.values(facet).contains(value)
    }

    pub fn toggle(&mut self, facet: PullRequestFilterFacet, value: String) {
        let values = self.values_mut(facet);
        if !values.remove(&value) {
            values.insert(value);
        }
    }

    pub fn remove(&mut self, facet: PullRequestFilterFacet, value: &str) {
        self.values_mut(facet).remove(value);
    }

    pub fn matches(&self, pull_request: &PullRequest) -> bool {
        self.matches_author(pull_request)
            && self.matches_labels(pull_request)
            && self.matches_assignees(pull_request)
    }

    pub fn sections(&self, pull_requests: &[PullRequest]) -> PullRequestFilterSections {
        PullRequestFilterSections {
            authors: filter_options(
                PullRequestFilterFacet::Author,
                pull_requests
                    .iter()
                    .map(|pull_request| pull_request.author.as_str()),
                self,
            ),
            labels: filter_options(
                PullRequestFilterFacet::Label,
                pull_requests
                    .iter()
                    .flat_map(|pull_request| pull_request.labels.iter())
                    .map(|label| label.name.as_str()),
                self,
            ),
            assignees: filter_options(
                PullRequestFilterFacet::Assignee,
                pull_requests
                    .iter()
                    .flat_map(|pull_request| pull_request.assignees.iter())
                    .map(|assignee| assignee.login.as_str()),
                self,
            ),
        }
    }

    pub fn github_search_query(&self) -> String {
        [
            github_qualifier_group("author", &self.authors),
            github_qualifier_group("label", &self.labels),
            github_qualifier_group("assignee", &self.assignees),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ")
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

fn github_qualifier_group(qualifier: &str, values: &HashSet<String>) -> Option<String> {
    if values.is_empty() {
        return None;
    }

    let mut values = values.iter().collect::<Vec<_>>();
    values.sort_by_cached_key(|value| value.to_lowercase());
    let qualifiers = values
        .into_iter()
        .map(|value| {
            let value = value.replace('\\', "\\\\").replace('"', "\\\"");
            format!("{qualifier}:\"{value}\"")
        })
        .collect::<Vec<_>>();

    Some(if qualifiers.len() == 1 {
        qualifiers.into_iter().next().unwrap_or_default()
    } else {
        format!("({})", qualifiers.join(" OR "))
    })
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
    use super::*;
    use crate::{Label, PullRequestPerson, test_fixtures::pull_request};

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

    #[test]
    fn builds_stable_github_search_qualifiers_with_or_within_facets() {
        let mut filters = PullRequestFilters::default();
        filters.toggle(PullRequestFilterFacet::Author, "octocat".to_string());
        filters.toggle(
            PullRequestFilterFacet::Label,
            "needs \"review\"".to_string(),
        );
        filters.toggle(PullRequestFilterFacet::Label, "bug".to_string());

        assert_eq!(
            filters.github_search_query(),
            "author:\"octocat\" (label:\"bug\" OR label:\"needs \\\"review\\\"\")"
        );
    }

    fn pull_request_person(login: &str) -> PullRequestPerson {
        PullRequestPerson {
            login: login.to_string(),
            avatar_url: None,
        }
    }
}
