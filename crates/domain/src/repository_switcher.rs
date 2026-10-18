use crate::RepoId;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepositorySwitcherChoice {
    Cached(RepoId),
    Typed(RepoId),
}

impl RepositorySwitcherChoice {
    pub fn repository(&self) -> &RepoId {
        match self {
            Self::Cached(repository) | Self::Typed(repository) => repository,
        }
    }
}

pub fn normalized_search_query(query: &str) -> String {
    query.trim().to_lowercase()
}

pub fn repository_switcher_choices_for_query(
    mut repositories: Vec<RepoId>,
    query: &str,
) -> Vec<RepositorySwitcherChoice> {
    let Some(typed_repository) = RepoId::parse(query) else {
        return repositories
            .into_iter()
            .map(RepositorySwitcherChoice::Cached)
            .collect();
    };

    let mut choices = Vec::with_capacity(repositories.len() + 1);
    match repositories
        .iter()
        .position(|repository| repository.matches_ignore_ascii_case(&typed_repository))
    {
        Some(index) => choices.push(RepositorySwitcherChoice::Cached(repositories.remove(index))),
        None => choices.push(RepositorySwitcherChoice::Typed(typed_repository)),
    }
    choices.extend(
        repositories
            .into_iter()
            .map(RepositorySwitcherChoice::Cached),
    );
    choices
}

pub fn repository_switcher_accepted_choice(
    choices: &[RepositorySwitcherChoice],
    selected_index: usize,
) -> Option<RepositorySwitcherChoice> {
    choices
        .get(selected_index.min(choices.len().saturating_sub(1)))
        .cloned()
}

pub fn repository_matches_query(repository: &RepoId, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    repository.full_name().to_lowercase().contains(query)
}

pub fn next_switcher_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }

    let current = current.min(len - 1) as isize;
    (current + delta).rem_euclid(len as isize) as usize
}
