use std::cmp::Ordering;

use chrono::{DateTime, Utc};

use crate::{
    PullRequestComment, PullRequestCommit, PullRequestReview, PullRequestReviewState, ReviewThread,
};

#[derive(Clone, Copy)]
pub enum OverviewTimelineItem<'a> {
    Commit(&'a PullRequestCommit),
    Comment(&'a PullRequestComment),
    Review(&'a PullRequestReview),
    Thread(&'a ReviewThread),
}

impl OverviewTimelineItem<'_> {
    fn time(self) -> Option<DateTime<Utc>> {
        match self {
            Self::Commit(commit) => commit.authored_at,
            Self::Comment(comment) => Some(comment.created_at),
            Self::Review(review) => review.submitted_at,
            Self::Thread(thread) => thread
                .comments
                .iter()
                .map(|comment| comment.created_at)
                .min(),
        }
    }
}

pub fn overview_timeline_items<'a>(
    commits: &'a [PullRequestCommit],
    reviews: &'a [PullRequestReview],
    comments: &'a [PullRequestComment],
    threads: &'a [ReviewThread],
) -> Vec<OverviewTimelineItem<'a>> {
    let mut items =
        Vec::with_capacity(commits.len() + reviews.len() + comments.len() + threads.len());
    items.extend(commits.iter().map(OverviewTimelineItem::Commit));
    items.extend(comments.iter().map(OverviewTimelineItem::Comment));
    items.extend(
        reviews
            .iter()
            .filter(|review| overview_review_visible(review))
            .map(OverviewTimelineItem::Review),
    );
    items.extend(
        threads
            .iter()
            .filter(|thread| !thread.comments.is_empty())
            .map(OverviewTimelineItem::Thread),
    );
    items.sort_by(|left, right| compare_present_first(left.time(), right.time()));
    items
}

fn overview_review_visible(review: &PullRequestReview) -> bool {
    match review.state {
        PullRequestReviewState::Pending => false,
        PullRequestReviewState::Commented => review
            .body
            .as_deref()
            .is_some_and(|body| !body.trim().is_empty()),
        PullRequestReviewState::Approved
        | PullRequestReviewState::ChangesRequested
        | PullRequestReviewState::Dismissed => true,
    }
}

fn compare_present_first<T: Ord>(left: Option<T>, right: Option<T>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests;
