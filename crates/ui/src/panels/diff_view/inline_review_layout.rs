use harbor_domain::{DiffFile, ReviewCommentRange, ReviewSide};

use crate::{
    diff::{DiffLine, DiffLineKind},
    workspace::ReviewLineTarget,
};

pub(super) fn review_line_target_for_line(
    file: &DiffFile,
    hunk_index: usize,
    line_index: usize,
    line: &DiffLine,
) -> Option<ReviewLineTarget> {
    match line.kind {
        DiffLineKind::Metadata => None,
        DiffLineKind::Removed => {
            let line_number = line.old_line?;
            Some(ReviewLineTarget {
                hunk_index,
                line_index,
                range: ReviewCommentRange {
                    path: file.path.clone(),
                    line: line_number,
                    side: ReviewSide::Left,
                    start_line: None,
                    start_side: None,
                },
            })
        }
        DiffLineKind::Added | DiffLineKind::Context => {
            line.new_line.map(|line_number| ReviewLineTarget {
                hunk_index,
                line_index,
                range: ReviewCommentRange {
                    path: file.path.clone(),
                    line: line_number,
                    side: ReviewSide::Right,
                    start_line: None,
                    start_side: None,
                },
            })
        }
    }
}

pub(super) fn review_comment_range_matches_line(
    file: &DiffFile,
    range: &ReviewCommentRange,
    line: &DiffLine,
) -> bool {
    if !review_comment_range_matches_file(file, range) {
        return false;
    }

    match range.side {
        ReviewSide::Left => line.old_line.is_some_and(|line_number| {
            line_number >= range.start_line.unwrap_or(range.line) && line_number <= range.line
        }),
        ReviewSide::Right => line.new_line.is_some_and(|line_number| {
            line_number >= range.start_line.unwrap_or(range.line) && line_number <= range.line
        }),
    }
}

pub(super) fn review_comment_range_matches_file(
    file: &DiffFile,
    range: &ReviewCommentRange,
) -> bool {
    path_matches_file(file, &range.path)
}

pub(super) fn review_comment_range_label(range: &ReviewCommentRange) -> String {
    let side = match range.side {
        ReviewSide::Left => "old",
        ReviewSide::Right => "new",
    };

    if let Some(start_line) = range.start_line {
        format!("{side} lines {start_line}-{} in {}", range.line, range.path)
    } else {
        format!("{side} line {} in {}", range.line, range.path)
    }
}

pub(super) fn review_diff_line_anchor_label(file: &DiffFile, line: &DiffLine) -> Option<String> {
    match line.kind {
        DiffLineKind::Removed => line
            .old_line
            .map(|line_number| format!("old line {line_number} in {}", file.path)),
        DiffLineKind::Added | DiffLineKind::Context => line
            .new_line
            .map(|line_number| format!("new line {line_number} in {}", file.path)),
        DiffLineKind::Metadata => None,
    }
}

fn path_matches_file(file: &DiffFile, path: &str) -> bool {
    path == file.path || file.previous_path.as_deref() == Some(path)
}

#[cfg(test)]
mod tests {
    use harbor_domain::{FileStatus, ReviewSide};

    use crate::{diff::parse_unified_diff, workspace::review_range_from_targets};

    use super::*;

    #[test]
    fn selects_right_side_target_for_added_line() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -1 +1,2 @@\n context\n+added\n");
        let target = review_line_target_for_line(&file, 0, 1, &diff.hunks[0].lines[1])
            .expect("added line should be commentable");

        assert_eq!(target.range.path, "src/lib.rs");
        assert_eq!(target.range.side, ReviewSide::Right);
        assert_eq!(target.range.line, 2);
        assert_eq!(target.range.start_line, None);
    }

    #[test]
    fn selects_left_side_target_for_removed_line() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -10,2 +10 @@\n-removed\n context\n");
        let target = review_line_target_for_line(&file, 0, 0, &diff.hunks[0].lines[0])
            .expect("removed line should be commentable");

        assert_eq!(target.range.path, "src/lib.rs");
        assert_eq!(target.range.side, ReviewSide::Left);
        assert_eq!(target.range.line, 10);
        assert_eq!(target.range.start_line, None);
    }

    #[test]
    fn builds_multiline_right_side_review_range() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -1 +1,3 @@\n context\n+added\n+again\n");
        let start = review_line_target_for_line(&file, 0, 1, &diff.hunks[0].lines[1])
            .expect("added line should be commentable");
        let end = review_line_target_for_line(&file, 0, 2, &diff.hunks[0].lines[2])
            .expect("added line should be commentable");

        let range = review_range_from_targets(&start, &end).unwrap();

        assert_eq!(range.path, "src/lib.rs");
        assert_eq!(range.side, ReviewSide::Right);
        assert_eq!(range.start_line, Some(2));
        assert_eq!(range.start_side, Some(ReviewSide::Right));
        assert_eq!(range.line, 3);
    }

    #[test]
    fn labels_review_comment_anchor_with_path_and_diff_side() {
        let range = ReviewCommentRange {
            path: "src/lib.rs".to_string(),
            line: 42,
            side: ReviewSide::Right,
            start_line: Some(40),
            start_side: Some(ReviewSide::Right),
        };

        assert_eq!(
            review_comment_range_label(&range),
            "new lines 40-42 in src/lib.rs"
        );
    }

    #[test]
    fn rejects_mixed_side_review_range() {
        let file = test_file("src/lib.rs");
        let diff = parse_unified_diff("@@ -1 +1 @@\n-old\n+new\n");
        let left = review_line_target_for_line(&file, 0, 0, &diff.hunks[0].lines[0])
            .expect("removed line should be commentable");
        let right = review_line_target_for_line(&file, 0, 1, &diff.hunks[0].lines[1])
            .expect("added line should be commentable");

        let error =
            review_range_from_targets(&left, &right).expect_err("mixed side selection should fail");

        assert_eq!(error, "Review comments can only span one diff side");
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
}
