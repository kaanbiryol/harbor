use harbor_domain::{ReviewCommentRange, ReviewSide};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewLineTarget {
    pub(crate) hunk_index: usize,
    pub(crate) line_index: usize,
    pub(crate) range: ReviewCommentRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewComposer {
    pub(crate) anchor: ReviewLineTarget,
    pub(crate) range: ReviewCommentRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewLineSelection {
    pub(crate) anchor: ReviewLineTarget,
    pub(crate) current: ReviewLineTarget,
}

pub(crate) fn review_composer_from_selection(
    anchor: &ReviewLineTarget,
    current: &ReviewLineTarget,
) -> std::result::Result<ReviewComposer, String> {
    let range = review_range_from_targets(anchor, current)?;
    let anchor = if anchor.line_index >= current.line_index {
        anchor.clone()
    } else {
        current.clone()
    };

    Ok(ReviewComposer { anchor, range })
}

pub(crate) fn review_range_from_targets(
    anchor: &ReviewLineTarget,
    current: &ReviewLineTarget,
) -> std::result::Result<ReviewCommentRange, String> {
    if anchor.hunk_index != current.hunk_index {
        return Err("Review comments can only span lines in one diff hunk".to_string());
    }

    if anchor.range.path != current.range.path {
        return Err("Review comments can only span one file".to_string());
    }

    if anchor.range.side != current.range.side {
        return Err("Review comments can only span one diff side".to_string());
    }

    let (start, end) = if anchor.line_index <= current.line_index {
        (anchor, current)
    } else {
        (current, anchor)
    };
    let mut range = end.range.clone();

    if start.line_index != end.line_index {
        range.start_line = Some(start.range.line);
        range.start_side = Some(start.range.side);
    } else {
        range.start_line = None;
        range.start_side = None;
    }

    Ok(range)
}

pub(crate) fn review_comment_range_label(range: &ReviewCommentRange) -> String {
    let side = match range.side {
        ReviewSide::Left => "left",
        ReviewSide::Right => "right",
    };

    if let Some(start_line) = range.start_line {
        format!("{side} lines {start_line}-{}", range.line)
    } else {
        format!("{side} line {}", range.line)
    }
}
