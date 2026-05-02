use harbor_domain::DiffFile;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParsedDiff {
    pub hunks: Vec<DiffHunk>,
}

impl ParsedDiff {
    pub fn is_empty(&self) -> bool {
        self.hunks.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffHunk {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
    Metadata,
}

pub fn parse_files(files: &[DiffFile]) -> Vec<Option<ParsedDiff>> {
    files
        .iter()
        .map(|file| file.patch.as_deref().map(parse_unified_diff))
        .collect()
}

pub fn parse_unified_diff(patch: &str) -> ParsedDiff {
    let mut hunks = Vec::new();
    let mut current_hunk: Option<DiffHunk> = None;
    let mut old_line = 0;
    let mut new_line = 0;

    for raw_line in patch.lines() {
        if let Some((old_start, old_lines, new_start, new_lines)) = parse_hunk_header(raw_line) {
            if let Some(hunk) = current_hunk.take() {
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
            });
            new_line += 1;
        } else if let Some(text) = raw_line.strip_prefix('-') {
            hunk.lines.push(DiffLine {
                kind: DiffLineKind::Removed,
                old_line: Some(old_line),
                new_line: None,
                text: text.to_string(),
            });
            old_line += 1;
        } else if let Some(text) = raw_line.strip_prefix(' ') {
            hunk.lines.push(DiffLine {
                kind: DiffLineKind::Context,
                old_line: Some(old_line),
                new_line: Some(new_line),
                text: text.to_string(),
            });
            old_line += 1;
            new_line += 1;
        } else {
            hunk.lines.push(DiffLine {
                kind: DiffLineKind::Metadata,
                old_line: None,
                new_line: None,
                text: raw_line.to_string(),
            });
        }
    }

    if let Some(hunk) = current_hunk {
        hunks.push(hunk);
    }

    ParsedDiff { hunks }
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
    use harbor_domain::{DiffFile, FileStatus};

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
    fn ignores_file_headers_before_first_hunk() {
        let parsed = parse_unified_diff(
            "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-x\n+y\n",
        );

        assert_eq!(parsed.hunks.len(), 1);
        assert_eq!(parsed.hunks[0].lines.len(), 2);
    }

    #[test]
    fn parses_only_files_with_patches() {
        let files = vec![
            DiffFile {
                path: "src/lib.rs".to_string(),
                previous_path: None,
                status: FileStatus::Modified,
                additions: 1,
                deletions: 1,
                changes: 2,
                patch: Some("@@ -1 +1 @@\n-old\n+new\n".to_string()),
            },
            DiffFile {
                path: "assets/logo.png".to_string(),
                previous_path: None,
                status: FileStatus::Modified,
                additions: 0,
                deletions: 0,
                changes: 0,
                patch: None,
            },
        ];

        let parsed = parse_files(&files);

        assert_eq!(parsed.len(), 2);
        assert!(parsed[0].is_some());
        assert!(parsed[1].is_none());
    }
}
