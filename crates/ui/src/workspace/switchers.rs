use crate::gpui_component::input::{InputEvent, InputState};
use gpui::{App, Context, Entity, Window};
use harbor_domain::{
    PullRequest, RepoId,
    repository_switcher::{
        RepositorySwitcherChoice, next_switcher_index, normalized_search_query,
        repository_matches_query, repository_switcher_accepted_choice,
        repository_switcher_choices_for_query,
    },
};

use crate::workspace::AppView;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PullRequestSwitcherResult {
    pub(crate) inbox_index: Option<usize>,
    pub(crate) pull_request: PullRequest,
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

    pub(crate) fn pull_request_switcher_results(&self, cx: &App) -> Vec<PullRequestSwitcherResult> {
        let search_query = self.current_pull_request_search_query(cx);
        if search_query.is_empty()
            || self.pull_request_search_state.query() != search_query
            || !self.pull_request_search_state.is_loaded()
        {
            return Vec::new();
        }

        self.pull_request_search_state
            .results()
            .iter()
            .cloned()
            .map(|pull_request| PullRequestSwitcherResult {
                inbox_index: self.pull_requests.iter().position(|loaded| {
                    loaded.repo == pull_request.repo && loaded.number == pull_request.number
                }),
                pull_request,
            })
            .collect()
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
        let pull_requests = self.pull_request_switcher_results(cx);
        let selected_pull_request = self.selected_pull_request();
        self.pull_request_switcher_selection = pull_requests
            .iter()
            .position(|result| {
                selected_pull_request.is_some_and(|selected| {
                    selected.repo == result.pull_request.repo
                        && selected.number == result.pull_request.number
                })
            })
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
        let len = self.pull_request_switcher_results(cx).len();
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
                "Loading repositories…".to_string()
            } else {
                "Type owner/repo to open a repository".to_string()
            };
            cx.notify();
            return;
        };

        self.select_repository_choice_from_switcher(choice, cx);
        self.repository_state.repository_switcher_open = false;
        self.pull_request_inbox_search_open = false;
        self.pull_request_filter_popover_open = false;
        cx.notify();
    }

    pub(crate) fn accept_pull_request_switcher_selection(&mut self, cx: &mut Context<Self>) {
        let pull_requests = self.pull_request_switcher_results(cx);
        let Some(result) = pull_requests
            .get(
                self.pull_request_switcher_selection
                    .min(pull_requests.len().saturating_sub(1)),
            )
            .cloned()
        else {
            self.status = if self.has_active_pull_request_filters() {
                "No pull requests match filters".to_string()
            } else {
                "No pull requests match search".to_string()
            };
            cx.notify();
            return;
        };

        self.select_pull_request_switcher_result(result, cx);
    }

    pub(crate) fn select_pull_request_switcher_result(
        &mut self,
        result: PullRequestSwitcherResult,
        cx: &mut Context<Self>,
    ) {
        let index = result.inbox_index.unwrap_or_else(|| {
            self.pull_requests.push(result.pull_request);
            self.pull_requests.len() - 1
        });

        self.select_pull_request(index, cx);
        self.pull_request_inbox_search_open = false;
        self.pull_request_filter_popover_open = false;
        self.clear_pull_request_search();
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
                    self.schedule_pull_request_search(cx);
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
