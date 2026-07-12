use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use harbor_domain::{
    PullRequestComment, PullRequestReview, ReviewComment, ReviewThread, ReviewThreadState,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReviewPanelSection {
    NeedsAttention,
    Conversation,
    Resolved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ReviewPanelItem {
    Section {
        section: ReviewPanelSection,
        item_count: usize,
    },
    FileHeader {
        path: String,
        thread_count: usize,
    },
    Thread {
        thread_id: String,
    },
    Review {
        review_id: String,
    },
    Comment {
        comment_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReviewConversationItem {
    pub(super) kind: ReviewConversationItemKind,
    sort_time: Option<DateTime<Utc>>,
    sequence: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ReviewConversationItemKind {
    Review { review_id: String },
    Comment { comment_id: String },
}

pub(super) fn review_panel_items(
    reviews: &[PullRequestReview],
    threads: &[ReviewThread],
    comments: &[PullRequestComment],
) -> Vec<ReviewPanelItem> {
    let mut items = Vec::new();
    append_review_thread_section(
        &mut items,
        ReviewPanelSection::NeedsAttention,
        threads
            .iter()
            .filter(|thread| thread.state == ReviewThreadState::Unresolved),
    );

    let conversation_items = review_conversation_items(reviews, threads, comments);
    if !conversation_items.is_empty() {
        items.push(ReviewPanelItem::Section {
            section: ReviewPanelSection::Conversation,
            item_count: conversation_items.len(),
        });
        items.extend(conversation_items.into_iter().map(|item| match item.kind {
            ReviewConversationItemKind::Review { review_id } => {
                ReviewPanelItem::Review { review_id }
            }
            ReviewConversationItemKind::Comment { comment_id } => {
                ReviewPanelItem::Comment { comment_id }
            }
        }));
    }

    append_review_thread_section(
        &mut items,
        ReviewPanelSection::Resolved,
        threads
            .iter()
            .filter(|thread| thread.state != ReviewThreadState::Unresolved),
    );

    items
}

pub(super) fn review_content_item_count(items: &[ReviewPanelItem]) -> usize {
    items
        .iter()
        .filter(|item| {
            matches!(
                item,
                ReviewPanelItem::Thread { .. }
                    | ReviewPanelItem::Review { .. }
                    | ReviewPanelItem::Comment { .. }
            )
        })
        .count()
}

pub(super) fn review_body_summary(body: &str) -> Option<String> {
    body.lines()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
}

fn append_review_thread_section<'a>(
    items: &mut Vec<ReviewPanelItem>,
    section: ReviewPanelSection,
    threads: impl Iterator<Item = &'a ReviewThread>,
) {
    let mut threads = threads.collect::<Vec<_>>();
    if threads.is_empty() {
        return;
    }
    threads.sort_by(compare_review_threads);

    items.push(ReviewPanelItem::Section {
        section,
        item_count: threads.len(),
    });

    let mut start = 0;
    while start < threads.len() {
        let path = &threads[start].path;
        let end = threads[start..]
            .iter()
            .position(|thread| thread.path != *path)
            .map_or(threads.len(), |offset| start + offset);
        items.push(ReviewPanelItem::FileHeader {
            path: path.clone(),
            thread_count: end - start,
        });
        items.extend(
            threads[start..end]
                .iter()
                .map(|thread| ReviewPanelItem::Thread {
                    thread_id: thread.id.clone(),
                }),
        );
        start = end;
    }
}

fn compare_review_threads(left: &&ReviewThread, right: &&ReviewThread) -> Ordering {
    left.path
        .cmp(&right.path)
        .then_with(|| {
            compare_review_thread_lines(review_thread_line(left), review_thread_line(right))
        })
        .then_with(|| left.id.cmp(&right.id))
}

fn compare_review_thread_lines(left: Option<u32>, right: Option<u32>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn review_thread_line(thread: &ReviewThread) -> Option<u32> {
    thread
        .range
        .as_ref()
        .map(|range| range.start_line.unwrap_or(range.line))
        .or_else(|| {
            thread.comments.iter().find_map(|comment| {
                comment
                    .position
                    .as_ref()
                    .and_then(|position| position.line.or(position.original_line))
            })
        })
}

pub(super) fn review_conversation_items(
    reviews: &[PullRequestReview],
    threads: &[ReviewThread],
    comments: &[PullRequestComment],
) -> Vec<ReviewConversationItem> {
    let mut items = Vec::new();
    let mut sequence = 0;

    for review in reviews {
        let has_summary = review
            .body
            .as_deref()
            .and_then(review_body_summary)
            .is_some();
        if !has_summary && review_has_inline_comment(review, threads) {
            continue;
        }

        items.push(ReviewConversationItem {
            kind: ReviewConversationItemKind::Review {
                review_id: review.id.clone(),
            },
            sort_time: review.submitted_at,
            sequence,
        });
        sequence += 1;
    }

    for comment in comments {
        items.push(ReviewConversationItem {
            kind: ReviewConversationItemKind::Comment {
                comment_id: comment.id.clone(),
            },
            sort_time: Some(comment.created_at),
            sequence,
        });
        sequence += 1;
    }

    items.sort_by(compare_review_conversation_items);
    items
}

fn compare_review_conversation_items(
    left: &ReviewConversationItem,
    right: &ReviewConversationItem,
) -> Ordering {
    match (left.sort_time.as_ref(), right.sort_time.as_ref()) {
        (Some(left_time), Some(right_time)) => left_time.cmp(right_time),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
    .then_with(|| left.sequence.cmp(&right.sequence))
}

fn review_has_inline_comment(review: &PullRequestReview, threads: &[ReviewThread]) -> bool {
    threads
        .iter()
        .flat_map(|thread| thread.comments.iter())
        .any(|comment| review_matches_comment(review, comment))
}

fn review_matches_comment(review: &PullRequestReview, comment: &ReviewComment) -> bool {
    comment
        .pull_request_review_id
        .as_deref()
        .is_some_and(|review_id| review_id == review.id)
        || review
            .node_id
            .as_deref()
            .is_some_and(|node_id| comment.pull_request_review_node_id.as_deref() == Some(node_id))
}
