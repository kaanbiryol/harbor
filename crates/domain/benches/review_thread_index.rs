use std::time::Instant;

use harbor_domain::{
    DiffFile, FileStatus, FileViewedState, ReviewCommentRange, ReviewSide, ReviewThread,
    ReviewThreadState, diff::DiffLine, diff_reviews::ReviewThreadIndex,
};

fn main() {
    const THREAD_COUNT: usize = 10_000;
    let threads = (1..=THREAD_COUNT)
        .map(|line| ReviewThread {
            id: format!("thread-{line}"),
            path: "src/lib.rs".to_string(),
            range: Some(ReviewCommentRange {
                path: "src/lib.rs".to_string(),
                line: line as u32,
                side: ReviewSide::Right,
                start_line: None,
                start_side: None,
            }),
            state: ReviewThreadState::Unresolved,
            comments: Vec::new(),
        })
        .collect::<Vec<_>>();
    let file = DiffFile {
        path: "src/lib.rs".to_string(),
        previous_path: None,
        status: FileStatus::Modified,
        additions: THREAD_COUNT as u32,
        deletions: 0,
        changes: THREAD_COUNT as u32,
        patch: None,
        viewed_state: FileViewedState::Unviewed,
    };

    let started_at = Instant::now();
    let index = ReviewThreadIndex::new(&threads);
    let build_elapsed = started_at.elapsed();
    let started_at = Instant::now();
    let mut matches = 0;
    for line in 1..=THREAD_COUNT {
        let diff_line = DiffLine::<()> {
            kind: harbor_domain::diff::DiffLineKind::Added,
            old_line: None,
            new_line: Some(line as u32),
            text: String::new(),
            syntax_highlights: Vec::new(),
        };
        index.for_each_thread_for_line(&file, &diff_line, |_| matches += 1);
    }
    let lookup_elapsed = started_at.elapsed();
    assert_eq!(matches, THREAD_COUNT);

    println!(
        "indexed {THREAD_COUNT} threads in {build_elapsed:?}; queried {THREAD_COUNT} lines in {lookup_elapsed:?}"
    );
}
