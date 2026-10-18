use std::{collections::HashSet, time::Instant};

use harbor_domain::{
    DiffFile, FileStatus, FileViewedState,
    changed_files::{ChangedFileFilters, changed_file_tree_rows},
    diff::parse_unified_diff,
};

fn main() {
    // GitHub lists at most 3,000 changed files for a pull request.
    const FILE_COUNT: usize = 3_000;
    const HUNK_COUNT: usize = 5_000;

    let files = (0..FILE_COUNT)
        .map(|index| DiffFile {
            path: format!(
                "crates/crate-{}/src/module-{}/file-{index}.rs",
                index % 40,
                index % 400
            ),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 1,
            changes: 2,
            patch: None,
            viewed_state: FileViewedState::Unviewed,
        })
        .collect::<Vec<_>>();
    let reviewed = files
        .iter()
        .step_by(3)
        .map(|file| file.path.clone())
        .collect::<HashSet<_>>();

    let started_at = Instant::now();
    let rows = changed_file_tree_rows(
        &files,
        &HashSet::new(),
        &reviewed,
        &ChangedFileFilters::default(),
    );
    let tree_elapsed = started_at.elapsed();
    assert!(rows.len() > FILE_COUNT);

    let mut patch = String::with_capacity(HUNK_COUNT * 160);
    for hunk in 0..HUNK_COUNT {
        let line = hunk * 10 + 1;
        patch.push_str(&format!("@@ -{line},4 +{line},5 @@ fn hunk_{hunk}()\n"));
        patch.push_str(
            " let context = 1;\n-let removed = 2;\n+let added = 2;\n+let extra = 3;\n context();\n",
        );
    }

    let started_at = Instant::now();
    let parsed = parse_unified_diff(&patch);
    let diff_elapsed = started_at.elapsed();
    assert!(!parsed.is_empty());

    println!(
        "built {} tree rows for {FILE_COUNT} files in {tree_elapsed:?}; parsed {HUNK_COUNT} hunks ({} bytes) in {diff_elapsed:?}",
        rows.len(),
        patch.len()
    );
}
