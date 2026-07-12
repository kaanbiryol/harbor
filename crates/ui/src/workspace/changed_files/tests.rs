use std::collections::HashSet;

use harbor_domain::FileStatus;

use super::*;
use crate::test_fixtures::diff_file;

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
            "dir:crates/ui:0:1/2:open",
            "dir:src:1:1/1:open",
            "file:workspace.rs:2:0",
            "file:Cargo.toml:1:1",
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
        vec!["dir:crates/ui:0:0/2:closed", "file:README.md:0:2"]
    );
}

#[test]
fn groups_deep_single_child_folder_chains() {
    let files = vec![
        diff_file(
            "android/libraries/services/src/main/kotlin/com/acme/android/Service.kt",
            FileStatus::Modified,
        ),
        diff_file(
            "android/libraries/services/src/main/kotlin/com/acme/android/Repository.kt",
            FileStatus::Modified,
        ),
    ];

    let rows = changed_file_tree_rows(
        &files,
        &HashSet::new(),
        &HashSet::new(),
        &ChangedFileFilters::default(),
    );

    assert_eq!(
        changed_file_tree_labels(&rows),
        vec![
            "dir:android/libraries/services/src/main/kotlin/com/acme/android:0:0/2:open",
            "file:Repository.kt:1:1",
            "file:Service.kt:1:0",
        ]
    );
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
        vec!["json:1:false", "mjs:1:true", "no extension:2:true"]
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
