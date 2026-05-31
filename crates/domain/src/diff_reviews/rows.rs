use crate::{ReviewComment, ReviewThread};

pub const REVIEW_THREAD_INLINE_ROWS: usize = 6;
pub(super) const REVIEW_THREAD_ROWS_PER_ADDITIONAL_COMMENT: usize = 4;
pub(super) const REVIEW_THREAD_EMPTY_INLINE_ROWS: usize = 4;
pub(super) const REVIEW_COMMENT_ROWS_PER_ADDITIONAL_BODY_LINE: usize = 1;

pub fn review_thread_inline_rows(thread: &ReviewThread) -> usize {
    if thread.comments.is_empty() {
        return REVIEW_THREAD_EMPTY_INLINE_ROWS;
    }

    REVIEW_THREAD_INLINE_ROWS
        + thread.comments.len().saturating_sub(1) * REVIEW_THREAD_ROWS_PER_ADDITIONAL_COMMENT
        + thread
            .comments
            .iter()
            .map(review_comment_additional_body_rows)
            .sum::<usize>()
}

fn review_comment_additional_body_rows(comment: &ReviewComment) -> usize {
    review_comment_body_row_count(&comment.body).saturating_sub(1)
        * REVIEW_COMMENT_ROWS_PER_ADDITIONAL_BODY_LINE
}

pub fn review_comment_body_row_count(body: &str) -> usize {
    let mut row_count = 0;
    let mut in_code_block = false;
    let mut saw_rendered_line = false;
    let mut pending_blank_row = false;

    for line in body.lines() {
        let trimmed = line.trim();

        if is_markdown_code_fence(trimmed) {
            if in_code_block {
                in_code_block = false;
            } else {
                if saw_rendered_line && pending_blank_row {
                    row_count += 1;
                }
                row_count += 1;
                saw_rendered_line = true;
                pending_blank_row = false;
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            row_count += 1;
            saw_rendered_line = true;
            continue;
        }

        if trimmed.is_empty() {
            if saw_rendered_line {
                pending_blank_row = true;
            }
            continue;
        }

        if is_markdown_table_separator(trimmed) {
            pending_blank_row = false;
            continue;
        }

        if saw_rendered_line && pending_blank_row {
            row_count += 1;
        }

        row_count += 1;
        saw_rendered_line = true;
        pending_blank_row = false;
    }

    row_count.max(1)
}

fn is_markdown_code_fence(line: &str) -> bool {
    line.starts_with("```") || line.starts_with("~~~")
}

fn is_markdown_table_separator(line: &str) -> bool {
    let mut cells = line.trim_matches('|').split('|');
    let Some(first_cell) = cells.next() else {
        return false;
    };

    markdown_table_separator_cell(first_cell)
        && cells.count() > 0
        && line
            .trim_matches('|')
            .split('|')
            .all(markdown_table_separator_cell)
}

fn markdown_table_separator_cell(cell: &str) -> bool {
    let cell = cell.trim();

    !cell.is_empty()
        && cell.contains('-')
        && cell.chars().all(|character| matches!(character, '-' | ':'))
}
