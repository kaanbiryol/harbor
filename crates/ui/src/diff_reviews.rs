use harbor_domain::{DiffFile, ReviewSide, ReviewThread};

use crate::diff::{DiffLine, ParsedDiff};

pub(crate) const REVIEW_THREAD_INLINE_ROWS: usize = 7;

#[derive(Clone, Copy)]
pub(crate) struct AnchoredReviewThread<'a> {
    anchor: ReviewThreadAnchor,
    thread: &'a ReviewThread,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReviewThreadAnchor {
    side: ReviewSide,
    line: u32,
}

pub(crate) fn diff_row_count_with_reviews(
    diff: &ParsedDiff,
    file: &DiffFile,
    review_threads: &[ReviewThread],
) -> usize {
    let anchored_threads = anchored_review_threads(file, review_threads);
    let mut row_count = diff_row_count(diff);

    for hunk in &diff.hunks {
        for line in &hunk.lines {
            row_count +=
                review_thread_count_for_line(&anchored_threads, line) * REVIEW_THREAD_INLINE_ROWS;
        }
    }

    row_count
}

pub(crate) fn diff_hunk_row_index_with_reviews(
    diff: &ParsedDiff,
    hunk_index: usize,
    file: &DiffFile,
    review_threads: &[ReviewThread],
) -> Option<usize> {
    let anchored_threads = anchored_review_threads(file, review_threads);
    let mut row_index = 0;

    for (index, hunk) in diff.hunks.iter().enumerate() {
        if index == hunk_index {
            return Some(row_index);
        }

        row_index += 1;
        for line in &hunk.lines {
            row_index += 1 + review_thread_count_for_line(&anchored_threads, line)
                * REVIEW_THREAD_INLINE_ROWS;
        }
    }

    None
}

pub(crate) fn anchored_review_threads<'a>(
    file: &DiffFile,
    review_threads: &'a [ReviewThread],
) -> Vec<AnchoredReviewThread<'a>> {
    review_threads
        .iter()
        .filter_map(|thread| {
            review_thread_anchor(file, thread).map(|anchor| AnchoredReviewThread { anchor, thread })
        })
        .collect()
}

pub(crate) fn review_threads_for_line<'a>(
    anchored_threads: &[AnchoredReviewThread<'a>],
    line: &DiffLine,
) -> Vec<&'a ReviewThread> {
    anchored_threads
        .iter()
        .filter_map(|anchored_thread| {
            anchored_thread_matches_line(anchored_thread, line).then_some(anchored_thread.thread)
        })
        .collect()
}

fn review_thread_count_for_line(
    anchored_threads: &[AnchoredReviewThread<'_>],
    line: &DiffLine,
) -> usize {
    anchored_threads
        .iter()
        .filter(|anchored_thread| anchored_thread_matches_line(anchored_thread, line))
        .count()
}

fn diff_row_count(diff: &ParsedDiff) -> usize {
    diff.hunks.iter().map(|hunk| hunk.lines.len() + 1).sum()
}

fn anchored_thread_matches_line(
    anchored_thread: &AnchoredReviewThread<'_>,
    line: &DiffLine,
) -> bool {
    review_anchor_matches_line(anchored_thread.anchor, line)
}

fn review_thread_anchor(
    file: &DiffFile,
    review_thread: &ReviewThread,
) -> Option<ReviewThreadAnchor> {
    if let Some(range) = review_thread.range.as_ref()
        && file_path_matches(file, &range.path)
    {
        return Some(ReviewThreadAnchor {
            side: range.side,
            line: range.line,
        });
    }

    review_thread.comments.iter().find_map(|comment| {
        let position = comment.position.as_ref()?;
        if !file_path_matches(file, &position.path) && !file_path_matches(file, &review_thread.path)
        {
            return None;
        }

        match position.side {
            ReviewSide::Left => {
                position
                    .original_line
                    .or(position.line)
                    .map(|line| ReviewThreadAnchor {
                        side: ReviewSide::Left,
                        line,
                    })
            }
            ReviewSide::Right => position
                .line
                .map(|line| ReviewThreadAnchor {
                    side: ReviewSide::Right,
                    line,
                })
                .or_else(|| {
                    position.original_line.map(|line| ReviewThreadAnchor {
                        side: ReviewSide::Left,
                        line,
                    })
                }),
        }
    })
}

fn review_anchor_matches_line(anchor: ReviewThreadAnchor, line: &DiffLine) -> bool {
    match anchor.side {
        ReviewSide::Left => line.old_line == Some(anchor.line),
        ReviewSide::Right => line.new_line == Some(anchor.line),
    }
}

fn file_path_matches(file: &DiffFile, path: &str) -> bool {
    path_matches(&file.path, path)
        || file
            .previous_path
            .as_deref()
            .is_some_and(|previous_path| path_matches(previous_path, path))
}

fn path_matches(expected: &str, candidate: &str) -> bool {
    normalize_path(expected) == normalize_path(candidate)
}

fn normalize_path(path: &str) -> &str {
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
}

#[cfg(test)]
fn review_thread_anchor_row(
    diff: &ParsedDiff,
    file: &DiffFile,
    review_thread: &ReviewThread,
) -> Option<usize> {
    let anchor = review_thread_anchor(file, review_thread)?;
    let mut row_index = 0;

    for hunk in &diff.hunks {
        row_index += 1;

        for line in &hunk.lines {
            if review_anchor_matches_line(anchor, line) {
                return Some(row_index);
            }
            row_index += 1;
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use harbor_domain::{
        DiffFile, FileStatus, ReviewComment, ReviewCommentPosition, ReviewSide, ReviewThread,
        ReviewThreadState,
    };

    use crate::diff::parse_unified_diff;

    use super::*;

    #[test]
    fn anchors_right_side_thread_to_added_line() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -1,2 +1,3 @@\n context\n+added\n unchanged\n");
        let thread = review_thread(
            "thread-1",
            "src/lib.rs",
            ReviewSide::Right,
            Some(2),
            Some(1),
        );

        assert_eq!(review_thread_anchor_row(&diff, &file, &thread), Some(2));
        assert_eq!(
            diff_row_count_with_reviews(&diff, &file, &[thread]),
            diff_row_count(&diff) + REVIEW_THREAD_INLINE_ROWS
        );
    }

    #[test]
    fn anchors_left_side_thread_to_removed_line() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -10,2 +10,1 @@\n-removed\n context\n");
        let thread = review_thread("thread-1", "src/lib.rs", ReviewSide::Left, None, Some(10));

        assert_eq!(review_thread_anchor_row(&diff, &file, &thread), Some(1));
        assert_eq!(
            diff_row_count_with_reviews(&diff, &file, &[thread]),
            diff_row_count(&diff) + REVIEW_THREAD_INLINE_ROWS
        );
    }

    #[test]
    fn skips_threads_for_other_files() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -1 +1,2 @@\n context\n+added\n");
        let thread = review_thread(
            "thread-1",
            "src/other.rs",
            ReviewSide::Right,
            Some(2),
            Some(1),
        );

        assert_eq!(review_thread_anchor_row(&diff, &file, &thread), None);
        assert_eq!(
            diff_row_count_with_reviews(&diff, &file, &[thread]),
            diff_row_count(&diff)
        );
    }

    #[test]
    fn hunk_row_index_accounts_for_inserted_review_rows() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -1 +1,2 @@\n context\n+added\n@@ -10 +11 @@\n later\n");
        let thread = review_thread(
            "thread-1",
            "src/lib.rs",
            ReviewSide::Right,
            Some(2),
            Some(1),
        );

        assert_eq!(
            diff_hunk_row_index_with_reviews(&diff, 1, &file, &[thread]),
            Some(3 + REVIEW_THREAD_INLINE_ROWS)
        );
    }

    fn test_file(path: &str) -> DiffFile {
        DiffFile {
            path: path.to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 1,
            changes: 2,
            patch: None,
        }
    }

    fn review_thread(
        id: &str,
        path: &str,
        side: ReviewSide,
        line: Option<u32>,
        original_line: Option<u32>,
    ) -> ReviewThread {
        ReviewThread {
            id: id.to_string(),
            path: path.to_string(),
            range: None,
            state: ReviewThreadState::Unresolved,
            comments: vec![ReviewComment {
                id: format!("{id}-comment"),
                author: "maria".to_string(),
                body: "Please check this line.".to_string(),
                created_at: test_time(),
                updated_at: None,
                position: Some(ReviewCommentPosition {
                    path: path.to_string(),
                    line,
                    original_line,
                    side,
                }),
            }],
        }
    }

    fn test_time() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-05-01T10:00:00Z")
            .expect("valid test timestamp")
            .with_timezone(&Utc)
    }
}
