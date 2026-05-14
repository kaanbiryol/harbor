use gpui::{App, Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};
use harbor_domain::{PullRequest, RepoId};

use crate::workspace::AppView;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RepositorySwitcherChoice {
    Cached(RepoId),
    Typed(RepoId),
}

impl RepositorySwitcherChoice {
    pub(crate) fn repository(&self) -> &RepoId {
        match self {
            Self::Cached(repository) | Self::Typed(repository) => repository,
        }
    }
}

impl AppView {
    pub(crate) fn switcher_repositories(&self) -> Vec<RepoId> {
        let mut repositories = self.repository_state.repositories().to_vec();

        if let Some(repository) = self.repository_state.configured_repo_cloned()
            && !repositories.iter().any(|existing| existing == &repository)
        {
            repositories.push(repository);
        }

        for pull_request in &self.pull_requests {
            if !repositories
                .iter()
                .any(|repository| repository == &pull_request.repo)
            {
                repositories.push(pull_request.repo.clone());
            }
        }

        repositories
    }

    pub(crate) fn filtered_switcher_repositories(&self, cx: &App) -> Vec<RepoId> {
        let query = normalized_search_query(
            &self
                .repository_state
                .repository_search_input
                .read(cx)
                .value(),
        );

        self.switcher_repositories()
            .into_iter()
            .filter(|repository| repository_matches_query(repository, &query))
            .collect()
    }

    pub(crate) fn repository_switcher_choices(&self, cx: &App) -> Vec<RepositorySwitcherChoice> {
        let query = self
            .repository_state
            .repository_search_input
            .read(cx)
            .value();

        repository_switcher_choices_for_query(self.filtered_switcher_repositories(cx), &query)
    }

    pub(crate) fn filtered_switcher_pull_requests(&self, cx: &App) -> Vec<(usize, PullRequest)> {
        let query = normalized_search_query(&self.pull_request_search_input.read(cx).value());

        self.current_repository()
            .map(|repository| {
                self.pull_requests
                    .iter()
                    .enumerate()
                    .filter(|(_, pull_request)| &pull_request.repo == repository)
                    .filter(|(_, pull_request)| pull_request_matches_query(pull_request, &query))
                    .map(|(index, pull_request)| (index, pull_request.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn reset_repository_switcher_selection(&mut self, cx: &App) {
        let current_repository = self.current_repository().cloned();
        let choices = self.repository_switcher_choices(cx);
        self.repository_state.repository_switcher_selection = current_repository
            .and_then(|current| {
                choices
                    .iter()
                    .position(|choice| choice.repository() == &current)
            })
            .unwrap_or(0);
    }

    pub(crate) fn reset_pull_request_switcher_selection(&mut self, cx: &App) {
        let pull_requests = self.filtered_switcher_pull_requests(cx);
        self.pull_request_switcher_selection = pull_requests
            .iter()
            .position(|(index, _)| *index == self.selected_pull_request_index())
            .unwrap_or(0);
    }

    pub(crate) fn move_repository_switcher_selection(
        &mut self,
        delta: isize,
        cx: &mut Context<Self>,
    ) {
        let len = self.repository_switcher_choices(cx).len();
        self.repository_state.repository_switcher_selection = next_switcher_index(
            self.repository_state.repository_switcher_selection,
            len,
            delta,
        );
        cx.notify();
    }

    pub(crate) fn move_pull_request_switcher_selection(
        &mut self,
        delta: isize,
        cx: &mut Context<Self>,
    ) {
        let len = self.filtered_switcher_pull_requests(cx).len();
        self.pull_request_switcher_selection =
            next_switcher_index(self.pull_request_switcher_selection, len, delta);
        cx.notify();
    }

    pub(crate) fn accept_repository_switcher_selection(&mut self, cx: &mut Context<Self>) {
        if self.github_auth_gate_visible() {
            self.repository_state.repository_switcher_open = false;
            cx.notify();
            return;
        }

        let choices = self.repository_switcher_choices(cx);
        let Some(choice) = repository_switcher_accepted_choice(
            &choices,
            self.repository_state.repository_switcher_selection,
        ) else {
            self.status = if self.repository_state.is_loading() {
                "Fetching repositories from GitHub...".to_string()
            } else {
                "Type owner/repo to open a repository".to_string()
            };
            cx.notify();
            return;
        };

        self.select_repository_choice_from_switcher(choice, cx);
        self.repository_state.repository_switcher_open = false;
        self.pull_request_inbox_search_open = false;
        cx.notify();
    }

    pub(crate) fn accept_pull_request_switcher_selection(&mut self, cx: &mut Context<Self>) {
        let pull_requests = self.filtered_switcher_pull_requests(cx);
        let Some((index, _)) = pull_requests
            .get(
                self.pull_request_switcher_selection
                    .min(pull_requests.len().saturating_sub(1)),
            )
            .cloned()
        else {
            self.status = "No pull requests match search".to_string();
            cx.notify();
            return;
        };

        self.select_pull_request(index, cx);
        self.pull_request_inbox_search_open = false;
        cx.notify();
    }

    pub(super) fn on_switcher_search_event(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let is_repository_input =
            input.entity_id() == self.repository_state.repository_search_input.entity_id();
        let is_pull_request_input = input.entity_id() == self.pull_request_search_input.entity_id();

        match event {
            InputEvent::Change => {
                if is_repository_input {
                    self.repository_state.repository_switcher_selection = 0;
                } else if is_pull_request_input {
                    self.pull_request_switcher_selection = 0;
                }

                cx.notify();
            }
            InputEvent::PressEnter { .. }
                if is_repository_input && self.repository_state.repository_switcher_open =>
            {
                self.accept_repository_switcher_selection(cx);
            }
            InputEvent::PressEnter { .. }
                if is_pull_request_input && self.pull_request_inbox_search_open =>
            {
                self.accept_pull_request_switcher_selection(cx);
            }
            _ => {}
        }
    }

    pub(crate) fn select_repository_choice_from_switcher(
        &mut self,
        choice: RepositorySwitcherChoice,
        cx: &mut Context<Self>,
    ) {
        match choice {
            RepositorySwitcherChoice::Cached(repository) => {
                self.select_repository_from_switcher(repository, cx);
            }
            RepositorySwitcherChoice::Typed(repository) => {
                self.open_typed_repository_from_switcher(repository, cx);
            }
        }
    }
}

pub(crate) fn normalized_search_query(query: &str) -> String {
    query.trim().to_lowercase()
}

pub(crate) fn repository_switcher_choices_for_query(
    repositories: Vec<RepoId>,
    query: &str,
) -> Vec<RepositorySwitcherChoice> {
    let typed_repository = parse_repo_id(query);
    let exact_match_index = typed_repository.as_ref().and_then(|typed_repository| {
        repositories
            .iter()
            .position(|repository| repository_ids_match(repository, typed_repository))
    });

    match (typed_repository, exact_match_index) {
        (Some(_), Some(index)) => {
            let mut choices = Vec::with_capacity(repositories.len());
            choices.push(RepositorySwitcherChoice::Cached(
                repositories[index].clone(),
            ));
            choices.extend(
                repositories
                    .into_iter()
                    .enumerate()
                    .filter(|(repository_index, _)| *repository_index != index)
                    .map(|(_, repository)| RepositorySwitcherChoice::Cached(repository)),
            );
            choices
        }
        (Some(repository), None) => {
            let mut choices = Vec::with_capacity(repositories.len() + 1);
            choices.push(RepositorySwitcherChoice::Typed(repository));
            choices.extend(
                repositories
                    .into_iter()
                    .map(RepositorySwitcherChoice::Cached),
            );
            choices
        }
        (None, None) => repositories
            .into_iter()
            .map(RepositorySwitcherChoice::Cached)
            .collect(),
        (None, Some(_)) => unreachable!("exact match requires a typed repository"),
    }
}

pub(crate) fn repository_switcher_accepted_choice(
    choices: &[RepositorySwitcherChoice],
    selected_index: usize,
) -> Option<RepositorySwitcherChoice> {
    choices
        .get(selected_index.min(choices.len().saturating_sub(1)))
        .cloned()
}

pub(crate) fn repository_matches_query(repository: &RepoId, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    repository.full_name().to_lowercase().contains(query)
}

fn repository_ids_match(left: &RepoId, right: &RepoId) -> bool {
    left.owner.eq_ignore_ascii_case(&right.owner) && left.name.eq_ignore_ascii_case(&right.name)
}

pub(crate) fn pull_request_matches_query(pull_request: &PullRequest, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    pull_request.title.to_lowercase().contains(query)
        || pull_request.number.to_string().contains(query)
        || pull_request.author.to_lowercase().contains(query)
}

pub(crate) fn next_switcher_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }

    let current = current.min(len - 1) as isize;
    (current + delta).rem_euclid(len as isize) as usize
}

pub(crate) fn parse_repo_id(value: &str) -> Option<RepoId> {
    let value = value.trim();
    let (owner, name) = value.split_once('/')?;

    if owner.is_empty()
        || name.is_empty()
        || name.contains('/')
        || owner.chars().any(char::is_whitespace)
        || name.chars().any(char::is_whitespace)
    {
        None
    } else {
        Some(RepoId::new(owner, name))
    }
}

#[cfg(test)]
mod tests {
    use harbor_domain::RepoId;

    use super::*;
    use crate::test_fixtures::pull_request;

    #[test]
    fn parses_owner_and_repo() {
        let repo = parse_repo_id("acme/app").unwrap();

        assert_eq!(repo.owner, "acme");
        assert_eq!(repo.name, "app");

        let repo = parse_repo_id("  Acme/Mobile-App  ").unwrap();

        assert_eq!(repo.owner, "Acme");
        assert_eq!(repo.name, "Mobile-App");
    }

    #[test]
    fn rejects_invalid_repo_values() {
        assert!(parse_repo_id("").is_none());
        assert!(parse_repo_id("acme").is_none());
        assert!(parse_repo_id("/app").is_none());
        assert!(parse_repo_id("acme/").is_none());
        assert!(parse_repo_id("acme/app/extra").is_none());
        assert!(parse_repo_id("acme /app").is_none());
        assert!(parse_repo_id("acme/app name").is_none());
    }

    #[test]
    fn normalizes_switcher_search_queries() {
        assert_eq!(normalized_search_query("  Acme/App  "), "acme/app");
    }

    #[test]
    fn matches_repositories_for_switcher_search() {
        let repository = RepoId::new("Acme", "Mobile-App");

        assert!(repository_matches_query(&repository, ""));
        assert!(repository_matches_query(&repository, "mobile"));
        assert!(repository_matches_query(&repository, "acme/mobile"));
        assert!(!repository_matches_query(&repository, "backend"));
    }

    #[test]
    fn repository_switcher_accepts_selected_existing_repository() {
        let repositories = vec![RepoId::new("acme", "app"), RepoId::new("octo", "tools")];
        let choices = repository_switcher_choices_for_query(repositories, "");

        assert_eq!(
            repository_switcher_accepted_choice(&choices, 1),
            Some(RepositorySwitcherChoice::Cached(RepoId::new(
                "octo", "tools"
            )))
        );
    }

    #[test]
    fn repository_switcher_prefers_typed_repository_without_exact_match() {
        let repositories = vec![RepoId::new("acme", "app-old")];
        let choices = repository_switcher_choices_for_query(repositories, "acme/app");

        assert_eq!(
            choices[0],
            RepositorySwitcherChoice::Typed(RepoId::new("acme", "app"))
        );
        assert_eq!(
            repository_switcher_accepted_choice(&choices, 0),
            Some(RepositorySwitcherChoice::Typed(RepoId::new("acme", "app")))
        );
    }

    #[test]
    fn repository_switcher_prefers_exact_cached_match_over_typed_repository() {
        let repositories = vec![RepoId::new("acme", "app-old"), RepoId::new("Acme", "App")];
        let choices = repository_switcher_choices_for_query(repositories, "acme/app");

        assert_eq!(
            choices[0],
            RepositorySwitcherChoice::Cached(RepoId::new("Acme", "App"))
        );
        assert!(
            !choices
                .iter()
                .any(|choice| matches!(choice, RepositorySwitcherChoice::Typed(_)))
        );
    }

    #[test]
    fn repository_switcher_rejects_invalid_typed_repository_without_matches() {
        let choices = repository_switcher_choices_for_query(Vec::new(), "typed");

        assert_eq!(repository_switcher_accepted_choice(&choices, 0), None);
    }

    #[test]
    fn matches_pull_requests_for_switcher_search() {
        let pull_request = pull_request();

        assert!(pull_request_matches_query(&pull_request, ""));
        assert!(pull_request_matches_query(&pull_request, "feature"));
        assert!(pull_request_matches_query(&pull_request, "7"));
        assert!(pull_request_matches_query(&pull_request, "octo"));
        assert!(!pull_request_matches_query(&pull_request, "backend"));
    }

    #[test]
    fn wraps_switcher_selection_indexes() {
        assert_eq!(next_switcher_index(0, 0, 1), 0);
        assert_eq!(next_switcher_index(0, 3, 1), 1);
        assert_eq!(next_switcher_index(2, 3, 1), 0);
        assert_eq!(next_switcher_index(0, 3, -1), 2);
        assert_eq!(next_switcher_index(10, 3, 1), 0);
    }
}
