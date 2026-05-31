use crate::{DiffFile, ReviewSide, ReviewThread};

use crate::diff::DiffLine;
#[cfg(test)]
use crate::diff::ParsedDiff;

#[path = "diff_reviews/rows.rs"]
mod rows;

pub use rows::{
    REVIEW_THREAD_INLINE_ROWS, review_comment_body_row_count, review_thread_inline_rows,
};

#[cfg(test)]
use rows::{
    REVIEW_COMMENT_ROWS_PER_ADDITIONAL_BODY_LINE, REVIEW_THREAD_EMPTY_INLINE_ROWS,
    REVIEW_THREAD_ROWS_PER_ADDITIONAL_COMMENT,
};

#[derive(Clone, Copy)]
pub struct AnchoredReviewThread<'a> {
    anchor: ReviewThreadAnchor,
    thread: &'a ReviewThread,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReviewThreadAnchor {
    side: ReviewSide,
    line: u32,
}

#[cfg(test)]
pub fn diff_row_count_with_reviews<T>(
    diff: &ParsedDiff<T>,
    file: &DiffFile,
    review_threads: &[ReviewThread],
) -> usize {
    let anchored_threads = anchored_review_threads(file, review_threads);
    let mut row_count = diff_row_count(diff);

    for hunk in &diff.hunks {
        for line in &hunk.lines {
            row_count += review_threads_for_line(&anchored_threads, line)
                .into_iter()
                .map(review_thread_inline_rows)
                .sum::<usize>();
        }
    }

    row_count
}

#[cfg(test)]
pub fn diff_hunk_row_index_with_reviews<T>(
    diff: &ParsedDiff<T>,
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
            row_index += 1 + review_threads_for_line(&anchored_threads, line)
                .into_iter()
                .map(review_thread_inline_rows)
                .sum::<usize>();
        }
    }

    None
}

pub fn anchored_review_threads<'a>(
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

pub fn review_threads_for_line<'a, T>(
    anchored_threads: &[AnchoredReviewThread<'a>],
    line: &DiffLine<T>,
) -> Vec<&'a ReviewThread> {
    anchored_threads
        .iter()
        .filter_map(|anchored_thread| {
            anchored_thread_matches_line(anchored_thread, line).then_some(anchored_thread.thread)
        })
        .collect()
}

#[cfg(test)]
fn diff_row_count<T>(diff: &ParsedDiff<T>) -> usize {
    diff.hunks.iter().map(|hunk| hunk.lines.len() + 1).sum()
}

fn anchored_thread_matches_line<T>(
    anchored_thread: &AnchoredReviewThread<'_>,
    line: &DiffLine<T>,
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

fn review_anchor_matches_line<T>(anchor: ReviewThreadAnchor, line: &DiffLine<T>) -> bool {
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
fn review_thread_anchor_row<T>(
    diff: &ParsedDiff<T>,
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
    use crate::{
        DiffFile, FileStatus, ReviewComment, ReviewCommentPosition, ReviewCommentRange, ReviewSide,
        ReviewThread, ReviewThreadState,
    };
    use chrono::{DateTime, Utc};

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
    fn anchors_left_side_thread_range_to_removed_line() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -11 +11,0 @@\n-removed\n");
        let mut thread = review_thread("thread-1", "src/lib.rs", ReviewSide::Left, None, Some(11));
        thread.range = Some(ReviewCommentRange {
            path: "src/lib.rs".to_string(),
            line: 11,
            side: ReviewSide::Left,
            start_line: None,
            start_side: None,
        });

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

    #[test]
    fn counts_additional_review_comment_rows() {
        let mut thread = review_thread(
            "thread-1",
            "src/lib.rs",
            ReviewSide::Right,
            Some(2),
            Some(1),
        );
        let mut reply = thread.comments[0].clone();
        reply.id = "reply-1".to_string();
        thread.comments.push(reply);

        assert_eq!(
            review_thread_inline_rows(&thread),
            REVIEW_THREAD_INLINE_ROWS + REVIEW_THREAD_ROWS_PER_ADDITIONAL_COMMENT
        );
    }

    #[test]
    fn counts_multiline_review_comment_rows() {
        let mut thread = review_thread(
            "thread-1",
            "src/lib.rs",
            ReviewSide::Right,
            Some(2),
            Some(1),
        );
        thread.comments[0].body = "first\nsecond\nthird".to_string();

        assert_eq!(
            review_thread_inline_rows(&thread),
            REVIEW_THREAD_INLINE_ROWS + 2 * REVIEW_COMMENT_ROWS_PER_ADDITIONAL_BODY_LINE
        );
    }

    #[test]
    fn counts_markdown_paragraph_spacing_rows() {
        let mut thread = review_thread(
            "thread-1",
            "src/lib.rs",
            ReviewSide::Right,
            Some(2),
            Some(1),
        );
        thread.comments[0].body = "first paragraph\n\nsecond paragraph".to_string();

        assert_eq!(review_comment_body_row_count(&thread.comments[0].body), 3);
        assert_eq!(
            review_thread_inline_rows(&thread),
            REVIEW_THREAD_INLINE_ROWS + 2 * REVIEW_COMMENT_ROWS_PER_ADDITIONAL_BODY_LINE
        );
    }

    #[test]
    fn counts_markdown_code_block_rows() {
        let mut thread = review_thread(
            "thread-1",
            "src/lib.rs",
            ReviewSide::Right,
            Some(2),
            Some(1),
        );
        thread.comments[0].body =
            "before\n\n```rust\nlet value = 1;\nvalue\n```\nafter".to_string();

        assert_eq!(review_comment_body_row_count(&thread.comments[0].body), 6);
        assert_eq!(
            review_thread_inline_rows(&thread),
            REVIEW_THREAD_INLINE_ROWS + 5 * REVIEW_COMMENT_ROWS_PER_ADDITIONAL_BODY_LINE
        );
    }

    #[test]
    fn skips_markdown_table_separator_rows() {
        let mut thread = review_thread(
            "thread-1",
            "src/lib.rs",
            ReviewSide::Right,
            Some(2),
            Some(1),
        );
        thread.comments[0].body = "| item | state |\n| --- | --- |\n| one | done |".to_string();

        assert_eq!(review_comment_body_row_count(&thread.comments[0].body), 2);
        assert_eq!(
            review_thread_inline_rows(&thread),
            REVIEW_THREAD_INLINE_ROWS + REVIEW_COMMENT_ROWS_PER_ADDITIONAL_BODY_LINE
        );
    }

    #[test]
    fn counts_markdown_list_items_as_rows() {
        let mut thread = review_thread(
            "thread-1",
            "src/lib.rs",
            ReviewSide::Right,
            Some(2),
            Some(1),
        );
        thread.comments[0].body = "- one\n- two\n  continuation".to_string();

        assert_eq!(review_comment_body_row_count(&thread.comments[0].body), 3);
        assert_eq!(
            review_thread_inline_rows(&thread),
            REVIEW_THREAD_INLINE_ROWS + 2 * REVIEW_COMMENT_ROWS_PER_ADDITIONAL_BODY_LINE
        );
    }

    #[test]
    fn keeps_empty_review_threads_compact() {
        let mut thread = review_thread(
            "thread-1",
            "src/lib.rs",
            ReviewSide::Right,
            Some(2),
            Some(1),
        );
        thread.comments.clear();

        assert_eq!(
            review_thread_inline_rows(&thread),
            REVIEW_THREAD_EMPTY_INLINE_ROWS
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
                author_avatar_url: None,
                body: "Please check this line.".to_string(),
                created_at: test_time(),
                updated_at: None,
                position: Some(ReviewCommentPosition {
                    path: path.to_string(),
                    line,
                    original_line,
                    side,
                }),
                viewer_did_author: false,
                viewer_can_update: false,
                viewer_can_delete: false,
                viewer_can_react: true,
                reactions: Vec::new(),
            }],
        }
    }

    fn test_time() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-05-01T10:00:00Z")
            .expect("valid test timestamp")
            .with_timezone(&Utc)
    }
}
