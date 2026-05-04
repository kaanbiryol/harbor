use std::{ops::Range, path::Path, time::Duration};

use gpui::HighlightStyle;
use gpui_component::{
    highlighter::{HighlightTheme, LanguageRegistry, SyntaxHighlighter},
    input::Rope,
};
use harbor_domain::DiffFile;

const MAX_SYNTAX_PATCH_BYTES: usize = 512 * 1024;
const MAX_SYNTAX_LINE_BYTES: usize = 10_000;
const SYNTAX_PARSE_TIMEOUT: Duration = Duration::from_millis(50);

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
    pub syntax_highlights: Vec<(Range<usize>, HighlightStyle)>,
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

pub fn parse_files_with_syntax(
    files: &[DiffFile],
    highlight_theme: &HighlightTheme,
) -> Vec<Option<ParsedDiff>> {
    files
        .iter()
        .map(|file| {
            file.patch
                .as_deref()
                .map(|patch| parse_unified_diff_with_syntax(file, patch, highlight_theme))
        })
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
                syntax_highlights: Vec::new(),
            });
            new_line += 1;
        } else if let Some(text) = raw_line.strip_prefix('-') {
            hunk.lines.push(DiffLine {
                kind: DiffLineKind::Removed,
                old_line: Some(old_line),
                new_line: None,
                text: text.to_string(),
                syntax_highlights: Vec::new(),
            });
            old_line += 1;
        } else if let Some(text) = raw_line.strip_prefix(' ') {
            hunk.lines.push(DiffLine {
                kind: DiffLineKind::Context,
                old_line: Some(old_line),
                new_line: Some(new_line),
                text: text.to_string(),
                syntax_highlights: Vec::new(),
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
            });
        }
    }

    if let Some(hunk) = current_hunk {
        hunks.push(hunk);
    }

    ParsedDiff { hunks }
}

pub fn parse_unified_diff_with_syntax(
    file: &DiffFile,
    patch: &str,
    highlight_theme: &HighlightTheme,
) -> ParsedDiff {
    let mut diff = parse_unified_diff(patch);
    apply_syntax_highlighting(file, patch, &mut diff, highlight_theme);
    diff
}

fn apply_syntax_highlighting(
    file: &DiffFile,
    patch: &str,
    diff: &mut ParsedDiff,
    highlight_theme: &HighlightTheme,
) {
    for line in diff.hunks.iter_mut().flat_map(|hunk| hunk.lines.iter_mut()) {
        line.syntax_highlights.clear();
    }

    let Some(language) = language_for_file(file) else {
        return;
    };
    if LanguageRegistry::singleton().language(language).is_none() {
        return;
    }
    if patch.len() > MAX_SYNTAX_PATCH_BYTES || has_oversized_line(diff) {
        return;
    }

    apply_syntax_highlighting_for_side(diff, language, SyntaxSide::Old, highlight_theme);
    apply_syntax_highlighting_for_side(diff, language, SyntaxSide::New, highlight_theme);
}

fn apply_syntax_highlighting_for_side(
    diff: &mut ParsedDiff,
    language: &str,
    side: SyntaxSide,
    highlight_theme: &HighlightTheme,
) {
    let (document, line_ranges) = syntax_document_for_side(diff, side);
    if document.is_empty() {
        return;
    }

    let mut highlighter = SyntaxHighlighter::new(language);
    let rope = Rope::from_str(&document);
    if !highlighter.update(None, &rope, Some(SYNTAX_PARSE_TIMEOUT)) {
        return;
    }

    let styles = highlighter.styles(&(0..document.len()), highlight_theme);
    apply_document_styles(diff, &line_ranges, styles);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SyntaxSide {
    Old,
    New,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SyntaxLineRange {
    hunk_index: usize,
    line_index: usize,
    start: usize,
    end: usize,
    attach: bool,
}

fn syntax_document_for_side(diff: &ParsedDiff, side: SyntaxSide) -> (String, Vec<SyntaxLineRange>) {
    let mut document = String::new();
    let mut line_ranges = Vec::new();

    for (hunk_index, hunk) in diff.hunks.iter().enumerate() {
        for (line_index, line) in hunk.lines.iter().enumerate() {
            let (include, attach) = match (side, line.kind) {
                (SyntaxSide::Old, DiffLineKind::Removed) => (true, true),
                (SyntaxSide::Old, DiffLineKind::Context) => (true, false),
                (SyntaxSide::New, DiffLineKind::Added | DiffLineKind::Context) => (true, true),
                _ => (false, false),
            };

            if !include {
                continue;
            }

            let start = document.len();
            document.push_str(&line.text);
            let end = document.len();
            document.push('\n');
            line_ranges.push(SyntaxLineRange {
                hunk_index,
                line_index,
                start,
                end,
                attach,
            });
        }
    }

    (document, line_ranges)
}

fn apply_document_styles(
    diff: &mut ParsedDiff,
    line_ranges: &[SyntaxLineRange],
    styles: Vec<(Range<usize>, HighlightStyle)>,
) {
    let mut first_candidate_line = 0;

    for (range, style) in styles {
        if style == HighlightStyle::default() {
            continue;
        }

        while first_candidate_line < line_ranges.len()
            && line_ranges[first_candidate_line].end <= range.start
        {
            first_candidate_line += 1;
        }

        let mut line_index = first_candidate_line;
        while let Some(line_range) = line_ranges.get(line_index) {
            if line_range.start >= range.end {
                break;
            }

            if line_range.attach {
                let start = range.start.max(line_range.start);
                let end = range.end.min(line_range.end);
                if start < end {
                    diff.hunks[line_range.hunk_index].lines[line_range.line_index]
                        .syntax_highlights
                        .push((start - line_range.start..end - line_range.start, style));
                }
            }

            line_index += 1;
        }
    }
}

fn has_oversized_line(diff: &ParsedDiff) -> bool {
    diff.hunks
        .iter()
        .flat_map(|hunk| hunk.lines.iter())
        .any(|line| line.text.len() > MAX_SYNTAX_LINE_BYTES)
}

fn language_for_file(file: &DiffFile) -> Option<&'static str> {
    language_for_path(&file.path)
        .or_else(|| file.previous_path.as_deref().and_then(language_for_path))
}

fn language_for_path(path: &str) -> Option<&'static str> {
    let path = Path::new(path);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match file_name.as_str() {
        "cargo.lock" => return Some("toml"),
        "makefile" | "gnumakefile" => return Some("make"),
        _ => {}
    }

    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())?
        .to_ascii_lowercase();

    match extension.as_str() {
        "rs" => Some("rust"),
        "ts" | "mts" | "cts" => Some("typescript"),
        "tsx" => Some("tsx"),
        "js" | "mjs" | "cjs" | "jsx" => Some("javascript"),
        "json" | "jsonc" => Some("json"),
        "md" | "markdown" | "mdx" => Some("markdown"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        "html" | "htm" => Some("html"),
        "css" | "scss" => Some("css"),
        "go" => Some("go"),
        "py" => Some("python"),
        "rb" => Some("ruby"),
        "java" => Some("java"),
        "kt" | "kts" | "ktm" => Some("kotlin"),
        "swift" => Some("swift"),
        "c" => Some("c"),
        "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => Some("cpp"),
        "sh" | "bash" | "zsh" => Some("bash"),
        "lua" => Some("lua"),
        "php" | "php3" | "php4" | "php5" | "phtml" => Some("php"),
        "sql" => Some("sql"),
        "proto" => Some("proto"),
        "zig" => Some("zig"),
        _ => None,
    }
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

    #[test]
    fn detects_common_languages_from_paths() {
        assert_eq!(language_for_path("src/lib.rs"), Some("rust"));
        assert_eq!(language_for_path("web/app.ts"), Some("typescript"));
        assert_eq!(language_for_path("web/app.tsx"), Some("tsx"));
        assert_eq!(language_for_path("data/config.json"), Some("json"));
        assert_eq!(language_for_path("README.md"), Some("markdown"));
        assert_eq!(language_for_path("Cargo.toml"), Some("toml"));
        assert_eq!(language_for_path("settings.yaml"), Some("yaml"));
        assert_eq!(language_for_path("asset.bin"), None);
    }

    #[test]
    fn attaches_syntax_highlights_to_diff_code_lines() {
        let file = DiffFile {
            path: "src/lib.rs".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 1,
            changes: 2,
            patch: None,
        };
        let parsed = parse_unified_diff_with_syntax(
            &file,
            "@@ -1,3 +1,3 @@\n fn main() {\n-let old_value = true;\n+let new_value = false;\n }\n",
            &HighlightTheme::default_dark(),
        );
        let lines = &parsed.hunks[0].lines;

        assert!(!lines[0].syntax_highlights.is_empty());
        assert!(!lines[1].syntax_highlights.is_empty());
        assert!(!lines[2].syntax_highlights.is_empty());
    }

    #[test]
    fn attaches_syntax_highlights_to_typescript_added_files() {
        let file = DiffFile {
            path: "src/hooks/useKeyboardInset.ts".to_string(),
            previous_path: None,
            status: FileStatus::Added,
            additions: 3,
            deletions: 0,
            changes: 3,
            patch: None,
        };
        let parsed = parse_unified_diff_with_syntax(
            &file,
            "@@ -0,0 +1,3 @@\n+import { useEffect } from 'react';\n+\n+const KEYBOARD_OVERLAY_THRESHOLD = 24;\n",
            &HighlightTheme::default_dark(),
        );
        let lines = &parsed.hunks[0].lines;

        assert!(!lines[0].syntax_highlights.is_empty());
        assert!(lines[1].syntax_highlights.is_empty());
        assert!(!lines[2].syntax_highlights.is_empty());
    }

    #[test]
    fn leaves_metadata_and_unknown_languages_plain() {
        let file = DiffFile {
            path: "notes.unknown".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 1,
            changes: 2,
            patch: None,
        };
        let parsed = parse_unified_diff_with_syntax(
            &file,
            "@@ -1 +1 @@\n-old\n+new\n\\ No newline at end of file\n",
            &HighlightTheme::default_dark(),
        );

        assert!(
            parsed
                .hunks
                .iter()
                .flat_map(|hunk| hunk.lines.iter())
                .all(|line| line.syntax_highlights.is_empty())
        );
    }

    #[test]
    fn skips_syntax_for_missing_patches_and_oversized_lines() {
        let missing_patch = DiffFile {
            path: "src/lib.rs".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 0,
            deletions: 0,
            changes: 0,
            patch: None,
        };
        assert!(
            parse_files_with_syntax(&[missing_patch], &HighlightTheme::default_dark())[0].is_none()
        );

        let long_line = "a".repeat(MAX_SYNTAX_LINE_BYTES + 1);
        let file = DiffFile {
            path: "src/lib.rs".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 0,
            changes: 1,
            patch: None,
        };
        let parsed = parse_unified_diff_with_syntax(
            &file,
            &format!("@@ -1 +1 @@\n+{long_line}\n"),
            &HighlightTheme::default_dark(),
        );

        assert!(parsed.hunks[0].lines[0].syntax_highlights.is_empty());
    }

    #[test]
    fn skips_syntax_for_oversized_patches() {
        let file = DiffFile {
            path: "src/lib.rs".to_string(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 0,
            changes: 1,
            patch: None,
        };
        let filler = " context\n".repeat(MAX_SYNTAX_PATCH_BYTES / " context\n".len() + 1);
        let parsed = parse_unified_diff_with_syntax(
            &file,
            &format!("@@ -1 +1 @@\n+let value = true;\n{filler}"),
            &HighlightTheme::default_dark(),
        );

        assert!(
            parsed
                .hunks
                .iter()
                .flat_map(|hunk| hunk.lines.iter())
                .all(|line| line.syntax_highlights.is_empty())
        );
    }
}
