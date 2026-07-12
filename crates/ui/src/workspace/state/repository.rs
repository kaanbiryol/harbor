use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use gpui::Entity;
use gpui_component::input::InputState;
use harbor_domain::RepoId;
use harbor_storage::SqliteStore;

pub(crate) struct RepositoryUiState {
    repositories: Vec<RepoId>,
    pinned_repositories: HashSet<RepoId>,
    pub(crate) repository_switcher_open: bool,
    pub(crate) repository_switcher_selection: usize,
    pub(crate) repository_search_input: Entity<InputState>,
    configured_repo: Option<RepoId>,
    repository_store: Option<SqliteStore>,
    repository_local_paths: HashMap<RepoId, PathBuf>,
    is_loading_repositories: bool,
    repository_error: Option<String>,
    repository_notice: Option<String>,
}

impl RepositoryUiState {
    pub(crate) fn new(
        repository_search_input: Entity<InputState>,
        is_loading: bool,
        storage: std::result::Result<SqliteStore, String>,
    ) -> Self {
        let (repository_store, repository_error) = match storage {
            Ok(store) => (Some(store), None),
            Err(error) => (None, Some(error)),
        };
        Self {
            repositories: Vec::new(),
            pinned_repositories: HashSet::new(),
            repository_switcher_open: false,
            repository_switcher_selection: 0,
            repository_search_input,
            configured_repo: None,
            repository_store,
            repository_local_paths: HashMap::new(),
            is_loading_repositories: is_loading,
            repository_error,
            repository_notice: None,
        }
    }

    pub(crate) fn repositories(&self) -> &[RepoId] {
        &self.repositories
    }

    pub(crate) fn is_pinned(&self, repository: &RepoId) -> bool {
        self.pinned_repositories.contains(repository)
    }

    pub(crate) fn pinned_repositories(&self) -> impl Iterator<Item = &RepoId> {
        self.repositories
            .iter()
            .filter(|repository| self.is_pinned(repository))
    }

    pub(crate) fn configured_repo(&self) -> Option<&RepoId> {
        self.configured_repo.as_ref()
    }

    pub(crate) fn configured_repo_cloned(&self) -> Option<RepoId> {
        self.configured_repo.clone()
    }

    pub(crate) fn has_configured_repo(&self) -> bool {
        self.configured_repo.is_some()
    }

    pub(crate) fn store(&self) -> Option<SqliteStore> {
        self.repository_store.clone()
    }

    pub(crate) fn local_path(&self, repository: &RepoId) -> Option<&PathBuf> {
        self.repository_local_paths.get(repository)
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.is_loading_repositories
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.repository_error.as_deref()
    }

    pub(crate) fn notice(&self) -> Option<&str> {
        self.repository_notice.as_deref()
    }

    pub(crate) fn start_loading(&mut self) {
        self.is_loading_repositories = true;
    }

    pub(crate) fn finish_loading(&mut self) {
        self.is_loading_repositories = false;
    }

    pub(crate) fn set_store(&mut self, store: SqliteStore) {
        self.repository_store = Some(store);
        self.repository_error = None;
        self.repository_notice = None;
    }

    pub(crate) fn clear_store_with_error(&mut self, error: impl Into<String>) {
        self.repository_store = None;
        self.is_loading_repositories = false;
        self.repository_error = Some(error.into());
        self.repository_notice = None;
    }

    pub(crate) fn set_error(&mut self, error: impl Into<String>) {
        self.repository_error = Some(error.into());
        self.repository_notice = None;
    }

    pub(crate) fn clear_error(&mut self) {
        self.repository_error = None;
    }

    pub(crate) fn set_notice(&mut self, notice: impl Into<String>) {
        self.repository_error = None;
        self.repository_notice = Some(notice.into());
    }

    pub(crate) fn clear_notice(&mut self) {
        self.repository_notice = None;
    }

    pub(crate) fn clear_visible_repositories(&mut self) {
        self.repositories.clear();
        self.pinned_repositories.clear();
        self.configured_repo = None;
        self.repository_local_paths.clear();
        self.repository_switcher_selection = 0;
        self.repository_error = None;
        self.repository_notice = None;
    }

    pub(crate) fn select_repository(&mut self, repository: RepoId) {
        self.configured_repo = Some(repository);
    }

    pub(crate) fn remember_repository(&mut self, repository: RepoId) {
        self.repositories.retain(|existing| existing != &repository);
        self.repositories.insert(0, repository);
    }

    pub(crate) fn replace_fetched_repositories(&mut self, repositories: Vec<RepoId>) {
        let pinned = self.pinned_repositories().cloned().collect::<Vec<_>>();
        self.repositories = pinned;
        for repository in repositories {
            if !self.repositories.contains(&repository) {
                self.repositories.push(repository);
            }
        }
    }

    pub(crate) fn set_pinned(&mut self, repository: RepoId, pinned: bool) {
        if pinned {
            self.pinned_repositories.insert(repository.clone());
            self.remember_repository(repository);
        } else {
            self.pinned_repositories.remove(&repository);
        }
    }

    pub(crate) fn set_local_path(&mut self, repository: RepoId, path: PathBuf) {
        self.repository_local_paths.insert(repository, path);
    }
}
