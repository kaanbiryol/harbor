use std::ops::Range;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParsedDiff<T = ()> {
    pub hunks: Vec<DiffHunk<T>>,
}

impl<T> ParsedDiff<T> {
    pub fn is_empty(&self) -> bool {
        self.hunks.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffHunk<T = ()> {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine<T>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffLine<T = ()> {
    pub kind: DiffLineKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: String,
    pub syntax_highlights: Vec<(Range<usize>, T)>,
    /// Byte range of the text that differs from the paired removed/added line.
    pub changed_range: Option<Range<usize>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
    Metadata,
}

pub fn parse_unified_diff(patch: &str) -> ParsedDiff {
    parse_unified_diff_with_syntax_payload(patch)
}

pub fn parse_unified_diff_with_syntax_payload<T>(patch: &str) -> ParsedDiff<T> {
    let mut hunks = Vec::new();
    let mut current_hunk: Option<DiffHunk<T>> = None;
    let mut old_line = 0;
    let mut new_line = 0;

    for raw_line in patch.lines() {
        if let Some((old_start, old_lines, new_start, new_lines)) = parse_hunk_header(raw_line) {
            if let Some(mut hunk) = current_hunk.take() {
                mark_changed_ranges(&mut hunk.lines);
                hunks.push(hunk);
            }

            current_hunk = Some(DiffHunk {
                header: raw_line.to_string(),
                old_start,
                old_lines,
                new_start,
                new_lines,
                lines: Vec::new(),
            });
            old_line = old_start;
            new_line = new_start;
            continue;
        }

        let Some(hunk) = current_hunk.as_mut() else {
            continue;
        };

        if let Some(text) = raw_line.strip_prefix('+') {
            hunk.lines.push(DiffLine {
                kind: DiffLineKind::Added,
                old_line: None,
                new_line: Some(new_line),
                text: text.to_string(),
                syntax_highlights: Vec::new(),
                changed_range: None,
            });
            new_line += 1;
        } else if let Some(text) = raw_line.strip_prefix('-') {
            hunk.lines.push(DiffLine {
                kind: DiffLineKind::Removed,
                old_line: Some(old_line),
                new_line: None,
                text: text.to_string(),
                syntax_highlights: Vec::new(),
                changed_range: None,
            });
            old_line += 1;
        } else if let Some(text) = raw_line.strip_prefix(' ') {
            hunk.lines.push(DiffLine {
                kind: DiffLineKind::Context,
                old_line: Some(old_line),
                new_line: Some(new_line),
                text: text.to_string(),
                syntax_highlights: Vec::new(),
                changed_range: None,
            });
            old_line += 1;
            new_line += 1;
        } else {
            hunk.lines.push(DiffLine {
                kind: DiffLineKind::Metadata,
                old_line: None,
                new_line: None,
                text: raw_line.to_string(),
                syntax_highlights: Vec::new(),
                changed_range: None,
            });
        }
    }

    if let Some(mut hunk) = current_hunk {
        mark_changed_ranges(&mut hunk.lines);
        hunks.push(hunk);
    }

    ParsedDiff { hunks }
}

/// Pairs each run of removed lines with the added lines that follow it and
/// marks the differing span on both sides, so edits within a line stand out.
fn mark_changed_ranges<T>(lines: &mut [DiffLine<T>]) {
    let mut index = 0;
    while index < lines.len() {
        let removed_start = index;
        while lines
            .get(index)
            .is_some_and(|line| line.kind == DiffLineKind::Removed)
        {
            index += 1;
        }
        let added_start = index;
        while lines
            .get(index)
            .is_some_and(|line| line.kind == DiffLineKind::Added)
        {
            index += 1;
        }
        if index == removed_start {
            index += 1;
            continue;
        }

        let pair_count = (added_start - removed_start).min(index - added_start);
        for offset in 0..pair_count {
            let removed_index = removed_start + offset;
            let added_index = added_start + offset;
            if let Some((removed_range, added_range)) =
                changed_ranges(&lines[removed_index].text, &lines[added_index].text)
            {
                lines[removed_index].changed_range = Some(removed_range);
                lines[added_index].changed_range = Some(added_range);
            }
        }
    }
}

/// Returns the differing middle of two lines after trimming their common
/// prefix and suffix, widened to word boundaries. Returns `None` when the
/// lines share nothing, since highlighting the whole line adds no signal.
fn changed_ranges(old: &str, new: &str) -> Option<(Range<usize>, Range<usize>)> {
    fn is_word(character: char) -> bool {
        character.is_alphanumeric() || character == '_'
    }
    fn starts_with_word(text: &str) -> bool {
        text.chars().next().is_some_and(is_word)
    }
    fn ends_with_word(text: &str) -> bool {
        text.chars().next_back().is_some_and(is_word)
    }

    let mut prefix = old
        .char_indices()
        .zip(new.chars())
        .find(|((_, old_char), new_char)| old_char != new_char)
        .map_or(old.len().min(new.len()), |((index, _), _)| index);
    if ends_with_word(&old[..prefix])
        && (starts_with_word(&old[prefix..]) || starts_with_word(&new[prefix..]))
    {
        prefix = old[..prefix]
            .char_indices()
            .rev()
            .find(|(_, character)| !is_word(*character))
            .map_or(0, |(index, character)| index + character.len_utf8());
    }

    let old_rest = &old[prefix..];
    let new_rest = &new[prefix..];
    let mut suffix = old_rest
        .chars()
        .rev()
        .zip(new_rest.chars().rev())
        .take_while(|(old_char, new_char)| old_char == new_char)
        .map(|(character, _)| character.len_utf8())
        .sum::<usize>();
    let old_end = old.len() - suffix;
    let new_end = new.len() - suffix;
    if starts_with_word(&old[old_end..])
        && (ends_with_word(&old[prefix..old_end]) || ends_with_word(&new[prefix..new_end]))
    {
        let common = &old[old_end..];
        suffix = common
            .char_indices()
            .find(|(_, character)| !is_word(*character))
            .map_or(0, |(index, _)| common.len() - index);
    }

    if prefix + suffix == 0 {
        return None;
    }
    Some((prefix..old.len() - suffix, prefix..new.len() - suffix))
}

fn parse_hunk_header(line: &str) -> Option<(u32, u32, u32, u32)> {
    let line = line.strip_prefix("@@ ")?;
    let (range_text, _) = line.split_once(" @@")?;
    let mut ranges = range_text.split_whitespace();
    let old_range = ranges.next()?;
    let new_range = ranges.next()?;

    if ranges.next().is_some() {
        return None;
    }

    let (old_start, old_lines) = parse_range(old_range, '-')?;
    let (new_start, new_lines) = parse_range(new_range, '+')?;

    Some((old_start, old_lines, new_start, new_lines))
}

fn parse_range(range: &str, prefix: char) -> Option<(u32, u32)> {
    let range = range.strip_prefix(prefix)?;
    let (start, lines) = range
        .split_once(',')
        .map_or((range, "1"), |(start, lines)| (start, lines));

    Some((start.parse().ok()?, lines.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hunks_and_line_numbers() {
        let parsed = parse_unified_diff(
            "@@ -1,3 +1,4 @@ fn main\n context\n-old\n+new\n+extra\n unchanged\n",
        );

        assert_eq!(parsed.hunks.len(), 1);
        let hunk = &parsed.hunks[0];
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.old_lines, 3);
        assert_eq!(hunk.new_start, 1);
        assert_eq!(hunk.new_lines, 4);
        assert_eq!(hunk.lines[0].kind, DiffLineKind::Context);
        assert_eq!(hunk.lines[0].old_line, Some(1));
        assert_eq!(hunk.lines[0].new_line, Some(1));
        assert_eq!(hunk.lines[1].kind, DiffLineKind::Removed);
        assert_eq!(hunk.lines[1].old_line, Some(2));
        assert_eq!(hunk.lines[1].new_line, None);
        assert_eq!(hunk.lines[2].kind, DiffLineKind::Added);
        assert_eq!(hunk.lines[2].old_line, None);
        assert_eq!(hunk.lines[2].new_line, Some(2));
    }

    #[test]
    fn parses_single_line_ranges() {
        let parsed = parse_unified_diff("@@ -10 +20 @@\n-old\n+new\n");

        assert_eq!(parsed.hunks.len(), 1);
        assert_eq!(parsed.hunks[0].old_start, 10);
        assert_eq!(parsed.hunks[0].old_lines, 1);
        assert_eq!(parsed.hunks[0].new_start, 20);
        assert_eq!(parsed.hunks[0].new_lines, 1);
    }

    #[test]
    fn marks_changed_words_within_paired_lines() {
        let parsed = parse_unified_diff(
            "@@ -1,3 +1,3 @@\n fixture: true\n-mode: base\n+mode: review-inbox-large\n",
        );
        let lines = &parsed.hunks[0].lines;

        assert_eq!(lines[0].changed_range, None);
        assert_eq!(lines[1].changed_range, Some(6..10));
        assert_eq!(lines[2].changed_range, Some(6..24));
    }

    #[test]
    fn widens_changed_ranges_to_word_boundaries() {
        assert_eq!(
            changed_ranges("let foo_bar = 1;", "let foo_baz = 1;"),
            Some((4..11, 4..11))
        );
        assert_eq!(
            changed_ranges("call(a, b)", "call(a, b, c)"),
            Some((9..9, 9..12))
        );
    }

    #[test]
    fn skips_changed_ranges_for_unrelated_or_unpaired_lines() {
        assert_eq!(changed_ranges("alpha", "omega"), None);

        let parsed = parse_unified_diff("@@ -1,2 +1,1 @@\n-first\n-second line\n+second\n");
        let lines = &parsed.hunks[0].lines;

        assert_eq!(lines[0].changed_range, None);
        assert_eq!(lines[1].changed_range, None);
        assert_eq!(lines[2].changed_range, None);
    }

    #[test]
    fn handles_multibyte_characters_in_changed_ranges() {
        assert_eq!(
            changed_ranges("name: café", "name: cafés"),
            Some((6..11, 6..12))
        );
    }

    #[test]
    fn ignores_file_headers_before_first_hunk() {
        let parsed = parse_unified_diff(
            "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-x\n+y\n",
        );

        assert_eq!(parsed.hunks.len(), 1);
        assert_eq!(parsed.hunks[0].lines.len(), 2);
    }
}
