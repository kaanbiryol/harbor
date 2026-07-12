use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use gpui::{ListOffset, px};
use harbor_domain::{PullRequestComment, PullRequestReview, PullRequestReviewState, ReviewThread};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum OverviewPanelItem {
    Description,
    Comment { id: String },
    Review { id: String },
    Thread { id: String },
    Message(OverviewTimelineMessage),
    Composer,
}

impl OverviewPanelItem {
    pub(super) fn key(&self) -> String {
        match self {
            Self::Description => "description".to_string(),
            Self::Comment { id } => format!("comment:{id}"),
            Self::Review { id } => format!("review:{id}"),
            Self::Thread { id } => format!("thread:{id}"),
            Self::Message(OverviewTimelineMessage::Loading) => "message:loading".to_string(),
            Self::Message(OverviewTimelineMessage::Empty) => "message:empty".to_string(),
            Self::Message(OverviewTimelineMessage::Error(_)) => "message:error".to_string(),
            Self::Composer => "composer".to_string(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum OverviewTimelineMessage {
    Loading,
    Empty,
    Error(String),
}

#[derive(Clone, Copy)]
pub(super) enum OverviewTimelineItem<'a> {
    Comment(&'a PullRequestComment),
    Review(&'a PullRequestReview),
    Thread(&'a ReviewThread),
}

impl OverviewTimelineItem<'_> {
    fn time(self) -> Option<DateTime<Utc>> {
        match self {
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

pub(super) fn sync_overview_list_items(
    list_state: &gpui::ListState,
    previous_keys: &mut Vec<String>,
    next_keys: Vec<String>,
) {
    let scroll_top = list_state.logical_scroll_top();
    let was_at_top = scroll_top.item_ix == 0 && scroll_top.offset_in_item == px(0.0);
    let current_item_count = list_state.item_count();
    if current_item_count != previous_keys.len() {
        if current_item_count == 0 {
            list_state.reset(next_keys.len());
        } else {
            list_state.splice(0..current_item_count, next_keys.len());
        }
        if was_at_top {
            list_state.scroll_to(ListOffset {
                item_ix: 0,
                offset_in_item: px(0.0),
            });
        }
        *previous_keys = next_keys;
        return;
    }

    if previous_keys == &next_keys {
        return;
    }

    let prefix_len = previous_keys
        .iter()
        .zip(&next_keys)
        .take_while(|(previous, next)| previous == next)
        .count();
    let mut previous_suffix_start = previous_keys.len();
    let mut next_suffix_start = next_keys.len();
    while previous_suffix_start > prefix_len
        && next_suffix_start > prefix_len
        && previous_keys[previous_suffix_start - 1] == next_keys[next_suffix_start - 1]
    {
        previous_suffix_start -= 1;
        next_suffix_start -= 1;
    }

    list_state.splice(
        prefix_len..previous_suffix_start,
        next_suffix_start - prefix_len,
    );
    if was_at_top {
        list_state.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: px(0.0),
        });
    }
    *previous_keys = next_keys;
}

pub(super) fn overview_thread_item_index(
    items: &[OverviewPanelItem],
    thread_id: &str,
) -> Option<usize> {
    items
        .iter()
        .position(|item| matches!(item, OverviewPanelItem::Thread { id } if id == thread_id))
}

pub(super) fn overview_panel_items(
    reviews: &[PullRequestReview],
    comments: &[PullRequestComment],
    threads: &[ReviewThread],
    loading: bool,
    error: Option<&str>,
) -> Vec<OverviewPanelItem> {
    let timeline_items = overview_timeline_items(reviews, comments, threads);
    let mut items = Vec::with_capacity(timeline_items.len() + 3);
    items.push(OverviewPanelItem::Description);

    if let Some(error) = error {
        items.push(OverviewPanelItem::Message(OverviewTimelineMessage::Error(
            error.to_string(),
        )));
    }

    if timeline_items.is_empty() && error.is_none() {
        items.push(OverviewPanelItem::Message(if loading {
            OverviewTimelineMessage::Loading
        } else {
            OverviewTimelineMessage::Empty
        }));
    } else {
        items.extend(timeline_items.into_iter().map(|item| match item {
            OverviewTimelineItem::Comment(comment) => OverviewPanelItem::Comment {
                id: comment.id.clone(),
            },
            OverviewTimelineItem::Review(review) => OverviewPanelItem::Review {
                id: review.id.clone(),
            },
            OverviewTimelineItem::Thread(thread) => OverviewPanelItem::Thread {
                id: thread.id.clone(),
            },
        }));
    }

    items.push(OverviewPanelItem::Composer);
    items
}

pub(super) fn overview_timeline_items<'a>(
    reviews: &'a [PullRequestReview],
    comments: &'a [PullRequestComment],
    threads: &'a [ReviewThread],
) -> Vec<OverviewTimelineItem<'a>> {
    let mut items = Vec::with_capacity(reviews.len() + comments.len() + threads.len());
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
    items.sort_by(|left, right| compare_timeline_times(left.time(), right.time()));
    items
}

pub(super) fn overview_review_visible(review: &PullRequestReview) -> bool {
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

fn compare_timeline_times(left: Option<DateTime<Utc>>, right: Option<DateTime<Utc>>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}
