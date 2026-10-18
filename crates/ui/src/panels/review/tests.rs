use harbor_domain::{
    DiffFile, FileStatus, FileViewedState, ReviewCommentRange, ReviewSide, ReviewThread,
    ReviewThreadState,
};

use super::*;
use crate::{
    diff::ParsedDiff,
    panels::review_markdown::{ReviewSuggestionContext, ReviewSuggestionOriginalLine},
    test_fixtures::review_thread as test_review_thread,
    visual::Tone,
};

#[test]
fn builds_diff_preview_for_inline_review_comments() {
    let mut thread = review_thread();
    let comment = thread
        .comments
        .first_mut()
        .expect("test thread should have a comment");
    comment.author = "alex".to_string();
    comment.body = "Please tighten this branch.\n\n```suggestion\nreplacement\n```".to_string();
    let position = comment
        .position
        .as_mut()
        .expect("test comment should have a position");
    position.line = Some(11);
    position.original_line = None;
    let (files, diffs) = review_diff_fixture();

    assert_eq!(
        review_thread_diff_preview(&thread, &files, &diffs),
        Some(ReviewDiffPreview {
            lines: vec![ReviewDiffPreviewLine {
                line: Some(11),
                marker: "+",
                text: "Please tighten this branch.".to_string(),
                tone: Tone::Success,
            }],
            suggestion_context: Some(ReviewSuggestionContext {
                original_lines: vec![ReviewSuggestionOriginalLine {
                    line_number: 11,
                    text: "Please tighten this branch.".to_string(),
                }],
                replacement_start_line: 11,
            }),
        })
    );
}

#[test]
fn builds_diff_preview_for_selected_review_ranges() {
    let mut thread = review_thread();
    thread
        .comments
        .first_mut()
        .expect("test thread should have a comment")
        .body = "```suggestion\nreplacement\n```".to_string();
    thread.range = Some(ReviewCommentRange {
        path: "src/lib.rs".to_string(),
        line: 12,
        side: ReviewSide::Right,
        start_line: Some(11),
        start_side: Some(ReviewSide::Right),
    });
    let (files, diffs) = review_diff_fixture();

    assert_eq!(
        review_thread_diff_preview(&thread, &files, &diffs),
        Some(ReviewDiffPreview {
            lines: vec![
                ReviewDiffPreviewLine {
                    line: Some(11),
                    marker: "+",
                    text: "Please tighten this branch.".to_string(),
                    tone: Tone::Success,
                },
                ReviewDiffPreviewLine {
                    line: Some(12),
                    marker: "+",
                    text: "Also cover this selected line.".to_string(),
                    tone: Tone::Success,
                },
            ],
            suggestion_context: Some(ReviewSuggestionContext {
                original_lines: vec![
                    ReviewSuggestionOriginalLine {
                        line_number: 11,
                        text: "Please tighten this branch.".to_string(),
                    },
                    ReviewSuggestionOriginalLine {
                        line_number: 12,
                        text: "Also cover this selected line.".to_string(),
                    },
                ],
                replacement_start_line: 11,
            }),
        })
    );
}

fn review_thread() -> ReviewThread {
    test_review_thread(ReviewThreadState::Unresolved)
}

fn review_diff_fixture() -> (Vec<DiffFile>, Vec<Option<ParsedDiff>>) {
    let file = DiffFile {
        path: "src/lib.rs".to_string(),
        previous_path: None,
        status: FileStatus::Modified,
        additions: 1,
        deletions: 0,
        changes: 1,
        patch: None,
        viewed_state: FileViewedState::Unviewed,
    };
    let diff = crate::diff::parse_unified_diff(
        "@@ -10,2 +10,4 @@\n context\n+Please tighten this branch.\n+Also cover this selected line.\n unchanged\n",
    );

    (vec![file], vec![Some(diff)])
}
