use harbor_domain::PullRequestReview;

use crate::date_time::{full_time_label, natural_time_label};

mod diff_preview;

#[cfg(test)]
use diff_preview::ReviewDiffPreviewLine;
pub(crate) use diff_preview::{
    ReviewDiffPreview, render_review_diff_preview, review_thread_diff_preview,
    review_thread_suggestion_context,
};

pub(crate) fn review_time_label(review: &PullRequestReview) -> String {
    review
        .submitted_at
        .map(natural_time_label)
        .unwrap_or_else(|| "not submitted".to_string())
}

pub(crate) fn review_time_tooltip(review: &PullRequestReview) -> Option<String> {
    review.submitted_at.map(full_time_label)
}

#[cfg(test)]
mod tests;
