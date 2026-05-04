use std::collections::HashSet;

use harbor_domain::{
    CheckConclusion, CheckRun, CheckStatus, ChecksSummary, DiffFile, FileStatus, MergeState,
    PullRequest, PullRequestState, ReactionContent, RepoId, ReviewComment, ReviewThread,
    ReviewThreadState,
};
use harbor_git::{ExternalApp, OpenTarget};

use crate::actions::COMMANDS;
use crate::panels::{
    checks_summary_from_runs, github_avatar_url_for_login, merge_blocker, review_action_blocker,
    review_comment_action_visibility, review_comment_avatar_url, review_reaction_button_label,
    review_reaction_emoji, review_thread_counts, visible_review_reaction_contents,
};
use crate::workspace::{
    ChangedFileFilters, ChangedFileTreeRow, OpenTargetStatus, PullRequestInboxMode,
    changed_file_matches_filters, changed_file_matches_query, changed_file_tree_rows,
    changed_file_type_filters, changed_file_type_key, github_file_url, next_switcher_index,
    normalized_search_query, open_target_for_app, open_with_app_disabled, parse_repo_id,
    pull_request_matches_query, repository_matches_query, repository_switcher_accepted_repository,
};

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
fn repository_switcher_accepts_selected_existing_repository_first() {
    let repositories = vec![RepoId::new("acme", "app"), RepoId::new("octo", "tools")];

    assert_eq!(
        repository_switcher_accepted_repository(&repositories, 1, "typed/repo"),
        Some(RepoId::new("octo", "tools"))
    );
}

#[test]
fn repository_switcher_accepts_typed_repository_without_matches() {
    assert_eq!(
        repository_switcher_accepted_repository(&[], 0, "  typed/repo  "),
        Some(RepoId::new("typed", "repo"))
    );
}

#[test]
fn repository_switcher_rejects_invalid_typed_repository_without_matches() {
    assert_eq!(
        repository_switcher_accepted_repository(&[], 0, "typed"),
        None
    );
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
fn builds_changed_file_tree_rows_with_folders() {
    let files = vec![
        diff_file("crates/ui/src/workspace.rs", FileStatus::Modified),
        diff_file("crates/ui/Cargo.toml", FileStatus::Modified),
        diff_file("README.md", FileStatus::Modified),
    ];
    let reviewed = HashSet::from(["crates/ui/src/workspace.rs".to_string()]);

    let rows = changed_file_tree_rows(
        &files,
        &HashSet::new(),
        &reviewed,
        &ChangedFileFilters::default(),
    );

    assert_eq!(
        changed_file_tree_labels(&rows),
        vec![
            "dir:crates:0:1/2:open",
            "dir:ui:1:1/2:open",
            "dir:src:2:1/1:open",
            "file:workspace.rs:3:0",
            "file:Cargo.toml:2:1",
            "file:README.md:0:2",
        ]
    );
}

#[test]
fn collapses_changed_file_tree_folders() {
    let files = vec![
        diff_file("crates/ui/src/workspace.rs", FileStatus::Modified),
        diff_file("crates/ui/Cargo.toml", FileStatus::Modified),
        diff_file("README.md", FileStatus::Modified),
    ];
    let collapsed = HashSet::from(["crates/ui".to_string()]);

    let rows = changed_file_tree_rows(
        &files,
        &collapsed,
        &HashSet::new(),
        &ChangedFileFilters::default(),
    );

    assert_eq!(
        changed_file_tree_labels(&rows),
        vec![
            "dir:crates:0:0/2:open",
            "dir:ui:1:0/2:closed",
            "file:README.md:0:2",
        ]
    );
}

#[test]
fn filters_changed_file_tree_and_expands_matches() {
    let files = vec![
        diff_file("crates/ui/src/workspace.rs", FileStatus::Modified),
        diff_file("crates/ui/Cargo.toml", FileStatus::Modified),
        diff_file("README.md", FileStatus::Modified),
    ];
    let collapsed = HashSet::from(["crates/ui".to_string()]);

    let rows = changed_file_tree_rows(
        &files,
        &collapsed,
        &HashSet::new(),
        &ChangedFileFilters {
            query: "workspace".to_string(),
            ..ChangedFileFilters::default()
        },
    );

    assert_eq!(
        changed_file_tree_labels(&rows),
        vec![
            "dir:crates:0:0/1:open",
            "dir:ui:1:0/1:open",
            "dir:src:2:0/1:open",
            "file:workspace.rs:3:0",
        ]
    );
}

#[test]
fn matches_changed_files_by_path_previous_path_and_status() {
    let mut file = diff_file("src/new_name.rs", FileStatus::Renamed);
    file.previous_path = Some("src/old_name.rs".to_string());

    assert!(changed_file_matches_query(&file, "new_name"));
    assert!(changed_file_matches_query(&file, "old_name"));
    assert!(changed_file_matches_query(&file, "renamed"));
    assert!(!changed_file_matches_query(&file, "deleted"));
}

#[test]
fn derives_changed_file_type_filters_from_extensions() {
    let files = vec![
        diff_file("script/build-worker.mjs", FileStatus::Modified),
        diff_file("fixtures/data.json", FileStatus::Modified),
        diff_file("Dockerfile", FileStatus::Modified),
        diff_file(".gitignore", FileStatus::Modified),
    ];
    let excluded = HashSet::from(["json".to_string()]);

    let filters = changed_file_type_filters(&files, &excluded);

    assert_eq!(changed_file_type_key(&files[0]), "mjs");
    assert_eq!(
        filters
            .into_iter()
            .map(|filter| { format!("{}:{}:{}", filter.label, filter.file_count, filter.included) })
            .collect::<Vec<_>>(),
        vec!["json:1:false", "mjs:1:true", "no extension:2:true",]
    );
}

#[test]
fn filters_changed_file_tree_by_selected_file_types() {
    let files = vec![
        diff_file("script/build-worker.mjs", FileStatus::Modified),
        diff_file("fixtures/data.json", FileStatus::Modified),
        diff_file("README.md", FileStatus::Modified),
    ];

    let rows = changed_file_tree_rows(
        &files,
        &HashSet::new(),
        &HashSet::new(),
        &ChangedFileFilters {
            excluded_file_types: HashSet::from(["json".to_string(), "mjs".to_string()]),
            ..ChangedFileFilters::default()
        },
    );

    assert_eq!(changed_file_tree_labels(&rows), vec!["file:README.md:0:2"]);
}

#[test]
fn filters_changed_file_tree_to_owned_files() {
    let files = vec![
        diff_file("src/owned.rs", FileStatus::Modified),
        diff_file("src/unowned.rs", FileStatus::Modified),
    ];

    let rows = changed_file_tree_rows(
        &files,
        &HashSet::new(),
        &HashSet::new(),
        &ChangedFileFilters {
            owned_by_current_user_only: true,
            owned_file_paths: HashSet::from(["src/owned.rs".to_string()]),
            ..ChangedFileFilters::default()
        },
    );

    assert_eq!(
        changed_file_tree_labels(&rows),
        vec!["dir:src:0:0/1:open", "file:owned.rs:1:0"]
    );
    assert!(changed_file_matches_filters(
        &files[0],
        &ChangedFileFilters {
            owned_by_current_user_only: true,
            owned_file_paths: HashSet::from(["src/owned.rs".to_string()]),
            ..ChangedFileFilters::default()
        }
    ));
}

#[test]
fn wraps_switcher_selection_indexes() {
    assert_eq!(next_switcher_index(0, 0, 1), 0);
    assert_eq!(next_switcher_index(0, 3, 1), 1);
    assert_eq!(next_switcher_index(2, 3, 1), 0);
    assert_eq!(next_switcher_index(0, 3, -1), 2);
    assert_eq!(next_switcher_index(10, 3, 1), 0);
}

#[test]
fn defaults_pull_request_inbox_to_open_mode() {
    assert_eq!(PullRequestInboxMode::default(), PullRequestInboxMode::Open);
    assert_eq!(PullRequestInboxMode::Open.label(), "Open");
    assert_eq!(PullRequestInboxMode::Closed.label(), "Closed");
    assert_eq!(PullRequestInboxMode::NeedsReview.label(), "Needs review");
    assert_eq!(
        PullRequestInboxMode::Closed.empty_message(),
        "No closed pull requests"
    );
}

#[test]
fn command_palette_lists_pull_request_inbox_toggle() {
    assert!(COMMANDS.iter().any(|command| {
        command.shortcut == "cmd+b" && command.title == "Toggle pull request inbox"
    }));
}

#[test]
fn summarizes_check_runs() {
    let check_runs = vec![
        check_run(CheckStatus::Completed, Some(CheckConclusion::Success)),
        check_run(CheckStatus::Completed, Some(CheckConclusion::Failure)),
        check_run(CheckStatus::Completed, Some(CheckConclusion::Skipped)),
        check_run(CheckStatus::InProgress, None),
    ];

    let summary = checks_summary_from_runs(&check_runs);

    assert_eq!(summary.total, 4);
    assert_eq!(summary.passed, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.skipped, 1);
    assert_eq!(summary.pending, 1);
}

#[test]
fn allows_review_actions_for_open_pull_requests() {
    assert_eq!(review_action_blocker(&pull_request()), None);
}

#[test]
fn blocks_merge_until_pull_request_is_ready() {
    let mut pr = pull_request();
    pr.checks_summary.pending = 1;

    assert_eq!(
        merge_blocker(&pr).as_deref(),
        Some("PR #7 still has pending checks")
    );

    pr.checks_summary.pending = 0;
    pr.unresolved_threads = 2;

    assert_eq!(
        merge_blocker(&pr).as_deref(),
        Some("PR #7 still has 2 unresolved review threads")
    );
}

#[test]
fn allows_clean_pull_request_merge() {
    assert_eq!(merge_blocker(&pull_request()), None);
}

#[test]
fn counts_review_threads_by_state() {
    let threads = vec![
        review_thread(ReviewThreadState::Unresolved),
        review_thread(ReviewThreadState::Resolved),
        review_thread(ReviewThreadState::Outdated),
        review_thread(ReviewThreadState::Unresolved),
    ];

    assert_eq!(review_thread_counts(&threads), (2, 1, 1));
}

#[test]
fn labels_review_reaction_buttons() {
    assert_eq!(
        review_reaction_button_label(ReactionContent::ThumbsUp, 0),
        "👍"
    );
    assert_eq!(
        review_reaction_button_label(ReactionContent::Heart, 3),
        "❤️ 3"
    );
    assert_eq!(review_reaction_emoji(ReactionContent::Rocket), "🚀");
}

#[test]
fn resolves_review_comment_avatar_urls() {
    let mut comment = review_comment();

    assert_eq!(
        review_comment_avatar_url(&comment).as_deref(),
        Some("https://github.com/octocat.png?size=48")
    );

    comment.author_avatar_url = Some("https://avatars.githubusercontent.com/u/1?v=4".to_string());
    assert_eq!(
        review_comment_avatar_url(&comment).as_deref(),
        Some("https://avatars.githubusercontent.com/u/1?v=4")
    );

    assert_eq!(github_avatar_url_for_login("ghost"), None);
    assert_eq!(github_avatar_url_for_login("bad login"), None);
}

#[test]
fn shows_only_active_review_reactions_inline() {
    let mut comment = review_comment();
    comment.reactions = vec![harbor_domain::ReviewReaction {
        content: ReactionContent::Heart,
        count: 2,
        viewer_has_reacted: false,
    }];

    assert_eq!(
        visible_review_reaction_contents(&comment),
        vec![ReactionContent::Heart]
    );
}

#[test]
fn exposes_review_comment_action_visibility() {
    let mut comment = review_comment();

    assert_eq!(review_comment_action_visibility(&comment), (false, false));

    comment.viewer_can_update = true;
    comment.viewer_can_delete = true;

    assert_eq!(review_comment_action_visibility(&comment), (true, true));
}

#[test]
fn builds_active_file_github_url() {
    let file = diff_file("src/ui/app view.rs", FileStatus::Modified);

    assert_eq!(
        github_file_url(&pull_request(), &file).as_deref(),
        Some("https://github.com/acme/app/blob/abc123/src/ui/app%20view.rs")
    );
}

#[test]
fn falls_back_for_removed_github_files() {
    let file = diff_file("src/deleted.rs", FileStatus::Removed);

    assert_eq!(github_file_url(&pull_request(), &file), None);
}

#[test]
fn opens_worktree_root_for_removed_local_files() {
    let root = std::path::Path::new("/tmp/harbor-worktree");
    let file = diff_file("src/deleted.rs", FileStatus::Removed);

    let (target, status) = open_target_for_app(ExternalApp::Zed, root, Some(&file));

    assert_eq!(target, OpenTarget::Directory(root.to_path_buf()));
    assert_eq!(status, OpenTargetStatus::RemovedFile);
}

#[test]
fn disables_open_with_apps_without_local_path() {
    assert!(open_with_app_disabled(false, false, ExternalApp::Finder));
    assert!(open_with_app_disabled(true, true, ExternalApp::Finder));
}

fn check_run(status: CheckStatus, conclusion: Option<CheckConclusion>) -> CheckRun {
    CheckRun {
        id: None,
        name: "check".to_string(),
        status,
        conclusion,
        details_url: None,
        html_url: None,
        started_at: None,
        completed_at: None,
    }
}

fn pull_request() -> PullRequest {
    PullRequest {
        repo: RepoId::new("acme", "app"),
        node_id: "pr-node".to_string(),
        number: 7,
        title: "Add feature".to_string(),
        body: None,
        author: "octocat".to_string(),
        url: "https://github.com/acme/app/pull/7".to_string(),
        state: PullRequestState::Open,
        is_draft: false,
        head_ref: "feature".to_string(),
        base_ref: "main".to_string(),
        head_sha: "abc123".to_string(),
        review_decision: None,
        merge_state: Some(MergeState::Clean),
        labels: Vec::new(),
        checks_summary: ChecksSummary {
            total: 1,
            passed: 1,
            failed: 0,
            pending: 0,
            skipped: 0,
        },
        unresolved_threads: 0,
    }
}

fn review_thread(state: ReviewThreadState) -> ReviewThread {
    ReviewThread {
        id: "thread".to_string(),
        path: "src/app.rs".to_string(),
        range: None,
        state,
        comments: Vec::new(),
    }
}

fn review_comment() -> ReviewComment {
    ReviewComment {
        id: "comment".to_string(),
        author: "octocat".to_string(),
        author_avatar_url: None,
        body: "Looks good".to_string(),
        created_at: chrono::DateTime::parse_from_rfc3339("2026-05-01T10:00:00Z")
            .expect("valid test timestamp")
            .with_timezone(&chrono::Utc),
        updated_at: None,
        position: None,
        viewer_did_author: false,
        viewer_can_update: false,
        viewer_can_delete: false,
        viewer_can_react: true,
        reactions: Vec::new(),
    }
}

fn diff_file(path: &str, status: FileStatus) -> DiffFile {
    DiffFile {
        path: path.to_string(),
        previous_path: None,
        status,
        additions: 1,
        deletions: 0,
        changes: 1,
        patch: Some("@@ -1 +1 @@".to_string()),
    }
}

fn changed_file_tree_labels(rows: &[ChangedFileTreeRow]) -> Vec<String> {
    rows.iter()
        .map(|row| match row {
            ChangedFileTreeRow::Folder(folder) => format!(
                "dir:{}:{}:{}/{}:{}",
                folder.name,
                folder.depth,
                folder.reviewed_file_count,
                folder.file_count,
                if folder.expanded { "open" } else { "closed" }
            ),
            ChangedFileTreeRow::File(file) => {
                format!("file:{}:{}:{}", file.name, file.depth, file.file_index)
            }
        })
        .collect()
}
