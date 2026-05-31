use std::{collections::HashSet, ops::Range};

use gpui::ListState;
use harbor_domain::{DiffFile, ReviewThread};

use crate::{
    diff::ParsedDiff,
    diff_reviews::{anchored_review_threads, review_threads_for_line},
    workspace::ReviewComposer,
};

use super::{
    LINE_NUMBER_DIGIT_WIDTH, LINE_NUMBER_PADDING, MIN_LINE_NUMBER_WIDTH,
    inline_review_layout::review_comment_range_matches_file,
};

#[derive(Clone, Copy)]
pub(crate) struct ContinuousDiffLayoutInput<'a> {
    pub(crate) files: &'a [DiffFile],
    pub(crate) diffs: &'a [Option<ParsedDiff>],
    pub(crate) visible_file_indices: &'a [usize],
    pub(crate) reviewed_file_paths: &'a HashSet<String>,
    pub(crate) review_threads: &'a [ReviewThread],
    pub(crate) review_composer: Option<&'a ReviewComposer>,
}

impl<'a> ContinuousDiffLayoutInput<'a> {
    fn review_controls(self) -> ReviewLayoutControls<'a> {
        ReviewLayoutControls {
            review_threads: self.review_threads,
            review_composer: self.review_composer,
        }
    }
}

#[derive(Clone, Copy)]
struct ReviewLayoutControls<'a> {
    review_threads: &'a [ReviewThread],
    review_composer: Option<&'a ReviewComposer>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DiffListItem {
    FileHeader {
        file_index: usize,
    },
    Hunk {
        file_index: usize,
        hunk_index: usize,
    },
    Line {
        file_index: usize,
        hunk_index: usize,
        line_index: usize,
    },
    ReviewComposer {
        file_index: usize,
        hunk_index: usize,
        line_index: usize,
    },
    ReviewThread {
        file_index: usize,
        hunk_index: usize,
        line_index: usize,
        thread_id: String,
    },
    DiffUnavailable {
        file_index: usize,
    },
}

pub(crate) fn continuous_diff_items(input: ContinuousDiffLayoutInput<'_>) -> Vec<DiffListItem> {
    let mut items = Vec::new();

    for file_index in input.visible_file_indices {
        let Some(file) = input.files.get(*file_index) else {
            continue;
        };

        items.push(DiffListItem::FileHeader {
            file_index: *file_index,
        });

        if file_is_reviewed(file, input.reviewed_file_paths) {
            continue;
        }

        let Some(diff) = parsed_diff_for_file(input.diffs, *file_index) else {
            items.push(DiffListItem::DiffUnavailable {
                file_index: *file_index,
            });
            continue;
        };

        diff_body_items(&mut items, diff, file, *file_index, input.review_controls());
    }

    items
}

pub(crate) fn sync_diff_list_state(
    list_state: &ListState,
    previous_items: &mut Vec<DiffListItem>,
    next_items: Vec<DiffListItem>,
) {
    if list_state.item_count() != previous_items.len() {
        let current_item_count = list_state.item_count();
        list_state.splice(0..current_item_count, next_items.len());
        *previous_items = next_items;
        return;
    }

    if previous_items == &next_items {
        return;
    }

    if let Some((old_range, inserted_item_count)) = diff_list_splice(previous_items, &next_items) {
        list_state.splice(old_range, inserted_item_count);
    }
    *previous_items = next_items;
}

fn diff_list_splice(
    previous_items: &[DiffListItem],
    next_items: &[DiffListItem],
) -> Option<(Range<usize>, usize)> {
    let prefix_len = previous_items
        .iter()
        .zip(next_items)
        .take_while(|(previous, next)| previous == next)
        .count();

    if prefix_len == previous_items.len() && prefix_len == next_items.len() {
        return None;
    }

    let mut previous_suffix_start = previous_items.len();
    let mut next_suffix_start = next_items.len();
    while previous_suffix_start > prefix_len
        && next_suffix_start > prefix_len
        && previous_items[previous_suffix_start - 1] == next_items[next_suffix_start - 1]
    {
        previous_suffix_start -= 1;
        next_suffix_start -= 1;
    }

    Some((
        prefix_len..previous_suffix_start,
        next_suffix_start - prefix_len,
    ))
}

pub(crate) fn continuous_diff_file_item_index(
    input: ContinuousDiffLayoutInput<'_>,
    target_file_index: usize,
) -> Option<usize> {
    continuous_diff_items(input).iter().position(|item| {
        matches!(item, DiffListItem::FileHeader { file_index } if *file_index == target_file_index)
    })
}

pub(crate) fn continuous_diff_hunk_item_index(
    input: ContinuousDiffLayoutInput<'_>,
    target_file_index: usize,
    target_hunk_index: usize,
) -> Option<usize> {
    continuous_diff_items(input).iter().position(|item| {
        matches!(
            item,
            DiffListItem::Hunk {
                file_index,
                hunk_index,
            } if *file_index == target_file_index && *hunk_index == target_hunk_index
        )
    })
}

pub(super) fn parsed_diff_for_file(
    diffs: &[Option<ParsedDiff>],
    file_index: usize,
) -> Option<&ParsedDiff> {
    diffs
        .get(file_index)
        .and_then(Option::as_ref)
        .filter(|diff| !diff.is_empty())
}

pub(super) fn file_is_reviewed(file: &DiffFile, reviewed_file_paths: &HashSet<String>) -> bool {
    reviewed_file_paths.contains(&file.path)
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DiffFileSection {
    pub(super) file_index: usize,
    pub(super) header_item_index: usize,
    pub(super) hunk_count: Option<usize>,
    pub(super) reviewed: bool,
}

pub(super) fn continuous_diff_section_for_item(
    input: ContinuousDiffLayoutInput<'_>,
    items: &[DiffListItem],
    target_item_index: usize,
) -> Option<DiffFileSection> {
    let target_item_index = target_item_index.min(items.len().checked_sub(1)?);

    for header_item_index in (0..=target_item_index).rev() {
        if let DiffListItem::FileHeader { file_index } = items[header_item_index] {
            let file = input.files.get(file_index)?;
            return Some(DiffFileSection {
                file_index,
                header_item_index,
                hunk_count: parsed_diff_for_file(input.diffs, file_index)
                    .map(|diff| diff.hunks.len()),
                reviewed: file_is_reviewed(file, input.reviewed_file_paths),
            });
        }
    }

    None
}

fn diff_body_items(
    items: &mut Vec<DiffListItem>,
    diff: &ParsedDiff,
    file: &DiffFile,
    file_index: usize,
    controls: ReviewLayoutControls<'_>,
) {
    let anchored_threads = anchored_review_threads(file, controls.review_threads);

    for (hunk_index, hunk) in diff.hunks.iter().enumerate() {
        items.push(DiffListItem::Hunk {
            file_index,
            hunk_index,
        });

        for (line_index, line) in hunk.lines.iter().enumerate() {
            items.push(DiffListItem::Line {
                file_index,
                hunk_index,
                line_index,
            });

            if controls.review_composer.is_some_and(|composer| {
                review_comment_range_matches_file(file, &composer.range)
                    && composer.anchor.hunk_index == hunk_index
                    && composer.anchor.line_index == line_index
            }) {
                items.push(DiffListItem::ReviewComposer {
                    file_index,
                    hunk_index,
                    line_index,
                });
            }

            for thread in review_threads_for_line(&anchored_threads, line) {
                items.push(DiffListItem::ReviewThread {
                    file_index,
                    hunk_index,
                    line_index,
                    thread_id: thread.id.clone(),
                });
            }
        }
    }
}

pub(super) fn line_number_width_for_diff(diff: &ParsedDiff) -> f32 {
    let max_line = diff
        .hunks
        .iter()
        .flat_map(|hunk| hunk.lines.iter())
        .flat_map(|line| [line.old_line, line.new_line])
        .flatten()
        .max()
        .unwrap_or(1);
    let digits = max_line.to_string().len() as f32;

    (digits * LINE_NUMBER_DIGIT_WIDTH + LINE_NUMBER_PADDING).max(MIN_LINE_NUMBER_WIDTH)
}

#[cfg(test)]
#[path = "layout/tests.rs"]
mod tests;
