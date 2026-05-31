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
